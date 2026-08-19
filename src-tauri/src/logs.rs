//! Log locations and retention (ticket 09).
//!
//! Raw outputs live under %LOCALAPPDATA%\Sprout\logs (ADR-0006): one folder
//! per run (`logs\runs\<run-id>`, see [`crate::worker::run_dir`]) plus the
//! database itself in the data root. The Logs screen never renders log
//! content — it shows where the files live, how big they are, and opens the
//! folder on request. `prune_run_logs` is the retention knob: runs delete
//! run folders older than the settings' `log_retention_days` after every
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
/// sizes, the database file, and one entry per run folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LogLocations {
    pub data_dir: String,
    pub logs_dir: String,
    pub db_path: String,
    pub db_size_bytes: u64,
    /// Total bytes across every run folder; 0 when there are no runs yet.
    pub total_logs_bytes: u64,
    /// One entry per run folder, newest first.
    pub runs: Vec<LogEntry>,
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
    let total_logs_bytes = runs.iter().map(|e| e.size_bytes).sum();

    LogLocations {
        data_dir: data_dir.to_string_lossy().into_owned(),
        logs_dir: logs_dir.to_string_lossy().into_owned(),
        db_path: db_path.to_string_lossy().into_owned(),
        db_size_bytes: file_size(db_path),
        total_logs_bytes,
        runs,
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
/// the real app data directory. Returns how many folders were removed. Runs
/// honor this after every completed run (worker) and at app start (main
/// process).
pub fn prune_run_logs(conn: &Connection) -> Result<usize, String> {
    prune_run_logs_at(conn, &crate::db::logs_dir())
}

/// The same as [`prune_run_logs`], against an explicit logs directory.
pub fn prune_run_logs_at(conn: &Connection, logs_dir: &Path) -> Result<usize, String> {
    let retention_days = settings::load(conn).log_retention_days;
    let cutoff_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
        .saturating_sub(i64::from(retention_days) * 86_400);

    let runs_dir = logs_dir.join("runs");
    let entries = match std::fs::read_dir(&runs_dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(0), // nothing exists yet — nothing to prune
    };

    let mut pruned = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let modified_at = run_dir_modified_at(&path).unwrap_or(i64::MIN);
        if modified_at < cutoff_secs {
            match std::fs::remove_dir_all(&path) {
                Ok(()) => pruned += 1,
                Err(_) => {} // a folder in use is left for the next run
            }
        }
    }
    Ok(pruned)
}

/// The per-run folders under `logs\runs`, each with its size and last-mod
/// time. Run ids encode their creation (`run-<epoch millis>`), which is the
/// most reliable age marker; the folder's mtime is the fallback.
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
            modified_at: run_dir_modified_at(&e.path()),
        })
        .collect()
}

/// A run folder's age: the millis embedded in its `run-<millis>` name when it
/// parses (robust against moved folders), else the folder's own mtime.
fn run_dir_modified_at(path: &Path) -> Option<i64> {
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if let Some(millis) = name.strip_prefix("run-") {
            if let Ok(millis) = millis.parse::<i64>() {
                return Some(millis / 1000);
            }
        }
    }
    path.metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
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