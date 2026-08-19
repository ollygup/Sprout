//! The floating Quick Launch window (ticket 52): a miniature, frameless
//! second window opened from the tray icon's left-click — raised when it is
//! already open. It shows two read-only tabs (Quick Launch / Quick Actions)
//! and hides to the tray on blur or close: the geometry is remembered, then
//! the window is destroyed, keeping the backend lean — the same disposable
//! pattern as the main window (ticket 43). The docked AppBar form is ticket
//! 53; this module owns nothing else.
//!
//! The blur/close handling itself lives in `lib.rs`'s `on_window_event`
//! (keyed on [`QUICK_LAUNCH_WINDOW`]); this module owns the window's
//! creation, geometry memory, and the close command's backend half.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, Window};

use crate::db::QuickWindowGeometry;
use crate::AppState;

/// The window's stable label — the event handler in `lib.rs` keys on it and
/// the capability scope names it.
pub const QUICK_LAUNCH_WINDOW: &str = "quick-launch";

/// The route the second webview loads — a SvelteKit SPA fallback serves it
/// in production, Vite's fallback in dev.
const ROUTE: &str = "quick-launch-window";

/// The miniature default size for a first-ever open, before the user moved
/// or resized anything.
const DEFAULT_WIDTH: u32 = 340;
const DEFAULT_HEIGHT: u32 = 460;

/// Opens the Quick Launch window: raises the existing one, or creates it at
/// its remembered size and position. The remembered spot is honored only
/// when it sits on a connected monitor — otherwise (first use, or the
/// monitor it lived on is gone) the window opens centered.
pub fn open(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(QUICK_LAUNCH_WINDOW) {
        window.set_focus()?;
        return Ok(());
    }
    let geometry = remembered_geometry(app);
    let mut builder = WebviewWindowBuilder::new(
        app,
        QUICK_LAUNCH_WINDOW,
        WebviewUrl::App(ROUTE.into()),
    )
    .title("Sprout — Quick Launch")
    .decorations(false)
    .resizable(true)
    .skip_taskbar(true)
    .inner_size(
        geometry.map(|g| g.width as f64).unwrap_or(DEFAULT_WIDTH as f64),
        geometry.map(|g| g.height as f64).unwrap_or(DEFAULT_HEIGHT as f64),
    )
    .min_inner_size(320.0, 420.0);
    match geometry {
        Some(g) if position_on_a_monitor(app, g.x, g.y) => {
            builder = builder.position(g.x as f64, g.y as f64);
        }
        _ => builder = builder.center(),
    }
    builder.build()?;
    Ok(())
}

/// The frontend × button's backend half (ticket 52): the geometry is
/// remembered first, then the window is destroyed — the same fate as blur,
/// so the tray reopens it fresh.
pub fn close(app: &AppHandle) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window(QUICK_LAUNCH_WINDOW) else {
        return Ok(());
    };
    let position = window.outer_position().ok();
    let size = window.outer_size().ok();
    if let (Some(position), Some(size)) = (position, size) {
        remember(app, position.x, position.y, size.width, size.height);
    }
    window.destroy()
}

/// Remembers the window's outer position and size before it goes away —
/// called from the blur/close handlers in `lib.rs`. A failed save is never a
/// failure of the hide itself.
pub fn save_geometry(window: &Window) {
    let Ok(position) = window.outer_position() else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };
    remember(
        window.app_handle(),
        position.x,
        position.y,
        size.width,
        size.height,
    );
}

/// Writes one geometry snapshot to the `meta` table, best effort.
fn remember(app: &AppHandle, x: i32, y: i32, width: u32, height: u32) {
    let state = app.state::<AppState>();
    let Ok(conn) = state.db.lock() else {
        return;
    };
    let _ = crate::db::save_quick_window_geometry(
        &conn,
        &QuickWindowGeometry {
            x,
            y,
            width,
            height,
        },
    );
}

/// The stored geometry, when a sane one exists.
fn remembered_geometry(app: &AppHandle) -> Option<QuickWindowGeometry> {
    let state = app.state::<AppState>();
    let conn = state.db.lock().ok()?;
    crate::db::load_quick_window_geometry(&conn)
}

/// Whether the point sits inside any connected monitor — the off-screen
/// guard: a window remembered on a now-disconnected monitor is never opened
/// into the void.
fn position_on_a_monitor(app: &AppHandle, x: i32, y: i32) -> bool {
    app.monitor_from_point(x as f64, y as f64).unwrap_or(None).is_some()
}