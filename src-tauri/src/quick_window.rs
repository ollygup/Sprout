//! The Quick Launch window (tickets 52 & 53): a miniature, frameless second
//! window opened from the tray icon's left-click — raised when it is already
//! open. It shows two read-only tabs (Quick Launch / Quick Actions). Floating
//! (ticket 52), it hides to the tray on blur or close: the window is destroyed
//! and the tray reopens it — always at a fixed size, centered (the user asked
//! for a fixed-size palette, so geometry is never remembered; a remembered
//! near-full-screen size is what once made it open huge and impossible to
//! move). Docked (ticket 53), it becomes a Win32 AppBar on the left/right
//! screen edge — auto-hiding to a sliver or fixed like a pinned taskbar — and
//! blur no longer hides it (the OS owns the slide-in/out). This module owns
//! the window's creation and both dock commands' backend halves; the
//! blur/close handling lives in `lib.rs`'s `on_window_event` (keyed on
//! [`QUICK_LAUNCH_WINDOW`]).

use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder,
};

use crate::{appbar, db, settings, AppState};

/// The window's stable label — the event handler in `lib.rs` keys on it and
/// the capability scope names it.
pub const QUICK_LAUNCH_WINDOW: &str = "quick-launch";

/// The route the second webview loads — a SvelteKit SPA fallback serves it
/// in production, Vite's fallback in dev.
const ROUTE: &str = "quick-launch-window";

/// The window's one and only size — the user asked for a fixed-size palette,
/// so it is never resizable and never remembered.
const DEFAULT_WIDTH: u32 = 340;
const DEFAULT_HEIGHT: u32 = 460;

/// The docked form's live state: which edge and visibility mode the window is
/// currently docked with, and which monitor it is attached to (its device
/// name — the per-monitor memory key).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockState {
    pub edge: String,
    pub mode: String,
    pub monitor: String,
}

/// Opens the Quick Launch window: raises the existing one, or creates it at
/// the fixed default size, centered on the current monitor.
pub fn open(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(QUICK_LAUNCH_WINDOW) {
        // When the window is docked (ticket 53), this raises it into focus —
        // the tray's left-click and the docked bar coexist.
        window.set_focus()?;
        return Ok(());
    }
    WebviewWindowBuilder::new(
        app,
        QUICK_LAUNCH_WINDOW,
        WebviewUrl::App(ROUTE.into()),
    )
    .title("Sprout — Quick Launch")
    .decorations(false)
    .resizable(false)
    .skip_taskbar(true)
    .inner_size(DEFAULT_WIDTH as f64, DEFAULT_HEIGHT as f64)
    .min_inner_size(DEFAULT_WIDTH as f64, DEFAULT_HEIGHT as f64)
    .max_inner_size(DEFAULT_WIDTH as f64, DEFAULT_HEIGHT as f64)
    .center()
    .build()?;
    Ok(())
}

/// The frontend × button's backend half (tickets 52 & 53): when floating, the
/// window is destroyed — the same fate as blur, so the tray reopens it fresh
/// at its fixed centered size. When docked, the AppBar is released first (the
/// edge is never left occupied) and the window is destroyed; the tray reopens
/// it floating.
pub fn close(app: &AppHandle) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window(QUICK_LAUNCH_WINDOW) else {
        return Ok(());
    };
    if is_docked(app) {
        let _ = release_dock(app);
    }
    window.destroy()
}

/// Whether the window is currently docked (ticket 53). The blur/close
/// handlers in `lib.rs` and the tray's left-click key on it.
pub fn is_docked(app: &AppHandle) -> bool {
    app.state::<AppState>()
        .dock
        .lock()
        .map(|d| d.is_some())
        .unwrap_or(false)
}

/// The live dock state — the frontend's dock chrome reads it to render the
/// current edge and mode.
pub fn docked_state(app: &AppHandle) -> Option<DockState> {
    app.state::<AppState>().dock.lock().ok()?.clone()
}

/// Docks the window to the current monitor's remembered (or Settings-default)
/// edge: registers the AppBar, resizes to a full-height strip, and records
/// full-height strip, and records the per-monitor edge/mode. When already
/// docked, the window is repositioned to `edge` (an edge switch) without
/// unregistering.
pub fn dock(app: &AppHandle, edge: Option<&str>) -> Result<(), String> {
    if is_docked(app) {
        return reposition(app, edge);
    }
    let window = app
        .get_webview_window(QUICK_LAUNCH_WINDOW)
        .ok_or_else(|| "Quick Launch window is not open".to_string())?;
    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    let state = app.state::<AppState>();
    let (edge, mode, monitor) = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let settings = settings::load(&conn);
        let monitor = appbar::monitor_key(hwnd.0)
            .ok_or_else(|| "cannot identify the current monitor".to_string())?;
        let edge = edge
            .map(str::to_string)
            .or_else(|| db::load_dock_edge(&conn, &monitor))
            .unwrap_or_else(|| settings.dock_edge.clone());
        let mode = db::load_dock_mode(&conn, &monitor)
            .unwrap_or_else(|| settings.dock_mode.clone());
        settings::validate_dock_edge(&edge)?;
        settings::validate_dock_mode(&mode)?;
        let _ = db::save_dock_edge(&conn, &monitor, &edge);
        let _ = db::save_dock_mode(&conn, &monitor, &mode);
        (edge, mode, monitor)
    };
    // The dock state is recorded BEFORE the OS calls: auto-hide can hide the
    // window the moment it is enabled (losing focus), and the blur handler
    // must already see "docked" or it would destroy the window. A failed
    // registration rolls the state back.
    *state.dock.lock().map_err(|e| e.to_string())? = Some(DockState {
        edge: edge.clone(),
        mode: mode.clone(),
        monitor: monitor.clone(),
    });
    let edge_u32 = appbar::edge_constant(&edge).expect("validated edge");
    let work = appbar::work_area(hwnd.0)
        .ok_or_else(|| "cannot find the monitor work area".to_string())?;
    let desired = appbar::appbar_rect(work, edge_u32, appbar::DOCK_WIDTH);
    let register = appbar::register(hwnd.0, edge_u32, desired, mode == "auto-hide");
    let rect = match register {
        Ok(rect) => rect,
        Err(e) => {
            *state.dock.lock().map_err(|err| err.to_string())? = None;
            return Err(e);
        }
    };
    if let Err(e) = reshape(&window, rect) {
        // The bar was registered but could not be placed — release it so the
        // edge is never left occupied by a half-docked window.
        appbar::remove(hwnd.0);
        *state.dock.lock().map_err(|err| err.to_string())? = None;
        return Err(e);
    }
    Ok(())
}

/// Moves the docked window to `edge` (or its current edge) without
/// unregistering: re-queries the new edge's rect, commits it, and re-applies
/// the remembered auto-hide state.
fn reposition(app: &AppHandle, edge: Option<&str>) -> Result<(), String> {
    let window = app
        .get_webview_window(QUICK_LAUNCH_WINDOW)
        .ok_or_else(|| "Quick Launch window is not open".to_string())?;
    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    let state = app.state::<AppState>();
    let current = docked_state(app).ok_or_else(|| "Quick Launch window is not docked".to_string())?;
    let edge = edge
        .map(str::to_string)
        .unwrap_or_else(|| current.edge.clone());
    settings::validate_dock_edge(&edge)?;
    let edge_u32 = appbar::edge_constant(&edge).expect("validated edge");
    let work = appbar::work_area(hwnd.0)
        .ok_or_else(|| "cannot find the monitor work area".to_string())?;
    let desired = appbar::appbar_rect(work, edge_u32, appbar::DOCK_WIDTH);
    let rect = appbar::reposition(hwnd.0, edge_u32, desired, current.mode == "auto-hide")?;
    reshape(&window, rect)?;
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let _ = db::save_dock_edge(&conn, &current.monitor, &edge);
    }
    *state.dock.lock().map_err(|e| e.to_string())? = Some(DockState {
        edge,
        mode: current.mode,
        monitor: current.monitor,
    });
    Ok(())
}

/// Unregisters the AppBar and restores the floating window at its fixed
/// centered size. No-op when the window is not docked.
pub fn undock(app: &AppHandle) -> Result<(), String> {
    if !is_docked(app) {
        return Ok(());
    }
    let window = app
        .get_webview_window(QUICK_LAUNCH_WINDOW)
        .ok_or_else(|| "Quick Launch window is not open".to_string())?;
    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    appbar::remove(hwnd.0);
    {
        let state = app.state::<AppState>();
        *state.dock.lock().map_err(|e| e.to_string())? = None;
    }
    // An auto-hidden bar was hidden by the OS — after ABM_REMOVE it stays
    // hidden until shown; restore the visible floating window.
    window.show().map_err(|e| e.to_string())?;
    let size = PhysicalSize::new(DEFAULT_WIDTH, DEFAULT_HEIGHT);
    window
        .set_min_size(Some(size))
        .map_err(|e| e.to_string())?;
    window.set_size(size).map_err(|e| e.to_string())?;
    window.center().map_err(|e| e.to_string())?;
    Ok(())
}

/// Releases the AppBar (ABM_REMOVE) and clears the dock state without
/// reshaping — the quit path (ticket 53: the edge is never left occupied) and
/// the close path, where the window is destroyed right after. No-op when the
/// window is not docked.
pub fn release_dock(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut dock = state.dock.lock().map_err(|e| e.to_string())?;
    let Some(current) = dock.take() else {
        return Ok(());
    };
    if let Some(window) = app.get_webview_window(QUICK_LAUNCH_WINDOW) {
        if let Ok(hwnd) = window.hwnd() {
            appbar::remove(hwnd.0);
        }
    }
    drop(current);
    Ok(())
}

/// Resizes and repositions the window to the AppBar's rectangle — the strip
/// gets no minimums (the 320×420 floating minimums would fight a slim strip).
fn reshape(window: &tauri::WebviewWindow, rect: windows_sys::Win32::Foundation::RECT) -> Result<(), String> {
    window
        .set_min_size(None::<PhysicalSize<u32>>)
        .map_err(|e| e.to_string())?;
    window
        .set_position(PhysicalPosition::new(rect.left, rect.top))
        .map_err(|e| e.to_string())?;
    window
        .set_size(PhysicalSize::new(
            (rect.right - rect.left) as u32,
            (rect.bottom - rect.top) as u32,
        ))
        .map_err(|e| e.to_string())?;
    Ok(())
}