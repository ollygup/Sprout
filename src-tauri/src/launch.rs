//! Quick Launch (ticket 37-44): the machine-local list of Launch entries the
//! Quick Launch window and page start through the shared runner (ticket 54),
//! plus the search that finds installed apps.
//!
//! Glossary (docs/CONTEXT.md): a **Launch entry** is one thing the Quick
//! Launch list can start — a found app (shortcut or exe) or a command.
//! A **Launch run** is one execution of the list through the capped, queued
//! pipeline (ticket 42). A **desktop assignment** is an entry's target
//! virtual desktop; `None` means "current desktop" (ticket 44).
//!
//! The list is machine-local like `install_dir` (ADR-0009's spirit): paths
//! and commands mean nothing on another machine, so it is never part of a
//! Preset, Plan, Run, or export.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use rusqlite::{params, Connection, OptionalExtension, Result};
use serde::{Deserialize, Serialize};

use crate::engine::{LauncherEngine, Spawned};
use crate::engine::windows::{powershell_argv, run_timed_process_in};

/// The Test-click timebox (ticket 41): long enough for a normal startup,
/// short enough that an interactive command is reported honestly as not
/// headless-verifiable instead of wedging the dialog. Shared with the Quick
/// Actions Test (ticket 50), whose box is deliberately the same.
pub const TEST_TIMEOUT: Duration = Duration::from_secs(20);

/// What a Launch entry starts: a picked app (shortcut or exe, launched as-is
/// via ShellExecuteExW) or a command the user wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LaunchEntryKind {
    App,
    Command,
}

fn kind_to_str(kind: LaunchEntryKind) -> &'static str {
    match kind {
        LaunchEntryKind::App => "app",
        LaunchEntryKind::Command => "command",
    }
}

fn kind_from_str(value: &str) -> Option<LaunchEntryKind> {
    match value {
        "app" => Some(LaunchEntryKind::App),
        "command" => Some(LaunchEntryKind::Command),
        _ => None,
    }
}

/// The shell a command entry runs under: PowerShell, cmd, or no shell at all
/// (the target is an executable launched directly).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LaunchShell {
    Powershell,
    Cmd,
    None,
}

fn shell_to_str(shell: LaunchShell) -> &'static str {
    match shell {
        LaunchShell::Powershell => "powershell",
        LaunchShell::Cmd => "cmd",
        LaunchShell::None => "none",
    }
}

fn shell_from_str(value: &str) -> Option<LaunchShell> {
    match value {
        "powershell" => Some(LaunchShell::Powershell),
        "cmd" => Some(LaunchShell::Cmd),
        "none" => Some(LaunchShell::None),
        _ => None,
    }
}

/// The editable shape of a Launch entry, as the frontend sends it. The stored
/// record ([`LaunchEntry`]) adds the id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchEntryInput {
    pub name: String,
    pub kind: LaunchEntryKind,
    /// For app entries: the .lnk or exe path to launch. For command entries:
    /// the command line itself.
    pub target: String,
    /// Command entries only; `None` for app entries.
    pub shell: Option<LaunchShell>,
    /// Command entries only: hidden by default (the engine's
    /// CREATE_NO_WINDOW convention), optional visible window for debugging.
    pub show_window: bool,
    /// The target virtual desktop's GUID (ticket 44); `None` = launch on the
    /// current desktop.
    pub desktop_id: Option<String>,
}

/// A Launch entry as stored: the input plus its library id. Position is
/// internal (order within the list) and never part of the payload — reorders
/// go through `move_launch_entry`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchEntry {
    pub id: i64,
    #[serde(flatten)]
    pub entry: LaunchEntryInput,
}

/// Rejects entries that would break a launch: a blank name or target, an
/// app entry pretending to be a command (shell set), a command entry without
/// a shell choice, or a malformed desktop GUID. A broken entry must never
/// reach the list.
pub fn validate_launch_entry(entry: &LaunchEntryInput) -> std::result::Result<(), String> {
    if entry.name.trim().is_empty() {
        return Err("Launch entry name must not be empty".into());
    }
    if entry.target.trim().is_empty() {
        return Err("Launch entry target must not be empty".into());
    }
    match entry.kind {
        LaunchEntryKind::App => {
            if entry.shell.is_some() {
                return Err(
                    "An app entry launches its target directly — it cannot carry a shell choice"
                        .into(),
                );
            }
        }
        LaunchEntryKind::Command => {
            if entry.shell.is_none() {
                return Err("A command entry needs a shell choice (PowerShell, cmd, or none)".into());
            }
        }
    }
    if let Some(guid) = &entry.desktop_id {
        if !looks_like_guid(guid) {
            return Err(format!(
                "'{guid}' is not a virtual desktop id — it must be a GUID like 8-4-4-4-12"
            ));
        }
    }
    Ok(())
}

/// A loose GUID shape check (8-4-4-4-12 hex digits) — enough to reject
/// typos before they reach the launch pipeline. The one copy of the
/// predicate: the engine's desktop-move parser builds its GUID on top of it.
pub(crate) fn looks_like_guid(value: &str) -> bool {
    let parts: Vec<&str> = value.split('-').collect();
    if parts.len() != 5 {
        return false;
    }
    let lengths = [8usize, 4, 4, 4, 12];
    parts
        .iter()
        .zip(lengths)
        .all(|(part, len)| part.len() == len && part.chars().all(|c| c.is_ascii_hexdigit()))
}

fn entry_from_row(row: &rusqlite::Row) -> Result<LaunchEntry> {
    let kind: String = row.get(2)?;
    let shell: Option<String> = row.get(4)?;
    Ok(LaunchEntry {
        id: row.get(0)?,
        entry: LaunchEntryInput {
            name: row.get(1)?,
            kind: kind_from_str(&kind).ok_or_else(|| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::other(format!("unknown launch entry kind '{kind}'"))),
                )
            })?,
            target: row.get(3)?,
            shell: match shell.as_deref() {
                None => None,
                Some(value) => Some(shell_from_str(value).ok_or_else(|| {
                    rusqlite::Error::FromSqlConversionFailure(
                        4,
                        rusqlite::types::Type::Text,
                        Box::new(std::io::Error::other(format!(
                            "unknown launch shell '{value}'"
                        ))),
                    )
                })?),
            },
            show_window: row.get(5)?,
            desktop_id: row.get(6)?,
        },
    })
}

/// Every Launch entry in list order (position, then insertion order).
pub fn list_launch_entries(conn: &Connection) -> Result<Vec<LaunchEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, kind, target, shell, show_window, desktop_id
         FROM launch_entries ORDER BY position, id",
    )?;
    let rows = stmt.query_map([], entry_from_row)?;
    rows.collect()
}

fn get_entry(conn: &Connection, id: i64) -> Result<Option<LaunchEntry>> {
    conn.query_row(
        "SELECT id, name, kind, target, shell, show_window, desktop_id
         FROM launch_entries WHERE id = ?1",
        params![id],
        entry_from_row,
    )
    .optional()
}

/// Appends an entry at the end of the list (the next free position).
pub fn create_launch_entry(conn: &Connection, entry: &LaunchEntryInput) -> Result<LaunchEntry> {
    let id = crate::ordered_list::OrderedList::LAUNCH_ENTRIES.create_at_end(
        conn,
        "INSERT INTO launch_entries (name, kind, target, shell, show_window, desktop_id, position)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        &[
            &entry.name.trim(),
            &kind_to_str(entry.kind),
            &entry.target.trim(),
            &entry.shell.map(shell_to_str),
            &entry.show_window,
            &entry.desktop_id,
        ],
    )?;
    Ok(get_entry(conn, id)?.expect("just inserted"))
}

/// Replaces an entry's metadata in place (same id). Position is untouched —
/// reorders go through `move_launch_entry`.
pub fn update_launch_entry(conn: &Connection, entry: &LaunchEntry) -> Result<()> {
    conn.execute(
        "UPDATE launch_entries
         SET name = ?1, kind = ?2, target = ?3, shell = ?4, show_window = ?5, desktop_id = ?6
         WHERE id = ?7",
        params![
            entry.entry.name.trim(),
            kind_to_str(entry.entry.kind),
            entry.entry.target.trim(),
            entry.entry.shell.map(shell_to_str),
            entry.entry.show_window,
            entry.entry.desktop_id,
            entry.id,
        ],
    )?;
    Ok(())
}

/// Removes an entry and compacts the positions so the list stays gapless.
pub fn delete_launch_entry(conn: &Connection, id: i64) -> Result<()> {
    crate::ordered_list::OrderedList::LAUNCH_ENTRIES.delete(conn, id)
}

/// Moves an entry to `to_position` (clamped to the list), renumbering the
/// rest. The list is small (user config), so a read-all-renumber-write in one
/// transaction is the obviously-correct approach.
pub fn move_launch_entry(conn: &Connection, id: i64, to_position: i64) -> Result<()> {
    crate::ordered_list::OrderedList::LAUNCH_ENTRIES.move_to(conn, id, to_position)
}

/// The result of one Test click in the add-command dialog (ticket 41): the
/// exit code and merged output of the timeboxed run. `timed_out` is honest —
/// an interactive command that outlives the box is not headless-verifiable,
/// never passed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TestResult {
    pub timed_out: bool,
    pub exit_code: Option<i32>,
    pub output: String,
}

/// Builds the argv a command entry runs under — the same shape the launch
/// pipeline uses (ticket 42): PowerShell gets the engine's non-interactive
/// one-liner convention, cmd gets `/c`, and "none" launches the command line
/// as-is.
pub fn command_argv(shell: LaunchShell, target: &str) -> (String, Vec<String>) {
    match shell {
        LaunchShell::Powershell => powershell_argv(target),
        LaunchShell::Cmd => ("cmd".into(), vec!["/c".into(), target.into()]),
        LaunchShell::None => split_command_line(target),
    }
}

/// Splits a Windows command line into the executable and its arguments,
/// honoring double-quoted segments the way cmd does. A blank line yields an
/// empty exe — the caller's validation keeps that from ever being tested.
fn split_command_line(line: &str) -> (String, Vec<String>) {
    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for ch in line.trim().chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    let mut parts = parts.into_iter();
    let exe = parts.next().unwrap_or_default();
    (exe, parts.collect())
}

/// Runs one command entry under its shell, timeboxed, and reports the exit
/// code plus captured output (ticket 41). The run is always headless — the
/// engine's CREATE_NO_WINDOW convention — whatever the entry's own
/// show-window toggle; a test that needs a visible window would not be
/// verifiable anyway.
pub fn test_launch_command(shell: LaunchShell, target: &str) -> TestResult {
    run_command_with_timeout(shell, target, TEST_TIMEOUT)
}

/// The timeboxed core behind `test_launch_command`, parameterized so tests
/// can use a short box.
pub(crate) fn run_command_with_timeout(
    shell: LaunchShell,
    target: &str,
    timeout: Duration,
) -> TestResult {
    let (exe, args) = command_argv(shell, target);
    timed_test_result(None, &exe, &args, timeout)
}

/// The one timed run behind both Test buttons (tickets 41 & 50): runs
/// `exe args` headlessly under `timeout` — cwd-aware, which is how the
/// Quick Action variant honors its configured directory — and maps the
/// engine's raw run to [`TestResult`].
pub(crate) fn timed_test_result(
    cwd: Option<&str>,
    exe: &str,
    args: &[String],
    timeout: Duration,
) -> TestResult {
    let run = run_timed_process_in(cwd, exe, args, timeout);
    TestResult {
        timed_out: run.timed_out,
        exit_code: run.exit_code,
        output: run.output,
    }
}

// ---------------------------------------------------------------------------
// The capped, queued launch pipeline (ticket 42)
// ---------------------------------------------------------------------------

/// How long an app entry may hold its slot while its main window appears.
/// After that the launch counts as *started* anyway — the queue must never
/// stall on an app that shows no window (a console tool, a tray app).
pub const WINDOW_TIMEOUT: Duration = Duration::from_secs(15);

/// The slice each in-flight slot is polled with. Chunked polling means a
/// window that appears frees its slot within one chunk of the next poll —
/// the queue drains as windows appear, never after a fixed wait.
const SLOT_POLL: Duration = Duration::from_millis(250);

/// The end-of-run summary (ticket 42): the names of every entry that
/// started, was skipped or failed — in list order — plus the
/// desktop-assignment notes (tickets 44 & 47): an entry whose desktop no
/// longer exists falls back to the current desktop and the note says so,
/// and a move that could not be performed (refused by the OS, or the window
/// never appeared) is a note too — never silent. Skipped entries carry
/// their reason (ticket 48) — "Command Prompt — already open on this
/// desktop" — so a no-op run is never silent. This is what the summary
/// notification and the `launch-run-done` event carry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LaunchReport {
    pub started: Vec<String>,
    /// Entry names with the reason, e.g. "Command Prompt — already open on
    /// this desktop" (ticket 48).
    pub skipped: Vec<String>,
    /// Entry names, with the reason when the entry failed before launch
    /// ("X — target no longer exists — update this entry", ticket 48).
    pub failed: Vec<String>,
    pub notes: Vec<String>,
}

/// A desktop move the queue owes (ticket 47): the target desktop's id, plus
/// the entry's name — the name appears in the note when the move cannot be
/// performed, so a failed move is never silent.
struct PendingMove {
    guid: String,
    name: String,
}

/// Runs the whole Quick Launch list through the capped, queued pipeline
/// (ticket 42): at most `cap` app entries in flight at once, the rest queue;
/// an entry frees its slot when its main window appears; command entries and
/// windowless apps free theirs at spawn; a failure never aborts the rest.
/// The 15 s window timeout counts as started, so the queue always drains.
/// The skip rule (ticket 48) is per-window-per-desktop: an app entry skips
/// only when a window of its image already sits on the target desktop — the
/// assigned one, or the current desktop when unassigned — and the reason is
/// reported, never silent. Every launch snapshots the app's windows before
/// it starts, so the window the queue waits on and moves is the one that
/// appeared after the launch — never one the user already has open — and an
/// entry whose target vanished from disk fails fast with a clear message,
/// no silent 15 s stall. Pure logic — driven by the [`LauncherEngine`] seam
/// and proven against a fake in tests.
pub fn run_launch_queue(
    engine: &dyn LauncherEngine,
    entries: &[LaunchEntry],
    cap: usize,
) -> LaunchReport {
    run_launch_queue_until(engine, entries, cap, WINDOW_TIMEOUT)
}

/// The parameterized core behind [`run_launch_queue`] — the window timeout is
/// injected so the fake-driven tests prove the timeout rules on a
/// millisecond scale instead of sleeping 15 s.
fn run_launch_queue_until(
    engine: &dyn LauncherEngine,
    entries: &[LaunchEntry],
    cap: usize,
    window_timeout: Duration,
) -> LaunchReport {
    let cap = cap.max(1);
    let mut report = LaunchReport::default();
    let mut in_flight: Vec<(Spawned, Vec<usize>, Instant)> = Vec::new();
    // Ticket 44 & 47: spawned launches whose main window still has to move to
    // their assigned desktop, spawn → move. The move happens when the window
    // appears — never before, winvd needs the real window — and the window is
    // the NEW one the resolution finds (the snapshot preference, ticket 48),
    // which is not necessarily owned by the spawned pid (a handed-off launch,
    // a wrapper's child).
    let mut pending_moves: HashMap<Spawned, PendingMove> = HashMap::new();
    let mut queue: VecDeque<&LaunchEntry> = entries.iter().collect();

    while let Some(entry) = queue.pop_front() {
        // The skip rule (app entries only — a command has no window to
        // match): does the app already have a window on the target desktop?
        // An assigned entry skips only when a window sits on its desktop
        // GUID; an unassigned entry only when a window sits on the current
        // desktop. Windows elsewhere are never skipped over and never moved.
        // The same query doubles as the pre-launch snapshot the new-window
        // resolution prefers windows against (ticket 48).
        if entry.entry.kind == LaunchEntryKind::App {
            let windows = engine.app_windows(&entry.entry.target);
            let on_target = match &entry.entry.desktop_id {
                Some(guid) => windows
                    .iter()
                    .any(|window| window.desktop.as_deref() == Some(guid.as_str())),
                None => windows.iter().any(|window| window.on_current_desktop),
            };
            if on_target {
                report.skipped.push(format!(
                    "{} — already open on this desktop",
                    entry.entry.name
                ));
                continue;
            }
            // Ticket 48: an app that updated its version folder fails fast —
            // no silent 15 s window stall.
            if !engine.target_exists(&entry.entry.target) {
                report.failed.push(format!(
                    "{} — target no longer exists — update this entry",
                    entry.entry.name
                ));
                continue;
            }
        }
        while in_flight.len() >= cap {
            free_slot(engine, &mut report, &mut in_flight, &mut pending_moves, window_timeout);
        }
        // Ticket 48: the visible-window snapshot is taken right before the
        // launch — the new-window resolution prefers windows that appeared
        // after it, never one the user already has open.
        let before = if entry.entry.kind == LaunchEntryKind::App {
            engine
                .app_windows(&entry.entry.target)
                .into_iter()
                .map(|window| window.hwnd)
                .collect()
        } else {
            Vec::new()
        };
        match engine.spawn(&entry.entry) {
            Ok(spawned) => {
                report.started.push(entry.entry.name.clone());
                if let Some(guid) = &entry.entry.desktop_id {
                    // The fallback (ticket 44): a desktop that no longer
                    // exists leaves the window on the current desktop — the
                    // launch itself is untouched — and the summary says so.
                    if engine.desktops().iter().any(|desktop| &desktop.id == guid) {
                        pending_moves.insert(
                            spawned.clone(),
                            PendingMove {
                                guid: guid.clone(),
                                name: entry.entry.name.clone(),
                            },
                        );
                    } else {
                        report.notes.push(format!(
                            "{} opened on the current desktop — its desktop no longer exists",
                            entry.entry.name
                        ));
                    }
                }
                // Entries that must land on a desktop hold their slot until
                // the window appears (the window is what gets moved);
                // command entries without an assignment free theirs at
                // spawn.
                if entry.entry.kind == LaunchEntryKind::App
                    || entry.entry.desktop_id.is_some()
                {
                    in_flight.push((spawned, before, Instant::now()));
                }
            }
            Err(_) => report.failed.push(entry.entry.name.clone()),
        }
    }
    // Drain the slots of the final wave.
    while !in_flight.is_empty() {
        free_slot(engine, &mut report, &mut in_flight, &mut pending_moves, window_timeout);
    }
    report
}

/// Polls the in-flight entries until one frees its slot: its NEW main
/// window appeared, or its window deadline passed (counts as started — the
/// queue never stalls on a windowless app). Always terminates: every slot
/// is bounded by `window_timeout`. A freed entry with a pending desktop move
/// has it performed right here — the new window the queue waited for is the
/// window that gets moved (tickets 44 & 48); a timeout frees the slot and
/// drops the move, and a refused move is a note, never a silent drop
/// (ticket 47).
fn free_slot(
    engine: &dyn LauncherEngine,
    report: &mut LaunchReport,
    in_flight: &mut Vec<(Spawned, Vec<usize>, Instant)>,
    pending_moves: &mut HashMap<Spawned, PendingMove>,
    window_timeout: Duration,
) {
    let mut index = 0;
    loop {
        if index >= in_flight.len() {
            index = 0;
        }
        let (spawned, before, spawned_at) = in_flight[index].clone();
        let elapsed = spawned_at.elapsed();
        if elapsed >= window_timeout {
            in_flight.swap_remove(index);
            // The window never appeared — the assignment cannot be honored,
            // and the note says so instead of dropping it silently.
            if let Some(pending) = pending_moves.remove(&spawned) {
                report.notes.push(format!(
                    "{} opened on the current desktop — could not move it",
                    pending.name
                ));
            }
            return;
        }
        let chunk = (window_timeout - elapsed).min(SLOT_POLL);
        // Ticket 48: the wait resolves the app's NEW window — never one the
        // user already had open — and the move hands winvd exactly that
        // window.
        if let Some(hwnd) = engine.wait_for_new_window(&spawned, &before, chunk) {
            in_flight.swap_remove(index);
            if let Some(pending) = pending_moves.remove(&spawned) {
                // A move failure never fails the entry — the launch already
                // happened — but it is a note, never a silent drop (ticket
                // 47); the reason rides along (ticket 48).
                if let Err(error) = engine.move_window_to_desktop(hwnd, &pending.guid) {
                    report.notes.push(format!(
                        "{} opened on the current desktop — could not move it: {error}",
                        pending.name
                    ));
                }
            }
            return;
        }
        index += 1;
    }
}

/// The end-of-run summary text (ticket 42): the started / skipped / failed
/// counts, plus the names of the failed when there are any — and the
/// skipped entries with their reasons (ticket 48), so a no-op run is never
/// silent — plus the desktop-assignment notes (ticket 44). What the summary
/// notification carries.
pub fn launch_summary_body(report: &LaunchReport) -> String {
    let mut body = format!(
        "started {}, skipped {}, failed {}",
        report.started.len(),
        report.skipped.len(),
        report.failed.len(),
    );
    if !report.skipped.is_empty() {
        body.push_str(" — skipped: ");
        body.push_str(&report.skipped.join(", "));
    }
    if !report.failed.is_empty() {
        body.push_str(" — failed: ");
        body.push_str(&report.failed.join(", "));
    }
    for note in &report.notes {
        body.push_str(" — ");
        body.push_str(note);
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};
    use std::path::Path;
    use std::sync::Mutex;

    fn conn() -> Connection {
        crate::db::init_at(&tempfile::tempdir().unwrap().into_path()).unwrap()
    }

    fn app_input(name: &str) -> LaunchEntryInput {
        LaunchEntryInput {
            name: name.into(),
            kind: LaunchEntryKind::App,
            target: format!(r"C:\Apps\{name}.exe"),
            shell: None,
            show_window: false,
            desktop_id: None,
        }
    }

    fn command_input(name: &str) -> LaunchEntryInput {
        LaunchEntryInput {
            name: name.into(),
            kind: LaunchEntryKind::Command,
            target: format!("start {name}"),
            shell: Some(LaunchShell::Cmd),
            show_window: false,
            desktop_id: None,
        }
    }

    #[test]
    fn validation_rejects_broken_entries() {
        let mut entry = app_input("Code");
        entry.name = " ".into();
        assert!(validate_launch_entry(&entry).is_err());

        let mut entry = app_input("Code");
        entry.target = String::new();
        assert!(validate_launch_entry(&entry).is_err());

        // An app entry cannot carry a shell choice.
        let mut entry = app_input("Code");
        entry.shell = Some(LaunchShell::Powershell);
        assert!(validate_launch_entry(&entry).is_err());

        // A command entry needs a shell choice.
        let mut entry = command_input("do-things");
        entry.shell = None;
        assert!(validate_launch_entry(&entry).is_err());

        // A desktop id must look like a GUID.
        let mut entry = app_input("Code");
        entry.desktop_id = Some("not-a-guid".into());
        assert!(validate_launch_entry(&entry).is_err());
        entry.desktop_id = Some("550fe0a1-3d41-4e5f-9a2b-c8d0e1f2a3b4".into());
        assert!(validate_launch_entry(&entry).is_ok());

        assert!(validate_launch_entry(&app_input("Code")).is_ok());
        assert!(validate_launch_entry(&command_input("do-things")).is_ok());
    }

    #[test]
    fn crud_roundtrips_across_reopen() {
        let dir = tempfile::tempdir().unwrap().into_path();
        {
            let conn = crate::db::init_at(&dir).unwrap();
            let first = create_launch_entry(&conn, &app_input("Code")).unwrap();
            let second = create_launch_entry(&conn, &command_input("start-postgres")).unwrap();
            create_launch_entry(&conn, &app_input("Postman")).unwrap();
            assert_eq!(first.id, 1);
            assert_eq!(second.id, 2);
            let list = list_launch_entries(&conn).unwrap();
            assert_eq!(list.len(), 3);
            assert_eq!(
                list.iter().map(|e| e.entry.name.as_str()).collect::<Vec<_>>(),
                vec!["Code", "start-postgres", "Postman"]
            );
            // Update keeps the position.
            let mut updated = list[1].clone();
            updated.entry.name = "Postgres".into();
            update_launch_entry(&conn, &updated).unwrap();
        }
        // Re-open: everything survives the connection.
        let conn = crate::db::init_at(&dir).unwrap();
        let list = list_launch_entries(&conn).unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[1].entry.name, "Postgres");
        assert_eq!(list[2].entry.name, "Postman");
    }

    #[test]
    fn delete_compacts_positions() {
        let conn = conn();
        create_launch_entry(&conn, &app_input("A")).unwrap();
        create_launch_entry(&conn, &app_input("B")).unwrap();
        create_launch_entry(&conn, &app_input("C")).unwrap();
        delete_launch_entry(&conn, 1).unwrap();
        let list = list_launch_entries(&conn).unwrap();
        assert_eq!(
            list.iter().map(|e| e.id).collect::<Vec<_>>(),
            vec![2, 3]
        );
        // A new entry lands at the end of the compacted list.
        create_launch_entry(&conn, &app_input("D")).unwrap();
        let list = list_launch_entries(&conn).unwrap();
        assert_eq!(
            list.iter().map(|e| e.id).collect::<Vec<_>>(),
            vec![2, 3, 4]
        );
        // Deleting an unknown id is a no-op.
        delete_launch_entry(&conn, 999).unwrap();
        assert_eq!(list_launch_entries(&conn).unwrap().len(), 3);
    }

    #[test]
    fn move_reorders_and_clamps() {
        let conn = conn();
        for name in ["A", "B", "C", "D"] {
            create_launch_entry(&conn, &app_input(name)).unwrap();
        }
        let ids = |conn: &Connection| {
            list_launch_entries(conn)
                .unwrap()
                .into_iter()
                .map(|e| e.id)
                .collect::<Vec<_>>()
        };
        // Move the last entry to the front.
        move_launch_entry(&conn, 4, 0).unwrap();
        assert_eq!(ids(&conn), vec![4, 1, 2, 3]);
        // Move the first entry to the end.
        move_launch_entry(&conn, 4, 99).unwrap();
        assert_eq!(ids(&conn), vec![1, 2, 3, 4]);
        // Out-of-range targets clamp.
        move_launch_entry(&conn, 1, -5).unwrap();
        assert_eq!(ids(&conn), vec![1, 2, 3, 4]);
        // Same position is a no-op.
        move_launch_entry(&conn, 2, 1).unwrap();
        assert_eq!(ids(&conn), vec![1, 2, 3, 4]);
        // Unknown id leaves the list untouched.
        move_launch_entry(&conn, 999, 0).unwrap();
        assert_eq!(ids(&conn), vec![1, 2, 3, 4]);
    }

    #[test]
    fn command_argv_wraps_each_shell() {
        assert_eq!(
            command_argv(LaunchShell::Powershell, "echo hi"),
            (
                "powershell".into(),
                vec![
                    "-NoProfile".into(),
                    "-NonInteractive".into(),
                    "-Command".into(),
                    "echo hi".into()
                ]
            )
        );
        assert_eq!(
            command_argv(LaunchShell::Cmd, "echo hi"),
            ("cmd".into(), vec!["/c".into(), "echo hi".into()])
        );
        assert_eq!(
            command_argv(LaunchShell::None, r#""C:\Program Files\App\app.exe" --flag"#),
            (
                r"C:\Program Files\App\app.exe".into(),
                vec!["--flag".into()]
            )
        );
        // A bare exe has no args; a blank line has no exe.
        assert_eq!(command_argv(LaunchShell::None, "tool.exe"), ("tool.exe".into(), vec![]));
        assert_eq!(command_argv(LaunchShell::None, "  "), (String::new(), vec![]));
    }

    #[test]
    fn completed_test_reports_exit_code_and_output() {
        let run = run_command_with_timeout(
            LaunchShell::Cmd,
            "echo sprout-command-test",
            Duration::from_secs(30),
        );
        assert!(!run.timed_out);
        assert_eq!(run.exit_code, Some(0));
        assert!(run.output.contains("sprout-command-test"), "{}", run.output);
    }

    #[test]
    fn failed_test_reports_the_nonzero_exit_code() {
        let run = run_command_with_timeout(LaunchShell::Cmd, "exit 3", Duration::from_secs(30));
        assert!(!run.timed_out);
        assert_eq!(run.exit_code, Some(3));
    }

    #[test]
    fn powershell_test_runs_through_the_engine_convention() {
        let run = run_command_with_timeout(
            LaunchShell::Powershell,
            "Write-Output sprout-ps-test",
            Duration::from_secs(30),
        );
        assert!(!run.timed_out);
        assert_eq!(run.exit_code, Some(0));
        assert!(run.output.contains("sprout-ps-test"), "{}", run.output);
    }

    #[test]
    fn direct_exe_test_launches_the_command_line_as_is() {
        let run = run_command_with_timeout(
            LaunchShell::None,
            "cmd /c echo sprout-direct-test",
            Duration::from_secs(30),
        );
        assert!(!run.timed_out);
        assert_eq!(run.exit_code, Some(0));
        assert!(run.output.contains("sprout-direct-test"), "{}", run.output);
    }

    #[test]
    fn interactive_command_is_reported_as_timed_out_not_passed() {
        let run = run_command_with_timeout(
            LaunchShell::Powershell,
            "Start-Sleep -Seconds 30",
            Duration::from_secs(2),
        );
        assert!(run.timed_out);
        assert_eq!(run.exit_code, None);
        assert!(run.output.contains("TIMED OUT"), "{}", run.output);
    }

    #[test]
    fn missing_executable_is_a_clean_failure() {
        let run = run_command_with_timeout(
            LaunchShell::None,
            "no-such-binary-sprout-test",
            Duration::from_secs(5),
        );
        assert!(!run.timed_out);
        assert_eq!(run.exit_code, None);
        assert!(run.output.contains("failed to start"), "{}", run.output);
    }

    // ------------------- the capped launch pipeline (ticket 42) -----------

    fn app_entry(name: &str, id: i64) -> LaunchEntry {
        LaunchEntry {
            id,
            entry: app_input(name),
        }
    }

    fn command_entry(name: &str, id: i64) -> LaunchEntry {
        LaunchEntry {
            id,
            entry: command_input(name),
        }
    }

    fn desktop_app_entry(name: &str, id: i64, guid: &str) -> LaunchEntry {
        let mut input = app_input(name);
        input.desktop_id = Some(guid.into());
        LaunchEntry { id, entry: input }
    }

    /// A desktop-assigned app entry pointing at an explicit target — a real
    /// entry's versioned install path, as opposed to the script's generic
    /// `C:\Apps\<name>.exe` (ticket 48).
    fn desktop_app_entry_at(name: &str, id: i64, target: &str, guid: &str) -> LaunchEntry {
        LaunchEntry {
            id,
            entry: LaunchEntryInput {
                name: name.into(),
                kind: LaunchEntryKind::App,
                target: target.into(),
                shell: None,
                show_window: false,
                desktop_id: Some(guid.into()),
            },
        }
    }

    #[derive(Debug, Clone)]
    enum Event {
        Spawned(String),
        Freed,
        /// A desktop move (tickets 44 & 48): the window handle the queue
        /// waited on — the NEW window, never one the user already had open —
        /// and the desktop id. Recorded in the same log as spawns and frees,
        /// so tests can assert the move happened after the window appeared
        /// and moved the window the queue waited for.
        Moved(usize, String),
    }

    /// One scripted window of a target's image: its handle — the key the
    /// snapshot and the move log use — and the desktop answers the skip
    /// rule is decided from (ticket 48). Mirrors what the real engine's
    /// per-window desktop queries report.
    #[derive(Debug, Clone)]
    struct FakeWindow {
        hwnd: usize,
        desktop: Option<String>,
        on_current_desktop: bool,
    }

    /// The image key the fake matches targets against — the lowercase
    /// basename of the target path, mirroring the real engine's image
    /// matching (ticket 48): a versioned install path and the running
    /// instance's unversioned path are the same app.
    fn image_key(target: &str) -> String {
        Path::new(target)
            .file_name()
            .map(|name| name.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    }

    /// The fake launcher: behavior is keyed by entry target, so tests script
    /// open windows (which desktop each is on), missing targets, spawn
    /// failures, and window timing before the run, and read back the call
    /// log (spawns, slot frees, desktop moves) after. The window model
    /// mirrors the real engine (ticket 48): a launch opens a NEW window — a
    /// fresh handle the pre-launch snapshot never contained — and a
    /// successful move relocates that window in the table, so a second run's
    /// skip check sees it on the assigned desktop, exactly like the real
    /// machine. Ticket 47's handed-off spawn (success without a pid) and the
    /// move-refusal path stay modeled.
    struct FakeLauncher {
        /// The app's visible windows by image basename, each with which
        /// desktop it is on. Scripted before the run and mutated as
        /// launches open (and moves land) windows — behind a lock because
        /// the trait's `&self` methods register new windows and moves.
        windows: Mutex<HashMap<String, Vec<FakeWindow>>>,
        /// Targets that no longer exist on disk (ticket 48).
        missing: HashSet<String>,
        spawn_fail: HashSet<String>,
        windowless: HashSet<String>,
        window_delay: HashMap<String, Duration>,
        desktops: HashSet<String>,
        /// Targets whose launch the shell hands to an existing process:
        /// `spawn` succeeds with no pid (Explorer).
        handed_off: HashSet<String>,
        /// Desktop GUIDs whose move is refused (every retry failed).
        move_fail: HashSet<String>,
        state: Mutex<FakeState>,
    }

    struct FakeState {
        next_pid: u32,
        next_hwnd: usize,
        spawned: HashMap<u32, (String, Instant)>,
        handed: HashMap<String, Instant>,
        log: Vec<(Instant, Event)>,
    }

    impl FakeLauncher {
        fn new() -> Self {
            FakeLauncher {
                windows: Mutex::new(HashMap::new()),
                missing: HashSet::new(),
                spawn_fail: HashSet::new(),
                windowless: HashSet::new(),
                window_delay: HashMap::new(),
                desktops: HashSet::new(),
                handed_off: HashSet::new(),
                move_fail: HashSet::new(),
                state: Mutex::new(FakeState {
                    next_pid: 1,
                    next_hwnd: 1,
                    spawned: HashMap::new(),
                    handed: HashMap::new(),
                    log: Vec::new(),
                }),
            }
        }

        /// A pre-existing window of the target's image, with which desktop
        /// it is on (ticket 48): `desktop: None` + `current: true` = on the
        /// current desktop, the same values the real per-window queries
        /// report.
        fn window(mut self, target: &str, hwnd: usize, desktop: Option<&str>, current: bool) -> Self {
            self.windows
                .get_mut()
                .unwrap()
                .entry(image_key(target))
                .or_default()
                .push(FakeWindow {
                    hwnd,
                    desktop: desktop.map(str::to_string),
                    on_current_desktop: current,
                });
            self
        }

        /// The target no longer exists on disk (ticket 48) — an app that
        /// updated its version folder.
        fn missing(mut self, target: &str) -> Self {
            self.missing.insert(target.into());
            self
        }

        fn failing(mut self, target: &str) -> Self {
            self.spawn_fail.insert(target.into());
            self
        }

        fn windowless(mut self, target: &str) -> Self {
            self.windowless.insert(target.into());
            self
        }

        fn window_after(mut self, target: &str, delay: Duration) -> Self {
            self.window_delay.insert(target.into(), delay);
            self
        }

        /// Ticket 47: the target launches through an already-running process
        /// — success with no pid of its own.
        fn handed_off(mut self, target: &str) -> Self {
            self.handed_off.insert(target.into());
            self
        }

        /// Ticket 47: moves to this desktop are refused.
        fn move_failing(mut self, guid: &str) -> Self {
            self.move_fail.insert(guid.into());
            self
        }

        /// The virtual desktops the engine knows (ticket 44): a desktop id
        /// outside this set is a stale assignment.
        fn desktops(mut self, ids: &[&str]) -> Self {
            self.desktops.extend(ids.iter().map(|id| id.to_string()));
            self
        }

        fn spawned_targets(&self) -> Vec<String> {
            self.state
                .lock()
                .unwrap()
                .log
                .iter()
                .filter_map(|(_, e)| match e {
                    Event::Spawned(name) => Some(name.clone()),
                    Event::Freed | Event::Moved(..) => None,
                })
                .collect()
        }

        fn frees(&self) -> usize {
            self.state
                .lock()
                .unwrap()
                .log
                .iter()
                .filter(|(_, e)| matches!(e, Event::Freed))
                .count()
        }

        /// The desktop moves, as (moved window handle, desktop id) — the
        /// handle is the NEW window the queue waited on (ticket 48).
        fn moved(&self) -> Vec<(usize, String)> {
            self.state
                .lock()
                .unwrap()
                .log
                .iter()
                .filter_map(|(_, e)| match e {
                    Event::Moved(hwnd, guid) => Some((*hwnd, guid.clone())),
                    _ => None,
                })
                .collect()
        }

        /// The current window table of a target's image — what a second
        /// run's skip check would see (ticket 48).
        fn windows_of(&self, target: &str) -> Vec<FakeWindow> {
            self.windows
                .lock()
                .unwrap()
                .get(&image_key(target))
                .cloned()
                .unwrap_or_default()
        }

        /// The launched instance's window: a fresh handle registered as open
        /// on the current desktop — the fake's model of the real engine's
        /// new window (ticket 48). Registered in the window table so a
        /// second run's skip check sees it, exactly like a real launch.
        fn open_window(&self, target: &str) -> usize {
            let mut state = self.state.lock().unwrap();
            let hwnd = state.next_hwnd;
            state.next_hwnd += 1;
            self.windows
                .lock()
                .unwrap()
                .entry(image_key(target))
                .or_default()
                .push(FakeWindow {
                    hwnd,
                    desktop: None,
                    on_current_desktop: true,
                });
            state
                .log
                .push((Instant::now(), Event::Freed));
            hwnd
        }

        /// The peak count of in-flight entries, derived from the spawn/free
        /// log — the direct observable of "the cap was honored".
        fn max_concurrency(&self) -> usize {
            let mut active: usize = 0;
            let mut max = 0;
            for (_, e) in &self.state.lock().unwrap().log {
                match e {
                    Event::Spawned(_) => {
                        active += 1;
                        max = max.max(active);
                    }
                    Event::Freed => active = active.saturating_sub(1),
                    Event::Moved(..) => {}
                }
            }
            max
        }
    }

    impl LauncherEngine for FakeLauncher {
        fn spawn(&self, entry: &LaunchEntryInput) -> Result<Spawned, String> {
            if self.spawn_fail.contains(&entry.target) {
                return Err("boom".into());
            }
            let mut state = self.state.lock().unwrap();
            let pid = state.next_pid;
            state.next_pid += 1;
            if self.handed_off.contains(&entry.target) {
                // The shell took the launch over: no pid of our own, exactly
                // like the real engine's handed-off ShellExecuteExW.
                state.handed.insert(entry.target.clone(), Instant::now());
            } else {
                state
                    .spawned
                    .insert(pid, (entry.target.clone(), Instant::now()));
            }
            state
                .log
                .push((Instant::now(), Event::Spawned(entry.name.clone())));
            Ok(Spawned {
                pid: if self.handed_off.contains(&entry.target) {
                    None
                } else {
                    Some(pid)
                },
                target: entry.target.clone(),
            })
        }

        fn app_windows(&self, target: &str) -> Vec<crate::engine::AppWindow> {
            self.windows
                .lock()
                .unwrap()
                .get(&image_key(target))
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|window| crate::engine::AppWindow {
                    hwnd: window.hwnd,
                    desktop: window.desktop,
                    on_current_desktop: window.on_current_desktop,
                })
                .collect()
        }

        fn target_exists(&self, target: &str) -> bool {
            !self.missing.contains(target)
        }

        fn wait_for_new_window(
            &self,
            spawned: &Spawned,
            _before: &[usize],
            timeout: Duration,
        ) -> Option<usize> {
            // The launched instance's window: a fresh handle the pre-launch
            // snapshot never contained, appearing `window_delay` after the
            // launch (handed-off and child-owned windows land here exactly
            // like the real engine's new-window resolution).
            let (target, spawned_at) = {
                let state = self.state.lock().unwrap();
                match spawned.pid {
                    Some(pid) => state
                        .spawned
                        .get(&pid)
                        .cloned()
                        .unwrap_or_else(|| (spawned.target.clone(), Instant::now())),
                    None => state
                        .handed
                        .get(&spawned.target)
                        .copied()
                        .map(|spawned_at| (spawned.target.clone(), spawned_at))
                        .unwrap_or_else(|| (spawned.target.clone(), Instant::now())),
                }
            };
            if let Some(delay) = self.window_delay.get(&target).copied() {
                match (spawned_at + delay).checked_duration_since(Instant::now()) {
                    Some(remaining) if remaining <= timeout => {
                        std::thread::sleep(remaining);
                        Some(self.open_window(&target))
                    }
                    Some(_) => {
                        std::thread::sleep(timeout);
                        None
                    }
                    None => Some(self.open_window(&target)),
                }
            } else if self.windowless.contains(&target) {
                std::thread::sleep(timeout);
                None
            } else {
                Some(self.open_window(&target))
            }
        }

        fn move_window_to_desktop(&self, hwnd: usize, guid: &str) -> Result<(), String> {
            if self.move_fail.contains(guid) {
                return Err("move refused".into());
            }
            let mut state = self.state.lock().unwrap();
            state
                .log
                .push((Instant::now(), Event::Moved(hwnd, guid.into())));
            // The moved window now lives on that desktop — a second run's
            // skip check sees it there (the fake's model of the real move).
            for windows in self.windows.lock().unwrap().values_mut() {
                if let Some(window) = windows.iter_mut().find(|window| window.hwnd == hwnd) {
                    window.desktop = Some(guid.into());
                    window.on_current_desktop = false;
                }
            }
            Ok(())
        }

        fn desktops(&self) -> Vec<crate::engine::DesktopInfo> {
            self.desktops
                .iter()
                .map(|id| crate::engine::DesktopInfo {
                    id: id.clone(),
                    name: format!("Desktop for {id}"),
                })
                .collect()
        }
    }

    const SHORT_WINDOW: Duration = Duration::from_millis(100);

    #[test]
    fn cap_is_honored_and_the_queue_drains() {
        let engine = FakeLauncher::new()
            .window_after(r"C:\Apps\A.exe", Duration::from_millis(10))
            .window_after(r"C:\Apps\B.exe", Duration::from_millis(10))
            .window_after(r"C:\Apps\C.exe", Duration::from_millis(10))
            .window_after(r"C:\Apps\D.exe", Duration::from_millis(10))
            .window_after(r"C:\Apps\E.exe", Duration::from_millis(10));
        let entries = vec![
            app_entry("A", 1),
            app_entry("B", 2),
            app_entry("C", 3),
            app_entry("D", 4),
            app_entry("E", 5),
        ];
        let report = run_launch_queue_until(&engine, &entries, 2, SHORT_WINDOW);
        assert_eq!(report.started, vec!["A", "B", "C", "D", "E"]);
        assert!(report.skipped.is_empty());
        assert!(report.failed.is_empty());
        // The cap is the observable peak of concurrent in-flight entries.
        assert_eq!(engine.max_concurrency(), 2);
        // Every slot freed — the final wave drained too.
        assert_eq!(engine.frees(), 5);
    }

    #[test]
    fn command_entries_free_their_slot_at_spawn() {
        let engine = FakeLauncher::new();
        let entries = vec![
            command_entry("X", 1),
            command_entry("Y", 2),
            command_entry("Z", 3),
        ];
        let report = run_launch_queue_until(&engine, &entries, 1, SHORT_WINDOW);
        assert_eq!(report.started, vec!["X", "Y", "Z"]);
        // Commands never hold a slot: everything spawned before any window
        // wait, so the engine never freed a slot.
        assert_eq!(engine.frees(), 0);
    }

    #[test]
    fn window_timeout_counts_as_started_and_the_queue_never_stalls() {
        let engine = FakeLauncher::new()
            .windowless(r"C:\Apps\A.exe")
            .windowless(r"C:\Apps\B.exe")
            .windowless(r"C:\Apps\C.exe");
        let entries = vec![app_entry("A", 1), app_entry("B", 2), app_entry("C", 3)];
        let report = run_launch_queue_until(&engine, &entries, 2, SHORT_WINDOW);
        // Windowless apps still count as started — a timeout is not a
        // failure and must not stall the queue behind them.
        assert_eq!(report.started, vec!["A", "B", "C"]);
        assert!(report.failed.is_empty());
    }

    #[test]
    fn skip_is_per_desktop_with_a_reason_and_never_holds_a_slot() {
        // A has a window on the current desktop — an unassigned entry skips
        // only then, and the skip carries the reason.
        let engine = FakeLauncher::new().window(r"C:\Apps\A.exe", 100, None, true);
        let entries = vec![app_entry("A", 1), app_entry("B", 2), app_entry("C", 3)];
        let report = run_launch_queue_until(&engine, &entries, 1, SHORT_WINDOW);
        assert_eq!(report.started, vec!["B", "C"]);
        assert_eq!(
            report.skipped,
            vec!["A — already open on this desktop".to_string()]
        );
        assert!(report.failed.is_empty());
        assert_eq!(engine.spawned_targets(), vec!["B", "C"]);
        // The skipped entry never held the single slot: B started directly.
        assert_eq!(engine.max_concurrency(), 1);
        // The reason rides into the summary text (notification + page
        // event), so the no-op is never silent.
        assert!(launch_summary_body(&report).contains("A — already open on this desktop"));
    }

    #[test]
    fn a_failure_never_aborts_the_rest() {
        let engine = FakeLauncher::new()
            .failing(r"C:\Apps\A.exe")
            .failing(r"C:\Apps\C.exe");
        let entries = vec![app_entry("A", 1), app_entry("B", 2), app_entry("C", 3), app_entry("D", 4)];
        let report = run_launch_queue_until(&engine, &entries, 1, SHORT_WINDOW);
        assert_eq!(report.started, vec!["B", "D"]);
        assert_eq!(report.failed, vec!["A", "C"]);
        assert!(report.skipped.is_empty());
        // A failed spawn never occupies a slot: B followed A immediately,
        // and D followed the failed C.
        assert_eq!(engine.spawned_targets(), vec!["B", "D"]);
    }

    #[test]
    fn desktop_assignments_move_the_window_after_spawn() {
        let guid_a = "550fe0a1-3d41-4e5f-9a2b-c8d0e1f2a3b4";
        let guid_c = "c3d4e5f6-1111-2222-3333-444455556666";
        let engine = FakeLauncher::new().desktops(&[guid_a, guid_c]);
        let entries = vec![
            desktop_app_entry("A", 1, guid_a),
            command_entry("B", 2),
            desktop_app_entry("C", 3, guid_c),
        ];
        let report = run_launch_queue_until(&engine, &entries, 2, SHORT_WINDOW);
        assert_eq!(report.started.len(), 3);
        assert!(report.notes.is_empty(), "known desktops never note");
        // The moves carry the windows the queue waited for — fresh handles
        // (1 and 2) the pre-launch snapshots never contained (ticket 48).
        assert_eq!(
            engine.moved(),
            vec![
                (1, guid_a.to_string()),
                (2, guid_c.to_string()),
            ]
        );
    }

    #[test]
    fn desktop_move_happens_after_the_window_appears() {
        let guid = "550fe0a1-3d41-4e5f-9a2b-c8d0e1f2a3b4";
        let engine = FakeLauncher::new()
            .desktops(&[guid])
            .window_after(r"C:\Apps\A.exe", Duration::from_millis(30));
        let entries = vec![desktop_app_entry("A", 1, guid)];
        let report = run_launch_queue_until(&engine, &entries, 1, SHORT_WINDOW);
        assert_eq!(report.started, vec!["A"]);
        // The move is ordered after the window appeared — never at spawn.
        let events: Vec<Event> = engine
            .state
            .lock()
            .unwrap()
            .log
            .iter()
            .map(|(_, e)| e.clone())
            .collect();
        let spawned = events
            .iter()
            .position(|e| matches!(e, Event::Spawned(_)))
            .unwrap();
        let freed = events
            .iter()
            .position(|e| matches!(e, Event::Freed))
            .unwrap();
        let moved = events
            .iter()
            .position(|e| matches!(e, Event::Moved(..)))
            .unwrap();
        assert!(
            spawned < freed && freed < moved,
            "move must follow the window appearing, got {events:?}"
        );
        assert_eq!(engine.moved(), vec![(1, guid.to_string())]);
    }

    #[test]
    fn handed_off_launch_moves_its_new_window_not_an_old_one() {
        let guid = "550fe0a1-3d41-4e5f-9a2b-c8d0e1f2a3b4";
        // Explorer: an old folder window is already open on the current
        // desktop, and the launch is handed to the running shell — success
        // with no pid (ticket 47). The snapshot preference (ticket 48) must
        // move the NEW window the launch opened, never the old one.
        let engine = FakeLauncher::new()
            .desktops(&[guid])
            .window(r"C:\Apps\File Explorer.exe", 100, None, true)
            .handed_off(r"C:\Apps\File Explorer.exe");
        let entries = vec![desktop_app_entry("File Explorer", 1, guid)];
        let report = run_launch_queue_until(&engine, &entries, 1, SHORT_WINDOW);
        assert_eq!(report.started, vec!["File Explorer"]);
        assert!(report.failed.is_empty(), "no pid is not a failure");
        assert!(report.notes.is_empty(), "known desktops never note");
        // The moved window is the new one — a fresh handle the snapshot
        // never contained. The old window stays untouched on the current
        // desktop.
        assert_eq!(engine.moved(), vec![(1, guid.to_string())]);
        let old = engine.windows_of(r"C:\Apps\File Explorer.exe");
        assert_eq!(old[0].hwnd, 100, "the old window is untouched");
        assert!(old[0].desktop.is_none(), "the old window was not moved");
    }

    #[test]
    fn wrapper_launch_moves_the_window_that_appeared_not_an_old_one() {
        let guid = "550fe0a1-3d41-4e5f-9a2b-c8d0e1f2a3b4";
        // A wrapper launcher (Discord's updater, an installer shim): an old
        // window is already open on the current desktop, and the launched
        // instance shows its window only after a delay. The queue waits on
        // and moves the window that appeared — the new one, never the old.
        let engine = FakeLauncher::new()
            .desktops(&[guid])
            .window(r"C:\Apps\Discord.exe", 100, None, true)
            .window_after(r"C:\Apps\Discord.exe", Duration::from_millis(20));
        let entries = vec![desktop_app_entry("Discord", 1, guid)];
        let report = run_launch_queue_until(&engine, &entries, 1, SHORT_WINDOW);
        assert_eq!(report.started, vec!["Discord"]);
        assert!(report.failed.is_empty());
        assert!(report.notes.is_empty());
        assert_eq!(engine.moved(), vec![(1, guid.to_string())]);
        let old = engine.windows_of(r"C:\Apps\Discord.exe");
        assert_eq!(old[0].hwnd, 100, "the old window is untouched");
        assert!(old[0].desktop.is_none(), "the old window was not moved");
    }

    #[test]
    fn failed_desktop_move_adds_a_note_and_the_entry_stays_started() {
        let guid = "550fe0a1-3d41-4e5f-9a2b-c8d0e1f2a3b4";
        let engine = FakeLauncher::new()
            .desktops(&[guid])
            .move_failing(guid);
        let entries = vec![desktop_app_entry("A", 1, guid)];
        let report = run_launch_queue_until(&engine, &entries, 1, SHORT_WINDOW);
        // The launch itself succeeded — a refused move is never a failure.
        assert_eq!(report.started, vec!["A"]);
        assert!(report.failed.is_empty());
        assert!(engine.moved().is_empty(), "the move was refused");
        assert_eq!(report.notes.len(), 1);
        assert!(
            report.notes[0].contains("A") && report.notes[0].contains("could not move it"),
            "got: {}",
            report.notes[0]
        );
        // The note rides into the summary text (notification + page event).
        assert!(launch_summary_body(&report).contains("could not move it"));
    }

    #[test]
    fn window_timeout_with_a_pending_move_notes_the_unmoved_window() {
        let guid = "550fe0a1-3d41-4e5f-9a2b-c8d0e1f2a3b4";
        let engine = FakeLauncher::new()
            .desktops(&[guid])
            .windowless(r"C:\Apps\A.exe");
        let entries = vec![desktop_app_entry("A", 1, guid)];
        let report = run_launch_queue_until(&engine, &entries, 1, SHORT_WINDOW);
        // The 15 s timeout rule is preserved: the entry counts as started.
        assert_eq!(report.started, vec!["A"]);
        assert!(report.failed.is_empty());
        // The window never appeared — the assignment cannot be honored, and
        // the note says so instead of dropping the move silently.
        assert_eq!(report.notes.len(), 1);
        assert!(
            report.notes[0].contains("A") && report.notes[0].contains("could not move it"),
            "got: {}",
            report.notes[0]
        );
    }

    #[test]
    fn stale_desktop_guid_falls_back_to_the_current_desktop_with_a_note() {
        let engine = FakeLauncher::new(); // no desktops at all
        let entries = vec![desktop_app_entry(
            "A",
            1,
            "550fe0a1-3d41-4e5f-9a2b-c8d0e1f2a3b4",
        )];
        let report = run_launch_queue_until(&engine, &entries, 1, SHORT_WINDOW);
        // The entry still launches — on the current desktop.
        assert_eq!(report.started, vec!["A"]);
        assert!(engine.moved().is_empty(), "nothing to move");
        assert_eq!(report.notes.len(), 1);
        assert!(
            report.notes[0].contains("A") && report.notes[0].contains("current desktop"),
            "got: {}",
            report.notes[0]
        );
        // The note rides into the summary text.
        assert!(launch_summary_body(&report).contains("current desktop"));
    }

    #[test]
    fn empty_list_and_zero_cap_are_safe() {
        let engine = FakeLauncher::new();
        let report = run_launch_queue_until(&engine, &[], 2, SHORT_WINDOW);
        assert_eq!(report, LaunchReport::default());

        let entries = vec![app_entry("A", 1)];
        let report = run_launch_queue_until(&engine, &entries, 0, SHORT_WINDOW);
        // A zero cap is clamped to 1 — one entry still launches.
        assert_eq!(report.started, vec!["A"]);
    }

    #[test]
    fn summary_text_counts_and_names_the_failed_and_skipped() {
        let report = LaunchReport {
            started: vec!["B".into(), "D".into()],
            skipped: vec!["A — already open on this desktop".into()],
            failed: vec!["C".into(), "E".into()],
            notes: vec!["F opened on the current desktop — its desktop no longer exists".into()],
        };
        assert_eq!(
            launch_summary_body(&report),
            "started 2, skipped 1, failed 2 — skipped: A — already open on this desktop — failed: C, E — F opened on the current desktop — its desktop no longer exists"
        );
        let clean = LaunchReport {
            started: vec!["B".into()],
            ..LaunchReport::default()
        };
        assert_eq!(launch_summary_body(&clean), "started 1, skipped 0, failed 0");
    }

    // ------------------- ticket 48: the new decision logic ---------------

    #[test]
    fn skip_is_per_desktop_not_per_process() {
        let guid_one = "550fe0a1-3d41-4e5f-9a2b-c8d0e1f2a3b4";
        let guid_two = "c3d4e5f6-1111-2222-3333-444455556666";
        // A is assigned to desktop 1 but its window sits on desktop 2 —
        // launch, and move the new window to desktop 1. B is unassigned
        // with a window on desktop 2 — launch on the current desktop, never
        // skipped over a window elsewhere. C is assigned to desktop 1 and
        // already has a window there — skipped. D is unassigned with a
        // window on the current desktop — skipped.
        let engine = FakeLauncher::new()
            .desktops(&[guid_one, guid_two])
            .window(r"C:\Apps\A.exe", 100, Some(guid_two), false)
            .window(r"C:\Apps\B.exe", 200, Some(guid_two), false)
            .window(r"C:\Apps\C.exe", 300, Some(guid_one), false)
            .window(r"C:\Apps\D.exe", 400, None, true);
        let entries = vec![
            desktop_app_entry("A", 1, guid_one),
            app_entry("B", 2),
            desktop_app_entry("C", 3, guid_one),
            app_entry("D", 4),
        ];
        let report = run_launch_queue_until(&engine, &entries, 2, SHORT_WINDOW);
        assert_eq!(report.started, vec!["A", "B"]);
        assert_eq!(
            report.skipped,
            vec![
                "C — already open on this desktop".to_string(),
                "D — already open on this desktop".to_string()
            ]
        );
        assert!(report.failed.is_empty());
        // Only A's new window is moved — B (unassigned) is never moved, and
        // windows on other desktops are never disturbed.
        assert_eq!(engine.moved(), vec![(1, guid_one.to_string())]);
        // The scripted windows are all still where they were.
        assert_eq!(engine.windows_of(r"C:\Apps\B.exe")[0].hwnd, 200);
        assert_eq!(engine.windows_of(r"C:\Apps\C.exe")[0].hwnd, 300);
        assert_eq!(engine.windows_of(r"C:\Apps\D.exe")[0].hwnd, 400);
    }

    #[test]
    fn versioned_and_unversioned_targets_are_the_same_app() {
        let guid = "550fe0a1-3d41-4e5f-9a2b-c8d0e1f2a3b4";
        // Edge's running instance image is the unversioned path; the entry
        // holds the versioned install directory (ticket 48). The basename
        // match — the fake mirrors the real engine's image key — makes them
        // the same app: the skip rule sees the window on the assigned
        // desktop either way.
        let versioned =
            r"C:\Program Files (x86)\Microsoft\Edge\Application\151.0.4129.86\msedge.exe";
        let unversioned = r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe";
        let engine = FakeLauncher::new()
            .desktops(&[guid])
            .window(unversioned, 100, Some(guid), false);
        let entries = vec![desktop_app_entry_at("Edge", 1, versioned, guid)];
        let report = run_launch_queue_until(&engine, &entries, 1, SHORT_WINDOW);
        assert_eq!(
            report.skipped,
            vec!["Edge — already open on this desktop".to_string()]
        );
        assert!(report.started.is_empty());
        // The same versioned entry with no window anywhere launches and its
        // new window is moved to the assigned desktop.
        let engine = FakeLauncher::new().desktops(&[guid]);
        let report = run_launch_queue_until(&engine, &entries, 1, SHORT_WINDOW);
        assert_eq!(report.started, vec!["Edge"]);
        assert!(report.skipped.is_empty());
        assert_eq!(engine.moved(), vec![(1, guid.to_string())]);
    }

    #[test]
    fn missing_target_fails_fast_with_a_clear_message() {
        // The app updated its version folder and the stored target is gone
        // (ticket 48): fail fast with the reason — no silent 15 s stall —
        // and the rest of the list still runs.
        let engine = FakeLauncher::new().missing(r"C:\Apps\A.exe");
        let entries = vec![app_entry("A", 1), app_entry("B", 2)];
        let report = run_launch_queue_until(&engine, &entries, 1, SHORT_WINDOW);
        assert_eq!(
            report.failed,
            vec!["A — target no longer exists — update this entry".to_string()]
        );
        assert_eq!(report.started, vec!["B"], "the rest of the list still runs");
        assert_eq!(engine.spawned_targets(), vec!["B"], "A was never spawned");
        // The reason rides into the summary text.
        assert!(launch_summary_body(&report).contains("target no longer exists"));
    }

    #[test]
    fn an_open_app_skips_even_when_its_stored_target_is_gone() {
        let guid = "550fe0a1-3d41-4e5f-9a2b-c8d0e1f2a3b4";
        let versioned =
            r"C:\Program Files (x86)\Microsoft\Edge\Application\151.0.4129.86\msedge.exe";
        // Edge is running on the assigned desktop after an update removed
        // the versioned folder: the skip must win — the app IS open there —
        // and the stale target must not turn the no-op into a failure.
        let engine = FakeLauncher::new()
            .desktops(&[guid])
            .window(
                r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
                100,
                Some(guid),
                false,
            )
            .missing(versioned);
        let entries = vec![desktop_app_entry_at("Edge", 1, versioned, guid)];
        let report = run_launch_queue_until(&engine, &entries, 1, SHORT_WINDOW);
        assert_eq!(
            report.skipped,
            vec!["Edge — already open on this desktop".to_string()]
        );
        assert!(report.failed.is_empty());
        assert!(report.started.is_empty());
    }

    #[test]
    fn a_second_run_skips_the_window_it_just_moved() {
        let guid = "550fe0a1-3d41-4e5f-9a2b-c8d0e1f2a3b4";
        // First Start: Edge has no window on the assigned desktop → launch,
        // and the new window is moved there. Second Start: the same entry
        // sees that window on the assigned desktop → skipped with the
        // reason, nothing disturbed — true idempotency.
        let engine = FakeLauncher::new().desktops(&[guid]);
        let entries = vec![desktop_app_entry("Edge", 1, guid)];
        let first = run_launch_queue_until(&engine, &entries, 1, SHORT_WINDOW);
        assert_eq!(first.started, vec!["Edge"]);
        assert!(first.skipped.is_empty());
        assert!(first.failed.is_empty());

        let second = run_launch_queue_until(&engine, &entries, 1, SHORT_WINDOW);
        assert!(second.started.is_empty());
        assert_eq!(
            second.skipped,
            vec!["Edge — already open on this desktop".to_string()]
        );
        assert!(second.failed.is_empty());

        // Exactly one launch happened, and exactly one move — the second
        // run spawned and moved nothing.
        assert_eq!(engine.spawned_targets(), vec!["Edge"]);
        assert_eq!(engine.moved(), vec![(1, guid.to_string())]);
    }
}

// ---------------------------------------------------------------------------
// The live device probe (ticket 48, AC 6)
// ---------------------------------------------------------------------------

/// The live device probe: drives the REAL [`WindowsLauncherEngine`] through
/// the real queue against live Edge windows and real virtual desktops.
/// Ignored by default — it launches and kills Edge on the machine and moves
/// real windows. Run it explicitly on the dev machine with:
///
/// ```text
/// cargo test --lib live_launch_probe -- --ignored --nocapture
/// ```
///
/// It proves the three states the ticket demands: Edge closed → a fresh
/// window lands on Desktop 2; Edge running on the current desktop → a NEW
/// window lands on Desktop 2 while the old one stays put; Edge already on
/// Desktop 2 → skipped, nothing disturbed. All Edge processes are killed at
/// the start and between states.
#[cfg(test)]
mod live_probe {
    use super::*;
    use crate::engine::windows::WindowsLauncherEngine;
    use std::os::windows::process::CommandExt;
    use std::path::Path;

    #[test]
    #[ignore = "device probe: launches and kills real Edge windows, needs 2+ desktops — run explicitly"]
    fn live_launch_probe_edge_on_desktop_two() {
        let engine = WindowsLauncherEngine;
        let desktops = engine.desktops();
        assert!(
            desktops.len() >= 2,
            "the probe needs at least two virtual desktops, got {} ({desktops:?})",
            desktops.len()
        );
        let desktop_two = &desktops[1];
        let edge = find_edge_exe().expect("the probe needs Microsoft Edge installed");
        let entry = LaunchEntry {
            id: 1,
            entry: LaunchEntryInput {
                name: "Edge".into(),
                kind: LaunchEntryKind::App,
                target: edge.clone(),
                shell: None,
                show_window: false,
                desktop_id: Some(desktop_two.id.clone()),
            },
        };

        // State A: Edge closed → a fresh window lands on Desktop 2.
        kill_edge();
        wait_for_no_edge_windows(&engine, &edge, Duration::from_secs(10));
        let report = run_launch_queue(&engine, &[entry.clone()], 1);
        assert!(report.started.iter().any(|name| name == "Edge"), "{report:?}");
        assert!(report.skipped.is_empty(), "{report:?}");
        assert!(report.failed.is_empty(), "{report:?}");
        assert!(report.notes.is_empty(), "{report:?}");
        assert!(
            settle(&engine, &edge, &desktop_two.id, 1, 0),
            "state A: a fresh Edge window should sit on Desktop 2, got {:?}",
            engine.app_windows(&edge)
        );
        kill_edge();

        // State B: Edge running on the current desktop → the launch opens a
        // NEW window and places it on Desktop 2; the old one stays put.
        let _ = seed_edge(&engine, &entry.entry, Duration::from_secs(20));
        assert!(
            settle(&engine, &edge, &desktop_two.id, 0, 1),
            "Edge must be open on the current desktop first, got {:?}",
            engine.app_windows(&edge)
        );
        let report = run_launch_queue(&engine, &[entry.clone()], 1);
        assert!(report.started.iter().any(|name| name == "Edge"), "{report:?}");
        assert!(report.skipped.is_empty(), "{report:?}");
        assert!(report.failed.is_empty(), "{report:?}");
        assert!(report.notes.is_empty(), "{report:?}");
        assert!(
            settle(&engine, &edge, &desktop_two.id, 1, 1),
            "state B: one Edge window on Desktop 2 and the old one still on the current desktop, got {:?}",
            engine.app_windows(&edge)
        );
        kill_edge();

        // State C: Edge already on Desktop 2 → skipped, nothing disturbed.
        let hwnd = seed_edge(&engine, &entry.entry, Duration::from_secs(20));
        engine
            .move_window_to_desktop(hwnd, &desktop_two.id)
            .expect("the Edge window moves to Desktop 2");
        assert!(
            settle(&engine, &edge, &desktop_two.id, 1, 0),
            "state C: Edge should be settled on Desktop 2, got {:?}",
            engine.app_windows(&edge)
        );
        let report = run_launch_queue(&engine, &[entry.clone()], 1);
        assert_eq!(
            report.skipped,
            vec!["Edge — already open on this desktop".to_string()],
            "{report:?}"
        );
        assert!(report.started.is_empty(), "{report:?}");
        assert!(report.failed.is_empty(), "{report:?}");
        assert!(report.notes.is_empty(), "{report:?}");
        let windows = engine.app_windows(&edge);
        assert_eq!(windows.len(), 1, "nothing disturbed: {windows:?}");
        assert_eq!(
            windows[0].desktop.as_deref(),
            Some(desktop_two.id.as_str()),
            "the window is still on Desktop 2: {windows:?}"
        );
        kill_edge();
    }

    /// The Edge executable — the versioned install directory when one
    /// exists, so the probe proves the versioned path matches the running
    /// instance's unversioned image (ticket 48).
    fn find_edge_exe() -> Option<String> {
        for base in [
            r"C:\Program Files (x86)\Microsoft\Edge\Application",
            r"C:\Program Files\Microsoft\Edge\Application",
        ] {
            let unversioned = Path::new(base).join("msedge.exe");
            if unversioned.exists() {
                for entry in std::fs::read_dir(base).ok()?.flatten() {
                    let candidate = entry.path().join("msedge.exe");
                    if entry.path().is_dir() && candidate.exists() {
                        return Some(candidate.to_string_lossy().into_owned());
                    }
                }
                return Some(unversioned.to_string_lossy().into_owned());
            }
        }
        None
    }

    /// Kills every Edge process — the probe's own state cleanup; a failure
    /// just means Edge was already closed.
    fn kill_edge() {
        let _ = std::process::Command::new("taskkill")
            .args(["/IM", "msedge.exe", "/F"])
            .creation_flags(0x0800_0000)
            .output();
    }

    /// Launches Edge and waits for its window, retrying across the brief
    /// single-instance mutex linger that survives a kill — a handed-off
    /// spawn right after `kill_edge` can die without ever showing a window,
    /// so the seed retries (kill, pause, spawn) until a window appears.
    fn seed_edge(
        engine: &WindowsLauncherEngine,
        entry: &LaunchEntryInput,
        window_wait: Duration,
    ) -> usize {
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let spawned = engine.spawn(entry).expect("Edge launches");
            if let Some(hwnd) = engine.wait_for_new_window(&spawned, &[], window_wait) {
                return hwnd;
            }
            kill_edge();
            std::thread::sleep(Duration::from_secs(2));
            assert!(
                Instant::now() < deadline,
                "Edge never showed a window while seeding"
            );
        }
    }

    /// Waits until the app has no visible window — Edge is fully closed.
    /// Gives up quietly at the deadline; the state asserts catch a still
    /// running Edge.
    fn wait_for_no_edge_windows(engine: &WindowsLauncherEngine, edge: &str, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        loop {
            if engine.app_windows(edge).is_empty() {
                return;
            }
            if Instant::now() >= deadline {
                return;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    /// Polls until the window picture settles: at least `on_two` Edge
    /// windows on the assigned desktop and `on_current` on the current one
    /// (ticket 48). Edge swaps a startup window for the real one, so the
    /// settled picture is the truth.
    fn settle(
        engine: &WindowsLauncherEngine,
        edge: &str,
        guid: &str,
        on_two: usize,
        on_current: usize,
    ) -> bool {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let windows = engine.app_windows(edge);
            let count_two = windows
                .iter()
                .filter(|w| w.desktop.as_deref() == Some(guid))
                .count();
            let count_current = windows.iter().filter(|w| w.on_current_desktop).count();
            if count_two >= on_two && count_current >= on_current {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }
}