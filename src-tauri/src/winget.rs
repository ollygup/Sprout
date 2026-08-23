//! Authoring-time winget queries (ticket 13): the live registry search and
//! package details behind the product dialog. Read-only — nothing is
//! installed, nothing is written. Both commands run non-interactively with
//! source agreements accepted up front, under a timebox, so a slow first
//! search can never hang the dialog (the frontend shows "Searching…" while
//! the call is in flight).
//!
//! Parsing strategy: winget's search/show output has no JSON mode (verified
//! on winget 1.29), so the primary parser reads the aligned-column table the
//! way `parse_winget_list` does, and an exact-row fallback (id-anchored
//! heuristics, no header words at all) covers localized headers and
//! misaligned rows.

use std::time::Duration;

use serde::Serialize;

use crate::engine::windows::run_timed_process;

/// The winget source is updated the first time it is used; a generous box
/// keeps the first search from ever looking hung.
const SEARCH_TIMEOUT: Duration = Duration::from_secs(120);
const SHOW_TIMEOUT: Duration = Duration::from_secs(60);

/// How many search rows the picker sees — a short query can match hundreds.
const MAX_SEARCH_ROWS: usize = 20;

/// The "Match" column's value prefixes (`Command: python`, `Tag: x`, ...).
const MATCH_PREFIXES: [&str; 6] = [
    "Command:",
    "Tag:",
    "Moniker:",
    "ProductCode:",
    "Searchable:",
    "Shortcut:",
];

/// Source names the exact-row fallback recognizes.
const KNOWN_SOURCES: [&str; 2] = ["winget", "msstore"];

/// One row of `winget search` — everything the product dialog's picker
/// shows. `publisher` is only known from `winget show` and stays `None`
/// here.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WingetMatch {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// The trailing "Match" column ("Command: python", "Tag: x", ...).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_kind: Option<String>,
}

/// One package's `winget show` details, enriching a picked match.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WingetShow {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moniker: Option<String>,
}

/// Runs one timeboxed winget invocation and returns its merged output.
/// `Err` for a timeout, a failed spawn, or a non-zero exit (the raw output
/// attached — winget's own message is useful to callers).
fn run_winget(args: &[&str], timeout: Duration) -> Result<String, String> {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let run = run_timed_process("winget", &args, timeout);
    if run.timed_out {
        return Err(format!(
            "winget did not finish in {} seconds — its processes were killed",
            timeout.as_secs()
        ));
    }
    match run.exit_code {
        Some(0) => Ok(run.output),
        Some(code) => Err(format!("winget exited {code}: {}", run.output.trim())),
        None => Err(format!("winget failed to start: {}", run.output.trim())),
    }
}

/// Live registry search: matches for `query` from the winget source, capped
/// at [`MAX_SEARCH_ROWS`] rows for the picker.
pub fn search(query: &str) -> Result<Vec<WingetMatch>, String> {
    let out = run_winget(
        &[
            "search",
            "--query",
            query,
            "--source",
            "winget",
            "--accept-source-agreements",
            "--disable-interactivity",
        ],
        SEARCH_TIMEOUT,
    )?;
    let mut matches = parse_search_output(&out);
    matches.truncate(MAX_SEARCH_ROWS);
    Ok(matches)
}

/// One package's registry details — what the dialog shows after a match is
/// picked (publisher, moniker, ...).
pub fn show(id: &str) -> Result<WingetShow, String> {
    let out = run_winget(
        &[
            "show",
            "--id",
            id,
            "--accept-source-agreements",
            "--disable-interactivity",
        ],
        SHOW_TIMEOUT,
    )?;
    parse_show_output(&out)
        .ok_or_else(|| "winget show returned no parseable package details".to_string())
}

/// Parses `winget search` stdout: the aligned-column table when the English
/// header words are present, the id-anchored exact-row fallback otherwise.
fn parse_search_output(text: &str) -> Vec<WingetMatch> {
    let lines: Vec<&str> = text.lines().collect();
    match lines.iter().find_map(|line| header_columns(line)) {
        Some(cols) => {
            let mut seen_header = false;
            let mut rows = Vec::new();
            for line in lines {
                if !seen_header {
                    seen_header = header_columns(line).is_some();
                    continue;
                }
                if let Some(row) = row_aligned(line, cols) {
                    rows.push(row);
                }
            }
            rows
        }
        None => lines.iter().filter_map(|line| row_fallback(line)).collect(),
    }
}

/// The column layout of a `winget search` header line, when it is the
/// English shape (Name … Id … Version … [Match] … [Source]).
#[derive(Clone, Copy)]
struct Columns {
    id_start: usize,
    version_start: usize,
    match_start: Option<usize>,
    source_start: Option<usize>,
}

impl Columns {
    /// Where the version column ends: the next known column, or the row end.
    fn version_end(&self, line_len: usize) -> usize {
        self.match_start
            .or(self.source_start)
            .unwrap_or(line_len)
    }
}

/// Finds a whole word's byte position in a line (whitespace-bounded) — the
/// one column locator for winget's aligned-column tables, shared by this
/// module's search parser and the engine adapter's `winget list` parser.
pub(crate) fn find_word(line: &str, word: &str) -> Option<usize> {
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

fn header_columns(line: &str) -> Option<Columns> {
    let name_start = find_word(line, "Name")?;
    let id_start = find_word(line, "Id")?;
    let version_start = find_word(line, "Version")?;
    if name_start >= id_start || id_start >= version_start {
        return None;
    }
    Some(Columns {
        id_start,
        version_start,
        match_start: find_word(line, "Match"),
        source_start: find_word(line, "Source"),
    })
}

/// One data row against the known column layout. Rows keep their trailing
/// padding — the column slices rely on it, exactly like `parse_winget_list`.
fn row_aligned(line: &str, cols: Columns) -> Option<WingetMatch> {
    if line.trim().is_empty() || line.trim_start().starts_with("---") {
        return None;
    }
    let id = line
        .get(cols.id_start..cols.version_start)
        .unwrap_or("")
        .trim();
    if id.is_empty() {
        return None;
    }
    let name = line.get(..cols.id_start).unwrap_or("").trim();
    if name.is_empty() {
        return None;
    }
    let version = line
        .get(cols.version_start..cols.version_end(line.len()))
        .unwrap_or("")
        .trim();
    let match_kind = cols.match_start.and_then(|start| {
        let end = cols.source_start.unwrap_or(line.len());
        line.get(start..end)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    });
    let source = cols.source_start.and_then(|start| {
        line.get(start..)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    });
    Some(WingetMatch {
        id: id.to_string(),
        name: name.to_string(),
        publisher: None,
        version: (!version.is_empty()).then(|| version.to_string()),
        source,
        match_kind,
    })
}

/// Splits a row into fields on runs of two or more spaces, keeping
/// single-space groups intact (product names like "Joplin (Pre-release)").
fn split_on_double_space(line: &str) -> Vec<&str> {
    let bytes = line.as_bytes();
    let mut fields = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b' ' {
            let mut j = i;
            while j < bytes.len() && bytes[j] == b' ' {
                j += 1;
            }
            if j - i >= 2 {
                if !line[start..i].trim().is_empty() {
                    fields.push(&line[start..i]);
                }
                start = j;
            }
            i = j;
        } else {
            i += 1;
        }
    }
    if !line[start..].trim().is_empty() {
        fields.push(&line[start..]);
    }
    fields
}

/// A winget id by shape: a whitespace-free token with a dotted vendor
/// prefix ("Anaconda.Miniconda3"). Versions like "26.1.4" and names with
/// inner spaces like "Python 3.14" are rejected.
fn looks_like_winget_id(field: &str) -> bool {
    if field.contains(' ') || !field.contains('.') {
        return false;
    }
    let vendor = field.split('.').next().unwrap_or("");
    vendor.chars().any(|c| c.is_ascii_alphabetic())
}

/// A version token: starts with a digit, or carries a digit together with a
/// dot ("py314_26.5.3-2"). Match-kind and source tokens never match.
fn looks_like_version(field: &str) -> bool {
    field.starts_with(|c: char| c.is_ascii_digit())
        || (field.contains('.') && field.chars().any(|c| c.is_ascii_digit()))
}

/// The exact-row fallback: no header words, no alignment — each row yields
/// its own match or nothing. The post-id remainder is re-split on single
/// spaces too, so a match column fused to the version ("3.2.10 Tag: joplin")
/// still parses. Version, match kind, and source are picked by content,
/// never by position.
fn row_fallback(line: &str) -> Option<WingetMatch> {
    if line.trim().is_empty() || line.trim_start().starts_with("---") {
        return None;
    }
    let fields = split_on_double_space(line);
    let id_index = fields.iter().position(|f| looks_like_winget_id(*f))?;
    let id = fields[id_index];
    let name = fields[0].trim();
    if name.is_empty() {
        return None;
    }
    let joined = fields[id_index + 1..].join(" ");
    let tokens: Vec<&str> = joined
        .split(' ')
        .filter(|t| !t.is_empty())
        .collect();

    let mut version = None;
    let mut match_kind = None;
    let mut source = None;
    let mut i = 0;
    while i < tokens.len() {
        let token = tokens[i];
        if let Some(prefix) = MATCH_PREFIXES.iter().find(|p| token.starts_with(**p)) {
            let mut value = String::from(*prefix);
            i += 1;
            while i < tokens.len()
                && !looks_like_version(tokens[i])
                && !KNOWN_SOURCES.contains(&tokens[i])
                && !MATCH_PREFIXES.iter().any(|p| tokens[i].starts_with(p))
            {
                value.push(' ');
                value.push_str(tokens[i]);
                i += 1;
            }
            match_kind = Some(value);
            continue;
        }
        if looks_like_version(token) && version.is_none() {
            version = Some(token);
            i += 1;
            continue;
        }
        if KNOWN_SOURCES.contains(&token) {
            source = Some(token);
            i += 1;
            continue;
        }
        i += 1;
    }
    Some(WingetMatch {
        id: id.to_string(),
        name: name.to_string(),
        publisher: None,
        version: version.map(str::to_string),
        source: source.map(str::to_string),
        match_kind,
    })
}

/// Parses `winget show` stdout: the "Found Name [id]" line plus the
/// `Key: value` fields (Publisher, Version, Source, Moniker). `None` when
/// the id cannot be found — the caller turns that into an error.
fn parse_show_output(text: &str) -> Option<WingetShow> {
    let mut id = None;
    let mut name = None;
    let mut publisher = None;
    let mut version = None;
    let mut source = None;
    let mut moniker = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(found) = trimmed.strip_prefix("Found ") {
            if let Some(open) = found.find('[') {
                if let Some(close) = found[open..].find(']') {
                    name = Some(found[..open].trim().to_string());
                    id = Some(found[open + 1..open + close].to_string());
                }
            }
            continue;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        match key {
            "Publisher" if publisher.is_none() => publisher = Some(value.to_string()),
            "Version" if version.is_none() => version = Some(value.to_string()),
            "Source" if source.is_none() => source = Some(value.to_string()),
            "Moniker" if moniker.is_none() => moniker = Some(value.to_string()),
            _ => {}
        }
    }
    Some(WingetShow {
        id: id?,
        name,
        publisher,
        version,
        source,
        moniker,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a search table row padded to the header widths, the way
    /// winget pads real output (trailing padding included). Widths are
    /// generous enough for the longest fixture content.
    fn row(name: &str, id: &str, version: &str, match_kind: &str, source: &str) -> String {
        format!("{name:<24}{id:<28}{version:<12}{match_kind:<20}{source}")
    }

    fn aligned_table(rows: &[String]) -> String {
        format!(
            "{}\n{}\n{}",
            row("Name", "Id", "Version", "Match", "Source"),
            "-".repeat(90),
            rows.join("\n")
        )
    }

    #[test]
    fn parses_aligned_rows_with_source_and_match() {
        // Real multi-source output shape (winget 1.29, joplin query).
        let text = aligned_table(&[
            row("Joplin", "Joplin.Joplin", "3.6.15", "", "winget"),
            row(
                "Joplin (Pre-release)",
                "Joplin.Joplin.Pre-release",
                "3.2.10",
                "Tag: joplin",
                "winget",
            ),
        ]);
        let matches = parse_search_output(&text);
        assert_eq!(matches.len(), 2);

        assert_eq!(
            matches[0],
            WingetMatch {
                id: "Joplin.Joplin".into(),
                name: "Joplin".into(),
                publisher: None,
                version: Some("3.6.15".into()),
                source: Some("winget".into()),
                match_kind: None,
            }
        );
        assert_eq!(matches[1].name, "Joplin (Pre-release)");
        assert_eq!(matches[1].match_kind.as_deref(), Some("Tag: joplin"));
        assert_eq!(matches[1].source.as_deref(), Some("winget"));
    }

    #[test]
    fn parses_single_source_shape_without_source_column() {
        // Real single-source output (winget 1.29, dbeaver query): no Source
        // column, empty Match cells keep their padding.
        let text = format!(
            "{}\n{}\n{}\n{}\n{}",
            format!("{:<14}{:<28}{:<9}{}", "Name", "Id", "Version", "Match"),
            "-".repeat(70),
            format!("{:<14}{:<28}{:<9}{}", "DBeaver", "DBeaver.DBeaver.Community", "26.1.4", "ProductCode: dbeaver"),
            format!("{:<14}{:<28}{:<9}{}", "DBeaverEE", "DBeaver.DBeaver.Enterprise", "26.1.0", ""),
            format!("{:<14}{:<28}{:<9}{}", "DBeaverLite", "DBeaver.DBeaver.Lite", "26.1.0", ""),
        );
        let matches = parse_search_output(&text);
        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0].id, "DBeaver.DBeaver.Community");
        assert_eq!(matches[0].version.as_deref(), Some("26.1.4"));
        assert_eq!(matches[0].match_kind.as_deref(), Some("ProductCode: dbeaver"));
        assert_eq!(matches[0].source, None);
        assert_eq!(matches[1].match_kind, None);
        assert_eq!(matches[2].name, "DBeaverLite");
    }

    #[test]
    fn empty_output_parses_to_no_matches() {
        assert!(parse_search_output("").is_empty());
        assert!(parse_search_output("No package found matching the query.\n").is_empty());
    }

    #[test]
    fn fallback_parses_localized_output_without_headers() {
        // A hypothetical localized winget: header words are not English, but
        // the rows still carry ids, versions, and a source.
        let text = "\
Name         Id                       Version Match
---------------------------------------------------
Anaconda3    Anaconda.Anaconda3       2025.12-2    Command: python
Miniconda3   Anaconda.Miniconda3      py314_26.5.3-2 Command: python
";
        // Strip the English-looking header words to simulate a locale where
        // even the header row differs — the fallback must not depend on it.
        let localized = text
            .replace("Name", "Nom")
            .replace("Id", "Identifikator")
            .replace("Version", "Version")
            .replace("Match", "Correspondance");
        let matches = parse_search_output(&localized);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].id, "Anaconda.Anaconda3");
        assert_eq!(matches[0].name, "Anaconda3");
        assert_eq!(matches[0].version.as_deref(), Some("2025.12-2"));
        assert_eq!(matches[1].id, "Anaconda.Miniconda3");
        assert_eq!(matches[1].version.as_deref(), Some("py314_26.5.3-2"));
    }

    #[test]
    fn fallback_handles_names_with_inner_spaces() {
        // The name "Joplin (Pre-release)" contains single spaces; splitting
        // on runs of 2+ spaces must keep it as one field.
        let text = "Joplin (Pre-release)  Joplin.Joplin.Pre-release  3.2.10  Tag: joplin  winget";
        let matches = parse_search_output(text);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "Joplin (Pre-release)");
        assert_eq!(matches[0].id, "Joplin.Joplin.Pre-release");
        assert_eq!(matches[0].version.as_deref(), Some("3.2.10"));
        assert_eq!(matches[0].match_kind.as_deref(), Some("Tag: joplin"));
        assert_eq!(matches[0].source.as_deref(), Some("winget"));
    }

    #[test]
    fn fallback_ignores_version_like_names_and_footers() {
        // "Python 3.14" must not be taken for the id; a footer line without
        // an id yields nothing.
        let text = "\
Python 3.14    Python.Python.3.14   3.14.7   Command: python
Found 42 packages
";
        let matches = parse_search_output(text);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, "Python.Python.3.14");
        assert_eq!(matches[0].name, "Python 3.14");
        assert_eq!(matches[0].version.as_deref(), Some("3.14.7"));
    }

    #[test]
    fn search_rows_are_capped_for_the_picker() {
        let rows: Vec<String> = (0..30)
            .map(|i| {
                row(
                    &format!("Package {i}"),
                    &format!("Pkg.Id.{i}"),
                    &format!("{i}.0"),
                    "",
                    "",
                )
            })
            .collect();
        let text = format!(
            "{}\n{}\n{}",
            row("Name", "Id", "Version", "Match", "Source"),
            "-".repeat(100),
            rows.join("\n")
        );
        let mut matches = parse_search_output(&text);
        assert_eq!(matches.len(), 30);
        matches.truncate(MAX_SEARCH_ROWS);
        assert_eq!(matches.len(), MAX_SEARCH_ROWS);
    }

    #[test]
    fn parses_show_output_real_shape() {
        let text = "\
Found DBeaver [DBeaver.DBeaver.Community]
Version: 26.1.4
Publisher: DBeaver Corp
Publisher Url: https://dbeaver.io/
Moniker: dbeaver
Description:
  DBeaver is free and open source universal database tool
Homepage: https://dbeaver.io/download/
";
        let show = parse_show_output(text).expect("show parses");
        assert_eq!(show.id, "DBeaver.DBeaver.Community");
        assert_eq!(show.name.as_deref(), Some("DBeaver"));
        assert_eq!(show.publisher.as_deref(), Some("DBeaver Corp"));
        assert_eq!(show.version.as_deref(), Some("26.1.4"));
        assert_eq!(show.moniker.as_deref(), Some("dbeaver"));
        assert_eq!(show.source, None);
    }

    #[test]
    fn show_without_found_line_is_none() {
        assert!(parse_show_output("").is_none());
        assert!(parse_show_output("Version: 1.0\nPublisher: X").is_none());
    }
}