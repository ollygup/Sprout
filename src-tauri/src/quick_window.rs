//! The Quick Launch window (tickets 52 & 53, 56): a miniature, frameless
//! second window opened from the tray icon's left-click — raised when it is
//! already open. It shows two read-only tabs (Quick Launch / Quick Actions).
//! Floating (ticket 56), it is a persistent, freely draggable palette: blur
//! does nothing and it stays open until the × button / Alt+F4 destroys it —
//! the tray reopens it fresh — always at the fixed 340×460 size, centered (the
//! user asked for a fixed-size palette, so geometry is never remembered; a
//! remembered near-full-screen size is what once made it open huge and
//! impossible to move). Docked (ticket 53), it becomes a Win32 AppBar on the
//! left/right screen edge — auto-hiding to a sliver or fixed like a pinned
//! taskbar — where the OS owns the slide-in/out. This module owns the
//! window's creation and both dock commands' backend halves; the
//! close/destroy handling lives in `lib.rs`'s `on_window_event` (keyed on
//! [`QUICK_LAUNCH_WINDOW`]).
//!
//! Ticket 60: the docked bar's auto-hide is made real. The window's window
//! proc is subclassed (`SetWindowLongPtrW`) so the AppBar's registered
//! callback message (`ABN_*`) is seen instead of dropped by the default proc;
//! `ABN_STATECHANGE` reconciles the recorded mode against the shell's own
//! answer (`ABM_GETAUTOHIDEBAR`) and tells the frontend to re-read. Auto-hide
//! is applied per edge and a refused engagement keeps the dock while surfacing
//! the honest fixed mode.
//!
//! Ticket 61: the dock follows the documented AppBar pattern end to end. The
//! window is placed at the rect `ABM_SETPOS` grants (position+size in one
//! `SetWindowPos` call — atomic edge switches, no hide/show, no flicker);
//! `ABN_POSCHANGED` re-asserts the bar when the shell's work area changes; a
//! watchdog thread compares `GetWindowRect` against the last placed rect every
//! second and re-docks on drift (Win+Shift+→, monitor reconnect) — skipping
//! the check while an auto-hide bar is hidden so it never fights the OS's
//! slide. Every failure is logged with the actual `SHAppBarMessage` result
//! and surfaced in the window via the `quick-launch-dock-error` event; a
//! registered-but-unplaced bar is released, never left half-docked.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use rusqlite::Connection;
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalSize, WebviewUrl, WebviewWindowBuilder,
};
use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::UI::Shell::{ABN_POSCHANGED, ABN_STATECHANGE};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, DefWindowProcW, GWLP_WNDPROC, SetWindowLongPtrW, WM_NCDESTROY,
};

use crate::{
    appbar, db, settings, window_constants::DOCK_WIDTH, window_constants::WINDOW_HEIGHT,
    window_constants::WINDOW_WIDTH, AppState,
};

/// The window's stable label — the event handler in `lib.rs` keys on it and
/// the capability scope names it.
pub const QUICK_LAUNCH_WINDOW: &str = "quick-launch";

/// The route the second webview loads — a SvelteKit SPA fallback serves it
/// in production, Vite's fallback in dev.
const ROUTE: &str = "quick-launch-window";

/// The docked form's live state: which edge and visibility mode the window is
/// currently docked with, which monitor it is attached to (its device name —
/// the per-monitor memory key), and the rect the bar was last placed at (the
/// `ABM_SETPOS`-granted rect — the drift check's expected rect, ticket 61).
#[derive(Clone)]
pub struct DockState {
    pub edge: String,
    pub mode: String,
    pub monitor: String,
    pub last_rect: Option<RECT>,
}

/// The installed window-proc override per hwnd (ticket 60): the AppBar's
/// callback message must reach us to see `ABN_STATECHANGE`, and the default
/// proc would drop it. The original proc is kept so every message still chains
/// through to the webview. Keyed by `hwnd as isize`; an entry is removed when
/// the window dies (`WM_NCDESTROY`).
static SUBCLASSED: Mutex<Option<HashMap<isize, isize>>> = Mutex::new(None);
/// The app handle the `ABN_*` handler uses to reconcile and notify — captured
/// once on first install (the app outlives the window, so it is always valid).
static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

type AppBarWndProc = unsafe extern "system" fn(HWND, u32, usize, isize) -> isize;

/// Subclasses the Quick Launch window so the registered AppBar callback
/// message (`ABN_*` notifications) reaches us. Installed once per window,
/// before the AppBar registration; a no-op when already installed for `hwnd`.
fn install_appbar_callback(app: &AppHandle, hwnd: HWND) {
    let _ = APP_HANDLE.set(app.clone());
    let mut map = SUBCLASSED.lock().unwrap();
    if map.is_none() {
        *map = Some(HashMap::new());
    }
    let m = map.as_mut().unwrap();
    if m.contains_key(&(hwnd as isize)) {
        return;
    }
    unsafe {
        let old = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, appbar_proc as AppBarWndProc as isize);
        if old == 0 {
            // A failing hook just means we never see ABN_*; the dock still
            // works, so the failure is logged rather than failing the dock.
            eprintln!("Could not hook the Quick Launch window for app bar messages");
            return;
        }
        m.insert(hwnd as isize, old);
    }
}

/// The subclassed window proc: forwards everything to the original proc, and
/// watches for the AppBar callback message carrying `ABN_STATECHANGE` — the
/// shell's notice that the bar's auto-hide / always-on-top attributes changed
/// (its own context menu can toggle auto-hide without us asking) — or
/// `ABN_POSCHANGED` — the notice that the screen's work area changed (ticket
/// 61). The default proc would drop both.
unsafe extern "system" fn appbar_proc(
    hwnd: HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    if msg == appbar::callback_message() {
        match wparam as u32 {
            ABN_STATECHANGE => {
                if let Some(app) = APP_HANDLE.get() {
                    on_appbar_state_change(app, hwnd);
                }
            }
            ABN_POSCHANGED => {
                if let Some(app) = APP_HANDLE.get() {
                    on_appbar_pos_changed(app, hwnd);
                }
            }
            _ => {}
        }
    }
    let chain = |old: isize| {
        let prev: AppBarWndProc = std::mem::transmute(old);
        CallWindowProcW(Some(prev), hwnd, msg, wparam, lparam)
    };
    let old = SUBCLASSED
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(&(hwnd as isize)).copied());
    let result = match old {
        Some(old) => chain(old),
        None => DefWindowProcW(hwnd, msg, wparam, lparam),
    };
    if msg == WM_NCDESTROY {
        // The window is going away — drop our hook so a reused hwnd never
        // chains through a dead proc.
        if let Ok(mut map) = SUBCLASSED.lock() {
            if let Some(m) = map.as_mut() {
                m.remove(&(hwnd as isize));
            }
        }
    }
    result
}

/// The `ABN_STATECHANGE` handler: reconciles the recorded dock mode against
/// what `ABM_GETAUTOHIDEBAR` actually says on the current edge and tells the
/// frontend to re-read its dock state — the chrome stays honest when the OS
/// changed the mode out from under us.
fn on_appbar_state_change(app: &AppHandle, hwnd: HWND) {
    let Some(current) = docked_state(app) else {
        return;
    };
    let Some(edge_u32) = appbar::edge_constant(&current.edge) else {
        return;
    };
    let engaged = appbar::autohide_engaged(hwnd, edge_u32);
    reconcile_dock_mode(app, if engaged { "auto-hide" } else { "fixed" });
}

/// Writes `mode` into the live dock state and the monitor's dock memory and
/// tells the frontend to re-read. Used when the shell refused a requested
/// auto-hide engagement or changed the mode itself — the honest state is
/// surfaced, never assumed.
fn reconcile_dock_mode(app: &AppHandle, mode: &str) {
    let state = app.state::<AppState>();
    let monitor = {
        let mut dock = match state.dock.lock() {
            Ok(d) => d,
            Err(_) => return,
        };
        let monitor = dock.as_ref().map(|d| d.monitor.clone());
        if let Some(d) = dock.as_mut() {
            if d.mode != mode {
                d.mode = mode.to_string();
            }
        }
        monitor
    };
    if let Some(monitor) = monitor {
        if let Ok(conn) = state.db.lock() {
            let _ = db::save_dock_mode(&conn, &monitor, mode);
        }
    }
    let _ = app.emit("quick-launch-changed", ());
}

/// The `ABN_POSCHANGED` handler (ticket 61): the shell's notice that the
/// screen's work area changed — the taskbar was resized, moved, or hidden,
/// another app bar appeared, a monitor arrangement changed. The bar re-queries
/// and re-sets its position against the new work area and is placed at the
/// rect the shell granted. While an auto-hide bar is hidden the placement is
/// skipped (the OS owns the sliver; placing the full strip would pop the bar
/// out) — the reservation is re-asserted either way, and the granted rect is
/// remembered for the drift check.
fn on_appbar_pos_changed(app: &AppHandle, hwnd: HWND) {
    let Some(current) = docked_state(app) else {
        return;
    };
    let Some(edge_u32) = appbar::edge_constant(&current.edge) else {
        return;
    };
    let Some(work) = appbar::work_area(hwnd) else {
        return;
    };
    let Some(actual) = appbar::window_rect(hwnd) else {
        return;
    };
    // While an auto-hide bar is hidden the OS owns the sliver — never
    // reposition it (would fight the slide); just remember where it is.
    if current.mode == "auto-hide" {
        if let Some(expected) = current.last_rect {
            if !appbar::mostly_overlapping(actual, expected, 0.5) {
                return;
            }
        }
    }
    let desired = appbar::desired_rect(edge_u32, work, actual, DOCK_WIDTH as i32);
    // The shell recomputed the work area from the bar's own placement — the
    // bar is already exactly where the shell expects it (ticket 61): nothing
    // to re-assert, just remember the rect.
    if !appbar::rects_diverged(desired, actual, 0) {
        record_last_rect(app, Some(actual));
        return;
    }
    let rect = match appbar::reposition(hwnd, edge_u32, desired) {
        Ok(rect) => rect,
        Err(e) => {
            report_dock_error(app, &format!("Could not re-assert the docked bar: {e}"));
            return;
        }
    };
    let auto_hidden = match (current.mode.as_str(), appbar::window_rect(hwnd)) {
        ("auto-hide", Some(actual)) => !appbar::mostly_overlapping(actual, rect, 0.5),
        _ => false,
    };
    if !auto_hidden {
        if let Err(e) = appbar::place(hwnd, rect) {
            report_dock_error(app, &format!("Could not re-assert the docked bar: {e}"));
            return;
        }
    }
    record_last_rect(app, Some(rect));
}

/// Records the rect the docked bar was last placed at (ticket 61) — the drift
/// check's expected rect. A `None` forgets the placement (undock, release).
fn record_last_rect(app: &AppHandle, rect: Option<RECT>) {
    if let Ok(mut dock) = app.state::<AppState>().dock.lock() {
        if let Some(d) = dock.as_mut() {
            d.last_rect = rect;
        }
    }
}

/// Surfaces a dock failure in the window (ticket 61): logs one line and emits
/// the `quick-launch-dock-error` event the Quick Launch window renders as its
/// error banner — no silent half-dock, no pretending to be docked.
fn report_dock_error(app: &AppHandle, message: &str) {
    eprintln!("{message}");
    let _ = app.emit("quick-launch-dock-error", message.to_string());
}

/// Applies the docked window's auto-hide state for `mode` at `edge` and
/// verifies the shell engaged it. When the shell refuses (another auto-hide
/// bar owns the edge), the recorded mode is reconciled to the honest "fixed"
/// and the refusal is surfaced as an error — the dock itself stays.
fn apply_dock_mode(app: &AppHandle, hwnd: HWND, edge: &str, mode: &str) -> Result<(), String> {
    let edge_u32 = appbar::edge_constant(edge).expect("validated edge");
    match appbar::set_autohide(hwnd, edge_u32, mode == "auto-hide") {
        Ok(_) => Ok(()),
        Err(e) => {
            reconcile_dock_mode(app, "fixed");
            Err(e)
        }
    }
}

/// Opens the Quick Launch window: raises the existing one, or creates it at
/// the fixed default size, centered on the current monitor. Ticket 57: a
/// freshly created window honors the persisted dock state — when
/// `settings.dock_state` is "docked" it docks immediately (per-monitor
/// edge/mode memory, falling back to the settings defaults). A failed dock
/// leaves it floating rather than failing the open.
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
    .inner_size(WINDOW_WIDTH as f64, WINDOW_HEIGHT as f64)
    .min_inner_size(WINDOW_WIDTH as f64, WINDOW_HEIGHT as f64)
    .max_inner_size(WINDOW_WIDTH as f64, WINDOW_HEIGHT as f64)
    .center()
    .build()?;
    let state = app.state::<AppState>();
    let docked = state
        .db
        .lock()
        .map(|conn| settings::load(&conn).dock_state == "docked")
        .unwrap_or(false);
    if docked {
        if let Err(e) = dock(app, None) {
            // The window stays floating; the failure is surfaced in the
            // window's error banner rather than swallowed silently (ticket 61).
            report_dock_error(app, &format!("Could not dock the Quick Launch window: {e}"));
        }
    }
    Ok(())
}

/// The frontend × button's backend half (tickets 52, 53 & 56): the window is
/// destroyed — the only way the floating window closes, since blur no longer
/// destroys it — so the tray reopens it fresh at its fixed centered size.
/// When docked, the AppBar is released first (the edge is never left
/// occupied) and the window is destroyed; the tray reopens it floating.
pub fn close(app: &AppHandle) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window(QUICK_LAUNCH_WINDOW) else {
        return Ok(());
    };
    if is_docked(app) {
        let _ = release_dock(app);
    }
    window.destroy()
}

/// Whether the window is currently docked (ticket 53). The close handler in
/// `lib.rs` and the tray's left-click key on it.
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

/// Resolves the dock preferences for `monitor` the way `dock(None)` does
/// (ticket 59): the monitor's remembered edge/mode, falling back to the
/// Settings defaults. Pure — never persists anything.
fn resolve_dock_prefs(
    conn: &Connection,
    settings: &settings::Settings,
    monitor: &str,
) -> Result<(String, String), String> {
    let edge = db::load_dock_edge(conn, monitor).unwrap_or_else(|| settings.dock_edge.clone());
    let mode = db::load_dock_mode(conn, monitor).unwrap_or_else(|| settings.dock_mode.clone());
    settings::validate_dock_edge(&edge)?;
    settings::validate_dock_mode(&mode)?;
    Ok((edge, mode))
}

/// The dock the window's toggle would produce right now (ticket 59): while
/// the window floats, the target edge/mode the toggle's `dock(None)`
/// resolution would use — the current monitor's remembered values, falling
/// back to the Settings defaults. Read-only; the header renders the target
/// edge's icon from it.
pub fn pending_dock(app: &AppHandle) -> Result<(String, String), String> {
    let window = app
        .get_webview_window(QUICK_LAUNCH_WINDOW)
        .ok_or_else(|| "Quick Launch window is not open".to_string())?;
    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    let state = app.state::<AppState>();
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let settings = settings::load(&conn);
    let monitor = appbar::monitor_key(hwnd.0)
        .ok_or_else(|| "cannot identify the current monitor".to_string())?;
    resolve_dock_prefs(&conn, &settings, &monitor)
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
        let (resolved_edge, resolved_mode) = resolve_dock_prefs(&conn, &settings, &monitor)?;
        let edge = edge.map(str::to_string).unwrap_or(resolved_edge);
        settings::validate_dock_edge(&edge)?;
        let mode = resolved_mode;
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
        last_rect: None,
    });
    let edge_u32 = appbar::edge_constant(&edge).expect("validated edge");
    let work = appbar::work_area(hwnd.0)
        .ok_or_else(|| "cannot find the monitor work area".to_string())?;
    let desired = appbar::appbar_rect(work, edge_u32, DOCK_WIDTH as i32);
    // The callback hook is in place before ABM_NEW so the shell's ABN_*
    // notifications are seen from the first moment (ticket 60).
    install_appbar_callback(app, hwnd.0);
    let register = appbar::register(hwnd.0, edge_u32, desired);
    let rect = match register {
        Ok(rect) => rect,
        Err(e) => {
            // A registration that partially succeeded (ABM_NEW accepted but a
            // later step refused) must not leave a half-docked bar: release
            // the AppBar so the edge is fully free (ticket 61).
            appbar::remove(hwnd.0);
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
    record_last_rect(app, Some(rect));
    // The mode is applied after the bar is registered and placed: a refused
    // auto-hide keeps the dock and surfaces the honest fixed mode instead of
    // unwinding the registration.
    apply_dock_mode(app, hwnd.0, &edge, &mode)?;
    Ok(())
}

/// Moves the docked window to `edge` (or its current edge) without
/// unregistering: re-queries the new edge's rect, re-applies the strip
/// thickness, commits it, and places the window at the granted rect in one
/// atomic `SetWindowPos` call — no hide/show, no flicker (ticket 61). The
/// remembered auto-hide state is re-applied after the move. A placement
/// failure after the reservation moved releases the AppBar — a half-docked
/// bar is never left behind.
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
    let actual = appbar::window_rect(hwnd.0).unwrap_or(work);
    // Never re-derive a same-edge dock from the work area alone: it already
    // reflects the bar's own reservation, which would grant a rect one width
    // into the screen (ticket 61). `desired_rect` keeps the current rect when
    // the bar is exactly where the shell expects it.
    let desired = appbar::desired_rect(edge_u32, work, actual, DOCK_WIDTH as i32);
    let rect = appbar::reposition(hwnd.0, edge_u32, desired)?;
    // The new edge's position is committed first; only then is the old edge's
    // auto-hide released — a refused query leaves the old dock untouched.
    if current.edge != edge {
        let old_edge_u32 = appbar::edge_constant(&current.edge).expect("validated edge");
        let _ = appbar::set_autohide(hwnd.0, old_edge_u32, false);
    }
    if let Err(e) = reshape(&window, rect) {
        // The reservation moved to the new edge but the window did not — a
        // half-docked bar: release the AppBar and report instead of leaving
        // the overlap (ticket 61).
        appbar::remove(hwnd.0);
        *state.dock.lock().map_err(|err| err.to_string())? = None;
        return Err(e);
    }
    record_last_rect(app, Some(rect));
    apply_dock_mode(app, hwnd.0, &edge, &current.mode)?;
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let _ = db::save_dock_edge(&conn, &current.monitor, &edge);
    }
    *state.dock.lock().map_err(|e| e.to_string())? = Some(DockState {
        edge,
        mode: current.mode,
        monitor: current.monitor,
        last_rect: Some(rect),
    });
    Ok(())
}

/// Unregisters the AppBar and restores the floating window at its fixed
/// centered size. No-op when the window is not docked.
///
/// The size is restored with the same inner-size family the builder used
/// (inner + min + max — `set_size` is the inner-size setter on the frameless
/// palette, `set_min_size`/`set_max_size` take inner sizes) and in the same
/// logical units: the builder's `inner_size(340, 460)` is logical (tauri
/// docs: "Window size in logical pixels"), so restoring with physical pixels
/// would shrink the window on any scaled display (e.g. 340 physical at 125%
/// = 272 logical). Logical restores the exact first-open size on any scale,
/// and the dock → undock → dock → undock round trip is lossless — the
/// window is never smaller than the floating 340×460.
pub fn undock(app: &AppHandle) -> Result<(), String> {
    if !is_docked(app) {
        return Ok(());
    }
    let window = app
        .get_webview_window(QUICK_LAUNCH_WINDOW)
        .ok_or_else(|| "Quick Launch window is not open".to_string())?;
    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    if let Some(current) = docked_state(app) {
        // Release the auto-hide registration on the docked edge before the bar
        // itself — the edge is fully given back (ticket 60).
        if let Some(edge) = appbar::edge_constant(&current.edge) {
            let _ = appbar::set_autohide(hwnd.0, edge, false);
        }
    }
    appbar::remove(hwnd.0);
    {
        let state = app.state::<AppState>();
        *state.dock.lock().map_err(|e| e.to_string())? = None;
    }
    // An auto-hidden bar was hidden by the OS — after ABM_REMOVE it stays
    // hidden until shown; restore the visible floating window.
    window.show().map_err(|e| e.to_string())?;
    let size = LogicalSize::new(WINDOW_WIDTH as f64, WINDOW_HEIGHT as f64);
    window
        .set_min_size(Some(size))
        .map_err(|e| e.to_string())?;
    window
        .set_max_size(Some(size))
        .map_err(|e| e.to_string())?;
    window.set_size(size).map_err(|e| e.to_string())?;
    window.center().map_err(|e| e.to_string())?;
    Ok(())
}

/// Re-applies the docked window's visibility mode (ticket 57): switches the
/// live window between "auto-hide" and "fixed" without undocking — the mode
/// change from the Settings screen lands immediately. No-op while floating.
/// The mode is persisted to the monitor's dock memory so the window's own
/// future docks stay aligned.
pub fn set_dock_mode(app: &AppHandle, mode: &str) -> Result<(), String> {
    settings::validate_dock_mode(mode)?;
    let Some(current) = docked_state(app) else {
        return Ok(());
    };
    if current.mode == mode {
        return Ok(());
    }
    let window = app
        .get_webview_window(QUICK_LAUNCH_WINDOW)
        .ok_or_else(|| "Quick Launch window is not open".to_string())?;
    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    {
        let state = app.state::<AppState>();
        *state.dock.lock().map_err(|e| e.to_string())? = Some(DockState {
            mode: mode.to_string(),
            ..current.clone()
        });
    }
    // A refused auto-hide reconciles the mode to the honest "fixed" and
    // returns the refusal — the requested mode is only persisted on success.
    apply_dock_mode(app, hwnd.0, &current.edge, mode)?;
    {
        let state = app.state::<AppState>();
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let _ = db::save_dock_mode(&conn, &current.monitor, mode);
    }
    Ok(())
}

/// Applies the dock-related settings to a live window (ticket 57) — the
/// Settings screen's changes land without reopening the window: a state
/// change docks/undocks, an edge change repositions, a mode change re-applies
/// auto-hide. No-op while the window is closed; the settings' values win over
/// the per-monitor memory (the user just asked for them explicitly).
pub fn apply_settings(app: &AppHandle, settings: &settings::Settings) -> Result<(), String> {
    if app.get_webview_window(QUICK_LAUNCH_WINDOW).is_none() {
        return Ok(());
    }
    match (is_docked(app), settings.dock_state.as_str()) {
        (false, "docked") => dock(app, None)?,
        (true, "floating") => undock(app)?,
        _ => {}
    }
    if is_docked(app) {
        let current = docked_state(app)
            .ok_or_else(|| "Quick Launch window is not docked".to_string())?;
        if current.edge != settings.dock_edge {
            dock(app, Some(&settings.dock_edge))?;
        }
        if current.mode != settings.dock_mode {
            set_dock_mode(app, &settings.dock_mode)?;
        }
    }
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
            // Release any auto-hide registration on the docked edge too — the
            // edge is fully given back, not left half-occupied (ticket 60).
            if let Some(edge) = appbar::edge_constant(&current.edge) {
                let _ = appbar::set_autohide(hwnd.0, edge, false);
            }
            appbar::remove(hwnd.0);
        }
    }
    drop(current);
    Ok(())
}

/// Places the docked window at the AppBar's granted rect — position and size
/// applied together in one `SetWindowPos` call, so re-docks and edge switches
/// never hide/show or flicker (ticket 61). The strip gets no min/max (the
/// floating 340×460 constraints would clamp the full-height strip, leaving
/// the window permanently shorter than the rect the shell reserved — a
/// divergence the drift watchdog would then fight every second).
fn reshape(window: &tauri::WebviewWindow, rect: RECT) -> Result<(), String> {
    window
        .set_min_size(None::<PhysicalSize<u32>>)
        .map_err(|e| e.to_string())?;
    window
        .set_max_size(None::<PhysicalSize<u32>>)
        .map_err(|e| e.to_string())?;
    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    appbar::place(hwnd.0, rect)
}

/// Starts the dock drift watchdog (ticket 61): one thread for the app's
/// lifetime that every second compares the window's `GetWindowRect` against
/// the rect the bar was last placed at. A persistent divergence — the bar was
/// moved (Win+Shift+→), a monitor reconnected, a stray resize — re-docks it
/// (re-query, re-set, atomic placement). A single transient divergence (e.g.
/// the OS mid auto-hide slide) never yanks the bar: the re-dock only fires
/// after two consecutive divergent ticks. In auto-hide mode the re-assert is
/// skipped while the OS has the bar hidden (its sliver overlaps a tiny
/// fraction of the strip), so the check never fights the OS's slide. Failures
/// are logged and surfaced in the window via `quick-launch-dock-error` —
/// never silent.
pub fn start_drift_guard(app: AppHandle) {
    std::thread::spawn(move || {
        let mut consecutive: u32 = 0;
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            match drift_check(&app, &mut consecutive) {
                Ok(true) => eprintln!("dock drift: bar re-docked to its edge"),
                Ok(false) => {}
                Err(e) => report_dock_error(&app, &format!("Could not re-dock the bar: {e}")),
            }
        }
    });
}

/// One drift-check pass (ticket 61): re-dock when the window's actual rect
/// diverges from the last placed rect. Returns `Ok(true)` when the bar was
/// re-docked. Nothing to do while the window floats, is closed, or has never
/// been placed.
fn drift_check(app: &AppHandle, consecutive: &mut u32) -> Result<bool, String> {
    let Some(current) = docked_state(app) else {
        return Ok(false);
    };
    let Some(window) = app.get_webview_window(QUICK_LAUNCH_WINDOW) else {
        return Ok(false);
    };
    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    let Some(actual) = appbar::window_rect(hwnd.0) else {
        return Ok(false);
    };
    let Some(expected) = current.last_rect else {
        return Ok(false);
    };
    if !appbar::rects_diverged(actual, expected, 2) {
        *consecutive = 0;
        return Ok(false);
    }
    // The OS owns the hidden state of an auto-hide bar: while hidden the
    // sliver overlaps a tiny fraction of the strip, so only re-assert when
    // the bar is revealed — never fight the OS's slide.
    if current.mode == "auto-hide" && !appbar::mostly_overlapping(actual, expected, 0.5) {
        *consecutive = 0;
        return Ok(false);
    }
    // A single stray sample (e.g. the OS mid slide) must not yank the bar —
    // only re-dock when the divergence persists across ticks.
    *consecutive += 1;
    if *consecutive < 2 {
        return Ok(false);
    }
    *consecutive = 0;
    let edge_u32 = appbar::edge_constant(&current.edge).ok_or("invalid dock edge")?;
    let work = appbar::work_area(hwnd.0)
        .ok_or_else(|| "cannot find the monitor work area".to_string())?;
    // The same self-aware derivation as the ABN handler (ticket 61): when the
    // work area already reflects the bar's own reservation the bar is where
    // the shell expects it — a re-dock derived from the work area would march
    // the bar into the screen instead of healing the drift.
    let desired = appbar::desired_rect(edge_u32, work, actual, DOCK_WIDTH as i32);
    let rect = appbar::reposition(hwnd.0, edge_u32, desired)?;
    appbar::place(hwnd.0, rect)?;
    record_last_rect(app, Some(rect));
    // A monitor reconnect can leave the new edge without an auto-hide
    // registration — re-assert it (a refusal reconciles to the honest mode).
    if current.mode == "auto-hide" {
        apply_dock_mode(app, hwnd.0, &current.edge, "auto-hide")?;
    }
    let _ = app.emit("quick-launch-changed", ());
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir() -> std::path::PathBuf {
        tempfile::tempdir().unwrap().into_path()
    }

    #[test]
    fn dock_prefs_fall_back_to_settings_defaults() {
        let dir = test_dir();
        let conn = db::init_at(&dir).unwrap();
        let settings = settings::Settings::default();
        // A fresh database remembers nothing — the Settings defaults win.
        let (edge, mode) = resolve_dock_prefs(&conn, &settings, r"\\.\DISPLAY1").unwrap();
        assert_eq!(edge, settings::DEFAULT_DOCK_EDGE);
        assert_eq!(mode, settings::DEFAULT_DOCK_MODE);
    }

    #[test]
    fn dock_prefs_prefer_per_monitor_memory() {
        let dir = test_dir();
        let conn = db::init_at(&dir).unwrap();
        db::save_dock_edge(&conn, r"\\.\DISPLAY1", "right").unwrap();
        db::save_dock_mode(&conn, r"\\.\DISPLAY1", "fixed").unwrap();
        let settings = settings::Settings::default();
        // The monitor's own memory overrides the Settings defaults…
        let (edge, mode) = resolve_dock_prefs(&conn, &settings, r"\\.\DISPLAY1").unwrap();
        assert_eq!(edge, "right");
        assert_eq!(mode, "fixed");
        // …and a different monitor still gets the defaults.
        let (edge, mode) = resolve_dock_prefs(&conn, &settings, r"\\.\DISPLAY2").unwrap();
        assert_eq!(edge, settings::DEFAULT_DOCK_EDGE);
        assert_eq!(mode, settings::DEFAULT_DOCK_MODE);
    }

    #[test]
    fn dock_prefs_ignore_invalid_stored_values() {
        let dir = test_dir();
        let conn = db::init_at(&dir).unwrap();
        // A broken stored edge (a leftover from a buggy build) reads back as
        // None — the Settings default wins, never an invalid edge.
        db::save_dock_edge(&conn, r"\\.\DISPLAY1", "top").unwrap();
        let settings = settings::Settings::default();
        let (edge, mode) = resolve_dock_prefs(&conn, &settings, r"\\.\DISPLAY1").unwrap();
        assert_eq!(edge, settings::DEFAULT_DOCK_EDGE);
        assert_eq!(mode, settings::DEFAULT_DOCK_MODE);
    }
}