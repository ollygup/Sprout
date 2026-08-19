//! Quick Actions (ticket 50): the machine-local list of named PowerShell
//! commands the Quick Launch window's Quick Actions tab fires — storage,
//! validation, the hidden fire-and-forget runner, and the timeboxed Test.
//!
//! Glossary (docs/CONTEXT.md): a **Quick Action** is a machine-local,
//! user-authored named command (PowerShell, optional working directory) run
//! fire-and-forget from the Quick Launch window's Quick Actions tab; it runs
//! hidden as the current user with no elevation and no status UI. Configured
//! in the main app's Quick Actions page (ticket 51); never part of Presets,
//! Plan, Run, or exports — machine-local like `install_dir` (ADR-0009's
//! spirit).

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension, Result};
use serde::{Deserialize, Serialize};

use crate::engine::windows::{hidden, run_timed_process_in};
use crate::launch::TestResult;

/// The Test-click timebox (ticket 50, prior art: the Launch entry Test
/// button, ticket 41): long enough for a normal command, short enough that an
/// interactive command is reported honestly as not headless-verifiable
/// instead of wedging the dialog.
pub const TEST_TIMEOUT: Duration = Duration::from_secs(20);

/// The editable shape of a Quick Action, as the frontend sends it. The stored
/// record ([`QuickAction`]) adds the id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickActionInput {
    pub name: String,
    /// The PowerShell script to run, multi-line allowed.
    pub command: String,
    /// Working directory the command starts in; `None` = the app's own.
    pub cwd: Option<String>,
}

/// A Quick Action as stored: the input plus its library id. Position is
/// internal (order within the list) and never part of the payload — reorders
/// go through `move_quick_action`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickAction {
    pub id: i64,
    #[serde(flatten)]
    pub action: QuickActionInput,
}

/// The working-directory rule shared by validation and the Test command: the
/// value, when set, must be an absolute path — a relative one would silently
/// mean different things per machine (the same rule as the install
/// directory, ADR-0009). Empty and whitespace-only values mean "the app's
/// own working directory" and pass.
pub fn validate_cwd(cwd: Option<&str>) -> std::result::Result<(), String> {
    if let Some(cwd) = cwd.map(str::trim).filter(|c| !c.is_empty()) {
        if !Path::new(cwd).is_absolute() {
            return Err(format!(
                "'{cwd}' is not an absolute path — the working directory must be a full path like D:\\Work"
            ));
        }
    }
    Ok(())
}

/// Rejects actions that could never run: a blank name or blank command, or a
/// working directory that is not an absolute path. A broken action must never
/// reach the list.
pub fn validate_quick_action(action: &QuickActionInput) -> std::result::Result<(), String> {
    if action.name.trim().is_empty() {
        return Err("Quick action name must not be empty".into());
    }
    if action.command.trim().is_empty() {
        return Err("Quick action command must not be empty".into());
    }
    validate_cwd(action.cwd.as_deref())
}

/// The stored working directory: whitespace-trimmed, empty values become
/// `None` (the app's own working directory), so only meaningful absolute
/// paths persist.
fn normalized_cwd(action: &QuickActionInput) -> Option<String> {
    action
        .cwd
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn action_from_row(row: &rusqlite::Row) -> Result<QuickAction> {
    Ok(QuickAction {
        id: row.get(0)?,
        action: QuickActionInput {
            name: row.get(1)?,
            command: row.get(2)?,
            cwd: row.get(3)?,
        },
    })
}

/// Every Quick Action in list order (position, then insertion order).
pub fn list_quick_actions(conn: &Connection) -> Result<Vec<QuickAction>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, command, cwd FROM quick_actions ORDER BY position, id",
    )?;
    let rows = stmt.query_map([], action_from_row)?;
    rows.collect()
}

/// Fetches one action by id — the runner's lookup (ticket 50).
pub fn get_quick_action(conn: &Connection, id: i64) -> Result<Option<QuickAction>> {
    conn.query_row(
        "SELECT id, name, command, cwd FROM quick_actions WHERE id = ?1",
        params![id],
        action_from_row,
    )
    .optional()
}

/// Appends an action at the end of the list (the next free position).
pub fn create_quick_action(conn: &Connection, action: &QuickActionInput) -> Result<QuickAction> {
    let tx = conn.unchecked_transaction()?;
    let position: i64 = tx.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM quick_actions",
        [],
        |row| row.get(0),
    )?;
    tx.execute(
        "INSERT INTO quick_actions (name, command, cwd, position)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            action.name.trim(),
            action.command.trim(),
            normalized_cwd(action),
            position,
        ],
    )?;
    let id = tx.last_insert_rowid();
    tx.commit()?;
    Ok(get_quick_action(conn, id)?.expect("just inserted"))
}

/// Replaces an action's script and metadata in place (same id). Position is
/// untouched — reorders go through `move_quick_action`.
pub fn update_quick_action(conn: &Connection, action: &QuickAction) -> Result<()> {
    conn.execute(
        "UPDATE quick_actions SET name = ?1, command = ?2, cwd = ?3 WHERE id = ?4",
        params![
            action.action.name.trim(),
            action.action.command.trim(),
            normalized_cwd(&action.action),
            action.id,
        ],
    )?;
    Ok(())
}

/// Removes an action and compacts the positions so the list stays gapless.
pub fn delete_quick_action(conn: &Connection, id: i64) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    let position: Option<i64> = tx
        .query_row("SELECT position FROM quick_actions WHERE id = ?1", params![id], |row| {
            row.get(0)
        })
        .optional()?;
    if let Some(position) = position {
        tx.execute("DELETE FROM quick_actions WHERE id = ?1", params![id])?;
        tx.execute(
            "UPDATE quick_actions SET position = position - 1 WHERE position > ?1",
            params![position],
        )?;
    }
    tx.commit()
}

/// Moves an action to `to_position` (clamped to the list), renumbering the
/// rest. The list is small (user config), so the same read-all-renumber-write
/// approach as the Launch list (ticket 38) is the obviously-correct one.
pub fn move_quick_action(conn: &Connection, id: i64, to_position: i64) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    let mut ids: Vec<i64> = {
        let mut stmt = tx.prepare("SELECT id FROM quick_actions ORDER BY position, id")?;
        let rows = stmt.query_map([], |row| row.get::<_, i64>(0))?;
        rows.collect::<Result<Vec<_>>>()?
    };
    let from = match ids.iter().position(|&candidate| candidate == id) {
        Some(index) => index,
        None => return Ok(()), // nothing to move
    };
    ids.remove(from);
    let clamped = to_position.clamp(0, ids.len() as i64) as usize;
    ids.insert(clamped, id);
    for (index, action_id) in ids.iter().enumerate() {
        tx.execute(
            "UPDATE quick_actions SET position = ?1 WHERE id = ?2",
            params![index as i64, action_id],
        )?;
    }
    tx.commit()
}

/// Builds the argv every Quick Action runs under: PowerShell's non-interactive
/// one-liner convention — the same shape the launch pipeline uses for its
/// PowerShell command entries (ticket 42).
pub fn powershell_argv(command: &str) -> (String, Vec<String>) {
    (
        "powershell".into(),
        vec![
            "-NoProfile".into(),
            "-NonInteractive".into(),
            "-Command".into(),
            command.into(),
        ],
    )
}

/// Spawns the action's command hidden (`CREATE_NO_WINDOW`), the working
/// directory honored when set, fire-and-forget: the `Child` is dropped
/// unwaited — Windows does not kill children when a handle closes, so the
/// process outlives this call. Current user, no elevation, no status UI, no
/// notification.
pub fn spawn_quick_action(action: &QuickActionInput) -> std::result::Result<(), String> {
    let (exe, args) = powershell_argv(&action.command);
    let mut command = hidden(Command::new(&exe));
    command.args(&args);
    if let Some(cwd) = normalized_cwd(action) {
        command.current_dir(&cwd);
    }
    command
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("failed to start '{exe}': {e}"))
}

/// The timeboxed Test (ticket 50, prior art: the Launch entry Test button,
/// ticket 41): runs the command under PowerShell and reports the exit code
/// plus captured output. A command that outlives the box comes back timed out
/// — honestly not headless-verifiable, never passed.
pub fn test_quick_action(command: &str, cwd: Option<&str>) -> TestResult {
    test_quick_action_with_timeout(command, cwd, TEST_TIMEOUT)
}

/// The timeboxed core behind `test_quick_action`, parameterized so tests can
/// use a short box.
pub(crate) fn test_quick_action_with_timeout(
    command: &str,
    cwd: Option<&str>,
    timeout: Duration,
) -> TestResult {
    let (exe, args) = powershell_argv(command);
    let run = run_timed_process_in(cwd, &exe, &args, timeout);
    TestResult {
        timed_out: run.timed_out,
        exit_code: run.exit_code,
        output: run.output,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        crate::db::init_at(&tempfile::tempdir().unwrap().into_path()).unwrap()
    }

    fn input(name: &str) -> QuickActionInput {
        QuickActionInput {
            name: name.into(),
            command: format!("Write-Output {name}"),
            cwd: None,
        }
    }

    #[test]
    fn validation_rejects_broken_actions() {
        let mut action = input("docker-start");
        action.name = " ".into();
        assert!(validate_quick_action(&action).is_err());

        let mut action = input("docker-start");
        action.command = String::new();
        assert!(validate_quick_action(&action).is_err());

        // A working directory must be an absolute path.
        let mut action = input("docker-start");
        action.cwd = Some("Work".into());
        assert!(validate_quick_action(&action).is_err());
        action.cwd = Some(r"D:\Work".into());
        assert!(validate_quick_action(&action).is_ok());
        // Empty and whitespace-only values mean "the app's own directory".
        action.cwd = Some("   ".into());
        assert!(validate_quick_action(&action).is_ok());

        assert!(validate_quick_action(&input("docker-start")).is_ok());
    }

    #[test]
    fn crud_roundtrips_across_reopen() {
        let dir = tempfile::tempdir().unwrap().into_path();
        {
            let conn = crate::db::init_at(&dir).unwrap();
            let first = create_quick_action(&conn, &input("docker-start")).unwrap();
            let second = create_quick_action(&conn, &input("dev-services")).unwrap();
            create_quick_action(&conn, &input("nightly-backup")).unwrap();
            assert_eq!(first.id, 1);
            assert_eq!(second.id, 2);
            let list = list_quick_actions(&conn).unwrap();
            assert_eq!(list.len(), 3);
            assert_eq!(
                list.iter().map(|a| a.action.name.as_str()).collect::<Vec<_>>(),
                vec!["docker-start", "dev-services", "nightly-backup"]
            );
            // Update keeps the position.
            let mut updated = list[1].clone();
            updated.action.command = "Write-Output updated".into();
            update_quick_action(&conn, &updated).unwrap();
        }
        // Re-open: everything survives the connection.
        let conn = crate::db::init_at(&dir).unwrap();
        let list = list_quick_actions(&conn).unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[1].action.name, "dev-services");
        assert_eq!(list[1].action.command, "Write-Output updated");
        assert_eq!(list[2].action.name, "nightly-backup");
    }

    #[test]
    fn delete_compacts_positions() {
        let conn = conn();
        create_quick_action(&conn, &input("A")).unwrap();
        create_quick_action(&conn, &input("B")).unwrap();
        create_quick_action(&conn, &input("C")).unwrap();
        delete_quick_action(&conn, 1).unwrap();
        let list = list_quick_actions(&conn).unwrap();
        assert_eq!(
            list.iter().map(|a| a.id).collect::<Vec<_>>(),
            vec![2, 3]
        );
        // A new action lands at the end of the compacted list.
        create_quick_action(&conn, &input("D")).unwrap();
        let list = list_quick_actions(&conn).unwrap();
        assert_eq!(
            list.iter().map(|a| a.id).collect::<Vec<_>>(),
            vec![2, 3, 4]
        );
        // Deleting an unknown id is a no-op.
        delete_quick_action(&conn, 999).unwrap();
        assert_eq!(list_quick_actions(&conn).unwrap().len(), 3);
    }

    #[test]
    fn move_reorders_and_clamps() {
        let conn = conn();
        for name in ["A", "B", "C", "D"] {
            create_quick_action(&conn, &input(name)).unwrap();
        }
        let ids = |conn: &Connection| {
            list_quick_actions(conn)
                .unwrap()
                .into_iter()
                .map(|a| a.id)
                .collect::<Vec<_>>()
        };
        // Move the last action to the front.
        move_quick_action(&conn, 4, 0).unwrap();
        assert_eq!(ids(&conn), vec![4, 1, 2, 3]);
        // Move the first action to the end.
        move_quick_action(&conn, 4, 99).unwrap();
        assert_eq!(ids(&conn), vec![1, 2, 3, 4]);
        // Out-of-range targets clamp.
        move_quick_action(&conn, 1, -5).unwrap();
        assert_eq!(ids(&conn), vec![1, 2, 3, 4]);
        // Same position is a no-op.
        move_quick_action(&conn, 2, 1).unwrap();
        assert_eq!(ids(&conn), vec![1, 2, 3, 4]);
        // Unknown id leaves the list untouched.
        move_quick_action(&conn, 999, 0).unwrap();
        assert_eq!(ids(&conn), vec![1, 2, 3, 4]);
    }

    #[test]
    fn cwd_is_normalized_on_save() {
        let dir = tempfile::tempdir().unwrap().into_path();
        {
            let conn = crate::db::init_at(&dir).unwrap();
            let mut with_cwd = input("placed");
            with_cwd.cwd = Some(r"D:\Work".into());
            create_quick_action(&conn, &with_cwd).unwrap();
            let stored = list_quick_actions(&conn).unwrap();
            assert_eq!(stored[0].action.cwd.as_deref(), Some(r"D:\Work"));
            // A whitespace-only cwd stores as None (the app's own directory).
            let mut cleared = input("cleared");
            cleared.cwd = Some("   ".into());
            create_quick_action(&conn, &cleared).unwrap();
            assert_eq!(list_quick_actions(&conn).unwrap()[1].action.cwd, None);
        }
        // Re-open: the normalized values survive the connection.
        let conn = crate::db::init_at(&dir).unwrap();
        let list = list_quick_actions(&conn).unwrap();
        assert_eq!(list[0].action.cwd.as_deref(), Some(r"D:\Work"));
        assert_eq!(list[1].action.cwd, None);
    }

    #[test]
    fn powershell_argv_is_the_engine_convention() {
        assert_eq!(
            powershell_argv("docker compose up -d"),
            (
                "powershell".into(),
                vec![
                    "-NoProfile".into(),
                    "-NonInteractive".into(),
                    "-Command".into(),
                    "docker compose up -d".into()
                ]
            )
        );
    }

    #[test]
    fn spawn_returns_ok_for_a_valid_action() {
        // Fire-and-forget: the call only reports whether the hidden
        // PowerShell started; the process itself is never waited on.
        let action = QuickActionInput {
            name: "spawn-test".into(),
            command: "exit 0".into(),
            cwd: None,
        };
        assert!(spawn_quick_action(&action).is_ok());
    }

    #[test]
    fn completed_test_reports_exit_code_and_output() {
        let run = test_quick_action_with_timeout(
            "Write-Output sprout-quick-action-test",
            None,
            Duration::from_secs(30),
        );
        assert!(!run.timed_out);
        assert_eq!(run.exit_code, Some(0));
        assert!(run.output.contains("sprout-quick-action-test"), "{}", run.output);
    }

    #[test]
    fn failed_test_reports_the_nonzero_exit_code() {
        let run = test_quick_action_with_timeout("exit 3", None, Duration::from_secs(30));
        assert!(!run.timed_out);
        assert_eq!(run.exit_code, Some(3));
    }

    #[test]
    fn the_working_directory_is_honored() {
        let cwd = tempfile::tempdir().unwrap().into_path();
        let run = test_quick_action_with_timeout(
            "Get-Location | Select-Object -ExpandProperty Path",
            Some(cwd.to_str().unwrap()),
            Duration::from_secs(30),
        );
        assert!(!run.timed_out);
        assert_eq!(run.exit_code, Some(0));
        assert!(run.output.contains(cwd.to_str().unwrap()), "{}", run.output);
    }

    #[test]
    fn interactive_command_is_reported_as_timed_out_not_passed() {
        let run = test_quick_action_with_timeout(
            "Start-Sleep -Seconds 30",
            None,
            Duration::from_secs(2),
        );
        assert!(run.timed_out);
        assert_eq!(run.exit_code, None);
        assert!(run.output.contains("TIMED OUT"), "{}", run.output);
    }
}