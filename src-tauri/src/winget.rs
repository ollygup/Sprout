//! ADR-0029 keeps winget compatibility changes local to this module.
mod authoring;
mod bootstrap;
mod mutation;
pub use authoring::{search, show, WingetMatch, WingetShow};
use std::{collections::HashMap, time::Duration};
use crate::{domain::{Requirement, Step}, engine::StepOutcome, windows_execution::{capture_hidden, run_timed_process, ProcessRun}};

trait WingetProcess {
    fn capture(&self, args: &[&str]) -> Option<String>;
    fn timed(&self, args: &[String], timeout: Duration) -> ProcessRun;
}

struct NativeWinget;
impl WingetProcess for NativeWinget {
    fn capture(&self, args: &[&str]) -> Option<String> {
        let out = capture_hidden("winget", args).ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    }
    fn timed(&self, args: &[String], timeout: Duration) -> ProcessRun {
        run_timed_process("winget", args, timeout)
    }
}

fn available(process: &impl WingetProcess) -> bool { process.capture(&["--version"]).is_some() }

pub(crate) fn prepare(requirements: &[&Requirement]) -> Result<(), String> {
    if available(&NativeWinget) || !requires_winget(requirements) { return Ok(()); }
    bootstrap::bootstrap_winget()
}

pub(crate) fn snapshot() -> Option<HashMap<String, (String, Option<String>)>> {
    snapshot_with(&NativeWinget)
}

fn snapshot_with(process: &impl WingetProcess) -> Option<HashMap<String, (String, Option<String>)>> {
    if !available(process) { return None; }
    let output = process.capture(&["list", "--source", "winget", "--accept-source-agreements", "--disable-interactivity"])?;
    Some(parse_winget_list(&output))
}

pub(crate) fn install(id: &str, timeout_minutes: u32, install_dir: Option<&str>) -> StepOutcome {
    mutation::apply(&NativeWinget, "install", id, timeout_minutes, install_dir)
}

pub(crate) fn upgrade(id: &str, timeout_minutes: u32, install_dir: Option<&str>) -> StepOutcome {
    mutation::apply(&NativeWinget, "upgrade", id, timeout_minutes, install_dir)
}

/// Does this Run need winget on PATH? Command-only runs (e.g. node-lts
/// via nvm) detect against the registry and never touch winget.
fn requires_winget(requirements: &[&Requirement]) -> bool {
    requirements.iter().any(|r| matches!(r.step, Step::Winget { .. }))
}
/// Finds a whole word's byte position in a line (whitespace-bounded) — the
/// one column locator for winget's aligned-column tables, shared by this
/// module's search parser and the engine adapter's `winget list` parser.
fn find_word(line: &str, word: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let needle = word.as_bytes();
    let mut i = 0;
    while i + needle.len() <= bytes.len() {
        if &bytes[i..i + needle.len()] == needle {
            let before_ok = i == 0 || bytes[i - 1] == b' ';
            let after_ok = i + needle.len() == bytes.len() || bytes[i + needle.len()] == b' ';
            if before_ok && after_ok {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// Parses `winget list` stdout (English column layout, as the legacy runner
/// assumed). Columns are aligned by the header words, so product names that
/// contain spaces parse correctly: the id column starts at "Id", the version
/// at "Version", and the optional available version at "Available".
fn parse_winget_list(text: &str) -> HashMap<String, (String, Option<String>)> {
    #[derive(Clone, Copy)]
    struct Columns {
        id_start: usize,
        version_start: usize,
        available_start: Option<usize>,
    }

    let mut map = HashMap::new();
    let mut columns: Option<Columns> = None;

    for line in text.lines() {
        // Note: no trim_end here — rows end with column padding that the
        // column slices rely on.
        if columns.is_none() {
            if let (Some(id_start), Some(version_start)) =
                (find_word(line, "Id"), find_word(line, "Version"))
            {
                columns = Some(Columns {
                    id_start,
                    version_start,
                    available_start: find_word(line, "Available"),
                });
            }
            continue;
        }
        let cols = columns.unwrap();
        if line.trim().is_empty() || line.trim_start().starts_with("---") {
            continue;
        }
        let id = line.get(cols.id_start..cols.version_start).unwrap_or("").trim();
        if id.is_empty() {
            continue;
        }
        let version = match cols.available_start {
            Some(available) => line.get(cols.version_start..available).unwrap_or("").trim(),
            None => line.get(cols.version_start..).unwrap_or("").trim(),
        };
        if version.is_empty() {
            continue;
        }
        let available = cols
            .available_start
            .and_then(|start| line.get(start..))
            .map(|s| s.trim())
            .filter(|s| !s.is_empty() && *s != "-")
            .map(str::to_string);
        map.insert(id.to_lowercase(), (version.to_string(), available));
    }
    map
}

#[cfg(test)]
mod tests {
use super::*;
use crate::domain::Product;
use std::cell::RefCell;

struct Transcript {
    captured: RefCell<Vec<Vec<String>>>,
    timed: RefCell<Vec<(Vec<String>, Duration)>>,
    reads: RefCell<std::collections::VecDeque<Option<String>>>,
    result: ProcessRun,
}

impl Transcript {
    fn new(code: Option<i32>, output: &str, timed_out: bool) -> Self {
        Self {
            captured: RefCell::new(vec![]), timed: RefCell::new(vec![]),
            reads: RefCell::new(std::collections::VecDeque::new()),
            result: ProcessRun { exit_code: code, output: output.into(), timed_out },
        }
    }
}

impl WingetProcess for Transcript {
    fn capture(&self, args: &[&str]) -> Option<String> {
        self.captured.borrow_mut().push(args.iter().map(|a| a.to_string()).collect());
        self.reads.borrow_mut().pop_front().expect("unexpected capture")
    }
    fn timed(&self, args: &[String], timeout: Duration) -> ProcessRun {
        self.timed.borrow_mut().push((args.to_vec(), timeout));
        self.result.clone()
    }
}

#[test]
fn transcript_keeps_catalog_and_mutation_exit_policies_distinct() {
    let process = Transcript::new(Some(3010), "reboot pending", false);
    assert!(authoring::search_with(&process, "Git").unwrap_err().contains("exited 3010"));
    let outcome = mutation::apply(&process, "install", "Git.Git", 7, Some(r"D:\My Apps"));
    assert!(outcome.ok && outcome.reboot_required);
    assert_eq!(outcome.log, "reboot pending");
    let calls = process.timed.borrow();
    assert_eq!(calls[0].0, ["search", "--query", "Git", "--source", "winget", "--accept-source-agreements", "--disable-interactivity"]);
    assert_eq!(calls[0].1, Duration::from_secs(120));
    assert_eq!(calls[1].0, ["install", "--id", "Git.Git", "-e", "--source", "winget", "--location", r"D:\My Apps", "--accept-source-agreements", "--accept-package-agreements"]);
    assert_eq!(calls[1].1, Duration::from_secs(420));
}

#[test]
fn transcript_distinguishes_timeout_spawn_failure_and_missing_package() {
    for (code, output, timeout, detail) in [
        (None, "failed to start", false, "failed to start"),
        (Some(0), "partial output", true, "processes were killed"),
        (Some(0), "No package found matching input criteria.", false, "check its ID"),
    ] {
        let process = Transcript::new(code, output, timeout);
        let result = mutation::apply(&process, "upgrade", "Missing.Product", 2, None);
        assert!(!result.ok);
        assert_eq!(result.timed_out, timeout);
        assert_eq!(result.log, output);
        assert!(result.detail.contains(detail), "{}", result.detail);
    }
    let process = Transcript::new(None, "partial", true);
    assert!(authoring::show_with(&process, "Git.Git").unwrap_err().contains("60 seconds"));
    let calls = process.timed.borrow();
    assert_eq!(calls[0].0, ["show", "--id", "Git.Git", "--accept-source-agreements", "--disable-interactivity"]);
    assert_eq!(calls[0].1, Duration::from_secs(60));
}

#[test]
fn transcript_snapshot_preserves_unavailable_and_failed_list_reads() {
    let process = Transcript::new(None, "", false);
    process.reads.borrow_mut().push_back(None);
    assert!(snapshot_with(&process).is_none());
    assert_eq!(process.captured.borrow().len(), 1);
    process.reads.borrow_mut().extend([Some("v1".into()), None]);
    assert!(snapshot_with(&process).is_none());
    process.reads.borrow_mut().extend([Some("v1".into()), Some(format!("{:<20}{:<20}{}\n{:<20}{:<20}{}", "Name", "Id", "Version", "Git", "Git.Git", "2.0"))]);
    assert_eq!(snapshot_with(&process).unwrap()["git.git"], ("2.0".into(), None));
    let calls = process.captured.borrow();
    assert_eq!(calls[0], ["--version"]);
    assert_eq!(calls[2], ["list", "--source", "winget", "--accept-source-agreements", "--disable-interactivity"]);
}
#[test]
fn parses_winget_list_rows_with_available_column() {
    // Real winget output pads every column to the header width; rows are
    // built here the same way so alignment is exact.
    let row = |name: &str, id: &str, version: &str, available: &str| {
        format!("{:<40}{:<32}{:<12}{}", name, id, version, available)
    };
    let text = format!(
        "{}\n{}\n{}\n{}\n{}",
        row("Name", "Id", "Version", "Available"),
        "-".repeat(96),
        row("7-Zip 24.09", "7zip.7zip", "24.09", "24.10"),
        row(
            "Eclipse Temurin 21.0.5",
            "EclipseAdoptium.Temurin.21.JDK",
            "21.0.5",
            ""
        ),
        row("Git 2.47.0", "Git.Git", "2.47.0", ""),
    );
    let map = parse_winget_list(&text);
    assert_eq!(map.len(), 3);
    let (version, available) = &map["7zip.7zip"];
    assert_eq!(version, "24.09");
    assert_eq!(available.as_deref(), Some("24.10"));
    let (version, available) = &map["eclipseadoptium.temurin.21.jdk"];
    assert_eq!(version, "21.0.5");
    assert_eq!(available, &None);
    let (version, available) = &map["git.git"];
    assert_eq!(version, "2.47.0");
    assert_eq!(available, &None);
}

#[test]
fn ignores_separator_and_empty_lines() {
    let map = parse_winget_list("\n-----\n\n");
    assert!(map.is_empty());
}

#[test]
fn requires_winget_is_false_for_command_only_runs() {
    let cmd_req = Requirement {
        product: Product {
            id: "node-lts".into(),
            name: "Node.js LTS (via NVM)".into(),
            winget_id: None,
            install_location_hint: None,
            install_dir: None,
            default_env: vec![],
        },
            step: Step::Command { exe: "nvm.cmd".into(), args: vec!["install".into(), "lts".into()], success_codes: vec![0] },
        version_policy: crate::domain::VersionPolicy::Latest,
        depends_on: vec![],
        timeout_minutes: 10,
        env: vec![],
        verify: vec![],
        unresolved: false,
    };
    assert!(!requires_winget(&[&cmd_req]));

    let winget_req = Requirement {
        step: Step::Winget {
            id: "Git.Git".into(),
            scope: "machine".into(),
        },
        ..cmd_req.clone()
    };
    assert!(requires_winget(&[&winget_req]));
    assert!(requires_winget(&[&cmd_req, &winget_req]));
}
}
