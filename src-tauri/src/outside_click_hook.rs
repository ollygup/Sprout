//! The app's one low-level mouse hook for "close this window on an outside
//! click" (ADR-0029). Two consumers share it: the clip viewer (spec 221 §B.6)
//! and the open bezel panel. One hook at a time, installed only while its owner
//! is on screen; when the other consumer opens over it, the first is parked and
//! gets the hook back when the newer one closes. The click is never swallowed —
//! the hook always passes it down the chain, so the dock or window underneath
//! still receives it.
//!
//! Windows delivers `WH_MOUSE_LL` callbacks to the thread that installed the
//! hook, and only while that thread pumps messages. The hook therefore owns a
//! dedicated thread with its own message loop: the app's main thread blocks
//! while it builds windows and touches SQLite, which would delay — and past
//! the system's hook timeout silently drop — callbacks exactly when a window
//! opens.
//!
//! Focus loss is a *viewer-only* fallback: the dock's bezel interaction is
//! click-gated by design (ticket 217 — alt-tabbing must not close the panel),
//! so only the viewer arms that path. When the hook cannot be installed, or
//! UIPI makes it unreliable against an elevated window, the viewer still
//! closes through [`note_focus_lost`]; the bezel panel keeps its in-window
//! pointer path and Esc.

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

/// Grace after a window opens (or the viewer is retargeted to another image)
/// during which outside clicks are ignored: the gesture that raised the window
/// must never dismiss the window it just opened.
const OPEN_GRACE: Duration = Duration::from_millis(200);

/// Which window the armed hook belongs to, and therefore which close path an
/// outside click runs. Exactly one is armed at a time.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Target {
    Viewer,
    Bezel,
}

struct Installed {
    /// `HHOOK` kept as a plain integer so the state stays `Send`.
    hook: isize,
    thread_id: u32,
    /// The window's top-level `HWND`, compared against `GetAncestor(GA_ROOT)`.
    window: isize,
    target: Target,
    armed_at: Instant,
    /// Bumped by every arm, so a close decided by one click can be discarded
    /// when a newer open already owns the window.
    generation: u64,
}

static INSTALLED: Mutex<Option<Installed>> = Mutex::new(None);
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// The consumer the live hook was taken from when the other one opened
/// (the clip viewer opened from an open bezel panel, or a panel opened while
/// the viewer is up). It keeps its window and generation so closing the newer
/// consumer can hand the same hook straight back instead of leaving the older
/// one with no outside-click. At most one entry: the slot itself is single.
struct Suspended {
    window: isize,
    target: Target,
    generation: u64,
}

static SUSPENDED: Mutex<Option<Suspended>> = Mutex::new(None);

thread_local! {
    /// The app handle for the hook thread's own callback. `SetWindowsHookExW`
    /// takes a plain function pointer, so the target rides this thread's
    /// storage instead of a global (there is at most one hook thread).
    static CLOSE_TARGET: RefCell<Option<AppHandle>> = const { RefCell::new(None) };
}

/// Arms the hook for one window, or refreshes the grace and generation when the
/// same hook is re-armed (the viewer retargeting to another image). A different
/// target takes the live hook over and parks the previous consumer in
/// [`SUSPENDED`], so [`disarm_target`] can hand it back. The thread reports as
/// soon as the hook is in place, so arming stays ordered with the real hook
/// lifetime.
pub(crate) fn arm(app: &AppHandle, window: HWND, target: Target) {
    let window = window as isize;
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let mut installed = lock();
    if let Some(hook) = installed.as_mut() {
        if hook.target != target {
            *suspended_lock() = Some(Suspended {
                window: hook.window,
                target: hook.target,
                generation: hook.generation,
            });
        }
        hook.window = window;
        hook.target = target;
        hook.armed_at = Instant::now();
        hook.generation = generation;
        return;
    }

    let (ready_tx, ready_rx) = mpsc::channel();
    let app = app.clone();
    let spawned = thread::Builder::new()
        .name("outside-click-hook".into())
        .spawn(move || run_hook_thread(app, ready_tx));
    if spawned.is_err() {
        // Without a hook the viewer's focus-loss fallback still closes it.
        return;
    }
    if let Ok(Some((hook, thread_id))) = ready_rx.recv() {
        *installed = Some(Installed {
            hook,
            thread_id,
            window,
            target,
            armed_at: Instant::now(),
            generation,
        });
    }
}

/// Releases `target`'s claim on the hook. The two consumers share one slot, so
/// a close path for one must never tear down the other's hook: closing the dock
/// while the viewer is open leaves the viewer armed, and closing the newer
/// consumer hands the hook back to the one it displaced. When nobody is waiting
/// the hook is removed immediately — no hook may outlive its window — and the
/// pumping thread retires. Safe to call repeatedly and from any thread,
/// including after the window was destroyed.
pub(crate) fn disarm_target(target: Target) {
    let mut installed = lock();
    let owned = installed
        .as_ref()
        .map(|installed| installed.target == target)
        .unwrap_or(false);
    {
        let mut suspended = suspended_lock();
        // A consumer closing while the other owns the hook must not be handed
        // back later against a window that is already gone.
        if suspended
            .as_ref()
            .map(|suspended| suspended.target == target)
            .unwrap_or(false)
        {
            *suspended = None;
        }
        if !owned {
            return;
        }
        if let Some(previous) = suspended.take() {
            let hook = installed.as_mut().expect("owned hook is installed");
            hook.window = previous.window;
            hook.target = previous.target;
            hook.generation = previous.generation;
            // Fresh grace: the click that closed the newer consumer must not
            // also dismiss the window it just uncovered.
            hook.armed_at = Instant::now();
            return;
        }
    }
    let removed = installed.take();
    drop(installed);
    if let Some(installed) = removed {
        remove(installed);
    }
}

fn suspended_lock() -> std::sync::MutexGuard<'static, Option<Suspended>> {
    match SUSPENDED.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn remove(installed: Installed) {
    unsafe {
        UnhookWindowsHookEx(installed.hook as HHOOK);
        PostThreadMessageW(installed.thread_id, WM_QUIT, 0, 0);
    }
}

/// Closes the viewer when a focus loss is a genuine outside interaction.
/// Focus moving to one of the viewer's own popups (a modal dialog) is not;
/// clicking the viewer's own WebView never deactivates the window at all.
/// The grace keeps every open gesture safe. The bezel target never closes on
/// focus loss (ticket 217), so it returns before any check.
pub(crate) fn note_focus_lost(app: &AppHandle) {
    let (window, target, armed_at, generation) = {
        let guard = lock();
        let Some(installed) = guard.as_ref() else {
            return;
        };
        (
            installed.window,
            installed.target,
            installed.armed_at,
            installed.generation,
        )
    };
    if target != Target::Viewer {
        return;
    }
    if armed_at.elapsed() < OPEN_GRACE {
        return;
    }
    let window = window as HWND;
    let popup = unsafe { GetWindow(window, GW_ENABLEDPOPUP) };
    if !popup.is_null() && popup != window {
        return;
    }
    crate::clip_viewer::close_generation(app, generation);
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
        if let Some((generation, target)) = outside_click(lparam) {
            close_outside(generation, target);
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

/// The generation and target to close, or `None` when the click belongs to the
/// armed window itself or still falls inside the post-open grace. The state
/// lock is only tried: a busy lock must never stall system-wide input.
fn outside_click(lparam: LPARAM) -> Option<(u64, Target)> {
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
    if !hit.is_null() && unsafe { GetAncestor(hit, GA_ROOT) } as isize == installed.window {
        return None;
    }
    Some((installed.generation, installed.target))
}

fn close_outside(generation: u64, target: Target) {
    CLOSE_TARGET.with(|stored| {
        let Some(app) = stored.borrow().as_ref().cloned() else {
            return;
        };
        let closing = app.clone();
        // The callback returns immediately; destroying a window, emitting, and
        // touching SQLite belong on the event loop, not in the system's input
        // hook.
        let _ = app.run_on_main_thread(move || match target {
            Target::Viewer => crate::clip_viewer::close_generation(&closing, generation),
            Target::Bezel => crate::quick_window::request_bezel_close(&closing),
        });
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

    #[test]
    fn target_distinguishes_the_two_consumers() {
        assert_ne!(Target::Viewer, Target::Bezel);
    }

    /// The slot assertions mutate process-wide statics; cargo runs tests in
    /// parallel, so each stateful test takes this lock first.
    static TEST_SLOT: Mutex<()> = Mutex::new(());

    fn fake(target: Target, window: isize, generation: u64) -> Installed {
        Installed {
            hook: 1,
            thread_id: 0,
            window,
            target,
            armed_at: Instant::now(),
            generation,
        }
    }

    /// A dock close path may only remove the dock's hook: the viewer keeps its
    /// own arm across an undock/close that happens to run at the same time.
    /// The fake handle/thread make the OS calls no-ops, so only the slot
    /// bookkeeping is under test.
    #[test]
    fn targeted_disarm_leaves_the_other_consumer_armed() {
        let _test = TEST_SLOT.lock().unwrap_or_else(|e| e.into_inner());
        *suspended_lock() = None;
        *lock() = Some(fake(Target::Viewer, 0, 7));
        disarm_target(Target::Bezel);
        assert_eq!(
            lock().as_ref().map(|installed| installed.target),
            Some(Target::Viewer)
        );
        disarm_target(Target::Viewer);
        assert!(lock().is_none());
    }

    /// Opening one consumer over the other parks it; closing the newer one
    /// hands the same hook back, with a fresh grace, instead of leaving the
    /// older window with no outside-click.
    #[test]
    fn closing_the_newer_consumer_hands_the_hook_back() {
        let _test = TEST_SLOT.lock().unwrap_or_else(|e| e.into_inner());
        *suspended_lock() = Some(Suspended {
            window: 0xAA,
            target: Target::Bezel,
            generation: 3,
        });
        *lock() = Some(fake(Target::Viewer, 0xBB, 4));
        disarm_target(Target::Viewer);
        {
            // Scoped so the slot lock is released before the next disarm.
            let live = lock();
            let live = live.as_ref().expect("hook handed back, not removed");
            assert_eq!(live.target, Target::Bezel);
            assert_eq!(live.window, 0xAA);
            assert_eq!(live.generation, 3);
        }
        disarm_target(Target::Bezel);
        assert!(lock().is_none());
    }

    /// Closing the parked consumer while the other owns the hook must not
    /// resurrect it against a window that is already gone.
    #[test]
    fn closing_the_parked_consumer_never_restores_it() {
        let _test = TEST_SLOT.lock().unwrap_or_else(|e| e.into_inner());
        *suspended_lock() = Some(Suspended {
            window: 0xCC,
            target: Target::Viewer,
            generation: 9,
        });
        *lock() = Some(fake(Target::Bezel, 0xDD, 10));
        disarm_target(Target::Viewer);
        assert!(suspended_lock().is_none());
        assert_eq!(
            lock().as_ref().map(|installed| installed.target),
            Some(Target::Bezel)
        );
        disarm_target(Target::Bezel);
        assert!(lock().is_none());
    }
}
