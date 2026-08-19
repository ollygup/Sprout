//! Tray residency (ticket 43): Sprout lives lean in the tray's "show hidden
//! icons" area. The window is disposable — closing it destroys it and keeps
//! only the Rust backend resident (no webview, a few MB); the tray is the
//! only permanent surface. Left-click starts the whole Quick Launch list;
//! the right-click menu offers Start all / per-app items / Open Sprout /
//! Quit — and Quit is the only way to exit the app for real.
//!
//! The exit-suppression side lives in `lib.rs` (the window-close handler +
//! the `App::run` `ExitRequested` callback); this module owns the tray icon
//! and its menu.

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

use crate::launch;
use crate::AppState;

/// The tray's stable id — `lib.rs`'s exit-suppression hook uses it to decide
/// whether the backend is meant to stay resident.
pub const TRAY_ID: &str = "sprout-tray";

/// Builds the resident tray icon: the brand icon, a one-line tooltip, the
/// right-click menu, and the click/menu event wiring. Left-click starts the
/// whole Quick Launch list; the menu is rebuilt from the entries whenever
/// they change.
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
                start_all(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Rebuilds the right-click menu from the current Quick Launch list — called
/// after every entry change so the menu never shows stale items (ticket 43).
pub fn rebuild_menu(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        match build_menu(app) {
            Ok(menu) => {
                let _ = tray.set_menu(Some(menu));
            }
            Err(error) => {
                let _ = notify(app, format!("Could not update the tray menu: {error}"));
            }
        }
    }
}

/// The right-click menu: Start all (N) / one item per entry / per-desktop
/// groups (ticket 44) / Open Sprout / Quit. Every item triggers the same
/// launch paths as the page's Start button — the guard, the queue, the
/// summary notification.
fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let entries = list_entries(app);
    let mut items: Vec<Box<dyn tauri::menu::IsMenuItem<tauri::Wry>>> = vec![
        Box::new(MenuItem::with_id(
            app,
            "start-all",
            format!("Start all ({})", entries.len()),
            true,
            None::<&str>,
        )?),
        Box::new(PredefinedMenuItem::separator(app)?),
    ];
    append_desktop_groups(app, &entries, &mut items)?;
    for entry in &entries {
        items.push(Box::new(MenuItem::with_id(
            app,
            format!("launch-{}", entry.id),
            entry.entry.name.clone(),
            true,
            None::<&str>,
        )?));
    }
    if !entries.is_empty() {
        items.push(Box::new(PredefinedMenuItem::separator(app)?));
    }
    items.push(Box::new(MenuItem::with_id(
        app,
        "open-sprout",
        "Open Sprout",
        true,
        None::<&str>,
    )?));
    items.push(Box::new(MenuItem::with_id(
        app,
        "quit",
        "Quit",
        true,
        None::<&str>,
    )?));
    let refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> =
        items.iter().map(|item| item.as_ref()).collect();
    Menu::with_items(app, &refs)
}

/// Adds one submenu per desktop group (ticket 44): entries assigned to a
/// desktop group under that desktop's label, unassigned entries under
/// "Current desktop". Each submenu opens with a "Start group (N)" item —
/// that group's entries through the same runner — followed by its entries,
/// one per item. Hidden entirely below Windows 11 24H2, where the engine
/// reports no desktops. A stale assignment — a desktop that no longer exists
/// — still groups under its own id; at launch the orchestrator falls back to
/// the current desktop and notes it in the summary.
fn append_desktop_groups(
    app: &AppHandle,
    entries: &[launch::LaunchEntry],
    items: &mut Vec<Box<dyn tauri::menu::IsMenuItem<tauri::Wry>>>,
) -> tauri::Result<()> {
    let state = app.state::<AppState>();
    let desktops = state.launcher.desktops();
    if desktops.is_empty() {
        return Ok(());
    }

    let mut keys: Vec<Option<String>> = Vec::new();
    if entries.iter().any(|e| e.entry.desktop_id.is_none()) {
        keys.push(None);
    }
    for desktop in &desktops {
        if entries
            .iter()
            .any(|e| e.entry.desktop_id.as_deref() == Some(desktop.id.as_str()))
        {
            keys.push(Some(desktop.id.clone()));
        }
    }
    for entry in entries {
        if let Some(guid) = entry.entry.desktop_id.as_deref() {
            if !desktops.iter().any(|d| d.id == guid)
                && !keys.contains(&Some(guid.to_string()))
            {
                keys.push(Some(guid.to_string()));
            }
        }
    }

    for key in keys {
        let group: Vec<&launch::LaunchEntry> = match &key {
            None => entries
                .iter()
                .filter(|e| e.entry.desktop_id.is_none())
                .collect(),
            Some(guid) => entries
                .iter()
                .filter(|e| e.entry.desktop_id.as_deref() == Some(guid.as_str()))
                .collect(),
        };
        let label = match &key {
            None => "Current desktop".into(),
            Some(guid) => desktops
                .iter()
                .find(|d| &d.id == guid)
                .map(|d| d.name.clone())
                .unwrap_or_else(|| format!("Desktop ({guid})")),
        };
        let key = key
            .as_deref()
            .map(str::to_string)
            .unwrap_or_else(|| "current".into());
        let mut sub_items: Vec<Box<dyn tauri::menu::IsMenuItem<tauri::Wry>>> = vec![
            Box::new(MenuItem::with_id(
                app,
                format!("group-start-{key}"),
                format!("Start group ({})", group.len()),
                true,
                None::<&str>,
            )?),
            Box::new(PredefinedMenuItem::separator(app)?),
        ];
        for entry in &group {
            sub_items.push(Box::new(MenuItem::with_id(
                app,
                format!("group-launch-{}", entry.id),
                entry.entry.name.clone(),
                true,
                None::<&str>,
            )?));
        }
        let refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> =
            sub_items.iter().map(|item| item.as_ref()).collect();
        items.push(Box::new(tauri::menu::Submenu::with_items(
            app, label, true, &refs,
        )?));
    }
    Ok(())
}

/// Routes a right-click menu item. "Start all", the desktop groups, and the
/// per-app items start through the exact same runner as the tray's left-click
/// and the page's Start button; "Open Sprout" recreates the destroyed window;
/// "Quit" is the only way to exit the app for real.
fn handle_menu_event(app: &AppHandle, id: &str) {
    match id {
        "start-all" => start_all(app),
        "open-sprout" => open_sprout(app),
        "quit" => app.exit(0),
        id => {
            if let Some(key) = id.strip_prefix("group-start-") {
                start_group(app, key);
            } else if let Some(entry_id) = id.strip_prefix("group-launch-") {
                if let Ok(entry_id) = entry_id.parse::<i64>() {
                    start_entry(app, entry_id);
                }
            } else if let Some(entry_id) = id.strip_prefix("launch-") {
                if let Ok(entry_id) = entry_id.parse::<i64>() {
                    start_entry(app, entry_id);
                }
            }
        }
    }
}

/// The tray's left-click and "Start all": one gesture, the whole Quick
/// Launch list. Zero entries is a notification, never a silent no-op.
fn start_all(app: &AppHandle) {
    let entries = list_entries(app);
    if entries.is_empty() {
        let _ = notify(app, "Nothing configured in Quick Launch.");
        return;
    }
    start(app, entries);
}

/// One per-app menu item: starts that single entry through the same capped,
/// queued runner the whole list uses — same guard, same summary.
fn start_entry(app: &AppHandle, entry_id: i64) {
    let entries = list_entries(app);
    let entry = entries.into_iter().find(|entry| entry.id == entry_id);
    match entry {
        Some(entry) => start(app, vec![entry]),
        None => {
            let _ = notify(app, "That Quick Launch entry no longer exists.");
        }
    }
}

/// One desktop-group submenu's "Start group" item (ticket 44): every entry
/// assigned to that desktop — "current" for the unassigned ones — through
/// the same runner as everything else. A stale group (its desktop was
/// deleted) starts too: the orchestrator falls back to the current desktop
/// and notes it in the summary.
fn start_group(app: &AppHandle, key: &str) {
    let entries = list_entries(app);
    let group: Vec<launch::LaunchEntry> = match key {
        "current" => entries
            .into_iter()
            .filter(|entry| entry.entry.desktop_id.is_none())
            .collect(),
        guid => entries
            .into_iter()
            .filter(|entry| entry.entry.desktop_id.as_deref() == Some(guid))
            .collect(),
    };
    if group.is_empty() {
        let _ = notify(app, "That desktop group is empty.");
        return;
    }
    start(app, group);
}

/// The shared tray-side start: hands the entries to the common runner
/// (guard + background queue + `launch-run-done` event + summary
/// notification), and turns a rejection — a run already in flight — into a
/// notification instead of a silent click.
fn start(app: &AppHandle, entries: Vec<launch::LaunchEntry>) {
    let state = app.state::<AppState>();
    match crate::launch_entries(app, &state, entries) {
        Ok(()) => {}
        Err(message) => {
            let _ = notify(app, message);
        }
    }
}

/// "Open Sprout": focuses the window, or recreates it when it was destroyed
/// — the window keeps its configured size (tauri.conf.json).
fn open_sprout(app: &AppHandle) {
    if let Err(error) = crate::open_main_window(app) {
        let _ = notify(app, format!("Could not open Sprout: {error}"));
    }
}

/// One "Sprout" system notification — the shared tray-side surface for
/// everything the tray needs to say: empty-list, already-in-progress,
/// entry-gone, menu errors. A failure to notify is never a failure of the
/// action.
fn notify(app: &AppHandle, body: impl AsRef<str>) -> Result<(), String> {
    use tauri_plugin_notification::NotificationExt;
    app.notification()
        .builder()
        .title("Sprout")
        .body(body.as_ref())
        .show()
        .map_err(|e| e.to_string())
}

/// The current Quick Launch list, in order — read fresh at every click so
/// the tray never runs a stale list. A failed read is a notification, never
/// a crash.
fn list_entries(app: &AppHandle) -> Vec<launch::LaunchEntry> {
    let state = app.state::<AppState>();
    let conn = match state.db.lock() {
        Ok(conn) => conn,
        Err(_) => return Vec::new(),
    };
    let entries = launch::list_launch_entries(&conn);
    drop(conn);
    drop(state);
    match entries {
        Ok(entries) => entries,
        Err(error) => {
            let _ = notify(app, format!("Could not read the Quick Launch list: {error}"));
            Vec::new()
        }
    }
}
