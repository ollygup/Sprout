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

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension, Result};
use serde::{Deserialize, Serialize};

use crate::engine::windows::{hidden, powershell_argv};
use crate::launch::{TestResult, TEST_TIMEOUT, timed_test_result};

/// The editable shape of a Quick Action, as the frontend sends it. The stored
/// record ([`QuickAction`]) adds the id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickActionInput {
    pub name: String,
    /// The PowerShell script to run, multi-line allowed.
    pub command: String,
    /// Working directory the command starts in; `None` = the app's own.
    pub cwd: Option<String>,
    /// Whether the Quick Launch window shows a Stop button for this action's
    /// runs (ticket 62). `false` keeps the fire-and-forget behavior.
    pub stoppable: bool,
    /// Runs when Stop is clicked; `None`/empty = kills the process tree.
    pub stop_command: Option<String>,
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

/// One tracked Quick Action run (ticket 62): the spawned process's id, held
/// in the per-session registry for as long as the process lives. The pid is
/// what a no-stop-command Stop kills (`taskkill /T /F`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningQuickAction {
    pub pid: u32,
    /// This run's `output.log`, when its folder could be created (ticket 64)
    /// — the Stop path appends the stop line and the stop command's own
    /// output to it.
    pub log_path: Option<PathBuf>,
}

/// The run-state event payload (ticket 62), emitted as
/// `quick-action-run-state-changed` when an action starts and again when its
/// process exits — the Quick Launch window flips Run ↔ Stop from these
/// events alone, with no polling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct QuickActionRunState {
    pub id: i64,
    pub running: bool,
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
/// reach the list. The Stop fields (ticket 62) add no rejections by design:
/// `stoppable` with an empty stop command is valid — empty means the process
/// tree is killed.
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

/// The stored stop command (ticket 62): whitespace-trimmed, empty values
/// become `None` — an explicitly empty field means "kill the process tree",
/// and only a real command persists. Shared with the Stop command's
/// resolution in lib.rs.
pub fn normalized_stop_command(action: &QuickActionInput) -> Option<String> {
    action
        .stop_command
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
            stoppable: row.get::<_, i64>(4)? != 0,
            stop_command: row.get(5)?,
        },
    })
}

/// Every Quick Action in list order (position, then insertion order).
pub fn list_quick_actions(conn: &Connection) -> Result<Vec<QuickAction>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, command, cwd, stoppable, stop_command
         FROM quick_actions ORDER BY position, id",
    )?;
    let rows = stmt.query_map([], action_from_row)?;
    rows.collect()
}

/// Fetches one action by id — the runner's lookup (ticket 50).
pub fn get_quick_action(conn: &Connection, id: i64) -> Result<Option<QuickAction>> {
    conn.query_row(
        "SELECT id, name, command, cwd, stoppable, stop_command
         FROM quick_actions WHERE id = ?1",
        params![id],
        action_from_row,
    )
    .optional()
}

/// The one INSERT shape for a Quick Action, position as the trailing
/// placeholder — shared by `create_quick_action` and `append_action`.
const INSERT_ACTION_SQL: &str = "INSERT INTO quick_actions (name, command, cwd, stoppable, stop_command, position)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6)";

/// Appends an action at the end of the list (the next free position).
pub fn create_quick_action(conn: &Connection, action: &QuickActionInput) -> Result<QuickAction> {
    let id = crate::ordered_list::OrderedList::QUICK_ACTIONS.create_at_end(
        conn,
        INSERT_ACTION_SQL,
        &[
            &action.name.trim(),
            &action.command.trim(),
            &normalized_cwd(action),
            &action.stoppable,
            &normalized_stop_command(action),
        ],
    )?;
    Ok(get_quick_action(conn, id)?.expect("just inserted"))
}

/// [`create_quick_action`]'s shape inside a caller-owned transaction — the
/// whole-app backup's merge appends every action under ONE transaction.
pub(crate) fn append_action(conn: &Connection, action: &QuickActionInput) -> Result<()> {
    crate::ordered_list::OrderedList::QUICK_ACTIONS
        .append_at_end(
            conn,
            INSERT_ACTION_SQL,
            &[
                &action.name.trim(),
                &action.command.trim(),
                &normalized_cwd(action),
                &action.stoppable,
                &normalized_stop_command(action),
            ],
        )
        .map(|_| ())
}

/// Replaces an action's script and metadata in place (same id). Position is
/// untouched — reorders go through `move_quick_action`.
pub fn update_quick_action(conn: &Connection, action: &QuickAction) -> Result<()> {
    conn.execute(
        "UPDATE quick_actions
         SET name = ?1, command = ?2, cwd = ?3, stoppable = ?4, stop_command = ?5
         WHERE id = ?6",
        params![
            action.action.name.trim(),
            action.action.command.trim(),
            normalized_cwd(&action.action),
            action.action.stoppable,
            normalized_stop_command(&action.action),
            action.id,
        ],
    )?;
    Ok(())
}

/// Removes an action and compacts the positions so the list stays gapless.
pub fn delete_quick_action(conn: &Connection, id: i64) -> Result<()> {
    crate::ordered_list::OrderedList::QUICK_ACTIONS.delete(conn, id)
}

/// Moves an action to `to_position` (clamped to the list), renumbering the
/// rest. The list is small (user config), so the same read-all-renumber-write
/// approach as the Launch list (ticket 38) is the obviously-correct one.
pub fn move_quick_action(conn: &Connection, id: i64, to_position: i64) -> Result<()> {
    crate::ordered_list::OrderedList::QUICK_ACTIONS.move_to(conn, id, to_position)
}

/// Spawns the action's command hidden (`CREATE_NO_WINDOW`), the working
/// directory honored when set, and returns the `Child` so the caller can
/// track it (ticket 62). When `output` is given, the command's stdout/stderr
/// are inherited from that open file — its live output lands in the run's
/// `output.log` (ticket 64). Windows does not kill children when a handle
/// closes, so dropping the `Child` would leave the process running
/// untracked; the caller decides — wait via a reaper thread, or drop for
/// fire-and-forget. Current user, no elevation, no status UI, no notification.
pub fn spawn_quick_action(
    action: &QuickActionInput,
    output: Option<&File>,
) -> std::result::Result<Child, String> {
    let (exe, args) = powershell_argv(&action.command);
    let mut command = hidden(Command::new(&exe));
    command.args(&args);
    if let Some(cwd) = normalized_cwd(action) {
        command.current_dir(&cwd);
    }
    if let Some(output) = output {
        let stdout = output
            .try_clone()
            .map_err(|e| format!("cannot attach the run log: {e}"))?;
        let stderr = output
            .try_clone()
            .map_err(|e| format!("cannot attach the run log: {e}"))?;
        command.stdout(Stdio::from(stdout));
        command.stderr(Stdio::from(stderr));
    }
    command
        .spawn()
        .map_err(|e| format!("failed to start '{exe}': {e}"))
}

/// Spawns the action's stop command (ticket 62) through the same hidden
/// PowerShell path as the run itself, the action's working directory honored
/// so relative stop commands (e.g. `docker compose stop`) land in the same
/// place the run did. When `output` is given, the stop command's output
/// appends to the run's `output.log` too (ticket 64). Fire-and-forget: a
/// graceful stop can take a while, and the reaper watching the tracked
/// process reports the actual exit.
pub fn spawn_stop_command(
    stop_command: &str,
    cwd: Option<&str>,
    output: Option<&File>,
) -> std::result::Result<(), String> {
    let (exe, args) = powershell_argv(stop_command);
    let mut command = hidden(Command::new(&exe));
    command.args(&args);
    if let Some(cwd) = cwd.map(str::trim).filter(|c| !c.is_empty()) {
        command.current_dir(cwd);
    }
    if let Some(output) = output {
        if let Ok(stdout) = output.try_clone() {
            command.stdout(Stdio::from(stdout));
        }
        if let Ok(stderr) = output.try_clone() {
            command.stderr(Stdio::from(stderr));
        }
    }
    command
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("failed to start '{exe}': {e}"))
}

// ---------------------------------------------------------------------------
// The per-run log folders (ticket 64)
// ---------------------------------------------------------------------------

/// The Quick Action run-log root under the logs dir (ticket 64): one folder
/// per run, sibling of `logs\runs`.
pub const QA_LOGS_DIR_NAME: &str = "quick-actions";

/// A Quick Action run folder's name prefix; the millis follow it
/// (`qa-<millis>-<action-id>`), the same age-embedding trick as
/// `run-<millis>`.
pub const QA_LOG_PREFIX: &str = "qa-";

/// Local wall-clock time via Win32 `GetLocalTime` — the timestamp source for
/// the run-log lines (ticket 64). No chrono dependency; the release size
/// budget (spec NFR 43) rules one out.
fn local_now() -> (u16, u16, u16, u16, u16, u16) {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    let mut st = SYSTEMTIME {
        wYear: 0,
        wMonth: 0,
        wDayOfWeek: 0,
        wDay: 0,
        wHour: 0,
        wMinute: 0,
        wSecond: 0,
        wMilliseconds: 0,
    };
    unsafe { GetLocalTime(&mut st) };
    (
        st.wYear,
        st.wMonth,
        st.wDay,
        st.wHour,
        st.wMinute,
        st.wSecond,
    )
}

/// The `[YYYY-MM-DD HH:MM:SS]` stamp from local-time components — pure, so
/// the zero-padding is testable without touching the clock.
fn format_stamp(year: u16, month: u16, day: u16, hour: u16, minute: u16, second: u16) -> String {
    format!("[{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}]")
}

/// The current local-time stamp prefix for a run-log line.
pub fn log_stamp() -> String {
    let (year, month, day, hour, minute, second) = local_now();
    format_stamp(year, month, day, hour, minute, second)
}

/// The local date and time as compact sortable components
/// (`("20260820", "140311")`) — the human-readable core of per-run folder
/// names (ticket 65).
fn local_date_time_compact() -> (String, String) {
    let (year, month, day, hour, minute, second) = local_now();
    (
        format!("{year:04}{month:02}{day:02}"),
        format!("{hour:02}{minute:02}{second:02}"),
    )
}

/// The action name as a filesystem-safe slug for a folder name (ticket 65):
/// whitespace and the characters Windows forbids (`< > : " / \ | ? *`)
/// become `-`, control characters too, repeats collapse, and the result is
/// trimmed of edge `-`/`.` and capped at ~40 chars. `None` when nothing
/// survives — the folder then carries only its timestamp.
pub(crate) fn sanitize_log_slug(name: &str) -> Option<String> {
    const MAX_SLUG_CHARS: usize = 40;
    let mapped: String = name
        .trim()
        .chars()
        .map(|ch| match ch {
            c if c.is_whitespace() => '-',
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '-',
            c if (c as u32) < 0x20 => '-',
            c => c,
        })
        .collect();
    let mut collapsed = mapped;
    while collapsed.contains("--") {
        collapsed = collapsed.replace("--", "-");
    }
    let trimmed = collapsed.trim_matches(|c| c == '-' || c == '.');
    // Char-boundary-safe cap at MAX_SLUG_CHARS.
    let mut end = 0;
    for (index, ch) in trimmed.char_indices() {
        if index >= MAX_SLUG_CHARS {
            break;
        }
        end = index + ch.len_utf8();
    }
    let cleaned = trimmed[..end].trim_matches(|c| c == '-' || c == '.');
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.to_string())
    }
}

/// Appends one finished line to a run's `output.log`. Best-effort: a logging
/// failure is never a run or stop failure (ticket 64).
pub fn append_log_line(log_path: &Path, line: &str) {
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) {
        let _ = writeln!(file, "{line}");
    }
}

/// Creates this run's log folder
/// (`logs\quick-actions\qa-<YYYYMMDD>-<HHMMSS>-<action-name>`) and returns
/// the path of its `output.log` (tickets 64 & 65). The name carries the
/// LOCAL start time plus the action's sanitized name so a human can find a
/// specific run in Explorer; the embedded timestamp stays lexically
/// sortable, which is what retention pruning reads as the age. The
/// `quick-actions` parent is part of the path — the Logs screen's listing
/// and the pruning both scan it. A same-second repeat gets a `-2`, `-3`, …
/// suffix via exclusive directory creation. `None` when the folder cannot
/// be created — the run then proceeds unlogged.
pub fn new_run_log_path(logs_dir: &Path, action_name: &str) -> Option<PathBuf> {
    let root = logs_dir.join(QA_LOGS_DIR_NAME);
    let (date, time) = local_date_time_compact();
    let base = match sanitize_log_slug(action_name) {
        Some(slug) => format!("{QA_LOG_PREFIX}{date}-{time}-{slug}"),
        None => format!("{QA_LOG_PREFIX}{date}-{time}"),
    };
    create_run_log_folder(&root, &base)
}

/// The shared per-run log-folder core behind both families (the Quick
/// Action runs' and, since ticket 77, the Quick Launch runs'): creates
/// `<root>\<base>` exclusively — a repeat gets a `-2`, `-3`, … suffix — and
/// returns the new folder's `output.log` path. `None` when the folder
/// cannot be created; callers treat that as "proceeds unlogged".
pub(crate) fn create_run_log_folder(root: &Path, base: &str) -> Option<PathBuf> {
    std::fs::create_dir_all(root).ok()?;
    let mut candidate = base.to_string();
    let mut bump: u32 = 1;
    loop {
        match std::fs::create_dir(root.join(&candidate)) {
            Ok(()) => break,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                bump += 1;
                candidate = format!("{base}-{bump}");
            }
            Err(_) => return None,
        }
    }
    Some(root.join(candidate).join("output.log"))
}

/// Opens a run's `output.log` for appending — the handle the spawned command
/// inherits as its stdout/stderr (ticket 64). `None` = the run proceeds
/// unlogged.
pub fn open_run_log(log_path: &Path) -> Option<File> {
    OpenOptions::new().create(true).append(true).open(log_path).ok()
}

/// Writes the run-log header lines: the start line (name, id, pid), then the
/// command and working directory it was launched with (ticket 64).
pub fn write_run_log_header(log_path: &Path, action: &QuickActionInput, action_id: i64, pid: u32) {
    append_log_line(
        log_path,
        &format!(
            "{} start \"{}\" (id={action_id}) pid={pid}",
            log_stamp(),
            action.name.trim()
        ),
    );
    append_log_line(
        log_path,
        &format!("  command: {}", action.command.trim()),
    );
    if let Some(cwd) = normalized_cwd(action) {
        append_log_line(log_path, &format!("  cwd: {cwd}"));
    }
}

/// Writes the stop-requested line: which path Stop took (ticket 64). The
/// stop command's own output appends below it when one was configured.
pub fn write_run_log_stop(log_path: &Path, detail: &str) {
    append_log_line(
        log_path,
        &format!("{} stop requested — {detail}", log_stamp()),
    );
}

/// Writes the exit line from the reaper's waited status (ticket 64).
pub fn write_run_log_exit(log_path: &Path, code: Option<i32>) {
    match code {
        Some(code) => append_log_line(
            log_path,
            &format!("{} exited code={code}", log_stamp()),
        ),
        None => append_log_line(
            log_path,
            &format!("{} exited (code unavailable)", log_stamp()),
        ),
    }
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
    timed_test_result(cwd, &exe, &args, timeout)
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
            stoppable: false,
            stop_command: None,
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
    fn validation_accepts_every_stoppable_shape() {
        // Ticket 62: stoppable with an empty stop command is valid — empty
        // means the process tree is killed.
        let mut tree_kill = input("dev-services");
        tree_kill.stoppable = true;
        tree_kill.stop_command = None;
        assert!(validate_quick_action(&tree_kill).is_ok());
        tree_kill.stop_command = Some("   ".into());
        assert!(validate_quick_action(&tree_kill).is_ok());
        // A real stop command is equally fine.
        tree_kill.stop_command = Some("docker compose stop".into());
        assert!(validate_quick_action(&tree_kill).is_ok());
        // Not stoppable: the Stop fields are simply unused, never rejected.
        assert!(validate_quick_action(&input("nightly-backup")).is_ok());
    }

    #[test]
    fn stop_fields_roundtrip_across_reopen() {
        let dir = tempfile::tempdir().unwrap().into_path();
        {
            let conn = crate::db::init_at(&dir).unwrap();
            let mut graceful = input("dev-services");
            graceful.stoppable = true;
            graceful.stop_command = Some("  docker compose stop  ".into());
            create_quick_action(&conn, &graceful).unwrap();

            let mut tree_kill = input("local-server");
            tree_kill.stoppable = true;
            tree_kill.stop_command = None;
            create_quick_action(&conn, &tree_kill).unwrap();

            create_quick_action(&conn, &input("nightly-backup")).unwrap();
        }
        // Re-open: the flag and the normalized command survive the connection.
        let conn = crate::db::init_at(&dir).unwrap();
        let list = list_quick_actions(&conn).unwrap();
        assert_eq!(list.len(), 3);
        assert!(list[0].action.stoppable);
        assert_eq!(
            list[0].action.stop_command.as_deref(),
            Some("docker compose stop")
        );
        assert!(list[1].action.stoppable);
        assert_eq!(list[1].action.stop_command, None);
        assert!(!list[2].action.stoppable);
        assert_eq!(list[2].action.stop_command, None);

        // Editing flips the flag off and clears the command in place.
        let mut edited = list[0].clone();
        edited.action.stoppable = false;
        edited.action.stop_command = Some("docker compose stop".into());
        update_quick_action(&conn, &edited).unwrap();
        let stored = get_quick_action(&conn, list[0].id)
            .unwrap()
            .expect("still there");
        assert!(!stored.action.stoppable);
    }

    #[test]
    fn stop_command_whitespace_stores_as_none() {
        let conn = conn();
        let mut blank = input("blank-stop");
        blank.stoppable = true;
        blank.stop_command = Some("   ".into());
        create_quick_action(&conn, &blank).unwrap();
        let stored = list_quick_actions(&conn).unwrap();
        assert!(stored[0].action.stoppable);
        assert_eq!(stored[0].action.stop_command, None);
    }

    #[test]
    fn spawn_returns_tracked_child_for_a_valid_action() {
        // Tracked spawn (ticket 62): the call hands back the Child so the
        // caller can wait on it; here it is dropped, which leaves the
        // process running untracked — the same fire-and-forget as before.
        let action = QuickActionInput {
            name: "spawn-test".into(),
            command: "exit 0".into(),
            cwd: None,
            stoppable: false,
            stop_command: None,
        };
        let mut child = spawn_quick_action(&action, None).expect("spawned");
        let _ = child.wait();
    }

    #[test]
    fn stop_command_spawns_through_the_hidden_powershell_path() {
        // The stop path (ticket 62): same hidden PowerShell convention, the
        // process outlives the dropped handle.
        assert!(spawn_stop_command("exit 0", None, None).is_ok());
    }

    #[test]
    fn stamp_pads_every_component() {
        assert_eq!(
            format_stamp(2026, 8, 20, 7, 3, 11),
            "[2026-08-20 07:03:11]"
        );
    }

    #[test]
    fn run_log_folder_and_header_are_created() {
        let logs_dir = tempfile::tempdir().unwrap().into_path();
        let log_path = new_run_log_path(&logs_dir, "dev-services").expect("folder created");
        assert_eq!(log_path.file_name().unwrap(), "output.log");
        // The folder lives under the quick-actions root — the listing and
        // pruning scan exactly that parent (regression: it once landed
        // directly under logs\, invisible to both).
        assert_eq!(
            log_path.parent().unwrap().parent().unwrap(),
            logs_dir.join(QA_LOGS_DIR_NAME)
        );
        // Readable name (ticket 65): qa-<8-digit date>-<6-digit time>-slug.
        let folder = log_path
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let segments: Vec<&str> = folder.split('-').collect();
        assert_eq!(segments[0], QA_LOG_PREFIX.trim_end_matches('-'));
        assert_eq!(segments[1].len(), 8, "{folder}");
        assert!(segments[1].bytes().all(|b| b.is_ascii_digit()), "{folder}");
        assert_eq!(segments[2].len(), 6, "{folder}");
        assert!(segments[2].bytes().all(|b| b.is_ascii_digit()), "{folder}");
        assert_eq!(segments[3..].join("-"), "dev-services", "{folder}");
        let mut action = input("dev-services");
        action.command = "docker compose up".into();
        action.cwd = Some(r"D:\Work".into());
        write_run_log_header(&log_path, &action, 7, 42);
        let contents = std::fs::read_to_string(&log_path).unwrap();
        assert!(contents.contains("start \"dev-services\" (id=7) pid=42"), "{contents}");
        assert!(contents.contains("command: docker compose up"), "{contents}");
        assert!(contents.contains(r"cwd: D:\Work"), "{contents}");
        // A second call appends — never truncates.
        write_run_log_exit(&log_path, Some(0));
        let contents = std::fs::read_to_string(&log_path).unwrap();
        assert!(contents.contains("exited code=0"), "{contents}");
        assert!(contents.contains("start \"dev-services\""), "{contents}");
    }

    #[test]
    fn stop_line_records_which_path_was_taken() {
        let dir = tempfile::tempdir().unwrap().into_path();
        let log_path = dir.join("output.log");
        write_run_log_stop(&log_path, "tree kill (taskkill /T /F)");
        write_run_log_stop(&log_path, "stop command: docker compose stop");
        write_run_log_exit(&log_path, None);
        let contents = std::fs::read_to_string(&log_path).unwrap();
        assert!(
            contents.contains("stop requested — tree kill (taskkill /T /F)"),
            "{contents}"
        );
        assert!(
            contents.contains("stop requested — stop command: docker compose stop"),
            "{contents}"
        );
        assert!(contents.contains("exited (code unavailable)"), "{contents}");
    }

    #[test]
    fn slugs_are_filesystem_safe_and_readable() {
        assert_eq!(
            sanitize_log_slug("docker compose: up!").as_deref(),
            Some("docker-compose-up!")
        );
        assert_eq!(sanitize_log_slug("  my   action  ").as_deref(), Some("my-action"));
        // Every character Windows forbids in a folder name maps away.
        assert_eq!(
            sanitize_log_slug(r#"C:\path<to>:"*?|"#).as_deref(),
            Some("C-path-to")
        );
        // Nothing survives → None (timestamp-only folder name).
        assert_eq!(sanitize_log_slug("   "), None);
        assert_eq!(sanitize_log_slug("***"), None);
    }

    #[test]
    fn same_second_reruns_get_a_suffix() {
        let logs_dir = tempfile::tempdir().unwrap().into_path();
        let first = new_run_log_path(&logs_dir, "dev-services").expect("first");
        let second = new_run_log_path(&logs_dir, "dev-services").expect("second");
        assert_ne!(first.parent(), second.parent());
        assert!(
            second
                .parent()
                .unwrap()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with("-2"),
            "{}",
            second.parent().unwrap().display()
        );
    }

    #[test]
    fn tracked_spawn_captures_the_command_output() {
        let logs_dir = tempfile::tempdir().unwrap().into_path();
        let log_path = new_run_log_path(&logs_dir, "capture-test").expect("folder created");
        let output = open_run_log(&log_path).expect("log opened");
        let action = QuickActionInput {
            name: "capture-test".into(),
            command: "Write-Output sprout-capture-test".into(),
            cwd: None,
            stoppable: false,
            stop_command: None,
        };
        let mut child = spawn_quick_action(&action, Some(&output)).expect("spawned");
        let _ = child.wait();
        drop(output);
        let contents = std::fs::read_to_string(&log_path).unwrap();
        assert!(contents.contains("sprout-capture-test"), "{contents}");
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