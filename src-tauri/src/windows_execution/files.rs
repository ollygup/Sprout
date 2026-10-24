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
/// Matched ASCII case-insensitively: the author's casing is never load-bearing
/// (One source of truth per Windows command keeps the single spelling here).
pub const FILES_DIR_PLACEHOLDER: &str = "<FilesDir>";

/// Every casing of the placeholder is the same byte length, so a
/// case-insensitive match still slices the command on valid boundaries.
fn find_placeholder(haystack: &str) -> Option<usize> {
    haystack
        .as_bytes()
        .windows(FILES_DIR_PLACEHOLDER.len())
        .position(|window| window.eq_ignore_ascii_case(FILES_DIR_PLACEHOLDER.as_bytes()))
}

/// How often the staged-dir release re-checks for surviving children, how
/// long a drained directory lingers for a handed-off open, and how old an
/// orphan must be before the launch sweep reaps it.
const STAGED_CHILD_POLL: Duration = Duration::from_millis(250);
const STAGED_HANDOFF_GRACE: Duration = Duration::from_secs(60);
const STAGED_SWEEP_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// Whether `command` references attached files at all. Without the
/// placeholder the files stay inert: nothing is staged and nothing fails.
pub fn contains_files_placeholder(command: &str) -> bool {
    find_placeholder(command).is_some()
}

/// A `\name` or `/name` suffix stays one shell argument only when quoted with
/// its folder: PowerShell reads `'folder'\name` as two arguments, and CMD
/// splits `"folder"\name with spaces` the same way. Matching runs against the
/// staged names so a spaced file still resolves; anything else consumes the
/// whole relative path behind the placeholder — every further `\` or `/`
/// segment included — so a typo fails as one missing path instead of two
/// stray arguments (`<FilesDir>\sub\a.txt` expands to one quoted
/// `'<dir>\sub\a.txt'`, never `'<dir>\sub'\a.txt`).
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
        // `get` keeps non-ASCII typing from panicking on a split boundary:
        // a failed slice simply misses and falls through to the token below.
        if rest.len() >= name.len()
            && rest
                .get(..name.len())
                .is_some_and(|head| head.eq_ignore_ascii_case(name))
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
    // Unknown text still joins as one relative path: separators keep
    // consuming so `sub\a.txt` stays a single argument. Only true shell
    // boundaries (whitespace, quotes, operators) terminate the suffix — a
    // space still ends it, since an unquoted space always separates
    // arguments in both shells. Trailing separators are trimmed back off so
    // `<FilesDir>\a.txt\` keeps the lone-separator-drop below load-bearing.
    let mut token_len = 0;
    for c in rest.chars() {
        if c.is_whitespace() || "<>\"'`&|();\r\n".contains(c) {
            break;
        }
        token_len += c.len_utf8();
    }
    while token_len > 0
        && rest[..token_len].chars().next_back().is_some_and(|c| c == '\\' || c == '/')
    {
        token_len -= rest[..token_len].chars().next_back().map(|c| c.len_utf8()).unwrap_or(0);
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
///
/// A glued prefix (`file://<FilesDir>\a.txt`, `--in=<FilesDir>\a.txt`,
/// `prefix<FilesDir>`) joins inside the owner's quotes as one argument
/// (`"file://C:\...\a.txt"`), since `prefix"C:\..."` splits in CMD while the
/// single quoted form stays one argument in both shells.
pub fn expand_files_dir(
    command: &str,
    shell: &str,
    dir: &Path,
    filenames: &[String],
) -> Result<String, String> {
    let mut expanded = String::with_capacity(command.len() + dir.as_os_str().len());
    let mut rest = command;
    while let Some(at) = find_placeholder(rest) {
        expanded.push_str(&rest[..at]);
        let after = &rest[at + FILES_DIR_PLACEHOLDER.len()..];
        let (suffix, mut consumed) = match_placeholder_suffix(after, filenames);
        // A glued prefix stays one argument only inside the owner's quotes:
        // scan back past the placeholder to the last shell boundary and lift
        // that text into the quoted path. Boundaries mirror the suffix
        // terminators (whitespace, quotes, operators) — everything else
        // (`file://`, `--flag=`, `prefix`) is prefix content.
        let token_start = expanded
            .rfind(|c: char| c.is_whitespace() || "\"'`&|();<>".contains(c))
            .map(|i| i + expanded[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1))
            .unwrap_or(0);
        let prefix: String = expanded[token_start..].to_string();
        expanded.truncate(token_start);
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
        // A lone separator left after the reference is a stray, not a new
        // segment: leaving it outside the owner's quotes glues it onto the
        // path in both shells (`...file.html\`), so a separator followed only
        // by a boundary is dropped instead (One source of truth per Windows
        // command keeps the quoting shape in this owner alone).
        if let Some(sep) = after[consumed..].chars().next() {
            if sep == '\\' || sep == '/' {
                let tail = &after[consumed + sep.len_utf8()..];
                let boundary = match tail.chars().next() {
                    None => true,
                    Some(next) => next.is_whitespace() || "<>\"'`&|();".contains(next),
                };
                if boundary {
                    consumed += sep.len_utf8();
                }
            }
        }
        let staged = match &suffix {
            Some(name) => dir.join(name),
            None => dir.to_path_buf(),
        };
        let full = format!("{}{}", prefix, staged.to_string_lossy());
        let quoted = quote_full_path(shell, &full)?;
        expanded.push_str(&quoted);
        rest = &after[consumed..];
    }
    expanded.push_str(rest);
    Ok(expanded)
}

/// Quotes one already-joined `prefix + staged absolute path` for the shell.
/// PowerShell takes a single-quoted literal (`''` escapes a quote); CMD takes
/// a double-quoted string; Python takes its own literal (see below). The whole
/// joined string sits inside one pair, so a prefix never glues onto the opening
/// quote as a separate argument.
fn quote_full_path(shell: &str, full: &str) -> Result<String, String> {
    match shell {
        "powershell" => Ok(format!("'{}'", full.replace('\'', "''"))),
        "cmd" => {
            if full.contains('"') || full.contains('\n') || full.contains('\r') {
                return Err("the staged files path cannot be quoted for cmd".to_string());
            }
            Ok(format!("\"{full}\""))
        }
        "python3" => quote_python_literal(full),
        other => Err(format!(
            "'{other}' is not a supported Quick Action shell — expected 'powershell', 'cmd', or 'python3'"
        )),
    }
}

/// A Windows path as a Python string literal: double-quoted with every
/// backslash doubled, so the script reads the path literally instead of
/// interpreting its segments as escapes (`\t`, `\n`). A path holding a line
/// break has nowhere honest to go on one literal line and is refused.
fn quote_python_literal(text: &str) -> Result<String, String> {
    if text.contains('\n') || text.contains('\r') {
        return Err("the staged files path cannot be quoted for python3".to_string());
    }
    Ok(format!(
        "\"{}\"",
        text.replace('\\', "\\\\").replace('"', "\\\"")
    ))
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
/// out from under a starting Notepad. The directory therefore always lingers
/// one grace past shell exit — even with no observed children, since a
/// handed-off open may not have appeared in the process snapshot yet
/// (ticket 211 repairs that observe-then-grace TOCTOU hole). When the shell
/// still owns direct children, the release drains them first and then lingers
/// the same grace, so a program the shell handed off to (a second Notepad
/// opening a tab in the already-running instance) still finds its file.
/// Tracking itself is untouched — the run still reports not-running at shell
/// exit; only the temp folder outlives it. Child liveness comes from the
/// process-inspection owner (One source of truth per Windows command), never
/// a second snapshot here: when its snapshot cannot be taken it reports no
/// children and the directory still lingers one grace past shell exit.
pub fn release_staged_dir(dir: &Path, shell_pid: u32) {
    release_staged_dir_with_grace(dir, shell_pid, STAGED_HANDOFF_GRACE);
}

fn release_staged_dir_with_grace(dir: &Path, shell_pid: u32, grace: Duration) {
    while crate::engine::windows::inspection::children_alive(shell_pid) {
        std::thread::sleep(STAGED_CHILD_POLL);
    }
    std::thread::sleep(grace);
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
            quote_full_path("powershell", r"C:\Temp\my files").unwrap(),
            r"'C:\Temp\my files'"
        );
        assert_eq!(
            quote_full_path("powershell", r"C:\Temp\o'brien").unwrap(),
            r"'C:\Temp\o''brien'"
        );
    }

    #[test]
    fn cmd_quotes_with_double_quotes() {
        assert_eq!(
            quote_full_path("cmd", r"C:\Temp\my files").unwrap(),
            r#""C:\Temp\my files""#
        );
    }

    #[test]
    fn python_quotes_with_doubled_backslashes() {
        assert_eq!(
            quote_full_path("python3", r"C:\Temp\my files").unwrap(),
            r#""C:\\Temp\\my files""#
        );
    }

    #[test]
    fn unknown_shell_fails_instead_of_guessing() {
        assert!(quote_full_path("bash", r"C:\Temp\x").is_err());
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
    fn placeholder_matches_regardless_of_case() {
        let dir = Path::new(r"C:\Temp\staged 1");
        let names = vec!["page.html".to_string()];
        assert!(contains_files_placeholder("run <filesdir>"));
        assert!(contains_files_placeholder("run <FILESDIR>"));
        assert!(contains_files_placeholder("run <Filesdir>"));
        assert!(!contains_files_placeholder("run hello"));
        assert_eq!(
            expand_files_dir("Start-Process <filesdir>\\page.html", "powershell", dir, &names)
                .unwrap(),
            r"Start-Process 'C:\Temp\staged 1\page.html'"
        );
        assert_eq!(
            expand_files_dir("echo prefix<filesdir>", "powershell", dir, &names).unwrap(),
            r"echo 'prefixC:\Temp\staged 1'"
        );
        assert_eq!(
            expand_files_dir(
                "copy <FILESDIR>\\a.txt <FilesDir>\\b.txt",
                "powershell",
                dir,
                &["a.txt".to_string(), "b.txt".to_string()]
            )
            .unwrap(),
            r"copy 'C:\Temp\staged 1\a.txt' 'C:\Temp\staged 1\b.txt'"
        );
    }

    #[test]
    fn lone_trailing_separator_after_reference_is_dropped() {
        let dir = Path::new(r"C:\Temp\staged 1");
        let names = vec!["a.txt".to_string()];
        assert_eq!(
            expand_files_dir("dir <FilesDir>\\", "cmd", dir, &[]).unwrap(),
            r#"dir "C:\Temp\staged 1""#
        );
        assert_eq!(
            expand_files_dir("cat <FilesDir>/", "powershell", dir, &[]).unwrap(),
            r"cat 'C:\Temp\staged 1'"
        );
        assert_eq!(
            expand_files_dir("notepad <FilesDir>\\a.txt\\", "powershell", dir, &names).unwrap(),
            r"notepad 'C:\Temp\staged 1\a.txt'"
        );
        assert_eq!(
            expand_files_dir("echo <FilesDir>\\ more", "powershell", dir, &[]).unwrap(),
            r"echo 'C:\Temp\staged 1' more"
        );
        // A separator with more path behind it stays one quoted argument:
        // even an unknown subpath fails as one missing path, never as a
        // quoted folder plus a stray `\file` outside the quotes.
        assert_eq!(
            expand_files_dir("type <FilesDir>\\sub\\a.txt", "powershell", dir, &names).unwrap(),
            r"type 'C:\Temp\staged 1\sub\a.txt'"
        );
        assert_eq!(
            expand_files_dir("type <FilesDir>/sub/deep/a.txt", "cmd", dir, &names).unwrap(),
            r#"type "C:\Temp\staged 1\sub/deep/a.txt""#
        );
    }

    #[test]
    fn multi_segment_suffix_stays_one_quoted_argument() {
        let dir = Path::new(r"C:\Temp\staged 1");
        let names = vec!["a.txt".to_string()];
        // Unknown subpaths join as one relative path: a single missing-path
        // error downstream, never a quoted folder plus a stray segment.
        assert_eq!(
            expand_files_dir("type <FilesDir>\\sub\\a.txt", "powershell", dir, &names).unwrap(),
            r"type 'C:\Temp\staged 1\sub\a.txt'"
        );
        // Shell operators still terminate the suffix: only path text joins.
        assert_eq!(
            expand_files_dir("type <FilesDir>\\sub\\a.txt&echo done", "powershell", dir, &names)
                .unwrap(),
            r"type 'C:\Temp\staged 1\sub\a.txt'&echo done"
        );
        // A known spaced file still resolves through the match, not the token.
        let spaced = vec!["my file.txt".to_string()];
        assert_eq!(
            expand_files_dir("notepad <FilesDir>\\my file.txt", "cmd", dir, &spaced).unwrap(),
            r#"notepad "C:\Temp\staged 1\my file.txt""#
        );
    }

    #[test]
    fn glued_prefix_joins_inside_one_quoted_argument() {
        let dir = Path::new(r"C:\Temp\staged 1");
        let names = vec!["index.html".to_string()];
        // URL prefix: one argument, not `prefix"path"`.
        assert_eq!(
            expand_files_dir("open file://<FilesDir>\\index.html", "cmd", dir, &names).unwrap(),
            r#"open "file://C:\Temp\staged 1\index.html""#
        );
        assert_eq!(
            expand_files_dir("open file://<FilesDir>\\index.html", "powershell", dir, &names)
                .unwrap(),
            r"open 'file://C:\Temp\staged 1\index.html'"
        );
        // Flag prefix with `=`.
        assert_eq!(
            expand_files_dir("tool --in=<FilesDir>\\index.html", "cmd", dir, &names).unwrap(),
            r#"tool "--in=C:\Temp\staged 1\index.html""#
        );
        // Bare glued word.
        assert_eq!(
            expand_files_dir("echo prefix<FilesDir>\\index.html", "cmd", dir, &names).unwrap(),
            r#"echo "prefixC:\Temp\staged 1\index.html""#
        );
        // Author-quoted prefix normalizes to one pair, never nested quotes.
        assert_eq!(
            expand_files_dir("\"prefix<FilesDir>\\index.html\"", "cmd", dir, &names).unwrap(),
            r#""prefixC:\Temp\staged 1\index.html""#
        );
        assert_eq!(
            expand_files_dir("\"file://<FilesDir>\\index.html\"", "cmd", dir, &names).unwrap(),
            r#""file://C:\Temp\staged 1\index.html""#
        );
        // The reported `start` shapes expand truthfully: bare placeholder
        // quotes the path (CMD then treats it as a title — the caller's
        // `start ""` fixes that, not the placeholder owner).
        assert_eq!(
            expand_files_dir("start <FilesDir>\\index.html", "cmd", dir, &names).unwrap(),
            r#"start "C:\Temp\staged 1\index.html""#
        );
        assert_eq!(
            expand_files_dir("start \"\" \"<FilesDir>\\index.html\"", "cmd", dir, &names).unwrap(),
            r#"start "" "C:\Temp\staged 1\index.html""#
        );
        assert_eq!(
            expand_files_dir("Start-Process <FilesDir>\\index.html", "powershell", dir, &names)
                .unwrap(),
            r"Start-Process 'C:\Temp\staged 1\index.html'"
        );
    }

    #[test]
    fn non_ascii_typing_never_panics_the_match() {
        let dir = Path::new(r"C:\Temp\staged 1");
        let names = vec!["ax".to_string()];
        assert_eq!(
            expand_files_dir("echo <FilesDir>\\aéx", "powershell", dir, &names).unwrap(),
            r"echo 'C:\Temp\staged 1\aéx'"
        );
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
    fn release_without_children_lingers_past_shell_exit() {
        let dir = stage_action_files(&[("a.txt".to_string(), b"hi".to_vec())]).unwrap();
        let mut shell =
            crate::windows_execution::spawn_action("cmd", "exit 0", None, None).unwrap();
        let pid = shell.id();
        let _ = shell.wait();
        let start = std::time::Instant::now();
        release_staged_dir_with_grace(&dir, pid, Duration::from_secs(1));
        assert!(start.elapsed() >= Duration::from_secs(1));
        assert!(!dir.exists());
    }

    #[test]
    fn handed_off_open_finds_its_file_after_shell_exit() {
        let files = vec![
            ("doc.pdf".to_string(), b"%PDF-1.4 staged".to_vec()),
            ("clip.mp3".to_string(), b"ID3 staged".to_vec()),
        ];
        let dir = stage_action_files(&files).unwrap();
        let mut shell =
            crate::windows_execution::spawn_action("cmd", "exit 0", None, None).unwrap();
        let pid = shell.id();
        let _ = shell.wait();
        let pending = dir.clone();
        let handle =
            std::thread::spawn(move || release_staged_dir_with_grace(&pending, pid, Duration::from_secs(2)));
        // Cold-start grace: the shell is gone but the handed-off viewer has
        // not appeared in any snapshot — the staged paths still resolve.
        std::thread::sleep(Duration::from_millis(250));
        assert_eq!(
            std::fs::read(dir.join("doc.pdf")).unwrap(),
            b"%PDF-1.4 staged"
        );
        assert_eq!(std::fs::read(dir.join("clip.mp3")).unwrap(), b"ID3 staged");
        handle.join().unwrap();
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
