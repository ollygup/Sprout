//! Per-run staging and `<FilesDir>` expansion for attached action files.
//!
//! The execution owner keeps the placeholder, both shell quotings, and the
//! staged directory's lifetime together (ADR-0029): a command referencing
//! files is expanded here, once per run, and nowhere else. Removing files
//! support deletes this module plus its two call sites, with nothing
//! left behind elsewhere.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// The placeholder an action command uses to reference its attached files.
/// Expanded at run time to the staged absolute path, shell-quoted below.
pub const FILES_DIR_PLACEHOLDER: &str = "<FilesDir>";

/// How often the staged-dir release re-checks for surviving children, how
/// long a drained directory lingers for a handed-off open, and how old an
/// orphan must be before the launch sweep reaps it.
const STAGED_CHILD_POLL: Duration = Duration::from_millis(250);
const STAGED_HANDOFF_GRACE: Duration = Duration::from_secs(60);
const STAGED_SWEEP_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// Whether `command` references attached files at all. Without the
/// placeholder the files stay inert: nothing is staged and nothing fails.
pub fn contains_files_placeholder(command: &str) -> bool {
    command.contains(FILES_DIR_PLACEHOLDER)
}

/// Shell-quotes an absolute staged path. PowerShell takes a single-quoted
/// literal (`''` escapes a quote); CMD takes a double-quoted string — the
/// two shells quote differently, which is why the expansion lives with the
/// execution owner instead of the caller (ADR-0029).
pub fn quote_staged_path(shell: &str, path: &Path) -> Result<String, String> {
    match shell {
        "powershell" => Ok(format!(
            "'{}'",
            path.to_string_lossy().replace('\'', "''")
        )),
        "cmd" => {
            let text = path.to_string_lossy();
            if text.contains('"') || text.contains('\n') || text.contains('\r') {
                return Err("the staged files path cannot be quoted for cmd".to_string());
            }
            Ok(format!("\"{text}\""))
        }
        other => Err(format!(
            "'{other}' is not a supported Quick Action shell — expected 'powershell' or 'cmd'"
        )),
    }
}

/// A `\name` or `/name` suffix stays one shell argument only when quoted with
/// its folder: PowerShell reads `'folder'\name` as two arguments, and CMD
/// splits `"folder"\name with spaces` the same way. Matching runs against the
/// staged names so a spaced file still resolves; anything else consumes one
/// token so a typo fails as one missing path instead of two stray arguments.
/// Returns the joined file name (or `None` for the folder alone) plus how many
/// `after` bytes the reference covered.
fn match_placeholder_suffix(after: &str, filenames: &[String]) -> (Option<String>, usize) {
    let mut chars = after.chars();
    match chars.next() {
        Some('\\') | Some('/') => {}
        _ => return (None, 0),
    }
    let rest = &after[1..];
    let mut best: Option<&String> = None;
    for name in filenames {
        if rest.len() >= name.len()
            && rest[..name.len()].eq_ignore_ascii_case(name)
            && best.is_none_or(|current: &String| name.len() > current.len())
        {
            let boundary = match rest[name.len()..].chars().next() {
                None => true,
                Some(next) => !next.is_ascii_alphanumeric() && !".-_~".contains(next),
            };
            if boundary {
                best = Some(name);
            }
        }
    }
    if let Some(name) = best {
        return (Some(name.clone()), 1 + name.len());
    }
    let mut token_len = 0;
    for c in rest.chars() {
        if c.is_whitespace()
            || "<>\"'`&|();\r\n\\/".contains(c)
        {
            break;
        }
        token_len += c.len_utf8();
    }
    if token_len == 0 {
        return (None, 0);
    }
    (Some(rest[..token_len].to_string()), 1 + token_len)
}

/// Replaces every `<FilesDir>` occurrence with the quoted staged path. One
/// pass per run: the replacement text is never rescanned, so a staged path
/// can neither recurse nor partially expand. A reference the author wrapped
/// in quotes normalizes to the same single path — the owner's quoting
/// replaces the author's adjacent pair instead of nesting inside it
/// (One source of truth per Windows command).
pub fn expand_files_dir(
    command: &str,
    shell: &str,
    dir: &Path,
    filenames: &[String],
) -> Result<String, String> {
    let mut expanded = String::with_capacity(command.len() + dir.as_os_str().len());
    let mut rest = command;
    while let Some(at) = rest.find(FILES_DIR_PLACEHOLDER) {
        expanded.push_str(&rest[..at]);
        let after = &rest[at + FILES_DIR_PLACEHOLDER.len()..];
        let (suffix, mut consumed) = match_placeholder_suffix(after, filenames);
        // Only an immediately adjacent matching pair normalizes: a quote
        // opened earlier spans shell syntax around the reference that the
        // owner must not rewrite.
        while let Some(quote) = expanded.chars().next_back() {
            if (quote != '"' && quote != '\'') || !after[consumed..].starts_with(quote) {
                break;
            }
            expanded.pop();
            consumed += quote.len_utf8();
        }
        let quoted = match suffix {
            Some(name) => quote_staged_path(shell, &dir.join(&name))?,
            None => quote_staged_path(shell, dir)?,
        };
        expanded.push_str(&quoted);
        rest = &after[consumed..];
    }
    expanded.push_str(rest);
    Ok(expanded)
}

/// The staging-time name rule mirrors the stored file-name rule: basenames
/// only, so a crafted record can never escape the staging directory.
/// Stored names already passed validation; this is the defense in depth.
fn staged_filename(name: &str) -> Result<String, String> {
    let clean = name.trim();
    if clean.is_empty() || clean == "." || clean == ".." {
        return Err(format!("'{name}' is not a usable file name."));
    }
    if clean.contains('/') || clean.contains('\\') {
        return Err(format!("'{clean}' must be a plain file name without folders."));
    }
    if clean.chars().any(|c| (c as u32) < 0x20) {
        return Err(format!("'{clean}' contains a control character and cannot be staged."));
    }
    Ok(clean.to_string())
}

fn now_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Copies the action's files to a fresh per-run temp directory and returns
/// it. The working directory stays untouched: scripts reach their files only
/// through the expanded placeholder, never through a relocated cwd. The
/// directory inherits the temp folder's user-only default permissions.
pub fn stage_action_files(files: &[(String, Vec<u8>)]) -> Result<PathBuf, String> {
    let pid = std::process::id();
    let mut dir = std::env::temp_dir();
    dir.push(format!("sprout-qa-files-{pid}-{}", now_millis()));
    let mut bump = 0u32;
    loop {
        match std::fs::create_dir(&dir) {
            Ok(()) => break,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                bump += 1;
                dir.set_file_name(format!("sprout-qa-files-{pid}-{}-{bump}", now_millis()));
            }
            Err(e) => return Err(format!("could not stage the action files: {e}")),
        }
    }
    for (name, bytes) in files {
        let clean = staged_filename(name)?;
        if let Err(e) = std::fs::write(dir.join(&clean), bytes) {
            cleanup_staged_dir(&dir);
            return Err(format!("could not stage '{clean}': {e}"));
        }
    }
    Ok(dir)
}

/// Removes a staged directory. Best-effort: logging already records the run,
/// so a leftover temp folder is litter, never a run failure.
pub fn cleanup_staged_dir(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

/// A foreground process owns its staged directory: the reaper releases it
/// once `Child::wait` returns. A detached command exits immediately while
/// its service keeps running, so detached scripts must copy what they need
/// synchronously — and that early exit is also what frees Stop and the run
/// registry (Quick Actions run hidden, unelevated, stoppable, and tracked:
/// tracking covers foreground processes only).
///
/// A GUI program the shell started is the middle case, not the detached
/// one: PowerShell returns as soon as Notepad's process exists while CMD
/// waits for its window to close, so releasing on shell exit pulls the file
/// out from under a starting Notepad. The directory therefore waits out the
/// shell's direct children first, then lingers one grace past the drain so a
/// program the shell handed off to (a second Notepad opening a tab in the
/// already-running instance) still finds its file. Tracking itself is
/// untouched — the run still reports not-running at shell exit; only the
/// temp folder outlives it. Child liveness comes from the process-inspection
/// owner (One source of truth per Windows command), never a second snapshot
/// here: when its snapshot cannot be taken it reports no children and the
/// directory releases on shell exit.
pub fn release_staged_dir(dir: &Path, shell_pid: u32) {
    release_staged_dir_with_grace(dir, shell_pid, STAGED_HANDOFF_GRACE);
}

fn release_staged_dir_with_grace(dir: &Path, shell_pid: u32, grace: Duration) {
    if crate::engine::windows::inspection::children_alive(shell_pid) {
        while crate::engine::windows::inspection::children_alive(shell_pid) {
            std::thread::sleep(STAGED_CHILD_POLL);
        }
        std::thread::sleep(grace);
    }
    cleanup_staged_dir(dir);
}

/// Reaps orphaned staged directories from runs no live session can
/// reference — a crash between spawn and release, or a daemon child that
/// outlived its session. Only directories older than a day: anything fresher
/// could belong to a run still active in a second Sprout instance sharing
/// the temp folder.
pub fn sweep_stale_staged_dirs() {
    sweep_staged_dirs_with_prefix("sprout-qa-files-", STAGED_SWEEP_MAX_AGE);
}

fn sweep_staged_dirs_with_prefix(prefix: &str, max_age: Duration) {
    let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else {
        return;
    };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let is_match = entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with(prefix));
        if !is_match {
            continue;
        }
        let stale = entry
            .metadata()
            .ok()
            .filter(|meta| meta.is_dir())
            .and_then(|meta| meta.modified().ok())
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age > max_age);
        if stale {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn powershell_quotes_with_single_quotes_and_doubles_embedded_ones() {
        assert_eq!(
            quote_staged_path("powershell", Path::new(r"C:\Temp\my files")).unwrap(),
            r"'C:\Temp\my files'"
        );
        assert_eq!(
            quote_staged_path("powershell", Path::new(r"C:\Temp\o'brien")).unwrap(),
            r"'C:\Temp\o''brien'"
        );
    }

    #[test]
    fn cmd_quotes_with_double_quotes() {
        assert_eq!(
            quote_staged_path("cmd", Path::new(r"C:\Temp\my files")).unwrap(),
            r#""C:\Temp\my files""#
        );
    }

    #[test]
    fn unknown_shell_fails_instead_of_guessing() {
        assert!(quote_staged_path("bash", Path::new(r"C:\Temp\x")).is_err());
    }

    #[test]
    fn every_placeholder_occurrence_expands_in_one_pass() {
        let dir = Path::new(r"C:\Temp\staged 1");
        let names = vec!["a.txt".to_string(), "b.txt".to_string()];
        let expanded =
            expand_files_dir("copy <FilesDir>\\a.txt <FilesDir>\\b.txt", "powershell", dir, &names)
                .unwrap();
        assert_eq!(
            expanded,
            r"copy 'C:\Temp\staged 1\a.txt' 'C:\Temp\staged 1\b.txt'"
        );
        let expanded = expand_files_dir("dir <FilesDir>", "cmd", dir, &[]).unwrap();
        assert_eq!(expanded, r#"dir "C:\Temp\staged 1""#);
    }

    #[test]
    fn file_suffix_stays_one_quoted_argument_per_shell() {
        let dir = Path::new(r"C:\Temp\staged 1");
        let names = vec!["notes.txt".to_string()];
        assert_eq!(
            expand_files_dir("notepad <FilesDir>\\notes.txt", "powershell", dir, &names).unwrap(),
            r"notepad 'C:\Temp\staged 1\notes.txt'"
        );
        assert_eq!(
            expand_files_dir("notepad <FilesDir>\\notes.txt", "cmd", dir, &names).unwrap(),
            r#"notepad "C:\Temp\staged 1\notes.txt""#
        );
        assert_eq!(
            expand_files_dir("cat <FilesDir>/notes.txt", "powershell", dir, &names).unwrap(),
            r"cat 'C:\Temp\staged 1\notes.txt'"
        );
    }

    #[test]
    fn spaced_names_match_the_staged_file_regardless_of_typed_case() {
        let dir = Path::new(r"C:\Temp\staged");
        let names = vec!["my file.txt".to_string()];
        assert_eq!(
            expand_files_dir("notepad <FilesDir>\\MY FILE.TXT", "powershell", dir, &names)
                .unwrap(),
            r"notepad 'C:\Temp\staged\my file.txt'"
        );
        assert_eq!(
            expand_files_dir("notepad <FilesDir>\\my file.txt", "cmd", dir, &names).unwrap(),
            r#"notepad "C:\Temp\staged\my file.txt""#
        );
    }

    #[test]
    fn author_quoted_references_normalize_to_one_quoted_path() {
        let dir = Path::new(r"C:\Temp\staged 1");
        let names = vec!["opencode.json".to_string()];
        assert_eq!(
            expand_files_dir("notepad \"<FilesDir>\\opencode.json\"", "powershell", dir, &names)
                .unwrap(),
            r"notepad 'C:\Temp\staged 1\opencode.json'"
        );
        assert_eq!(
            expand_files_dir("notepad \"<FilesDir>\\opencode.json\"", "cmd", dir, &names).unwrap(),
            r#"notepad "C:\Temp\staged 1\opencode.json""#
        );
        assert_eq!(
            expand_files_dir("cat '<FilesDir>/opencode.json'", "powershell", dir, &names).unwrap(),
            r"cat 'C:\Temp\staged 1\opencode.json'"
        );
        assert_eq!(
            expand_files_dir("dir \"<FilesDir>\"", "cmd", dir, &[]).unwrap(),
            r#"dir "C:\Temp\staged 1""#
        );
        // A mismatched pair is not the author's quoting — it stays untouched.
        assert_eq!(
            expand_files_dir("echo \"<FilesDir>\\a.txt'", "powershell", Path::new(r"C:\Temp\staged"), &[])
                .unwrap(),
            r#"echo "'C:\Temp\staged\a.txt''"#
        );
    }

    #[test]
    fn unknown_suffix_fails_as_one_missing_path_not_two_stray_arguments() {
        let dir = Path::new(r"C:\Temp\staged");
        assert_eq!(
            expand_files_dir("notepad <FilesDir>\\typo.txt", "powershell", dir, &[]).unwrap(),
            r"notepad 'C:\Temp\staged\typo.txt'"
        );
        let names = vec!["a.txt".to_string()];
        assert_eq!(
            expand_files_dir("notepad <FilesDir>\\a.txt2", "powershell", dir, &names).unwrap(),
            r"notepad 'C:\Temp\staged\a.txt2'"
        );
    }

    #[test]
    fn replacement_text_is_never_rescanned() {
        let dir = Path::new(r"C:\x<FilesDir>\y");
        let names = vec!["a.txt".to_string()];
        assert_eq!(
            expand_files_dir("run <FilesDir>\\a.txt", "powershell", dir, &names).unwrap(),
            r"run 'C:\x<FilesDir>\y\a.txt'"
        );
    }

    #[test]
    fn command_without_placeholder_passes_through_untouched() {
        let dir = Path::new(r"C:\Temp\staged");
        assert!(!contains_files_placeholder("echo hello"));
        assert_eq!(
            expand_files_dir("echo hello", "powershell", dir, &[]).unwrap(),
            "echo hello"
        );
        assert!(contains_files_placeholder("echo <FilesDir>"));
    }

    #[test]
    fn staged_names_cannot_escape_the_directory() {
        assert!(staged_filename("notes.txt").is_ok());
        assert!(staged_filename("..\\evil.txt").is_err());
        assert!(staged_filename("sub/dir.txt").is_err());
        assert!(staged_filename("..").is_err());
        assert!(staged_filename("   ").is_err());
    }

    #[test]
    fn staging_roundtrips_bytes_and_cleanup_removes_the_directory() {
        let files = vec![
            ("a.txt".to_string(), b"hello".to_vec()),
            ("b.bin".to_string(), vec![0u8, 1, 2, 255]),
        ];
        let dir = stage_action_files(&files).unwrap();
        assert_eq!(std::fs::read(dir.join("a.txt")).unwrap(), b"hello");
        assert_eq!(
            std::fs::read(dir.join("b.bin")).unwrap(),
            vec![0u8, 1, 2, 255]
        );
        cleanup_staged_dir(&dir);
        assert!(!dir.exists());
    }

    #[test]
    fn release_without_children_deletes_on_shell_exit() {
        let dir = stage_action_files(&[("a.txt".to_string(), b"hi".to_vec())]).unwrap();
        let mut shell =
            crate::windows_execution::spawn_action("cmd", "exit 0", None, None).unwrap();
        let pid = shell.id();
        let _ = shell.wait();
        release_staged_dir(&dir, pid);
        assert!(!dir.exists());
    }

    #[test]
    fn release_waits_out_a_live_child_before_deleting() {
        let dir = stage_action_files(&[("a.txt".to_string(), b"hi".to_vec())]).unwrap();
        let mut shell =
            crate::windows_execution::spawn_action("cmd", "ping 127.0.0.1 -n 4 > NUL", None, None)
                .unwrap();
        let pid = shell.id();
        std::thread::sleep(Duration::from_secs(1));
        let start = std::time::Instant::now();
        release_staged_dir_with_grace(&dir, pid, Duration::from_secs(1));
        assert!(start.elapsed() >= Duration::from_secs(2));
        assert!(!dir.exists());
        let _ = shell.wait();
    }

    #[test]
    fn sweep_reaps_only_stale_prefixed_directories() {
        let tmp = std::env::temp_dir();
        let stale = tmp.join("sprout-qa-files-test-probe-stale");
        let lone_file = tmp.join("sprout-qa-files-test-probe-file");
        let other = tmp.join("sprout-qa-other-probe");
        for dir in [&stale, &other] {
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(dir.join("a.txt"), b"x").unwrap();
        }
        std::fs::write(&lone_file, b"x").unwrap();
        sweep_staged_dirs_with_prefix("sprout-qa-files-test-probe-", Duration::from_secs(24 * 60 * 60));
        assert!(stale.exists());
        assert!(lone_file.exists());
        assert!(other.exists());
        sweep_staged_dirs_with_prefix("sprout-qa-files-test-probe-", Duration::ZERO);
        assert!(!stale.exists());
        assert!(lone_file.exists());
        assert!(other.exists());
        let _ = std::fs::remove_file(&lone_file);
        let _ = std::fs::remove_dir_all(&other);
    }

    #[test]
    fn empty_file_set_stages_an_empty_directory() {
        let dir = stage_action_files(&[]).unwrap();
        assert!(dir.is_dir());
        cleanup_staged_dir(&dir);
        assert!(!dir.exists());
    }
}
