//! Auto-start registration (ADR-0013, ticket 75): keeps the HKCU Run entry
//! (via the standard autostart plugin, written with the `--autostart`
//! launcher argument ticket 76 consumes) in step with the persisted
//! `autostart` setting. The setting records the preference; this module owns
//! the side effect. Debug builds never touch the Run key — dev sessions must
//! not pollute the boot path — they log the skip instead.
//!
//! The decision logic (default-on lives with the setting's default in
//! `settings`) is pure and unit-tested here; the registry effects are
//! verified manually against the release exe.

use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

/// What a sync has to do to make the registration match the preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyncAction {
    LeaveAsIs,
    Enable,
    Disable,
}

/// Pure decision over the desired state and the registration as it stands:
/// only a real difference acts, so repeated syncs (startup + every toggle)
/// stay idempotent no-ops.
fn sync_action(desired_on: bool, registered_now: bool) -> SyncAction {
    match (desired_on, registered_now) {
        (true, false) => SyncAction::Enable,
        (false, true) => SyncAction::Disable,
        _ => SyncAction::LeaveAsIs,
    }
}

/// Debug builds never touch the Run key (ADR-0013); installed builds own it.
fn registration_allowed(debug_build: bool) -> bool {
    !debug_build
}

/// Reconciles the Run-key registration with `desired_on` — called once at
/// startup and beside every save of the toggle. In a debug build this logs
/// and skips instead of writing; elsewhere a failed registry write is an
/// error the caller surfaces (the toggle) or logs (startup).
pub(crate) fn sync_registration(app: &AppHandle, desired_on: bool) -> Result<(), String> {
    if !registration_allowed(cfg!(debug_assertions)) {
        eprintln!(
            "Auto-start: debug build — skipping Run-key sync (desired: {})",
            if desired_on { "on" } else { "off" }
        );
        return Ok(());
    }
    let autolaunch = app.autolaunch();
    let registered_now = autolaunch.is_enabled().unwrap_or(false);
    match sync_action(desired_on, registered_now) {
        SyncAction::LeaveAsIs => Ok(()),
        SyncAction::Enable => autolaunch
            .enable()
            .map_err(|e| format!("could not register Sprout to start with Windows: {e}")),
        SyncAction::Disable => autolaunch
            .disable()
            .map_err(|e| format!("could not remove Sprout's start-with-Windows registration: {e}")),
    }
}

/// Whether these process arguments mark an auto-start launch: the Run-key
/// registration starts Sprout with `--autostart`, and such a boot brings up
/// backend + tray only — never the main window (ADR-0013).
pub(crate) fn is_autostart_launch(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--autostart")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enables_only_when_desired_but_not_registered() {
        assert_eq!(sync_action(true, false), SyncAction::Enable);
    }

    #[test]
    fn disables_only_when_registered_but_not_desired() {
        assert_eq!(sync_action(false, true), SyncAction::Disable);
    }

    #[test]
    fn matching_states_never_touch_the_registry() {
        assert_eq!(sync_action(true, true), SyncAction::LeaveAsIs);
        assert_eq!(sync_action(false, false), SyncAction::LeaveAsIs);
    }

    #[test]
    fn debug_builds_never_touch_the_run_key() {
        assert!(!registration_allowed(true));
        assert!(registration_allowed(false));
    }

    #[test]
    fn the_run_key_argument_marks_an_autostart_boot() {
        let args = vec![
            "C:\\Program Files\\Sprout\\sprout.exe".to_string(),
            "--autostart".to_string(),
        ];
        assert!(is_autostart_launch(&args));
    }

    #[test]
    fn plain_and_import_launches_are_not_autostart_boots() {
        let args = vec![
            "sprout.exe".to_string(),
            "--import".to_string(),
            "C:\\preset.sprout.json".to_string(),
        ];
        assert!(!is_autostart_launch(&args));
        assert!(!is_autostart_launch(&[]));
    }
}
