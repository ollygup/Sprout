//! The clip viewer: one reusable floating window that shows an
//! image Clip at a readable size, with its name, pixel dimensions, and a Copy
//! control. Opening another image reuses the same window; closing destroys it
//! so the next open recreates it at the remembered size.
//!
//! The viewer is a plain resizable window — never always-on-top, never an
//! AppBar. Its geometry comes from the invoking window's monitor work area
//! (`appbar::work_area`, the same owner the dock uses), and its remembered
//! size lives in the machine-local `meta` table: local-only by design, so
//! backup exports never carry it (ADR-0026's machine-local boundary).
//!
//! Image bytes never travel in a URL or an event: the open command resolves
//! the Clip by id, and the viewer fetches the resolved record through the
//! ordinary `get_clip` command. Only the id rides the `clip-viewer-open`
//! event used to retarget an already-open viewer.

use std::sync::Mutex;
use std::time::Duration;

use rusqlite::Connection;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};
use windows_sys::Win32::Foundation::{HWND, RECT};

use crate::{
    appbar, clips, db, quick_window,
    constants::window::{
        CLIP_VIEWER_MIN_HEIGHT, CLIP_VIEWER_MIN_WIDTH, clip_viewer_clamp_size,
        clip_viewer_default_size,
    },
    AppState,
};

/// The viewer's stable label — the event handler in `lib.rs` keys on it and
/// the capability scope names it.
pub const CLIP_VIEWER_WINDOW: &str = "clip-viewer";

/// The route the viewer webview loads — a SvelteKit SPA fallback serves it in
/// production, Vite's fallback in dev.
const ROUTE: &str = "clip-viewer";

/// The main window's label, the source fallback when no invoking window is
/// known (an old command, a future entry point).
const SOURCE_MAIN: &str = "main";

/// The `meta` keys holding the viewer's last inner size. Namespaced and
/// machine-local: the backup document never reads this table.
const SIZE_WIDTH_KEY: &str = "clipviewer.width";
const SIZE_HEIGHT_KEY: &str = "clipviewer.height";

/// The Clip the next viewer mount should show. Covers the create → mount race:
/// the command records the id before the window exists, and the page consumes
/// it once. Also retargets an already-mounted viewer through the event.
static PENDING_CLIP: Mutex<Option<i64>> = Mutex::new(None);

/// The window that invoked the current viewer — where focus returns when the
/// viewer closes (main or dock).
static LAST_SOURCE: Mutex<Option<String>> = Mutex::new(None);

/// The live viewer window, if one exists. A `Window` lookup (not
/// `WebviewWindow`) keeps this working if the host ever grows child webviews,
/// matching the Quick Launch seam.
pub fn clip_viewer_window(app: &AppHandle) -> Option<tauri::Window> {
    app.get_window(CLIP_VIEWER_WINDOW)
}

/// Consumes the pending Clip id once; `None` means the page mounted without a
/// request (a reload, or the request was already delivered by the event).
pub fn take_pending() -> Option<i64> {
    PENDING_CLIP.lock().ok().and_then(|mut pending| pending.take())
}

/// Loads the remembered viewer size from the machine-local `meta` table.
/// `None` means first open (or a broken pair) — the default derivation runs.
pub(crate) fn load_size(conn: &Connection) -> Option<(u32, u32)> {
    let width = db::read_meta(conn, SIZE_WIDTH_KEY)?.parse::<u32>().ok()?;
    let height = db::read_meta(conn, SIZE_HEIGHT_KEY)?.parse::<u32>().ok()?;
    if width == 0 || height == 0 {
        return None;
    }
    Some((width, height))
}

/// Persists the viewer's last inner size. Local-only: `meta` is never part of
/// an export or backup, so window geometry stays on this machine.
pub(crate) fn save_size(conn: &Connection, width: u32, height: u32) -> rusqlite::Result<()> {
    db::upsert_meta(conn, SIZE_WIDTH_KEY, &width.to_string())?;
    db::upsert_meta(conn, SIZE_HEIGHT_KEY, &height.to_string())
}

/// Opens the viewer for one image Clip: raises the existing viewer with the
/// new Clip, or creates it centered on the invoking window's monitor work
/// area at the remembered (clamped) size. The Clip is resolved first, so a
/// deleted id never opens a blank window; a Clip that disappears while the
/// viewer is open is handled by the page's empty state instead.
pub fn open(app: &AppHandle, id: i64, source: Option<&str>) -> Result<(), String> {
    let stored = {
        let state = app.state::<AppState>();
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let clip = clips::get_clip(&conn, id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "This clip no longer exists — refresh and try again.".to_string())?;
        if clip.clip.image.is_none() {
            return Err("That clip has no image to open.".into());
        }
        load_size(&conn)
    };

    let source = normalize_source(source);
    *PENDING_CLIP.lock().map_err(|e| e.to_string())? = Some(id);
    *LAST_SOURCE.lock().map_err(|e| e.to_string())? = Some(source.clone());

    if let Some(window) = clip_viewer_window(app) {
        let _ = window.show();
        let _ = window.unminimize();
        if window.set_focus().is_ok() {
            let _ = window.emit("clip-viewer-open", id);
            return Ok(());
        }
        // A lingering destroyed handle: drop it so the create path below can
        // take the label instead of reporting "already exists" forever.
        let _ = window.destroy();
    }

    let work = work_area_for(app, &source);
    let (width, height) = clamp_for_work(stored, &work);
    create_window(app, &work, width, height).map_err(|e| e.to_string())?;
    Ok(())
}

/// Closes the viewer through the frontend's Esc / close control:
/// the size is remembered, the window destroyed, and focus returns to the
/// window that invoked it. A no-op when no viewer is open.
pub fn close(app: &AppHandle) -> Result<(), String> {
    let Some(window) = clip_viewer_window(app) else {
        return Ok(());
    };
    if let Ok(size) = window.inner_size() {
        if size.width > 0 && size.height > 0 {
            let state = app.state::<AppState>();
            // Bind the guard before the `if let` so it drops before `state`.
            let locked = state.db.lock();
            if let Ok(conn) = locked {
                let _ = save_size(&conn, size.width, size.height);
            }
        }
    }
    let source = LAST_SOURCE
        .lock()
        .ok()
        .and_then(|source| source.clone())
        .unwrap_or_else(|| SOURCE_MAIN.to_string());
    if let Ok(mut pending) = PENDING_CLIP.lock() {
        *pending = None;
    }
    // Focus returns to the invoker *before* the viewer goes away: while this
    // call runs the viewer is still the foreground window, so Windows honors
    // `SetForegroundWindow`; after `destroy()` activation may already have
    // moved on and the request is refused, leaving focus nowhere.
    focus_source(app, &source);
    window.destroy().map_err(|e| e.to_string())?;
    Ok(())
}

/// Only the two real invoking windows may steer placement; anything else
/// falls back to the main window's monitor.
fn normalize_source(source: Option<&str>) -> String {
    match source {
        Some(quick_window::QUICK_LAUNCH_WINDOW) => quick_window::QUICK_LAUNCH_WINDOW.to_string(),
        _ => SOURCE_MAIN.to_string(),
    }
}

/// The invoking window's HWND, if it is alive.
fn source_hwnd(app: &AppHandle, source: &str) -> Option<HWND> {
    let window = if source == quick_window::QUICK_LAUNCH_WINDOW {
        quick_window::quick_launch_window(app)
    } else {
        app.get_window(SOURCE_MAIN)
    };
    // tao hands back the `windows` crate wrapper; `appbar` speaks `windows_sys`.
    window.and_then(|window| window.hwnd().ok()).map(|hwnd| hwnd.0)
}

/// The invoking monitor's work area (taskbar excluded), falling back to the
/// primary monitor and then a sane 1280×720 rect — placement must never fail
/// an open just because a window vanished mid-call.
fn work_area_for(app: &AppHandle, source: &str) -> RECT {
    if let Some(work) = source_hwnd(app, source).and_then(appbar::work_area) {
        return work;
    }
    if let Ok(Some(monitor)) = app.primary_monitor() {
        let area = monitor.work_area();
        return RECT {
            left: area.position.x,
            top: area.position.y,
            right: area.position.x + area.size.width as i32,
            bottom: area.position.y + area.size.height as i32,
        };
    }
    RECT {
        left: 0,
        top: 0,
        right: 1280,
        bottom: 720,
    }
}

/// The remembered size clamped into `work`, or the default when nothing is
/// remembered yet.
fn clamp_for_work(stored: Option<(u32, u32)>, work: &RECT) -> (u32, u32) {
    let width = (work.right - work.left).max(1);
    let height = (work.bottom - work.top).max(1);
    match stored {
        Some((stored_width, stored_height)) => {
            clip_viewer_clamp_size(stored_width as i64, stored_height as i64, width, height)
        }
        None => clip_viewer_default_size(width, height),
    }
}

/// The viewer's top-left so it sits centered in `work`. The size is already
/// clamped to the work area, so both offsets are non-negative.
fn centered_position(work: &RECT, width: u32, height: u32) -> (i32, i32) {
    let work_width = (work.right - work.left).max(1);
    let work_height = (work.bottom - work.top).max(1);
    let x = work.left + (work_width - width.min(work_width as u32) as i32) / 2;
    let y = work.top + (work_height - height.min(work_height as u32) as i32) / 2;
    (x, y)
}

/// Builds the frameless, resizable viewer hidden at first so its physical
/// geometry lands before the first paint (the builder's size is logical).
/// Retries the label briefly: a close→reopen can race the old window's
/// destruction, the same lifecycle the main window guards against.
fn create_window(
    app: &AppHandle,
    work: &RECT,
    width: u32,
    height: u32,
) -> tauri::Result<WebviewWindow> {
    let mut last_error = None;
    for attempt in 0..3 {
        let build = WebviewWindowBuilder::new(
            app,
            CLIP_VIEWER_WINDOW,
            WebviewUrl::App(ROUTE.into()),
        )
        .title("Sprout — Clip viewer")
        .decorations(false)
        .resizable(true)
        .always_on_top(false)
        .skip_taskbar(false)
        .visible(false)
        .inner_size(width as f64, height as f64)
        .build();
        match build {
            Ok(window) => {
                window.set_size(PhysicalSize::new(width, height))?;
                // `set_position` places the *outer* rect while the size above
                // is the inner one; centering by the inner size would push the
                // window half a frame off-center. Center what the user sees.
                let outer = window
                    .outer_size()
                    .unwrap_or(PhysicalSize::new(width, height));
                let (x, y) = centered_position(work, outer.width, outer.height);
                window.set_position(PhysicalPosition::new(x, y))?;
                // The floor is physical like the remembered size (`inner_size`
                // stores physical pixels), so a 480×360 memory means the same
                // window at every scale factor.
                window.set_min_size(Some(PhysicalSize::new(
                    CLIP_VIEWER_MIN_WIDTH,
                    CLIP_VIEWER_MIN_HEIGHT,
                )))?;
                window.show()?;
                window.set_focus()?;
                return Ok(window);
            }
            Err(e) => {
                last_error = Some(e);
                if attempt < 2 {
                    if let Some(stale) = clip_viewer_window(app) {
                        let _ = stale.destroy();
                    }
                    std::thread::sleep(Duration::from_millis(120));
                }
            }
        }
    }
    Err(last_error.expect("at least one build attempt"))
}

/// Returns focus to the invoking window after the viewer closes, falling back
/// to the main window when the dock is gone.
fn focus_source(app: &AppHandle, source: &str) {
    let dock = app.get_webview_window(quick_window::QUICK_LAUNCH_WINDOW);
    let window = if source == quick_window::QUICK_LAUNCH_WINDOW {
        dock
    } else {
        None
    }
    .or_else(|| app.get_webview_window(SOURCE_MAIN));
    let Some(window) = window else {
        return;
    };
    // Two steps, both needed: activating the frame brings the window forward,
    // and focusing the webview itself restores the typing target. Without the
    // second, the source comes back as the foreground window with no focused
    // child — `document.hasFocus()` stays false and the first keystroke after
    // closing the viewer goes nowhere.
    let _ = window.set_focus();
    let _ = AsRef::<tauri::Webview>::as_ref(&window).set_focus();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        crate::db::init_at(&tempfile::tempdir().unwrap().into_path()).unwrap()
    }

    #[test]
    fn remembered_size_roundtrips_in_machine_local_meta() {
        let c = conn();
        assert_eq!(load_size(&c), None);
        save_size(&c, 900, 700).unwrap();
        assert_eq!(load_size(&c), Some((900, 700)));
        // Zero never reads as a remembered size — the default path runs.
        save_size(&c, 0, 700).unwrap();
        assert_eq!(load_size(&c), None);
    }

    #[test]
    fn open_size_clamps_the_stored_pair_into_the_work_area() {
        let work = RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        };
        assert_eq!(clamp_for_work(None, &work), (960, 720));
        assert_eq!(clamp_for_work(Some((4000, 3000)), &work), (1920, 1040));
        assert_eq!(clamp_for_work(Some((200, 100)), &work), (480, 360));
    }

    #[test]
    fn centered_position_uses_the_invoking_monitors_origin() {
        let work = RECT {
            left: 1920,
            top: 0,
            right: 3840,
            bottom: 1040,
        };
        assert_eq!(centered_position(&work, 960, 720), (2400, 160));
    }
}
