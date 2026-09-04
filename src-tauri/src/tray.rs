//! Tray residency (ticket 43): Sprout lives lean in the tray's "show hidden
//! icons" area. The window is disposable — closing it destroys it and keeps
//! only the Rust backend resident (no webview, a few MB); the tray is the
//! only permanent surface. Left-click opens (or raises) the Quick Launch
//! window (ticket 52); the right-click menu is just Open Sprout and Quit
//! (ticket 54) — Quit is the only way to exit the app for real. Launch
//! triggers live in the Quick Launch window and the Quick Launch page, not
//! the tray.
//!
//! The exit-suppression side lives in `lib.rs` (the window-close handler +
//! the `App::run` `ExitRequested` callback); this module owns the tray icon
//! and its menu.

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle,
};

/// The tray's stable id — `lib.rs`'s exit-suppression hook uses it to decide
/// whether the backend is meant to stay resident.
pub const TRAY_ID: &str = "sprout-tray";

/// Builds the resident tray icon: the brand icon, a one-line tooltip, the
/// right-click menu (Open Sprout / Quit), and the click/menu event wiring.
/// Left-click opens (or raises) the Quick Launch window; the menu is static
/// and never rebuilt.
pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::AssetNotFound("default window icon".into()))?;
    let menu = build_menu(app)?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("Sprout — Quick Launch")
        .menu(&menu)
        .on_menu_event(|app, event| handle_menu_event(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                // Ticket 52: left-click opens (or raises) the Quick Launch
                // window — the resident quick-access surface (ADR-0011),
                // where the Start button lives. When the window is docked
                // (ticket 53), `quick_window::open` raises it into focus.
                let _ = crate::quick_window::open(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// The right-click menu (ticket 54): Open Sprout and Quit, nothing else —
/// the launch-item menu path (Start all, per-desktop groups, per-entry items)
/// is gone; launching lives in the Quick Launch window and page.
fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let items: Vec<Box<dyn tauri::menu::IsMenuItem<tauri::Wry>>> = vec![
        Box::new(MenuItem::with_id(
            app,
            "open-sprout",
            "Open Sprout",
            true,
            None::<&str>,
        )?),
        Box::new(MenuItem::with_id(
            app,
            "quit",
            "Quit",
            true,
            None::<&str>,
        )?),
    ];
    let refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> =
        items.iter().map(|item| item.as_ref()).collect();
    Menu::with_items(app, &refs)
}

/// Routes a right-click menu item: "Open Sprout" recreates/focuses the
/// destroyed main window; "Quit" is the only way to exit the app for real.
fn handle_menu_event(app: &AppHandle, id: &str) {
    match id {
        "open-sprout" => open_sprout(app),
        "quit" => app.exit(0),
        _ => {}
    }
}

/// "Open Sprout": focuses the window, or recreates it when it was destroyed
/// — the window keeps its configured size (`constants::window`). The Quick
/// Launch bar follows the same restore rule as boot (ADR-0013): a remembered
/// "docked" preference brings it back; floating waits for its explicit
/// left-click.
pub(crate) fn open_sprout(app: &AppHandle) {
    // WHY off-thread: the menu/IPC caller runs on the event thread, which the
    // blocking open would hang while it waits out the close grace — enqueue
    // instead (see `request_open_main_window`); a rebuild failure surfaces in
    // the worker's own error path, same as before.
    crate::request_open_main_window(app);
    if let Err(error) = crate::quick_window::open_if_docked(app) {
        eprintln!("Could not restore the Quick Launch dock: {error}");
    }
}