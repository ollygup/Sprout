//! The clip viewer's outside-click hook (spec 221 §B.6): one low-level mouse
//! hook, installed only while the viewer is open, that closes the viewer on
//! the first button-down outside its top-level window. The click is never
//! swallowed — the hook always passes it down the chain — so the dock or
//! window underneath still receives it.
//!
//! Windows delivers `WH_MOUSE_LL` callbacks to the thread that installed the
//! hook, and only while that thread pumps messages. The hook therefore owns a
//! dedicated thread with its own message loop: the app's main thread blocks
//! while it builds windows and touches SQLite, which would delay — and past
//! the system's hook timeout silently drop — callbacks exactly when the
//! viewer opens.
//!
//! This module is the operation's owner (ADR-0029); the dock and AppBar never
//! install a hook. When the hook cannot be installed, or UIPI makes it
//! unreliable against an elevated window, the viewer still closes through the
//! focus-loss fallback in [`note_focus_lost`], after the same open grace.

use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Mutex, TryLockError};
use std::thread;
use std::time::{Duration, Instant};

use tauri::AppHandle;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetAncestor, GetMessageW, GetWindow, HC_ACTION, HHOOK,
    MSLLHOOKSTRUCT, MSG, PostThreadMessageW, SetWindowsHookExW, TranslateMessage,
    UnhookWindowsHookEx, WindowFromPoint, GA_ROOT, GW_ENABLEDPOPUP, WH_MOUSE_LL, WM_LBUTTONDOWN,
    WM_MBUTTONDOWN, WM_QUIT, WM_RBUTTONDOWN, WM_XBUTTONDOWN,
};

/// Grace after the viewer opens (or is retargeted to another image) during
/// which outside clicks are ignored: the gesture that raised the viewer must
/// never dismiss the window it just opened.
const OPEN_GRACE: Duration = Duration::from_millis(200);

struct Installed {
    /// `HHOOK` kept as a plain integer so the state stays `Send`.
    hook: isize,
    thread_id: u32,
    /// The viewer's top-level `HWND`, compared against `GetAncestor(GA_ROOT)`.
    viewer: isize,
    armed_at: Instant,
    /// Bumped by every open/retarget, so a close decided by one click can be
    /// discarded when a newer open already owns the viewer.
    generation: u64,
}

static INSTALLED: Mutex<Option<Installed>> = Mutex::new(None);
static GENERATION: AtomicU64 = AtomicU64::new(0);

thread_local! {
    /// The app handle for the hook thread's own callback. `SetWindowsHookExW`
    /// takes a plain function pointer, so the target rides this thread's
    /// storage instead of a global (there is at most one hook thread).
    static CLOSE_TARGET: RefCell<Option<AppHandle>> = const { RefCell::new(None) };
}

/// Arms the hook for one viewer open, or refreshes the grace and generation
/// when the same window is reused for another image. The thread reports as
/// soon as the hook is in place, so `arm` and [`disarm`] stay ordered with the
/// real hook lifetime.
pub(crate) fn arm(app: &AppHandle, viewer: HWND) {
    let viewer = viewer as isize;
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let mut installed = lock();
    if let Some(hook) = installed.as_mut() {
        hook.viewer = viewer;
        hook.armed_at = Instant::now();
        hook.generation = generation;
        return;
    }

    let (ready_tx, ready_rx) = mpsc::channel();
    let app = app.clone();
    let spawned = thread::Builder::new()
        .name("clip-viewer-hook".into())
        .spawn(move || run_hook_thread(app, ready_tx));
    if spawned.is_err() {
        // Without a hook the focus-loss fallback still closes the viewer.
        return;
    }
    if let Ok(Some((hook, thread_id))) = ready_rx.recv() {
        *installed = Some(Installed {
            hook,
            thread_id,
            viewer,
            armed_at: Instant::now(),
            generation,
        });
    }
}

/// Removes the hook immediately (no hook may exist while the viewer is
/// closed) and lets the pumping thread retire. Safe to call repeatedly and
/// from any thread, including after the window was destroyed.
pub(crate) fn disarm() {
    let installed = {
        let mut guard = lock();
        guard.take()
    };
    let Some(installed) = installed else {
        return;
    };
    unsafe {
        UnhookWindowsHookEx(installed.hook as HHOOK);
        PostThreadMessageW(installed.thread_id, WM_QUIT, 0, 0);
    }
}

/// Closes the viewer when a focus loss is a genuine outside interaction.
/// Focus moving to one of the viewer's own popups (a modal dialog) is not;
/// clicking the viewer's own WebView never deactivates the window at all.
/// The grace keeps every open gesture safe.
pub(crate) fn note_focus_lost(app: &AppHandle) {
    let (viewer, armed_at, generation) = {
        let guard = lock();
        let Some(installed) = guard.as_ref() else {
            return;
        };
        (installed.viewer, installed.armed_at, installed.generation)
    };
    if armed_at.elapsed() < OPEN_GRACE {
        return;
    }
    let viewer = viewer as HWND;
    let popup = unsafe { GetWindow(viewer, GW_ENABLEDPOPUP) };
    if !popup.is_null() && popup != viewer {
        return;
    }
    super::close_generation(app, generation);
}

/// The open generation still owns the viewer. A close decided before another
/// image was opened (or after the viewer already closed) is discarded, so a
/// retarget never fights the click that requested it.
pub(crate) fn is_current(generation: u64) -> bool {
    lock()
        .as_ref()
        .map(|installed| installed.generation == generation)
        .unwrap_or(false)
}

fn lock() -> std::sync::MutexGuard<'static, Option<Installed>> {
    match INSTALLED.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn run_hook_thread(app: AppHandle, ready: mpsc::Sender<Option<(isize, u32)>>) {
    CLOSE_TARGET.with(|target| *target.borrow_mut() = Some(app));
    // `hMod` is the current module because the hook procedure lives in this
    // process; a module handle keeps the call valid on every Windows version.
    let hook = unsafe {
        SetWindowsHookExW(
            WH_MOUSE_LL,
            Some(mouse_proc),
            GetModuleHandleW(std::ptr::null()),
            0,
        )
    };
    if hook.is_null() {
        let _ = ready.send(None);
        return;
    }
    let thread_id = unsafe { GetCurrentThreadId() };
    let _ = ready.send(Some((hook as isize, thread_id)));

    let mut message: MSG = unsafe { std::mem::zeroed() };
    loop {
        let result = unsafe { GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) };
        if result <= 0 {
            break;
        }
        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    unsafe { UnhookWindowsHookEx(hook) };
    CLOSE_TARGET.with(|target| *target.borrow_mut() = None);
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && is_button_down(wparam) {
        if let Some(generation) = outside_click_generation(lparam) {
            close_outside(generation);
        }
    }
    // Never swallow the click: the dock or window underneath still sees it.
    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}

fn is_button_down(wparam: WPARAM) -> bool {
    matches!(
        wparam as u32,
        WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN
    )
}

/// The generation to close, or `None` when the click belongs to the viewer
/// itself or still falls inside the post-open grace. The state lock is only
/// tried: a busy lock must never stall system-wide input.
fn outside_click_generation(lparam: LPARAM) -> Option<u64> {
    let mouse = unsafe { &*(lparam as *const MSLLHOOKSTRUCT) };
    let point = POINT {
        x: mouse.pt.x,
        y: mouse.pt.y,
    };
    let installed = match INSTALLED.try_lock() {
        Ok(guard) => guard,
        Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
        Err(TryLockError::WouldBlock) => return None,
    };
    let installed = installed.as_ref()?;
    if installed.armed_at.elapsed() < OPEN_GRACE {
        return None;
    }
    let hit = unsafe { WindowFromPoint(point) };
    if !hit.is_null() && unsafe { GetAncestor(hit, GA_ROOT) } as isize == installed.viewer {
        return None;
    }
    Some(installed.generation)
}

fn close_outside(generation: u64) {
    CLOSE_TARGET.with(|target| {
        let Some(app) = target.borrow().as_ref().cloned() else {
            return;
        };
        let closing = app.clone();
        // The callback returns immediately; destroying a window and touching
        // SQLite belong on the event loop, not in the system's input hook.
        let _ = app.run_on_main_thread(move || super::close_generation(&closing, generation));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONUP,
    };

    #[test]
    fn grace_is_two_hundred_milliseconds() {
        assert_eq!(OPEN_GRACE, Duration::from_millis(200));
    }

    #[test]
    fn only_button_down_messages_start_an_outside_click() {
        for message in [WM_LBUTTONDOWN, WM_RBUTTONDOWN, WM_MBUTTONDOWN, WM_XBUTTONDOWN] {
            assert!(is_button_down(message as WPARAM), "message {message:#06x}");
        }
        for message in [WM_LBUTTONUP, WM_RBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL] {
            assert!(!is_button_down(message as WPARAM), "message {message:#06x}");
        }
    }
}
