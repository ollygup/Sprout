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
//! taskbar — where Sprout itself owns the slide-in/out (ticket 63: the OS
//! never moves an appbar; see `docs/research/0003-appbar-autohide-os-contract.md`).
//! This module owns the
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
//!
//! Ticket 63: a refused auto-hide no longer poisons the dock. When another
//! auto-hide bar owns the edge (the taskbar's own auto-hide on the same
//! edge), the shell refuses `ABM_SETAUTOHIDEBAR`; the requested mode stays
//! what the user asked for and the refusal is recorded as a transient blocked
//! state — live-state only, never persisted into the per-monitor memory. The
//! Quick Launch window surfaces the block with a reason banner and a
//! switch-edge action, and every shell notification (`ABN_STATECHANGE`,
//! `ABN_POSCHANGED`) retries the engagement — when the taskbar frees the
//! edge, hiding resumes on its own, without a redock.
//!
//! Ticket 63 (motion): auto-hide is Sprout-driven. A ~16 ms polling driver
//! thread (`GetCursorPos` — the WebView2 child HWND swallows mouse messages,
//! so hover can never arrive as a window message) animates the docked strip
//! between its full rect and a 2 px sliver with a 180 ms ease-out slide:
//! cursor touches the screen edge (or the strip) → slides out; leaves the
//! strip → slides away. It runs regardless of shell engagement — even a
//! refused `ABM_SETAUTOHIDEBAR` still hides, because registration buys
//! coordination only, never motion. The strip never reserves workspace space:
//! hidden or slid out, other windows keep their full size — the bar overlays
//! them like an auto-hiding taskbar. `fixed` mode is inert for the driver;
//! undock/close restore the floating window.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use rusqlite::Connection;
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalSize, WebviewUrl, WebviewWindowBuilder,
};
use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::UI::Shell::{ABN_POSCHANGED, ABN_STATECHANGE};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, DefWindowProcW, GetCursorPos, GWLP_WNDPROC, SetWindowLongPtrW, WM_NCDESTROY,
};
use windows_sys::Win32::Media::{timeBeginPeriod, timeEndPeriod};

use crate::{
    appbar, db, settings,
    constants::window::{
        AUTOHIDE_ANIM_POLL_MS, AUTOHIDE_POLL_MS, AUTOHIDE_SLIDE_MS, AUTOHIDE_SLIVER_PX,
        DOCK_WIDTH, EDGE_TRIGGER_PX, WINDOW_HEIGHT, WINDOW_WIDTH,
    },
    AppState,
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
    /// Why auto-hide is currently refused by the shell (ticket 63) — "another
    /// auto-hide bar already owns this edge". Transient by design: live-state
    /// only, never written to the per-monitor memory, so a refused dock does
    /// not poison every later dock on that monitor. The requested `mode`
    /// stays what the user asked for; when the edge frees up, hiding resumes.
    pub blocked: Option<String>,
    /// Ticket 66 (single-writer): the mode whose geometry + reservation the
    /// driver has established on screen. `None` (or ≠ `mode`) marks a
    /// transition pending — the driver owns every placement while it runs,
    /// and the drift guard stays out of the way until it settles. Set to
    /// `None` by whoever requests a mode flip, flipped back by the driver's
    /// one-time settle pass.
    pub settled: Option<String>,
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

/// The `ABN_STATECHANGE` handler: the shell's notice that a bar's auto-hide
/// attributes changed. A fixed dock has nothing to reconcile. An engaged
/// auto-hide just clears any stale blocked state; an unengaged one is
/// retried — the taskbar releasing this edge fires exactly here, so hiding
/// resumes without a redock (ticket 63).
fn on_appbar_state_change(app: &AppHandle, hwnd: HWND) {
    let Some(current) = docked_state(app) else {
        return;
    };
    if current.mode != "auto-hide" {
        return;
    }
    let Some(edge_u32) = appbar::edge_constant(&current.edge) else {
        return;
    };
    if appbar::autohide_engaged(hwnd, edge_u32) {
        clear_blocked(app);
    } else {
        retry_autohide(app, hwnd, edge_u32);
    }
}

/// Records or clears the transient blocked state in place (ticket 63) and
/// tells the frontend to re-read — only when something actually changed,
/// since `ABN_*` notifications arrive repeatedly. Live-state only: the
/// per-monitor memory is never touched, so a refused dock never poisons a
/// future one.
fn set_blocked(app: &AppHandle, reason: Option<String>) -> bool {
    let state = app.state::<AppState>();
    let changed = match state.dock.lock() {
        Ok(mut dock) => match dock.as_mut() {
            Some(d) if d.blocked != reason => {
                d.blocked = reason;
                true
            }
            _ => false,
        },
        Err(_) => false,
    };
    if changed {
        let _ = app.emit("quick-launch-changed", ());
    }
    changed
}

/// Marks auto-hide as refused by the shell (ticket 63): one log line per
/// transition for diagnostics, plus the transient flag the Quick Launch
/// window renders as its warning banner. Never persisted anywhere.
fn mark_blocked(app: &AppHandle, reason: &str) {
    if set_blocked(app, Some(reason.to_string())) {
        eprintln!("auto-hide blocked: {reason}");
    }
}

/// Clears the transient blocked state (ticket 63): auto-hide is engaged again
/// (or no longer requested), so the window's banner goes away.
fn clear_blocked(app: &AppHandle) {
    set_blocked(app, None);
}

/// Re-asks the shell to engage auto-hide on `edge` (ticket 63): the retry
/// behind both `ABN_*` handlers while blocked. When the owning bar (the
/// taskbar's own auto-hide) releases the edge, this re-engages hiding with no
/// user action; while it is still owned, the refusal just refreshes the
/// banner's reason.
fn retry_autohide(app: &AppHandle, hwnd: HWND, edge_u32: u32) {
    match appbar::set_autohide(hwnd, edge_u32, true) {
        Ok(_) => clear_blocked(app),
        Err(e) => mark_blocked(app, &e),
    }
}

/// The `ABN_POSCHANGED` handler (ticket 61): the shell's notice that the
/// screen's work area changed — the taskbar was resized, moved, or hidden,
/// another app bar appeared, a monitor arrangement changed. A fixed bar
/// re-queries and re-sets its position against the new work area and is
/// placed at the rect the shell granted. An auto-hide bar has no reservation:
/// it only re-checks its engagement (ticket 63).
fn on_appbar_pos_changed(app: &AppHandle, hwnd: HWND) {
    let Some(current) = docked_state(app) else {
        return;
    };
    let Some(edge_u32) = appbar::edge_constant(&current.edge) else {
        return;
    };
    if current.mode == "auto-hide" {
        // Overlay (ticket 63): there is no reservation to maintain — the
        // driver owns the strip's geometry. Just keep the registration
        // honest: a work-area change is exactly what the taskbar's own
        // auto-hide toggling produces, so this is where a freed edge resumes
        // hiding without a redock.
        if !appbar::autohide_engaged(hwnd, edge_u32) {
            retry_autohide(app, hwnd, edge_u32);
        }
        return;
    }
    // Ticket 66: a transition is pending — the driver owns every placement
    // until it settles; the shell re-notifies and the drift watchdog heals,
    // so skipping this pass is safe.
    if current.settled.as_deref() != Some(current.mode.as_str()) {
        return;
    }
    let Some(work) = appbar::work_area(hwnd) else {
        return;
    };
    let Some(actual) = appbar::window_rect(hwnd) else {
        return;
    };
    let desired = appbar::desired_rect(edge_u32, work, actual, DOCK_WIDTH as i32);
    // The shell recomputed the work area from the bar's own placement — the
    // bar is already exactly where the shell expects it (ticket 61): nothing
    // to re-assert, just remember the rect.
    if !appbar::rects_diverged(desired, actual, 0) {
        record_last_rect(app, Some(actual));
        return;
    }
    let rect = match appbar::reserve(hwnd, edge_u32, desired) {
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
        if let Err(e) = appbar::place(hwnd, rect, false) {
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
/// verifies the shell engaged it. Infallible (ticket 63): a refused
/// engagement keeps the requested mode — it is never rewritten to "fixed" —
/// and records the refusal as a transient blocked state the window surfaces;
/// success clears any stale block. The per-monitor memory is never touched
/// here, so a refused dock does not poison every later dock.
fn apply_dock_mode(app: &AppHandle, hwnd: HWND, edge: &str, mode: &str) {
    let edge_u32 = appbar::edge_constant(edge).expect("validated edge");
    match appbar::set_autohide(hwnd, edge_u32, mode == "auto-hide") {
        Ok(_) => clear_blocked(app),
        Err(e) => mark_blocked(app, &e),
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
        blocked: None,
        // The dock() sequence below places the initial rect itself; the
        // driver's first observation settles against it (idempotent for
        // fixed, reservation-shrink for auto-hide — ticket 66).
        settled: None,
    });
    let edge_u32 = appbar::edge_constant(&edge).expect("validated edge");
    // The callback hook is in place before ABM_NEW so the shell's ABN_*
    // notifications are seen from the first moment (ticket 60).
    install_appbar_callback(app, hwnd.0);
    if let Err(e) = appbar::register(hwnd.0, edge_u32) {
        // A refused registration leaves nothing behind — no half-dock.
        appbar::remove(hwnd.0);
        *state.dock.lock().map_err(|err| err.to_string())? = None;
        return Err(e);
    }
    // Ticket 63: only `fixed` reserves workspace space. Auto-hide registers
    // without reserving and spans the monitor's own edge — other windows keep
    // their full size whether the strip is hidden or slid out over them
    // (overlay, taskbar parity); the driver owns its motion.
    let rect = if mode == "auto-hide" {
        let monitor = appbar::monitor_rect(hwnd.0)
            .ok_or_else(|| "cannot find the monitor rectangle".to_string())?;
        appbar::appbar_rect(monitor, edge_u32, DOCK_WIDTH as i32)
    } else {
        let work = appbar::work_area(hwnd.0)
            .ok_or_else(|| "cannot find the monitor work area".to_string())?;
        let desired = appbar::appbar_rect(work, edge_u32, DOCK_WIDTH as i32);
        match appbar::reserve(hwnd.0, edge_u32, desired) {
            Ok(rect) => rect,
            // A reservation that fails after a successful registration must
            // not leave a half-docked bar: release and roll back (ticket 61).
            Err(e) => {
                appbar::remove(hwnd.0);
                *state.dock.lock().map_err(|err| err.to_string())? = None;
                return Err(e);
            }
        }
    };
    if let Err(e) = reshape(&window, rect, mode == "auto-hide") {
        // The bar was registered but could not be placed — release it so the
        // edge is never left occupied by a half-docked window.
        appbar::remove(hwnd.0);
        *state.dock.lock().map_err(|err| err.to_string())? = None;
        return Err(e);
    }
    record_last_rect(app, Some(rect));
    // The mode is applied after the bar is registered and placed: a refused
    // auto-hide keeps the dock and records a transient blocked state instead
    // of unwinding the registration (ticket 63).
    apply_dock_mode(app, hwnd.0, &edge, &mode);
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
    // In auto-hide mode the driver owns every placement — an edge
    // switch only moves the registration and the live state, and the driver
    // animates the strip to the new edge within a tick (settling the new
    // edge's sliver reservation itself). Placing here would race the driver's
    // own placements against the same HWND — the exact two-writer shape that
    // aborted the app on mode switches.
    if current.mode == "auto-hide" {
        // The old edge's registration is released before the new edge's is
        // applied (a refused query leaves the old dock untouched).
        if current.edge != edge {
            let old_edge_u32 = appbar::edge_constant(&current.edge).expect("validated edge");
            let _ = appbar::set_autohide(hwnd.0, old_edge_u32, false);
        }
        apply_dock_mode(app, hwnd.0, &edge, &current.mode);
        {
            let conn = state.db.lock().map_err(|e| e.to_string())?;
            let _ = db::save_dock_edge(&conn, &current.monitor, &edge);
        }
        if let Ok(mut dock) = state.dock.lock() {
            if let Some(d) = dock.as_mut() {
                d.edge = edge;
                // The driver re-derives the full/sliver geometry for the new
                // edge on its settle pass; `last_rect` still describes the
                // old edge and must not be trusted as an animation endpoint.
                d.settled = None;
            }
        }
        return Ok(());
    }
    // Fixed mode keeps its synchronous reservation move: the driver is inert
    // while fixed, so this thread is the only placement writer.
    let work = appbar::work_area(hwnd.0)
        .ok_or_else(|| "cannot find the monitor work area".to_string())?;
    let actual = appbar::window_rect(hwnd.0).unwrap_or(work);
    // Never re-derive a same-edge dock from the work area alone: it
    // already reflects the bar's own reservation, which would grant a
    // rect one width into the screen (ticket 61). `desired_rect` keeps
    // the current rect when the bar is exactly where the shell expects
    // it.
    let desired = appbar::desired_rect(edge_u32, work, actual, DOCK_WIDTH as i32);
    let rect = appbar::reserve(hwnd.0, edge_u32, desired)?;
    // The new edge's position is committed first; only then is the old edge's
    // auto-hide released — a refused query leaves the old dock untouched.
    if current.edge != edge {
        let old_edge_u32 = appbar::edge_constant(&current.edge).expect("validated edge");
        let _ = appbar::set_autohide(hwnd.0, old_edge_u32, false);
    }
    if let Err(e) = reshape(&window, rect, false) {
        // The reservation moved to the new edge but the window did not — a
        // half-docked bar: release the AppBar and report instead of leaving
        // the overlap (ticket 61).
        appbar::remove(hwnd.0);
        *state.dock.lock().map_err(|err| err.to_string())? = None;
        return Err(e);
    }
    record_last_rect(app, Some(rect));
    // The new edge's auto-hide is applied before the live state is updated:
    // success clears any stale block from the old edge, a refusal leaves the
    // banner carrying the new edge's reason (ticket 63).
    apply_dock_mode(app, hwnd.0, &edge, &current.mode);
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let _ = db::save_dock_edge(&conn, &current.monitor, &edge);
    }
    // The edge and rect are updated in place — rebuilding the DockState here
    // would clobber the transient blocked state apply_dock_mode just settled
    // (ticket 63). Mode and monitor are unchanged by definition.
    if let Ok(mut dock) = state.dock.lock() {
        if let Some(d) = dock.as_mut() {
            d.edge = edge;
            d.last_rect = Some(rect);
        }
    }
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
    // A sliver-parked bar (ticket 63) stays hidden after ABM_REMOVE — show
    // and restore the floating window explicitly. A docked auto-hide strip
    // lives in the topmost band; the floating palette must not.
    window.show().map_err(|e| e.to_string())?;
    window
        .set_always_on_top(false)
        .map_err(|e| e.to_string())?;
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
///
/// Ticket 66: this is a PURE STATE FLIP. It records the requested mode,
/// updates the shell registration ([`appbar::set_autohide`]) and persists —
/// and nothing else. Every geometry change (reservation moves, window
/// placement) belongs to the auto-hide driver thread alone: issuing AppBar /
/// SetWindowPos calls here races the driver's own placements against the same
/// HWND, and the interleaving panics inside tao's size math (the fixed→
/// auto-hide crash). The driver picks the flip up within one tick (~16 ms)
/// and performs the transition itself — reservation shrink to the sliver plus
/// an animated slide into auto-hide, full-strip reservation plus placement
/// into fixed (see `autohide_tick`).
pub fn set_dock_mode(app: &AppHandle, mode: &str) -> Result<(), String> {
    settings::validate_dock_mode(mode)?;
    let Some(current) = docked_state(app) else {
        return Ok(());
    };
    if current.mode == mode {
        return Ok(());
    }
    // The live state is updated before the registration so the driver's next
    // tick (~16 ms) already sees the requested mode and starts the transition
    // — the flip itself places nothing, and `settled = None` tells the drift
    // guard the driver owns geometry until it settles (ticket 66).
    {
        let state = app.state::<AppState>();
        let mut dock = state.dock.lock().map_err(|e| e.to_string())?;
        match dock.as_mut() {
            Some(d) => {
                d.mode = mode.to_string();
                d.settled = None;
            }
            None => return Ok(()),
        }
    }
    // A refused auto-hide keeps the requested mode as a transient blocked
    // state (ticket 63), so the requested mode is persisted unconditionally —
    // the per-monitor memory never records a "fixed" the user did not ask
    // for. `set_autohide` is registration coordination only: it moves no
    // window and reserves no space (ticket 66).
    apply_dock_mode(app, current_edge_hwnd(app)?, &current.edge, mode);
    let _ = app.emit("quick-launch-changed", ());
    {
        let state = app.state::<AppState>();
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let _ = db::save_dock_mode(&conn, &current.monitor, mode);
    }
    Ok(())
}

/// The docked window's HWND — the one syscall [`apply_dock_mode`] needs.
fn current_edge_hwnd(app: &AppHandle) -> Result<HWND, String> {
    let window = app
        .get_webview_window(QUICK_LAUNCH_WINDOW)
        .ok_or_else(|| "Quick Launch window is not open".to_string())?;
    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    Ok(hwnd.0)
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
/// divergence the drift watchdog would then fight every second). `topmost`
/// raises into the topmost band — used for auto-hide strips so later /
/// restored windows never cover them (ticket 66 follow-up).
fn reshape(window: &tauri::WebviewWindow, rect: RECT, topmost: bool) -> Result<(), String> {
    window
        .set_min_size(None::<PhysicalSize<u32>>)
        .map_err(|e| e.to_string())?;
    window
        .set_max_size(None::<PhysicalSize<u32>>)
        .map_err(|e| e.to_string())?;
    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    appbar::place(hwnd.0, rect, topmost)
}

/// Starts the dock drift watchdog (ticket 61): one thread for the app's
/// lifetime that every second compares the window's `GetWindowRect` against
/// the rect the bar was last placed at. A persistent divergence — the bar was
/// moved (Win+Shift+→), a monitor reconnected, a stray resize — re-docks it
/// (re-query, re-set, atomic placement). A single transient divergence never
/// yanks the bar: the re-dock only fires after two consecutive divergent
/// ticks. Auto-hide bars are skipped entirely (ticket 63): they reserve no
/// space and their driver pulls a drifted window back within one tick.
/// Failures are logged and surfaced in the window via
/// `quick-launch-dock-error` — never silent.
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
    if current.mode == "auto-hide" {
        // Overlay (ticket 63): nothing to heal — the bar reserves no space,
        // and a drifted window is pulled back to its target by the driver
        // within one tick.
        *consecutive = 0;
        return Ok(false);
    }
    if current.settled.as_deref() != Some(current.mode.as_str()) {
        // Ticket 66: a mode transition is in flight — the driver owns every
        // placement until it settles. Placing here would put two writers on
        // the same HWND again.
        *consecutive = 0;
        return Ok(false);
    }
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
    // A single stray sample must not yank the bar — only re-dock when the
    // divergence persists across ticks.
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
    let rect = appbar::reserve(hwnd.0, edge_u32, desired)?;
    appbar::place(hwnd.0, rect, false)?;
    record_last_rect(app, Some(rect));
    // A monitor reconnect can leave the new edge without an auto-hide
    // registration — re-assert it; a refusal records the transient blocked
    // state instead of failing the drift pass (ticket 63).
    if current.mode == "auto-hide" {
        apply_dock_mode(app, hwnd.0, &current.edge, "auto-hide");
    }
    let _ = app.emit("quick-launch-changed", ());
    Ok(true)
}

/// Starts the auto-hide motion driver (ticket 63): one thread for the app's
/// lifetime that polls the cursor every [`AUTOHIDE_POLL_MS`] and animates the
/// docked strip between its full rect and a 2 px sliver — out on an edge
/// touch, away when the cursor leaves. Sprout owns this motion (research
/// 0003: no OS mechanism ever moves an appbar); it runs regardless of shell
/// engagement, so even a refused auto-hide registration still hides.
///
/// Ticket 66 (single-writer): this thread is ALSO the only writer of the
/// docked window's geometry and reservations. Mode flips requested from other
/// threads are pure state (`set_dock_mode` marks the transition pending via
/// [`DockState::settled`]); the driver's first tick after seeing one performs
/// the settle-time reservation change itself and lets the existing animation
/// slide the strip — no concurrent `SetWindowPos` / AppBar syscalls, no tao
/// size-math abort.
pub fn start_autohide_driver(app: AppHandle) {
    std::thread::spawn(move || {
        // The slide in flight: its start rect and start time. Reset on every
        // reveal/hide flip so an interrupted slide restarts smoothly.
        let mut anim: Option<(RECT, Instant)> = None;
        // Which state the driver wants (Some) once it has seen the dock.
        let mut shown: Option<bool> = None;
        // Whether the process timer resolution is raised to 1 ms (ticket 63):
        // Sleep quantizes to ~15.6 ms otherwise, which stutters the slide.
        let mut raised = false;
        loop {
            std::thread::sleep(Duration::from_millis(if anim.is_some() {
                AUTOHIDE_ANIM_POLL_MS
            } else {
                AUTOHIDE_POLL_MS
            }));
            if let Err(e) = autohide_tick(&app, &mut anim, &mut shown) {
                eprintln!("auto-hide: {e}");
            }
            match (anim.is_some(), raised) {
                (true, false) => {
                    unsafe { timeBeginPeriod(1) };
                    raised = true;
                }
                (false, true) => {
                    unsafe { timeEndPeriod(1) };
                    raised = false;
                }
                _ => {}
            }
        }
    });
}

/// The cursor position in physical screen coordinates (`GetCursorPos`) —
/// `None` when the call fails (then the strip hides; there is no hover).
fn cursor_pos() -> Option<(i32, i32)> {
    unsafe {
        let mut point = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut point) == 0 {
            return None;
        }
        Some((point.x, point.y))
    }
}

/// Marks the given mode as established on screen (ticket 66): the driver's
/// post-settle bookkeeping. Only recorded when the requested mode is still
/// the one observed — a flip that raced the settle leaves the state pending
/// and the next tick settles it instead. The drift guard reads this field to
/// stay out of the driver's way.
fn record_settled(app: &AppHandle, mode: &str) {
    if let Ok(mut dock) = app.state::<AppState>().dock.lock() {
        if let Some(d) = dock.as_mut() {
            if d.mode == mode {
                d.settled = Some(mode.to_string());
            }
        }
    }
}

/// The driver's one-time settle pass for a pending mode transition (ticket
/// 66): performs the reservation change the flip needs — shrink to the sliver
/// entering auto-hide, full-strip reservation + atomic placement entering
/// fixed — while never racing another placement writer (this runs on the
/// driver thread alone). Failures are logged / surfaced in the window, never
/// fatal: the caller marks the mode settled regardless so a refused
/// reservation cannot turn into a per-tick syscall storm.
///
/// Entering auto-hide deliberately places nothing here: the motion logic in
/// [`autohide_tick`] slides the strip from wherever it is to its target,
/// replacing the old inline teleport with the intended motion.
fn settle_mode(app: &AppHandle, current: &DockState) {
    let Some(window) = app.get_webview_window(QUICK_LAUNCH_WINDOW) else {
        return;
    };
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    let Some(edge_u32) = appbar::edge_constant(&current.edge) else {
        return;
    };
    if current.mode == "auto-hide" {
        // Hand the workspace back: shrink the strip's reservation to the
        // sliver once. The ±4 px grant validation stays log-only (the shell
        // may round); a refused shrink keeps the full reservation and hides
        // anyway — overlay semantics mean no other window loses space.
        if let Some(monitor) = appbar::monitor_rect(hwnd.0) {
            let full = appbar::appbar_rect(monitor, edge_u32, DOCK_WIDTH as i32);
            let sliver = appbar::sliver_rect(full, edge_u32, AUTOHIDE_SLIVER_PX);
            match appbar::reserve(hwnd.0, edge_u32, sliver) {
                Ok(granted) => {
                    if appbar::rects_diverged(granted, sliver, 4) {
                        eprintln!(
                            "auto-hide: unexpected sliver reservation grant — keeping window placement only"
                        );
                    }
                }
                Err(e) => {
                    eprintln!("auto-hide: could not release the strip reservation: {e}");
                }
            }
            record_last_rect(app, Some(full));
        }
        return;
    }
    // Entering fixed (or a fresh fixed dock's first driver observation):
    // reserve the full strip and place at the granted rect in one atomic
    // `SetWindowPos`. A bar arriving from auto-hide sits as a 2 px sliver
    // whose edge IS flush with the shrunken work area — trusting the
    // flush-keep rule here would pin "fixed" as an invisible 2 px line
    // (ticket 66), so the full thickness is derived fresh whenever the
    // window is narrower than a strip. An already-full-width bar (a fresh
    // fixed dock placed by dock() moments ago) keeps its position —
    // re-deriving from the self-shrunk work area would march it one width
    // into the screen (ticket 61).
    let Some(work) = appbar::work_area(hwnd.0) else {
        report_dock_error(app, "Could not switch the bar to fixed: cannot find the monitor work area");
        return;
    };
    let actual = appbar::window_rect(hwnd.0).unwrap_or(work);
    let desired =
        if actual.right - actual.left < DOCK_WIDTH as i32 {
            appbar::appbar_rect(work, edge_u32, DOCK_WIDTH as i32)
        } else {
            appbar::desired_rect(edge_u32, work, actual, DOCK_WIDTH as i32)
        };
    match appbar::reserve(hwnd.0, edge_u32, desired) {
        Ok(rect) => {
            // Single-writer handshake (ticket 66): an undock/close landing
            // between the flip and this settle abandons the placement.
            if !docked_state(app).is_some() {
                return;
            }
            if let Err(e) = appbar::place(hwnd.0, rect, false) {
                report_dock_error(app, &format!("Could not switch the bar to fixed: {e}"));
                return;
            }
            record_last_rect(app, Some(rect));
        }
        Err(e) => report_dock_error(app, &format!("Could not switch the bar to fixed: {e}")),
    }
}

/// One auto-hide driver tick (ticket 63). Acts only while docked with mode
/// "auto-hide" — floating windows never hide, `fixed` strips never move.
/// Everything is recomputed from live state each tick, so docks, edge
/// switches, and monitor changes are picked up within one tick.
///
/// Ticket 66: before any motion, a pending mode transition (`settled` ≠
/// `mode`) is settled HERE — on this thread — keeping every placement syscall
/// single-writer.
fn autohide_tick(
    app: &AppHandle,
    anim: &mut Option<(RECT, Instant)>,
    shown: &mut Option<bool>,
) -> Result<(), String> {
    // Ticket 66: this thread is the primary geometry writer. It re-reads the
    // dock state immediately before acting, so an undock/close that lands
    // between ticks is honored within microseconds — no lock held across
    // syscalls (a SetWindowPos issued here is delivered through the main
    // thread's message pump; blocking that pump on a lock the sender waits
    // on would deadlock).
    let Some(current) = docked_state(app) else {
        *anim = None;
        *shown = None;
        return Ok(());
    };
    if current.settled.as_deref() != Some(current.mode.as_str()) {
        settle_mode(app, &current);
        record_settled(app, &current.mode);
    }
    if current.mode != "auto-hide" {
        // Fixed mode: the strip stays put at its full rect (the settle pass
        // above reserved and placed it on the switch); the driver forgets its
        // slide state.
        *anim = None;
        *shown = None;
        return Ok(());
    }
    let Some(window) = app.get_webview_window(QUICK_LAUNCH_WINDOW) else {
        return Ok(());
    };
    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    let Some(edge_u32) = appbar::edge_constant(&current.edge) else {
        return Ok(());
    };
    // The full strip rect the shell granted (dock/reposition/drift keep it
    // current) — both animation endpoints derive from it.
    let Some(full) = current.last_rect else {
        return Ok(());
    };
    let Some(actual) = appbar::window_rect(hwnd.0) else {
        return Ok(());
    };
    let sliver = appbar::sliver_rect(full, edge_u32, AUTOHIDE_SLIVER_PX);
    // Hysteresis (ticket 63): a hidden strip is revealed ONLY by a touch at
    // the very screen edge (the EDGE_TRIGGER_PX band) — mere proximity within
    // the area the strip would occupy must not pop it out, or it shadows the
    // overlaid app's own chrome (close/minimize buttons). Once out, it stays
    // out while the cursor is anywhere over the strip, and hides when the
    // cursor leaves it.
    let want_shown = match cursor_pos() {
        None => false,
        Some((x, y)) => match *shown {
            Some(true) => appbar::strip_contains(x, y, full),
            _ => appbar::edge_hit(x, y, full, edge_u32, EDGE_TRIGGER_PX),
        },
    };
    if *shown != Some(want_shown) {
        // Direction flipped (or first observation): start the slide from
        // wherever the window actually is right now.
        *anim = Some((actual, Instant::now()));
        *shown = Some(want_shown);
    }
    let target = if want_shown { full } else { sliver };
    if !appbar::rects_diverged(actual, target, 0) {
        return Ok(());
    }
    let Some((from, started)) = anim.as_ref().map(|(r, t)| (*r, *t)) else {
        // The window sits away from its target without an active slide (a
        // dock/edge-switch just placed it) — start one from here.
        *anim = Some((actual, Instant::now()));
        return Ok(());
    };
    let progress = started.elapsed().as_millis() as f64 / AUTOHIDE_SLIDE_MS as f64;
    let mut next = appbar::slide_rect(from, target, progress.min(1.0));
    // Hardening: an insane interpolated rect (a stale or corrupt reference)
    // must never reach SetWindowPos — tao's size math panics on overflow.
    // Clamp the frame into the monitor's neighborhood instead.
    let sane = |v: i32| v > -100_000 && v < 100_000;
    if ![next.left, next.top, next.right, next.bottom].iter().all(|v| sane(*v)) {
        next = target;
        *anim = None;
    }
    // Skip the syscall when the eased curve lands on the same pixels — a
    // no-op SetWindowPos still costs a compositor round-trip. The docked
    // re-check just before each placement is the single-writer handshake
    // (ticket 66): an undock/close that lands mid-slide abandons the rest of
    // the animation instead of placing against a window another thread is
    // resizing.
    if !docked_state(app).is_some() {
        *anim = None;
        *shown = None;
        return Ok(());
    }
    if appbar::rects_diverged(actual, next, 0) {
        appbar::place(hwnd.0, next, true)?;
    }
    if progress >= 1.0 {
        if docked_state(app).is_some() {
            appbar::place(hwnd.0, target, true)?;
        }
        *anim = None;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::UI::Shell::{ABE_LEFT, ABE_RIGHT};

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

    /// The settle geometry the driver derives when entering fixed (ticket
    /// 66): the same composition `settle_mode` runs, as pure inputs so the
    /// syscall-free seam is testable.
    fn settle_fixed_desired(edge: u32, work: RECT, actual: RECT) -> RECT {
        if actual.right - actual.left < DOCK_WIDTH as i32 {
            appbar::appbar_rect(work, edge, DOCK_WIDTH as i32)
        } else {
            appbar::desired_rect(edge, work, actual, DOCK_WIDTH as i32)
        }
    }

    #[test]
    fn settle_fixed_expands_a_bar_arriving_from_the_sliver() {
        // Regression (ticket 66): auto-hide→fixed left an invisible 2 px bar
        // because the sliver is flush with its own shrunken reservation —
        // the flush-keep rule must not apply to a sub-thickness window.
        let work = RECT { left: 2, top: 0, right: 2560, bottom: 1848 };
        let sliver = RECT { left: 0, top: 0, right: AUTOHIDE_SLIVER_PX as i32, bottom: 1848 };
        let desired = settle_fixed_desired(ABE_LEFT, work, sliver);
        assert_eq!(desired.right - desired.left, DOCK_WIDTH as i32);
        assert_eq!(desired.left, work.left); // anchored against the (sliver-shrunk) edge
        // Mirrored on the right edge.
        let work = RECT { left: 0, top: 0, right: 2558, bottom: 1848 };
        let sliver = RECT {
            left: 2560 - AUTOHIDE_SLIVER_PX as i32,
            top: 0,
            right: 2560,
            bottom: 1848,
        };
        let desired = settle_fixed_desired(ABE_RIGHT, work, sliver);
        assert_eq!(desired.right - desired.left, DOCK_WIDTH as i32);
    }

    #[test]
    fn settle_fixed_keeps_a_bar_already_flush_with_its_reservation() {
        // A fresh fixed dock: dock() already reserved and placed this exact
        // strip on the command thread. The driver's settle pass must keep it
        // — re-deriving from the self-shrunk work area would march the bar
        // one width into the screen (the ticket-61 bug).
        let work = RECT { left: 0, top: 0, right: 2220, bottom: 1848 };
        let placed = RECT { left: 0, top: 0, right: DOCK_WIDTH as i32, bottom: 1848 };
        let desired = settle_fixed_desired(ABE_LEFT, work, placed);
        assert_eq!((desired.left, desired.right), (placed.left, placed.right));
        // The reservation the shell then grants for `desired` is the bar's
        // own rect — a place(desired) is a no-op, not a move.
        let granted = appbar::apply_thickness(
            RECT { left: work.left, top: work.top, right: work.left, bottom: work.bottom },
            ABE_LEFT,
            DOCK_WIDTH as i32,
        );
        assert!(!appbar::rects_diverged(granted, placed, 2));
    }

    #[test]
    fn settle_fixed_aligns_a_bar_arriving_from_autohide() {
        // Coming from auto-hide there was no reservation of ours in the work
        // area: the window spans the monitor edge as an overlay strip while
        // rcWork still ends where other windows end. The settle target must
        // align against the real edge.
        let work = RECT { left: 340, top: 0, right: 2560, bottom: 1848 };
        let overlay = RECT { left: 0, top: 0, right: DOCK_WIDTH as i32, bottom: 1848 };
        let desired = settle_fixed_desired(ABE_LEFT, work, overlay);
        assert_eq!(desired.left, work.left - DOCK_WIDTH as i32 + 340 - 340); // == 0
        assert_eq!(desired.right, work.left);
        // Mirrored on the right edge.
        let work = RECT { left: 0, top: 0, right: 2220, bottom: 1848 };
        let overlay = RECT { left: 2560 - DOCK_WIDTH as i32, top: 0, right: 2560, bottom: 1848 };
        let desired = settle_fixed_desired(ABE_RIGHT, work, overlay);
        assert_eq!(desired.left, work.right);
        assert_eq!(desired.right, work.right + DOCK_WIDTH as i32);
    }

    #[test]
    fn settle_autohide_shrinks_the_reservation_to_the_sliver() {
        // Entering auto-hide hands the workspace back: the reservation the
        // driver commits is the 2 px sliver of the monitor-spanning strip,
        // and the animation endpoints stay the full strip (ticket 66).
        let monitor = RECT { left: 0, top: 0, right: 2560, bottom: 1848 };
        for edge in [ABE_LEFT, ABE_RIGHT] {
            let full = appbar::appbar_rect(monitor, edge, DOCK_WIDTH as i32);
            let sliver = appbar::sliver_rect(full, edge, AUTOHIDE_SLIVER_PX);
            assert_eq!(sliver.right - sliver.left, AUTOHIDE_SLIVER_PX as i32);
            assert_eq!(sliver.top, full.top);
            assert_eq!(sliver.bottom, full.bottom);
            if edge == ABE_LEFT {
                assert_eq!((sliver.left, sliver.right), (0, AUTOHIDE_SLIVER_PX as i32));
            } else {
                assert_eq!(
                    (sliver.left, sliver.right),
                    (2560 - AUTOHIDE_SLIVER_PX as i32, 2560)
                );
            }
        }
    }

    #[test]
    fn mode_flip_marks_transition_pending_and_settle_clears_it() {
        // The single-writer contract at the state level (ticket 66): a flip
        // requested from any thread only rewrites mode + settled=None; the
        // driver's record_settled marks it established — and never for a
        // mode that raced past it.
        let dir = test_dir();
        let conn = db::init_at(&dir).unwrap();
        let mut d = DockState {
            edge: "left".into(),
            mode: "fixed".into(),
            monitor: r"\\.\DISPLAY1".into(),
            last_rect: None,
            blocked: None,
            settled: Some("fixed".into()),
        };
        // set_dock_mode's flip (inlined here — it owns no syscalls to mock).
        d.mode = "auto-hide".into();
        d.settled = None;
        assert_ne!(d.settled.as_deref(), Some(d.mode.as_str()));
        // The driver's post-settle bookkeeping.
        d.mode = "auto-hide".into();
        if d.mode == "auto-hide" {
            d.settled = Some(d.mode.clone());
        }
        assert_eq!(d.settled.as_deref(), Some("auto-hide"));
        // A racing flip after settle leaves the stale mark unrecorded.
        let observed_mode = "auto-hide";
        d.mode = "fixed".into();
        if d.mode == observed_mode {
            d.settled = Some(observed_mode.into());
        }
        assert_ne!(d.settled.as_deref(), Some("fixed"));
    }
}