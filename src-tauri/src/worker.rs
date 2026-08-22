//! The elevated worker (ADR-0003, ticket 06).
//!
//! The main process stays non-elevated throughout; the run phase relaunches
//! this same exe as `Sprout.exe --worker --run <id>` under one UAC prompt.
//! There is no cross-elevation IPC — the only shared state is the per-run
//! working directory on disk:
//!
//! - `request.json`  — the Plan (preset names + resolved Requirements), written
//!   by the main process before the relaunch;
//! - `status.jsonl`  — JSON-lines progress the worker appends, tailed by the
//!   UI (one `ProgressEvent` per line, flushed per event);
//! - `cancel`        — a marker the main process touches to request a stop; the
//!   worker checks it between Requirements, so the in-flight step always
//!   completes (a hung one is still killed by its timebox);
//! - `done.json`     — written last, atomically via rename, with the overall
//!   outcome (or the error when the worker could not run at all);
//! - `<product>.log` — the raw per-Requirement outputs, as in dev-mode runs.
//!
//! The worker runs no Tauri and shows no window: it executes the Plan through
//! the exact `run::execute_run_observed` pipeline (no fork), persists the Run
//! to the same Library SQLite file, and exits.
//!
//! This module also owns the run-active query (ticket 18): the on-disk state
//! is the single source of truth for whether a run is in progress, so a
//! layout-level banner can show it from any page — and even after the app
//! restarts while the worker is still installing.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::domain::Requirement;
use crate::engine::windows::WindowsWingetEngine;
use crate::run::{execute_run_observed, ProgressEvent, RunOutcome};

/// How long a freshly created run folder may sit without any progress before
/// it is no longer "live": the worker writes its first status line right
/// after the UAC prompt, so a folder that never gets one (prompt declined,
/// worker killed before writing) stops counting as active shortly after the
/// prompt would have resolved.
const BOOT_GRACE: Duration = Duration::from_secs(3 * 60);

/// Fixed margin on top of the sum of a run's Requirement timeboxes: the
/// worker never exceeds its own timeboxes, so a status file this quiet is a
/// dead worker, not a slow one.
const DEADLINE_MARGIN_SECS: i64 = 180;

/// How long a finished run stays "recently finished": the window in which the
/// run-active query hands its completion to the UI (banner + toast) exactly
/// once, even when nothing was watching when it ended.
const RECENT_FINISH_WINDOW: Duration = Duration::from_secs(60);

/// The Plan the main process hands to the worker.
#[derive(Debug, Serialize, Deserialize)]
pub struct RequestPayload {
    pub preset_names: Vec<String>,
    pub requirements: Vec<Requirement>,
}

/// The completion marker the worker writes last (`done.json`). `error` is
/// present when the worker could not run or persist at all — then there is no
/// RunRecord in the Library to load.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoneInfo {
    pub outcome: RunOutcome,
    #[serde(default)]
    pub error: Option<String>,
}

/// The per-run working directory for a run id.
pub fn run_dir(run_id: &str) -> PathBuf {
    crate::db::logs_dir().join("runs").join(run_id)
}

/// Picks `--run <id>` out of the command line.
pub fn worker_run_id(args: &[String]) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == "--run")
        .map(|pair| pair[1].clone())
}

/// Relaunches `exe` with `args` elevated, via `ShellExecuteW`'s `runas` verb —
/// the single UAC prompt of the run phase (ADR-0003). Returns an error when
/// the launch failed, which covers the user declining the prompt.
pub fn launch_elevated(exe: &Path, args: &[&str]) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }
    let mut file: Vec<u16> = exe.as_os_str().encode_wide().collect();
    file.push(0);
    let parameters = wide(&args.join(" "));
    let operation = wide("runas");

    // > 32 means the verb was handed off; <= 32 is an error code (5 = access
    // denied / declined, 1223 = cancelled).
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            file.as_ptr(),
            parameters.as_ptr(),
            std::ptr::null(),
            SW_HIDE,
        )
    };
    let code = result as isize;
    if code > 32 {
        Ok(())
    } else {
        Err(match code {
            5 => "the UAC prompt was declined or blocked".to_string(),
            1223 => "the UAC prompt was cancelled".to_string(),
            _ => format!("Windows rejected the relaunch (error {code})"),
        })
    }
}

/// The elevated worker's entry point: read the request, execute the Plan
/// streaming progress to `status.jsonl`, persist the Run, and finish with
/// `done.json`. Exits nonzero when the run could not complete.
pub fn run_worker() {
    let args: Vec<String> = std::env::args().collect();
    let Some(run_id) = worker_run_id(&args) else {
        std::process::exit(2);
    };
    let dir = run_dir(&run_id);

    let mut status = match open_status(&dir) {
        Ok(file) => file,
        Err(e) => {
            finish(&dir, RunOutcome::Failed, Some(&format!("cannot open the status file: {e}")));
            std::process::exit(1);
        }
    };
    append_event(&mut status, ProgressEvent::Phase { phase: "starting".into() });

    let request = match read_request(&dir) {
        Ok(request) => request,
        Err(e) => {
            finish(&dir, RunOutcome::Failed, Some(&format!("cannot read the run request: {e}")));
            std::process::exit(1);
        }
    };

    let cancel_path = dir.join("cancel");
    let engine = WindowsWingetEngine;
    // The machine-local default install directory (ticket 34, ADR-0009) is a
    // Settings value read here, at run time — the run honors whatever the
    // settings said when it started, with per-Product overrides (ticket 36)
    // riding inside the requirement. A failed read (locked DB) just means
    // winget's own default directory.
    let install_dir = crate::db::init()
        .ok()
        .map(|conn| crate::settings::load(&conn).install_dir)
        .unwrap_or_default();
    let install_dir = if install_dir.is_empty() {
        None
    } else {
        Some(install_dir.as_str())
    };
    let result = execute_run_observed(
        &engine,
        &run_id,
        &request.preset_names,
        &request.requirements,
        &dir,
        install_dir,
        &mut |event| append_event(&mut status, event),
        &mut || cancel_path.exists(),
    );
    let _ = status.flush();

    let (outcome, error) = match result {
        Ok(record) => match crate::db::init().and_then(|conn| {
            crate::db::create_run(&conn, &record)?;
            // Runs honor the log-retention setting (ticket 09): expired run
            // folders are pruned right after a run finishes.
            let _ = crate::logs::prune_run_logs(&conn);
            Ok(())
        }) {
            Ok(()) => (record.outcome, None),
            Err(e) => (
                RunOutcome::Failed,
                Some(format!("the run finished but its results could not be persisted: {e}")),
            ),
        },
        Err(e) => (RunOutcome::Failed, Some(e)),
    };
    finish(&dir, outcome, error.as_deref());
    // A run that completed — even one that needed attention — is not a worker
    // failure (ticket 16): only hard failures and aborted runs exit nonzero.
    std::process::exit(if matches!(outcome, RunOutcome::Ok | RunOutcome::WithNotes) {
        0
    } else {
        1
    });
}

/// Opens `status.jsonl` for appending (created on first use).
fn open_status(dir: &Path) -> Result<File, String> {
    File::options()
        .create(true)
        .append(true)
        .open(dir.join("status.jsonl"))
        .map_err(|e| e.to_string())
}

/// Reads the Plan the main process wrote before the relaunch.
fn read_request(dir: &Path) -> Result<RequestPayload, String> {
    let bytes = std::fs::read(dir.join("request.json"))
        .map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

/// Appends one JSON-lines progress event and flushes, so the main process
/// sees it immediately.
fn append_event(status: &mut File, event: ProgressEvent) {
    if let Ok(mut line) = serde_json::to_vec(&event) {
        line.push(b'\n');
        let _ = status.write_all(&line);
        let _ = status.flush();
    }
}

/// Writes the completion marker atomically (temp file + rename), so a reader
/// never observes a half-written `done.json`.
fn finish(dir: &Path, outcome: RunOutcome, error: Option<&str>) {
    let info = DoneInfo {
        outcome,
        error: error.map(str::to_string),
    };
    let temp = dir.join("done.json.tmp");
    let done = dir.join("done.json");
    if let Ok(bytes) = serde_json::to_vec(&info) {
        let _ = std::fs::write(&temp, &bytes);
        let _ = std::fs::rename(&temp, &done);
    }
}

/// Reads the events appended since the given byte offset. Lines that are not
/// complete (a trailing partial line from an in-flight write) are left for
/// the next read; the returned offset is where the next read resumes.
pub fn read_status_events(dir: &Path, offset: usize) -> (Vec<ProgressEvent>, usize) {
    let bytes = match std::fs::read(dir.join("status.jsonl")) {
        Ok(bytes) => bytes,
        Err(_) => return (Vec::new(), offset),
    };
    let offset = offset.min(bytes.len());
    let mut events = Vec::new();
    let mut pos = offset;
    while pos < bytes.len() {
        let Some(relative_end) = bytes[pos..].iter().position(|&b| b == b'\n') else {
            break;
        };
        let end = pos + relative_end;
        if end > pos {
            if let Ok(event) = serde_json::from_slice::<ProgressEvent>(&bytes[pos..end]) {
                events.push(event);
            }
        }
        pos = end + 1;
    }
    (events, pos)
}

/// The completion marker, when the worker has written it.
pub fn read_done(dir: &Path) -> Option<DoneInfo> {
    let bytes = std::fs::read(dir.join("done.json")).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// The run-active query's answer (ticket 18): which run is live right now,
/// plus its completion marker when it just finished and no UI has surfaced
/// it yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActiveRunInfo {
    pub run_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done: Option<DoneInfo>,
}

/// Whether a run folder is still live: the worker has started writing
/// progress (`status.jsonl` exists) and has not finished (`done.json` is
/// absent), and the status file has been touched within the run's own
/// deadline — the sum of its Requirements' timeboxes plus margin. A worker
/// that died (killed process, machine reboot) stops touching the file, so the
/// deadline expires and the run is no longer active — the banner must never
/// pin a ghost.
pub fn run_is_live(dir: &Path, now: SystemTime) -> bool {
    if dir.join("done.json").is_file() {
        return false;
    }
    let status = dir.join("status.jsonl");
    if !status.is_file() {
        // Never started writing: live only during the boot window after the
        // folder was created (the worker writes its first line right after
        // the UAC prompt; a declined prompt or a killed worker never does).
        return dir_age(dir, now)
            .map(|age| age <= BOOT_GRACE)
            .unwrap_or(false);
    }
    let Some(age) = dir_age(&status, now) else {
        return false;
    };
    (age.as_secs() as i64) < run_deadline_secs(dir)
}

/// How long a run's status file may go quiet: the sum of every Requirement's
/// timebox (the worker can never take longer than them) plus a fixed margin
/// for detection and persistence. Without a readable request (e.g. the run
/// never started), a generous day is the fallback.
fn run_deadline_secs(dir: &Path) -> i64 {
    let Ok(bytes) = std::fs::read(dir.join("request.json")) else {
        return 24 * 60 * 60;
    };
    let Ok(request) = serde_json::from_slice::<RequestPayload>(&bytes) else {
        return 24 * 60 * 60;
    };
    let minutes: u64 = request
        .requirements
        .iter()
        .map(|r| u64::from(r.timeout_minutes))
        .sum();
    (minutes * 60) as i64 + DEADLINE_MARGIN_SECS
}

/// The age of a file or folder, when it exists.
fn dir_age(path: &Path, now: SystemTime) -> Option<Duration> {
    let modified = path.metadata().and_then(|m| m.modified()).ok()?;
    now.duration_since(modified).ok()
}

/// The run-active query (ticket 18): the newest live run when one is in
/// progress, else the newest run that just finished (with its outcome), else
/// `None`. Purely on-disk — no in-memory vote — so the answer is the same
/// from any page and after an app restart while the worker kept installing.
pub fn active_run(runs_dir: &Path) -> Option<ActiveRunInfo> {
    let now = SystemTime::now();
    find_active_run_at(runs_dir, now)
        .map(|run_id| ActiveRunInfo { run_id, done: None })
        .or_else(|| find_recently_finished_run_at(runs_dir, now))
}

/// The newest live run folder under `runs_dir` — the app-restart fallback of
/// the run-active query. Newest means the highest run-id millis, which is
/// creation order.
fn find_active_run_at(runs_dir: &Path, now: SystemTime) -> Option<String> {
    list_run_folders(runs_dir)
        .into_iter()
        .filter(|dir| run_is_live(dir, now))
        .map(|dir| dir.file_name().unwrap_or_default().to_string_lossy().into_owned())
        .max_by(|a, b| run_millis(a).cmp(&run_millis(b)))
}

/// The newest run whose worker just finished: `done.json` exists and was
/// written within [`RECENT_FINISH_WINDOW`]. This is how a completion the UI
/// was not watching (app closed, or on another page) still surfaces exactly
/// once — the banner shows it and the toast announces it.
fn find_recently_finished_run_at(runs_dir: &Path, now: SystemTime) -> Option<ActiveRunInfo> {
    list_run_folders(runs_dir)
        .into_iter()
        .filter_map(|dir| {
            let name = dir.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let done = read_done(&dir)?;
            let modified = dir.join("done.json").metadata().and_then(|m| m.modified()).ok()?;
            let age = now.duration_since(modified).ok()?;
            if age > RECENT_FINISH_WINDOW {
                return None;
            }
            Some((run_millis(&name), name, done))
        })
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, run_id, done)| ActiveRunInfo {
            run_id,
            done: Some(done),
        })
}

/// Every run folder under `runs_dir` (created lazily; a missing folder is an
/// empty list).
fn list_run_folders(runs_dir: &Path) -> Vec<PathBuf> {
    match std::fs::read_dir(runs_dir) {
        Ok(entries) => entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// The seconds a run id embeds, for ordering — the shared name-age parser
/// (ticket 65) understands both the legacy `run-<epoch millis>` ids and the
/// readable `run-<YYYYMMDD>-<HHMMSS>` ones; an unparseable id orders as 0.
fn run_millis(name: &str) -> i64 {
    crate::logs::embedded_age_secs(name).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_run_flag() {
        let args = vec!["sprout.exe".to_string(), "--worker".to_string(), "--run".to_string(), "run-123".to_string()];
        assert_eq!(worker_run_id(&args), Some("run-123".to_string()));

        assert_eq!(worker_run_id(&["sprout.exe".to_string()]), None);
        assert_eq!(worker_run_id(&["--run".to_string()]), None);
        assert_eq!(
            worker_run_id(&["sprout.exe".to_string(), "--import".to_string(), "x.sprout.json".to_string()]),
            None
        );
    }

    #[test]
    fn request_payload_roundtrips() {
        let payload = RequestPayload {
            preset_names: vec!["Backend dev box".into()],
            requirements: vec![crate::domain::Requirement {
                product: crate::domain::Product {
                    id: "git".into(),
                    name: "Git".into(),
                    winget_id: Some("Git.Git".into()),
                    install_location_hint: None,
                    install_dir: None,
                    default_env: vec![],
                },
                step: crate::domain::Step::Winget {
                    id: "Git.Git".into(),
                    scope: "machine".into(),
                },
                version_policy: crate::domain::VersionPolicy::Latest,
                depends_on: vec![],
                timeout_minutes: 10,
                env: vec![],
                verify: vec![],
                unresolved: false,
            }],
        };
        let bytes = serde_json::to_vec(&payload).unwrap();
        let back: RequestPayload = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back.preset_names, payload.preset_names);
        assert_eq!(back.requirements, payload.requirements);
    }

    #[test]
    fn status_reader_resumes_from_the_given_offset() {
        let dir = tempfile::tempdir().unwrap();
        let mut file = File::options()
            .create(true)
            .append(true)
            .open(dir.path().join("status.jsonl"))
            .unwrap();
        let write = |file: &mut File, event: &ProgressEvent| {
            let mut line = serde_json::to_vec(event).unwrap();
            line.push(b'\n');
            file.write_all(&line).unwrap();
        };
        let started = ProgressEvent::RequirementStarted {
            index: 0,
            total: 1,
            product_id: "git".into(),
            product_name: "Git".into(),
            action: "install".into(),
        };
        let finished = ProgressEvent::RequirementFinished(crate::run::RequirementOutcome {
            product_id: "git".into(),
            product_name: "Git".into(),
            status: crate::run::RunStatus::Installed,
            detail: "installed".into(),
            reboot_required: false,
            log_path: String::new(),
        });
        write(&mut file, &started);
        // A partial trailing line (worker mid-write) must be left unread…
        file.write_all(b"{\"type\":\"phase\",\"ph").unwrap();
        file.flush().unwrap();

        let (events, offset) = read_status_events(dir.path(), 0);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], started);
        assert!(offset > 0);

        // …even after the worker completes it (the next read picks it up).
        file.write_all(b"ase\":\"detecting\"}\n").unwrap();
        write(&mut file, &finished);
        file.flush().unwrap();

        let (events, next) = read_status_events(dir.path(), offset);
        assert_eq!(
            events,
            vec![
                ProgressEvent::Phase { phase: "detecting".into() },
                finished,
            ]
        );
        assert!(next > offset);

        // Nothing new from the fresh offset.
        let (events, same) = read_status_events(dir.path(), next);
        assert!(events.is_empty());
        assert_eq!(same, next);
    }

    #[test]
    fn missing_status_file_reads_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        let (events, offset) = read_status_events(dir.path(), 0);
        assert!(events.is_empty());
        assert_eq!(offset, 0);
    }

    #[test]
    fn done_info_roundtrips_and_missing_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_done(dir.path()).is_none());

        finish(dir.path(), RunOutcome::Cancelled, None);
        assert_eq!(read_done(dir.path()).unwrap(), DoneInfo {
            outcome: RunOutcome::Cancelled,
            error: None,
        });

        finish(dir.path(), RunOutcome::Failed, Some("boom"));
        assert_eq!(read_done(dir.path()).unwrap(), DoneInfo {
            outcome: RunOutcome::Failed,
            error: Some("boom".into()),
        });
    }

    fn write_status(dir: &Path) {
        std::fs::write(
            dir.join("status.jsonl"),
            b"{\"type\":\"phase\",\"phase\":\"starting\"}\n",
        )
        .unwrap();
    }

    fn write_request(dir: &Path, timeout_minutes: u32) {
        let request = RequestPayload {
            preset_names: vec!["Preset A".into()],
            requirements: vec![crate::domain::Requirement {
                product: crate::domain::Product {
                    id: "git".into(),
                    name: "Git".into(),
                    winget_id: Some("Git.Git".into()),
                    install_location_hint: None,
                    install_dir: None,
                    default_env: vec![],
                },
                step: crate::domain::Step::Winget {
                    id: "Git.Git".into(),
                    scope: "machine".into(),
                },
                version_policy: crate::domain::VersionPolicy::Latest,
                depends_on: vec![],
                timeout_minutes,
                env: vec![],
                verify: vec![],
                unresolved: false,
            }],
        };
        std::fs::write(dir.join("request.json"), serde_json::to_vec(&request).unwrap()).unwrap();
    }

    #[test]
    fn run_is_live_while_the_worker_writes_and_no_done() {
        let dir = tempfile::tempdir().unwrap();
        write_status(dir.path());
        assert!(run_is_live(dir.path(), SystemTime::now()));
    }

    #[test]
    fn finished_run_is_not_live() {
        let dir = tempfile::tempdir().unwrap();
        write_status(dir.path());
        finish(dir.path(), RunOutcome::Ok, None);
        assert!(!run_is_live(dir.path(), SystemTime::now()));
    }

    #[test]
    fn never_started_run_is_live_only_during_the_boot_grace() {
        let dir = tempfile::tempdir().unwrap();
        // A fresh folder without a status line: the worker may still be
        // booting behind the UAC prompt.
        assert!(run_is_live(dir.path(), SystemTime::now()));
        // Once the boot window passes without a first line, it is a ghost
        // (declined prompt, worker killed before writing) — never "active".
        let later = SystemTime::now() + BOOT_GRACE + Duration::from_secs(10);
        assert!(!run_is_live(dir.path(), later));
    }

    #[test]
    fn a_run_whose_status_is_older_than_its_deadline_is_no_longer_active() {
        let dir = tempfile::tempdir().unwrap();
        // A one-minute timebox → deadline of 60 s + the 180 s margin.
        write_request(dir.path(), 1);
        write_status(dir.path());
        let started = SystemTime::now();
        assert!(run_is_live(dir.path(), started));
        // Five minutes of silence without a done marker is a dead worker.
        assert!(!run_is_live(dir.path(), started + Duration::from_secs(301)));
        // …but the same status stays live for its full budget when the plan
        // is long (one 60-minute timebox → 60 min + margin).
        write_request(dir.path(), 60);
        assert!(run_is_live(dir.path(), started + Duration::from_secs(1200)));
        assert!(!run_is_live(dir.path(), started + Duration::from_secs(60 * 60 + 181)));
    }

    #[test]
    fn readable_run_ids_order_by_their_embedded_date() {
        // Ticket 65: the new `run-<date>-<time>` ids order chronologically,
        // and legacy millis ids still parse.
        assert!(run_millis("run-20260820-141210") > run_millis("run-20260819-141210"));
        assert!(run_millis("run-1787252173294") > 0);
        assert_eq!(run_millis("not-a-run"), 0);
    }

    #[test]
    fn active_run_picks_the_newest_live_folder() {
        let runs = tempfile::tempdir().unwrap();
        let older = runs.path().join("run-1000");
        let newer = runs.path().join("run-2000");
        std::fs::create_dir_all(&older).unwrap();
        std::fs::create_dir_all(&newer).unwrap();
        write_status(&older);
        write_status(&newer);
        let now = SystemTime::now();
        assert_eq!(
            find_active_run_at(runs.path(), now).as_deref(),
            Some("run-2000")
        );
        // A finished run is never "active", whatever its age.
        finish(&newer, RunOutcome::Ok, None);
        assert_eq!(
            find_active_run_at(runs.path(), now).as_deref(),
            Some("run-1000")
        );
    }

    #[test]
    fn recently_finished_run_surfaces_its_outcome_within_the_window() {
        let runs = tempfile::tempdir().unwrap();
        let dir = runs.path().join("run-1000");
        std::fs::create_dir_all(&dir).unwrap();
        finish(&dir, RunOutcome::WithNotes, None);
        let finished = SystemTime::now();
        assert_eq!(
            find_recently_finished_run_at(runs.path(), finished),
            Some(ActiveRunInfo {
                run_id: "run-1000".into(),
                done: Some(DoneInfo {
                    outcome: RunOutcome::WithNotes,
                    error: None,
                }),
            })
        );
        // Once the window passes, the completion is no longer surfaced — a
        // run from yesterday must not re-announce itself at app start.
        let later = finished + RECENT_FINISH_WINDOW + Duration::from_secs(5);
        assert!(find_recently_finished_run_at(runs.path(), later).is_none());
    }

    #[test]
    fn active_run_prefers_a_live_run_over_a_recently_finished_one() {
        let runs = tempfile::tempdir().unwrap();
        let live = runs.path().join("run-3000");
        let finished = runs.path().join("run-2000");
        std::fs::create_dir_all(&live).unwrap();
        std::fs::create_dir_all(&finished).unwrap();
        write_status(&live);
        finish(&finished, RunOutcome::Ok, None);
        let info = active_run(runs.path()).unwrap();
        assert_eq!(info.run_id, "run-3000");
        assert!(info.done.is_none());
        // Only the finished one left: its outcome rides along.
        finish(&live, RunOutcome::Cancelled, None);
        let info = active_run(runs.path()).unwrap();
        assert_eq!(info.run_id, "run-3000");
        assert_eq!(info.done.map(|d| d.outcome), Some(RunOutcome::Cancelled));
    }

    #[test]
    fn active_run_with_no_runs_is_none() {
        let runs = tempfile::tempdir().unwrap();
        assert!(active_run(runs.path()).is_none());
    }
}
