//! Log locations and retention (ticket 09).
//!
//! Raw outputs live under %LOCALAPPDATA%\Sprout\logs (ADR-0006): one folder
//! per run (`logs\runs\<run-id>`, see [`crate::worker::run_dir`]), one folder
//! per Quick Action run (`logs\quick-actions\qa-…`, ticket 64), one per
//! Quick Launch list-run (`logs\quick-launch\ql-<millis>`, ticket 77), plus
//! the database itself in the data root. The Logs screen never renders
//! log content — it shows where the files live, how big they are, and opens
//! the folder on request. `prune_run_logs` is the retention knob: expired
//! folders are deleted per the settings' `log_retention_days` after every
//! completed run and at app start. History rows are kept forever; only raw
//! log files expire.

use std::path::Path;

use rusqlite::Connection;
use serde::Serialize;

use crate::settings;

/// One browsable log location: a run folder or the data root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LogEntry {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    /// Last modified epoch seconds, when the entry is a folder that exists.
    pub modified_at: Option<i64>,
}

/// Everything the Logs screen shows: the data root and logs root with their
/// sizes, the database file, one entry per run folder, and — since tickets
/// 64 & 77 — one entry per Quick Action / Quick Launch run folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LogLocations {
    pub data_dir: String,
    pub logs_dir: String,
    pub db_path: String,
    pub db_size_bytes: u64,
    /// Total bytes across every run, Quick Action, and Quick Launch folder;
    /// 0 when there are none yet.
    pub total_logs_bytes: u64,
    /// One entry per run folder, newest first.
    pub runs: Vec<LogEntry>,
    /// One entry per Quick Action run folder, newest first (ticket 64).
    pub quick_action_runs: Vec<LogEntry>,
    /// One entry per Quick Launch run folder, newest first (ticket 77).
    pub quick_launch_runs: Vec<LogEntry>,
}

/// Collects the on-disk log picture under the real app data directory. Never
/// fails on a missing folder — nothing exists before the first run
/// (ADR-0006), so missing paths simply report zero-sized entries.
pub fn list_log_locations() -> LogLocations {
    list_log_locations_at(
        &crate::db::data_dir(),
        &crate::db::logs_dir(),
        &crate::db::db_path(),
    )
}

/// The same as [`list_log_locations`], against explicit paths (tests use
/// temporary directories).
pub fn list_log_locations_at(data_dir: &Path, logs_dir: &Path, db_path: &Path) -> LogLocations {
    let mut runs = list_run_dirs(&logs_dir.join("runs"));
    runs.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
    let mut quick_action_runs =
        list_run_dirs(&logs_dir.join(crate::quick_actions::QA_LOGS_DIR_NAME));
    quick_action_runs.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
    let mut quick_launch_runs = list_run_dirs(&logs_dir.join(crate::launch::QL_LOGS_DIR_NAME));
    quick_launch_runs.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
    let total_logs_bytes = runs
        .iter()
        .chain(&quick_action_runs)
        .chain(&quick_launch_runs)
        .map(|e| e.size_bytes)
        .sum();

    LogLocations {
        data_dir: data_dir.to_string_lossy().into_owned(),
        logs_dir: logs_dir.to_string_lossy().into_owned(),
        db_path: db_path.to_string_lossy().into_owned(),
        db_size_bytes: file_size(db_path),
        total_logs_bytes,
        runs,
        quick_action_runs,
        quick_launch_runs,
    }
}

/// Opens `path` in Explorer — the Logs screen's open-folder action. The
/// folder must exist; anything else is an error surfaced to the UI.
pub fn open_folder(path: &str) -> Result<(), String> {
    let target = Path::new(path);
    if !target.is_dir() {
        return Err(format!("'{path}' is not a folder — it may have been pruned"));
    }
    std::process::Command::new("explorer.exe")
        .arg(target)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("cannot open '{path}': {e}"))
}

/// Deletes run log folders older than the settings' retention window under
/// the real app data directory. Returns how many folders were removed.
/// Covers the preset-run folders (`logs\runs`), the Quick Action run
/// folders (`logs\quick-actions`, ticket 64), and the Quick Launch run
/// folders (`logs\quick-launch`, ticket 77). Runs honor this after every
/// completed run (worker) and at app start (main process).
pub fn prune_run_logs(conn: &Connection) -> Result<usize, String> {
    prune_run_logs_at(conn, &crate::db::logs_dir())
}

/// The same as [`prune_run_logs`], against an explicit logs directory.
pub fn prune_run_logs_at(conn: &Connection, logs_dir: &Path) -> Result<usize, String> {
    let retention_days = settings::load(conn).log_retention_days;
    let cutoff_secs = crate::db::now_ts().saturating_sub(i64::from(retention_days) * 86_400);

    let mut pruned = prune_expired_dirs(&logs_dir.join("runs"), cutoff_secs);
    pruned += prune_expired_dirs(&logs_dir.join(crate::quick_actions::QA_LOGS_DIR_NAME), cutoff_secs);
    pruned += prune_expired_dirs(&logs_dir.join(crate::launch::QL_LOGS_DIR_NAME), cutoff_secs);
    Ok(pruned)
}

/// Removes the expired per-run folders under `root` — both folder families
/// use the same shape (a directory whose name embeds its creation millis).
/// A missing root is a no-op; a folder in use is left for the next pass.
fn prune_expired_dirs(root: &Path, cutoff_secs: i64) -> usize {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return 0, // nothing exists yet — nothing to prune
    };
    let mut pruned = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let modified_at = folder_age(&path).unwrap_or(i64::MIN);
        if modified_at < cutoff_secs {
            if std::fs::remove_dir_all(&path).is_ok() {
                pruned += 1;
            }
        }
    }
    pruned
}

/// The per-run folders under `root` (`logs\runs` and, since tickets 64 & 77,
/// `logs\quick-actions` and `logs\quick-launch`), each with its size and
/// last-mod time. Folder ids embed their creation (`run-<epoch millis>`,
/// `qa-<millis>-<id>`, `ql-<millis>`), which is the most reliable age
/// marker; the folder's mtime is the fallback.
fn list_run_dirs(runs_dir: &Path) -> Vec<LogEntry> {
    let entries = match std::fs::read_dir(runs_dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };
    entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| LogEntry {
            name: e.file_name().to_string_lossy().into_owned(),
            path: e.path().to_string_lossy().into_owned(),
            size_bytes: folder_size(&e.path()),
            modified_at: folder_age(&e.path()),
        })
        .collect()
}

/// A run folder's age: the timestamp its name embeds when it parses (robust
/// against moved folders), else the folder's own mtime.
fn folder_age(path: &Path) -> Option<i64> {
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if let Some(secs) = embedded_age_secs(name) {
            return Some(secs);
        }
    }
    path.metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
}

/// The epoch seconds a log/run folder name embeds, when it carries a
/// recognizable timestamp (ticket 65). Two shapes are understood:
/// the legacy `run-<millis>` / `qa-<millis>-<id>` / `ql-<millis>` names, and
/// the readable `…-<YYYYMMDD>-<HHMMSS>[-more]` names whose LOCAL time
/// converts through the current UTC offset. Pure — no disk access.
pub(crate) fn embedded_age_secs(name: &str) -> Option<i64> {
    for prefix in ["run-", crate::quick_actions::QA_LOG_PREFIX, crate::launch::QL_LOG_PREFIX] {
        let Some(rest) = name.strip_prefix(prefix) else {
            continue;
        };
        let segments: Vec<&str> = rest.split('-').collect();
        // Readable shape: an 8-digit local date followed by a 6-digit time,
        // optionally followed by more (slug, collision suffix).
        if segments.len() >= 2
            && segments[0].len() == 8
            && segments[0].bytes().all(|b| b.is_ascii_digit())
            && segments[1].len() == 6
            && segments[1].bytes().all(|b| b.is_ascii_digit())
        {
            let year: i64 = segments[0][0..4].parse().ok()?;
            let month: i64 = segments[0][4..6].parse().ok()?;
            let day: i64 = segments[0][6..8].parse().ok()?;
            let hour: i64 = segments[1][0..2].parse().ok()?;
            let minute: i64 = segments[1][2..4].parse().ok()?;
            let second: i64 = segments[1][4..6].parse().ok()?;
            if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
                return None;
            }
            let as_if_utc =
                days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second;
            return Some(as_if_utc - local_utc_offset_secs());
        }
        // Legacy shape: bare epoch millis (`run-…`, `qa-…-<id>`, `ql-…`).
        return segments[0].parse::<i64>().ok().map(|m| m / 1000);
    }
    None
}

/// Seconds between the local wall clock and UTC right now — the conversion
/// a folder name's LOCAL stamp needs to become an epoch age (ticket 65).
/// Computed at query time; a DST shift between creation and pruning moves
/// the answer by one hour, far inside the days-scale retention window.
fn local_utc_offset_secs() -> i64 {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::SystemInformation::{GetLocalTime, GetSystemTime};
    fn read(get: unsafe extern "system" fn(*mut SYSTEMTIME)) -> SYSTEMTIME {
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
        unsafe { get(&mut st) };
        st
    }
    systemtime_to_secs(read(GetLocalTime)) - systemtime_to_secs(read(GetSystemTime))
}

/// A SYSTEMTIME as epoch seconds read as-if-UTC — only ever used in a
/// difference of two values read back-to-back, where the absolute base
/// cancels out (ticket 65).
fn systemtime_to_secs(st: windows_sys::Win32::Foundation::SYSTEMTIME) -> i64 {
    days_from_civil(st.wYear as i64, st.wMonth as i64, st.wDay as i64) * 86_400
        + st.wHour as i64 * 3_600
        + st.wMinute as i64 * 60
        + st.wSecond as i64
}

/// Days since 1970-01-01 from a civil date — Howard Hinnant's algorithm,
/// the date math behind the local-stamp conversion (ticket 65). Pure.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Total bytes of every file under `dir`, recursively.
fn folder_size(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| {
            let path = e.path();
            if path.is_dir() {
                folder_size(&path)
            } else {
                file_size(&path)
            }
        })
        .sum()
}

fn file_size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use crate::db::init_at;
    use crate::run::{RequirementOutcome, RunOutcome, RunRecord, RunStatus};
    use crate::settings::save;

    fn test_dir() -> PathBuf {
        // Unique per call — libtest reuses worker threads across tests, so
        // pid+thread-id dirs collide on re-runs.
        tempfile::tempdir().unwrap().into_path()
    }

    /// Removes leftovers from a previous test run — pids and thread ids get
    /// reused, and stale run folders/databases would skew pruning counts.
    fn clean_dir() -> PathBuf {
        let dir = test_dir();
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn write_file(path: &Path, bytes: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    /// Creates a run folder named `run-<millis>` with some files, and a row
    /// in the runs table (as a finished run would leave behind). The id
    /// embeds the folder's creation millis, exactly like `run::new_run_id`.
    fn seed_run(dir: &Path, conn: &Connection, millis: i64) {
        let id = format!("run-{millis}");
        let run_dir = dir.join("logs").join("runs").join(&id);
        write_file(&run_dir.join("status.jsonl"), b"{\"type\":\"phase\",\"phase\":\"starting\"}\n");
        write_file(&run_dir.join(format!("{id}.log")), b"raw output\n");
        let record = RunRecord {
            id,
            started_at: millis / 1000,
            finished_at: millis / 1000 + 60,
            preset_names: vec!["Preset A".into()],
            outcome: RunOutcome::Ok,
            results: vec![RequirementOutcome {
                product_id: "git".into(),
                product_name: "Git".into(),
                status: RunStatus::Installed,
                detail: "installed".into(),
                reboot_required: false,
                log_path: run_dir.join("git.log").to_string_lossy().into_owned(),
            }],
        };
        crate::db::create_run(conn, &record).unwrap();
    }

    fn now_millis() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }

    /// Creates a Quick Action run folder named `qa-<millis>-<id>` with an
    /// output.log, exactly like `quick_actions::new_run_log_path` leaves
    /// behind (ticket 64).
    fn seed_quick_action_run(dir: &Path, millis: i64, action_id: i64) {
        let run_dir = dir
            .join("logs")
            .join(crate::quick_actions::QA_LOGS_DIR_NAME)
            .join(format!(
                "{}{millis}-{action_id}",
                crate::quick_actions::QA_LOG_PREFIX
            ));
        write_file(&run_dir.join("output.log"), b"[stamp] start \"x\" (id=1) pid=1\n");
    }

    /// Creates a Quick Launch run folder named `ql-<millis>` with an
    /// output.log, exactly like `launch::new_launch_run_log_path` leaves
    /// behind (ticket 77).
    fn seed_quick_launch_run(dir: &Path, millis: i64) {
        let run_dir = dir
            .join("logs")
            .join(crate::launch::QL_LOGS_DIR_NAME)
            .join(format!("{}{millis}", crate::launch::QL_LOG_PREFIX));
        write_file(&run_dir.join("output.log"), b"[stamp] quick launch run started\n");
    }

    #[test]
    fn locations_report_missing_paths_as_empty() {
        let dir = clean_dir();
        let locations = list_log_locations_at(&dir, &dir.join("logs"), &dir.join("sprout.db"));
        assert!(locations.runs.is_empty());
        assert_eq!(locations.total_logs_bytes, 0);
        assert_eq!(locations.db_size_bytes, 0);
        assert_eq!(locations.data_dir, dir.to_string_lossy());
    }

    #[test]
    fn run_folders_are_listed_with_sizes() {
        let dir = clean_dir();
        let conn = init_at(&dir).unwrap();
        let now = now_millis();
        seed_run(&dir, &conn, now);
        seed_run(&dir, &conn, now - 1_000_000);

        let locations = list_log_locations_at(&dir, &dir.join("logs"), &dir.join("sprout.db"));
        assert_eq!(locations.runs.len(), 2);
        // Newest first.
        assert_eq!(locations.runs[0].name, format!("run-{now}"));
        assert_eq!(locations.runs[1].name, format!("run-{}", now - 1_000_000));
        // Each folder's two files are counted.
        assert!(locations.runs[0].size_bytes >= 2, "{}", locations.runs[0].size_bytes);
        assert_eq!(
            locations.total_logs_bytes,
            locations.runs[0].size_bytes + locations.runs[1].size_bytes
        );
        assert!(locations.db_size_bytes > 0);
    }

    #[test]
    fn quick_action_folders_are_listed_and_counted() {
        let dir = clean_dir();
        let conn = init_at(&dir).unwrap();
        let now = now_millis();
        seed_run(&dir, &conn, now);
        seed_quick_action_run(&dir, now - 2_000, 3);
        seed_quick_action_run(&dir, now - 1_000, 5);

        let locations = list_log_locations_at(&dir, &dir.join("logs"), &dir.join("sprout.db"));
        // The Quick Action folders are their own newest-first list.
        assert_eq!(locations.quick_action_runs.len(), 2);
        assert_eq!(
            locations.quick_action_runs[0].name,
            format!("{}{}-5", crate::quick_actions::QA_LOG_PREFIX, now - 1_000)
        );
        assert_eq!(
            locations.quick_action_runs[1].name,
            format!("{}{}-3", crate::quick_actions::QA_LOG_PREFIX, now - 2_000)
        );
        // Their bytes count toward the total alongside the run folders.
        assert_eq!(
            locations.total_logs_bytes,
            locations.runs.iter().chain(&locations.quick_action_runs)
                .map(|e| e.size_bytes).sum::<u64>()
        );
    }

    #[test]
    fn quick_launch_folders_are_listed_and_counted() {
        let dir = clean_dir();
        let conn = init_at(&dir).unwrap();
        let now = now_millis();
        seed_run(&dir, &conn, now);
        seed_quick_launch_run(&dir, now - 2_000);
        seed_quick_launch_run(&dir, now - 1_000);

        let locations = list_log_locations_at(&dir, &dir.join("logs"), &dir.join("sprout.db"));
        // The Quick Launch folders are their own newest-first list.
        assert_eq!(locations.quick_launch_runs.len(), 2);
        assert_eq!(
            locations.quick_launch_runs[0].name,
            format!("{}{}", crate::launch::QL_LOG_PREFIX, now - 1_000)
        );
        assert_eq!(
            locations.quick_launch_runs[1].name,
            format!("{}{}", crate::launch::QL_LOG_PREFIX, now - 2_000)
        );
        // Their bytes count toward the total alongside the other families.
        assert_eq!(
            locations.total_logs_bytes,
            locations
                .runs
                .iter()
                .chain(&locations.quick_action_runs)
                .chain(&locations.quick_launch_runs)
                .map(|e| e.size_bytes)
                .sum::<u64>()
        );
    }

    #[test]
    fn pruning_removes_only_expired_quick_launch_folders() {
        let dir = clean_dir();
        let conn = init_at(&dir).unwrap();

        let now = now_millis();
        // 40 days ago — beyond the default 30-day retention; 5 days — inside.
        seed_quick_launch_run(&dir, now - 40 * 86_400_000);
        seed_quick_launch_run(&dir, now - 5 * 86_400_000);

        assert_eq!(prune_run_logs_at(&conn, &dir.join("logs")).unwrap(), 1);
        let ql_dir = dir.join("logs").join(crate::launch::QL_LOGS_DIR_NAME);
        assert!(!ql_dir
            .join(format!("{}{}", crate::launch::QL_LOG_PREFIX, now - 40 * 86_400_000))
            .exists());
        assert!(ql_dir
            .join(format!("{}{}", crate::launch::QL_LOG_PREFIX, now - 5 * 86_400_000))
            .exists());
        // The second pass finds nothing left to remove.
        assert_eq!(prune_run_logs_at(&conn, &dir.join("logs")).unwrap(), 0);
    }

    #[test]
    fn pruning_removes_only_expired_quick_action_folders() {
        let dir = clean_dir();
        let conn = init_at(&dir).unwrap();

        let now = now_millis();
        // 40 days ago — beyond the default 30-day retention; 5 days — inside.
        seed_quick_action_run(&dir, now - 40 * 86_400_000, 1);
        seed_quick_action_run(&dir, now - 5 * 86_400_000, 2);

        assert_eq!(prune_run_logs_at(&conn, &dir.join("logs")).unwrap(), 1);
        let qa_dir = dir.join("logs").join(crate::quick_actions::QA_LOGS_DIR_NAME);
        assert!(!qa_dir
            .join(format!("{}{}-1", crate::quick_actions::QA_LOG_PREFIX, now - 40 * 86_400_000))
            .exists());
        assert!(qa_dir
            .join(format!("{}{}-2", crate::quick_actions::QA_LOG_PREFIX, now - 5 * 86_400_000))
            .exists());
        // The second pass finds nothing left to remove.
        assert_eq!(prune_run_logs_at(&conn, &dir.join("logs")).unwrap(), 0);
    }

    #[test]
    fn readable_quick_action_names_age_and_prune_by_their_embedded_date() {
        let dir = clean_dir();
        let conn = init_at(&dir).unwrap();

        // Local-date components for an epoch second, read AS IF UTC — the
        // at-most-±14h timezone error is noise against the 30-day window.
        fn stamp_name(age_secs: i64) -> String {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let secs = now - age_secs;
            let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
            let sod = secs.rem_euclid(86_400);
            format!(
                "{}{y:04}{m:02}{d:02}-{:02}{:02}{:02}-docker-start",
                crate::quick_actions::QA_LOG_PREFIX,
                sod / 3_600,
                (sod % 3_600) / 60,
                sod % 60
            )
        }

        let expired = stamp_name(40 * 86_400);
        let fresh = stamp_name(0);
        for name in [&expired, &fresh] {
            std::fs::create_dir_all(
                dir.join("logs")
                    .join(crate::quick_actions::QA_LOGS_DIR_NAME)
                    .join(name),
            )
            .unwrap();
        }

        assert_eq!(prune_run_logs_at(&conn, &dir.join("logs")).unwrap(), 1);
        let qa_root = dir.join("logs").join(crate::quick_actions::QA_LOGS_DIR_NAME);
        assert!(!qa_root.join(&expired).exists());
        assert!(qa_root.join(&fresh).exists());

        // The survivor lists newest-first under its readable name.
        let locations = list_log_locations_at(&dir, &dir.join("logs"), &dir.join("sprout.db"));
        assert_eq!(
            locations.quick_action_runs.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            vec![fresh.as_str()]
        );
    }

    /// The inverse of [`days_from_civil`] (Howard Hinnant's algorithm) —
    /// test-only, for building date-stamped folder names.
    fn civil_from_days(z: i64) -> (i64, i64, i64) {
        let z = z + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        (if m <= 2 { y + 1 } else { y }, m, d)
    }

    #[test]
    fn pruning_removes_only_expired_run_folders() {
        let dir = clean_dir();
        let conn = init_at(&dir).unwrap();

        let now = now_millis();
        // 40 days ago — beyond the default 30-day retention.
        seed_run(&dir, &conn, now - 40 * 86_400_000);
        // 5 days ago — still inside the window.
        seed_run(&dir, &conn, now - 5 * 86_400_000);

        let pruned = prune_run_logs_at(&conn, &dir.join("logs")).unwrap();
        assert_eq!(pruned, 1);
        let runs_dir = dir.join("logs").join("runs");
        assert!(!runs_dir.join(format!("run-{}", now - 40 * 86_400_000)).exists());
        assert!(runs_dir.join(format!("run-{}", now - 5 * 86_400_000)).exists());

        // History rows survive the pruning — only raw logs expire.
        assert!(crate::db::get_run(&conn, &format!("run-{}", now - 40 * 86_400_000))
            .unwrap()
            .is_some());
        assert_eq!(crate::db::list_runs(&conn).unwrap().len(), 2);

        // The second pass finds nothing left to remove.
        assert_eq!(prune_run_logs_at(&conn, &dir.join("logs")).unwrap(), 0);
    }

    #[test]
    fn pruning_honors_the_configured_retention() {
        let dir = clean_dir();
        let conn = init_at(&dir).unwrap();

        let now = now_millis();
        // 60 days old: expired under the default 30, kept under a 90-day setting.
        seed_run(&dir, &conn, now - 60 * 86_400_000);

        assert_eq!(prune_run_logs_at(&conn, &dir.join("logs")).unwrap(), 1);

        save(
            &conn,
            &settings::Settings {
                default_timeout_minutes: 10,
                log_retention_days: 90,
                theme: "system".to_string(),
                install_dir: String::new(),
                launch_concurrency: 8,
                dock_mode: settings::DEFAULT_DOCK_MODE.to_string(),
                dock_edge: settings::DEFAULT_DOCK_EDGE.to_string(),
                dock_state: settings::DEFAULT_DOCK_STATE.to_string(),
                autostart: settings::DEFAULT_AUTOSTART.to_string(),
                launch_groups: settings::DEFAULT_GROUPS_FEATURE.to_string(),
                action_groups: settings::DEFAULT_GROUPS_FEATURE.to_string(),
                clip_groups: settings::DEFAULT_GROUPS_FEATURE.to_string(),
                reveal_dwell_ms: settings::DEFAULT_REVEAL_DWELL_MS,
                reveal_sensitivity_px: settings::DEFAULT_REVEAL_SENSITIVITY_PX,
                companion_url: None,
                companion_height_ratio: settings::DEFAULT_COMPANION_HEIGHT_RATIO,
                companion_url_list: Vec::new(),
            },
        )
        .unwrap();
        // A second 60-day-old run: kept now, because the retention window
        // grew to 90 days.
        seed_run(&dir, &conn, now - 61 * 86_400_000);
        assert_eq!(prune_run_logs_at(&conn, &dir.join("logs")).unwrap(), 0);
    }

    #[test]
    fn pruning_without_a_runs_folder_is_a_noop() {
        let dir = clean_dir();
        let conn = init_at(&dir).unwrap();
        assert_eq!(prune_run_logs_at(&conn, &dir.join("logs")).unwrap(), 0);
    }
}