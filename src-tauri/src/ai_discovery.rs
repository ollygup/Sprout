//! Scoped local target discovery for AI-assisted authoring (ADR-0031).
//!
//! Resolves requests such as "open X" against installed-app metadata and
//! user-approved folders, lets the user pick among ambiguous matches, and
//! binds the chosen path into a reviewed command. Discovery is an explicit,
//! bounded, read-only lookup: no background crawl, no content reading unless
//! separately requested, and no execution of anything found — a reference
//! grants no run authority (ADR-0030).
//!
//! Separation of grants, enforced by construction: finding names/paths never
//! reads bodies and never discloses anything to a provider. Reading contents
//! is a separate call, and approving raw fields for disclosure is a third one
//! whose record a later cloud path must consult before uploading (ADR-0031).
//! The model never receives paths here — only opaque request-scoped
//! references that trusted local code validates and quotes at bind time.
//!
//! Windows compatibility stays with its owners (ADR-0029): installed apps
//! come from the walker's snapshot, never a second enumeration, and binding
//! builds script text only — it never spawns a process, so no new execution
//! site is introduced.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use serde::Serialize;

use crate::quick_actions::QuickActionShell;

/// Bounds are documented implementation defaults with boundary tests, not
/// measured user commitments.
pub const MAX_QUERY_CHARS: usize = 200;
pub const MAX_RESULTS: usize = 25;
pub const MAX_READ_BYTES: usize = 8192;
const MAX_WALK_FILES: usize = 20_000;
const MAX_WALK_DEPTH: usize = 8;
const WALK_BUDGET: Duration = Duration::from_secs(5);
const SESSION_TTL: Duration = Duration::from_secs(15 * 60);
const MAX_SESSIONS: usize = 32;

/// The only raw fields a disclosure grant may name. Names stay fixed so a
/// later cloud path can match them exactly instead of inheriting filesystem
/// access as upload permission (ADR-0031).
pub const DISCLOSURE_FIELDS: [&str; 3] = ["name", "path", "contents"];

/// One approved discovery folder: the canonical path plus when it was
/// approved. Permission covers names/paths only (ADR-0031).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApprovedRoot {
    pub path: String,
    pub added_at: i64,
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Comparison form for stored roots and live paths: backslashes, lowercase
/// (Windows paths are case-insensitive), no trailing separator.
fn compare_key(value: &str) -> String {
    let mut key = value.replace('/', "\\").to_lowercase();
    while key.len() > 3 && key.ends_with('\\') {
        key.pop();
    }
    key
}

/// The canonical on-disk form of an existing path, for containment checks.
/// Both sides of every comparison pass through here, so the `\\?\`-prefixed
/// Windows form compares consistently.
fn canonical_existing(path: &Path) -> Result<String, String> {
    std::fs::canonicalize(path)
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| format!("Sprout could not open '{}' ({e}) — check it still exists.", path.display()))
}

/// Whether a canonical candidate lies inside a canonical root: equal, or
/// below it at a separator boundary (so `C:\Tools2` never matches `C:\Tools`).
fn is_within(canonical_candidate: &str, canonical_root: &str) -> bool {
    let candidate = compare_key(canonical_candidate);
    let root = compare_key(canonical_root);
    candidate == root || candidate.starts_with(&format!("{root}\\"))
}

/// A reparse point (symlink, junction, mount point): discovery never follows
/// one, so an approved folder cannot leak outside itself through a link
/// (ADR-0031). Rust reports every Windows reparse point here, not just
/// symlinks.
fn is_reparse(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(true)
}

/// Records one approved discovery folder. The path must name an existing
/// directory; it is stored canonicalized so later containment checks compare
/// like with like. Re-approving refreshes the timestamp.
pub fn approve_root(conn: &Connection, raw: &str) -> Result<ApprovedRoot, String> {
    let trimmed = raw.trim().trim_matches('"').trim();
    if trimmed.is_empty() {
        return Err("Pick a folder to approve — for example D:\\Tools.".into());
    }
    if !Path::new(trimmed).is_absolute() {
        return Err(format!(
            "'{trimmed}' is not a full path — approve a folder like D:\\Tools."
        ));
    }
    let canonical = canonical_existing(Path::new(trimmed))?;
    if !Path::new(&canonical).is_dir() {
        return Err("That is a file — approve the folder that holds it instead.".into());
    }
    let added_at = now_unix();
    crate::db::add_approved_root(conn, &canonical, added_at).map_err(|e| e.to_string())?;
    Ok(ApprovedRoot { path: canonical, added_at })
}

/// Forgets one approved discovery folder. Matches by canonical path when the
/// folder still exists, otherwise by normalized text so a deleted folder can
/// still be revoked. `true` when a row was removed.
pub fn revoke_root(conn: &Connection, raw: &str) -> Result<bool, String> {
    let trimmed = raw.trim().trim_matches('"').trim();
    if trimmed.is_empty() {
        return Err("Pick an approved folder to revoke.".into());
    }
    let stored = crate::db::list_approved_roots(conn).map_err(|e| e.to_string())?;
    let want = std::fs::canonicalize(trimmed)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| trimmed.to_string());
    let want = compare_key(&want);
    for (path, _) in &stored {
        if compare_key(path) == want {
            return crate::db::remove_approved_root(conn, path).map_err(|e| e.to_string());
        }
    }
    Ok(false)
}

/// Every approved discovery folder, in approval order.
pub fn list_roots(conn: &Connection) -> Result<Vec<ApprovedRoot>, String> {
    crate::db::list_approved_roots(conn)
        .map_err(|e| e.to_string())
        .map(|rows| {
            rows.into_iter()
                .map(|(path, added_at)| ApprovedRoot { path, added_at })
                .collect()
        })
}

/// What a match is: an installed app from the shared snapshot, or a file or
/// folder under an approved root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetKind {
    App,
    File,
    Folder,
}

/// One user-visible match. Carries names/paths for the user's own review —
/// never file contents, and never anything addressed to a model.
#[derive(Debug, Clone, Serialize)]
pub struct FoundTarget {
    pub ref_id: String,
    pub kind: TargetKind,
    pub name: String,
    pub path: String,
    pub publisher: Option<String>,
}

/// One find request's answer: the session its references belong to, the
/// bounded matches, whether more existed, and an honest notice when part of
/// the answer needs explaining (no match, unreadable folders, no roots yet).
#[derive(Debug, Clone, Serialize)]
pub struct FindOutcome {
    pub session_id: u64,
    pub matches: Vec<FoundTarget>,
    pub truncated: bool,
    pub notice: Option<String>,
}

#[derive(Debug, Clone)]
struct StoredTarget {
    kind: TargetKind,
    name: String,
    path: String,
    publisher: Option<String>,
}

/// Which raw fields of one reference the user approved for disclosure to a
/// provider. Recorded per find-request session; the cloud path must consult
/// this before uploading anything (ADR-0031). Local binding never needs it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DisclosureGrant {
    pub session_id: u64,
    pub ref_id: String,
    pub fields: Vec<String>,
}

struct Session {
    created: Instant,
    targets: Vec<StoredTarget>,
    disclosed: Vec<DisclosureGrant>,
}

static SESSIONS: OnceLock<Mutex<HashMap<u64, Session>>> = OnceLock::new();
static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);
static REF_SECRET: OnceLock<u64> = OnceLock::new();

fn sessions() -> &'static Mutex<HashMap<u64, Session>> {
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn locked() -> std::sync::MutexGuard<'static, HashMap<u64, Session>> {
    sessions().lock().unwrap_or_else(|e| e.into_inner())
}

/// Process-random tag key: references are unforgeable outside this process,
/// and meaningless after a restart (every session dies with the process).
fn ref_secret() -> u64 {
    *REF_SECRET.get_or_init(|| {
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9e3779b97f4a7c15);
        time.wrapping_mul(0x9e3779b97f4a7c15)
            .wrapping_add(std::process::id() as u64)
            .wrapping_add(sessions() as *const _ as u64)
    })
}

fn ref_tag(session: u64, index: usize) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for word in [ref_secret(), session, index as u64] {
        for byte in word.to_le_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    format!("{hash:016x}")
}

fn ref_id(session: u64, index: usize) -> String {
    format!("ait-{session}-{index}-{}", ref_tag(session, index))
}

fn purge_expired(map: &mut HashMap<u64, Session>) {
    map.retain(|_, session| session.created.elapsed() < SESSION_TTL);
    while map.len() > MAX_SESSIONS {
        if let Some(oldest) = map
            .iter()
            .min_by_key(|(_, session)| session.created)
            .map(|(id, _)| *id)
        {
            map.remove(&oldest);
        } else {
            break;
        }
    }
}

fn register(targets: Vec<StoredTarget>) -> u64 {
    let mut map = locked();
    purge_expired(&mut map);
    let id = NEXT_SESSION.fetch_add(1, Ordering::SeqCst);
    map.insert(
        id,
        Session { created: Instant::now(), targets, disclosed: Vec::new() },
    );
    id
}

/// Validates an opaque reference against the live session table and returns
/// the stored target — the stored path, never anything parsed from the
/// reference. Unknown, modified, cross-request, and stale references all fail
/// here, before any path is touched.
fn resolve_ref(ref_id: &str) -> Result<(u64, StoredTarget), String> {
    const UNKNOWN: &str = "That target is no longer available — run Find again and pick a fresh match.";
    let body = ref_id.strip_prefix("ait-").ok_or_else(|| {
        "That target reference is not recognized — run Find again and pick a fresh match.".to_string()
    })?;
    let mut parts = body.split('-');
    let (Some(session_text), Some(index_text), Some(tag), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err("That target reference is not recognized — run Find again and pick a fresh match.".to_string());
    };
    let session_id: u64 = session_text.parse().map_err(|_| UNKNOWN.to_string())?;
    let index: usize = index_text.parse().map_err(|_| UNKNOWN.to_string())?;
    if tag != ref_tag(session_id, index) {
        return Err("That target reference is not recognized — run Find again and pick a fresh match.".to_string());
    }
    let mut map = locked();
    if !map.contains_key(&session_id) {
        return Err(UNKNOWN.to_string());
    }
    if map.get(&session_id).is_some_and(|session| session.created.elapsed() >= SESSION_TTL) {
        map.remove(&session_id);
        return Err("That find request expired — run Find again and pick a fresh match.".to_string());
    }
    map.get(&session_id)
        .and_then(|session| session.targets.get(index).cloned())
        .map(|target| (session_id, target))
        .ok_or_else(|| UNKNOWN.to_string())
}

#[cfg(test)]
fn force_expire(session_id: u64) {
    let mut map = locked();
    if let Some(session) = map.get_mut(&session_id) {
        session.created = Instant::now() - SESSION_TTL - Duration::from_secs(1);
    }
}

/// Scores one installed-app candidate against the needle: exact name, name
/// prefix, name substring, target file-name substring, publisher substring.
/// `None` means no match at all — the query is literal text, never a path.
fn score_app(candidate: &crate::walker::Candidate, needle: &str) -> Option<(u8, StoredTarget)> {
    let name = candidate.name.to_lowercase();
    let rank = if name == needle {
        0
    } else if name.starts_with(needle) {
        1
    } else if name.contains(needle) {
        2
    } else {
        let file = Path::new(&candidate.target)
            .file_name()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if file.contains(needle) {
            3
        } else if candidate
            .publisher
            .as_deref()
            .unwrap_or_default()
            .to_lowercase()
            .contains(needle)
        {
            4
        } else {
            return None;
        }
    };
    Some((
        rank,
        StoredTarget {
            kind: TargetKind::App,
            name: candidate.name.clone(),
            path: candidate.exe_path.clone().unwrap_or_else(|| candidate.target.clone()),
            publisher: candidate.publisher.clone(),
        },
    ))
}

/// Searches installed-app metadata through the shared walker snapshot — the
/// one enumeration behind both the picker and this lookup (ADR-0029).
fn search_apps(candidates: &[crate::walker::Candidate], query: &str) -> Vec<StoredTarget> {
    let needle = query.to_lowercase();
    let mut scored: Vec<(u8, String, StoredTarget)> = candidates
        .iter()
        .filter_map(|c| score_app(c, &needle))
        .map(|(rank, target)| {
            let key = target.name.to_lowercase();
            (rank, key, target)
        })
        .collect();
    scored.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, _, target)| target).collect()
}

struct WalkHit {
    kind: TargetKind,
    name: String,
    path: String,
}

/// Walks approved roots matching entry names only — bodies are never opened.
/// Reparse points are never descended into nor reported, unreadable
/// directories are skipped, and item, depth, and time budgets bound the walk
/// outside any model's control (ADR-0031).
fn walk_roots(roots: &[String], needle: &str, limit: usize) -> (Vec<WalkHit>, Vec<String>, bool) {
    let mut hits = Vec::new();
    let mut unreadable = Vec::new();
    let mut truncated = false;
    let mut seen_files: usize = 0;
    let started = Instant::now();
    'roots: for root in roots {
        let canonical = match std::fs::canonicalize(root) {
            Ok(path) => path,
            Err(_) => {
                unreadable.push(root.clone());
                continue;
            }
        };
        let mut stack = vec![(canonical, 0usize)];
        while let Some((dir, depth)) = stack.pop() {
            if hits.len() >= limit || seen_files >= MAX_WALK_FILES || started.elapsed() >= WALK_BUDGET {
                truncated = true;
                break 'roots;
            }
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                seen_files += 1;
                if seen_files % 256 == 0 && started.elapsed() >= WALK_BUDGET {
                    truncated = true;
                    break 'roots;
                }
                if hits.len() >= limit || seen_files >= MAX_WALK_FILES {
                    truncated = true;
                    break 'roots;
                }
                let path = entry.path();
                if is_reparse(&path) {
                    continue;
                }
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                let name = path
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if name.to_lowercase().contains(needle) {
                    hits.push(WalkHit {
                        kind: if kind.is_dir() { TargetKind::Folder } else { TargetKind::File },
                        name,
                        path: path.to_string_lossy().into_owned(),
                    });
                    if hits.len() >= limit {
                        truncated = true;
                        break 'roots;
                    }
                } else if kind.is_dir() && depth < MAX_WALK_DEPTH {
                    stack.push((path, depth + 1));
                }
            }
        }
    }
    (hits, unreadable, truncated)
}

/// Runs one explicit find request: installed-app metadata and/or approved
/// folder names and paths. Returns request-scoped references, never raw
/// model input, and never reads a body or runs a target. The async command
/// uses [`find_with_roots`] (roots snapshot off-lock for the blocking pool);
/// this sync form is the exercised seam for tests.
#[allow(dead_code)]
pub fn find_targets(conn: &Connection, query: &str, scope: &str) -> Result<FindOutcome, String> {
    let roots = list_roots(conn)?;
    find_with_roots(&roots.iter().map(|r| r.path.clone()).collect::<Vec<_>>(), query, scope)
}

/// The blocking-pool core behind [`find_targets`]: the caller snapshots the
/// approved roots first, then the registry and filesystem walk runs off the
/// UI thread.
pub fn find_with_roots(roots: &[String], query: &str, scope: &str) -> Result<FindOutcome, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("Type what to look for — for example the app or file name.".into());
    }
    if query.chars().count() > MAX_QUERY_CHARS {
        return Err(format!(
            "That search is longer than the bounded {MAX_QUERY_CHARS} characters — shorten it and try again."
        ));
    }
    let search_apps_scope = match scope {
        "apps" => true,
        "files" => false,
        "both" => true,
        _ => return Err("Search scope must be \"apps\", \"files\", or \"both\".".into()),
    };
    let search_files_scope = scope != "apps";

    let mut stored: Vec<StoredTarget> = Vec::new();
    let mut notices: Vec<String> = Vec::new();
    let mut truncated = false;

    if search_apps_scope {
        stored.extend(search_apps(&crate::walker::snapshot(), query));
    }
    if search_files_scope {
        if roots.is_empty() {
            notices.push("No approved folders yet — approve one below to search files.".into());
        } else {
            let room = MAX_RESULTS.saturating_sub(stored.len().min(MAX_RESULTS));
            let (hits, unreadable, walked_out) = walk_roots(roots, &query.to_lowercase(), room.max(1));
            if walked_out {
                truncated = true;
            }
            for hit in hits {
                if stored.len() >= MAX_RESULTS {
                    truncated = true;
                    break;
                }
                stored.push(StoredTarget {
                    kind: hit.kind,
                    name: hit.name,
                    path: hit.path,
                    publisher: None,
                });
            }
            for root in unreadable {
                notices.push(format!("'{root}' could not be opened — it was skipped."));
            }
        }
    }

    if stored.len() > MAX_RESULTS {
        stored.truncate(MAX_RESULTS);
        truncated = true;
    }
    if stored.is_empty() && notices.is_empty() {
        notices.push(format!("No match for '{query}' — check the spelling or widen the scope."));
    }
    let session_id = register(stored.clone());
    let matches = stored
        .iter()
        .enumerate()
        .map(|(index, target)| FoundTarget {
            ref_id: ref_id(session_id, index),
            kind: target.kind,
            name: target.name.clone(),
            path: target.path.clone(),
            publisher: target.publisher.clone(),
        })
        .collect();
    Ok(FindOutcome {
        session_id,
        matches,
        truncated,
        notice: if notices.is_empty() { None } else { Some(notices.join(" ")) },
    })
}

/// Revalidates a file/folder target against the live machine: still a plain
/// entry (no link swapped in), still under an approved root, still the
/// expected kind. Anything else is stale, revoked, or refused — never bound.
fn revalidate_file(conn: &Connection, stored: &StoredTarget) -> Result<PathBuf, String> {
    let live = Path::new(&stored.path);
    // Existence first: a deleted target is stale, not a link. `exists`
    // follows links, so a dangling link also reads as gone — honest either
    // way, and nothing is followed. Only a live path reaches the reparse
    // check below (ADR-0031).
    if !live.exists() {
        return Err("That target is no longer there — run Find again and pick a fresh match.".into());
    }
    if is_reparse(live) {
        return Err("That path now leads through a link Sprout does not follow — run Find again.".into());
    }
    let canonical = canonical_existing(live).map_err(|_| {
        "That target is no longer there — run Find again and pick a fresh match.".to_string()
    })?;
    let roots = list_roots(conn)?;
    if !roots.iter().any(|root| is_within(&canonical, &root.path)) {
        return Err(
            "Its folder is no longer approved — approve the folder again, then run Find again.".into(),
        );
    }
    let path = PathBuf::from(&canonical);
    let is_dir = path.is_dir();
    match stored.kind {
        TargetKind::File if is_dir => Err("That target changed shape — run Find again.".into()),
        TargetKind::Folder if !is_dir => Err("That target is no longer there — run Find again.".into()),
        _ => Ok(path),
    }
}

/// Single-quotes a path for PowerShell: inside single quotes everything is
/// literal except the quote itself, which doubles.
fn quote_powershell(path: &str) -> String {
    format!("'{}'", path.replace('\'', "''"))
}

/// Double-quotes a path for CMD: inside quotes the shell metacharacters are
/// literal, while `%` still expands — so it doubles. An embedded `"` cannot
/// be quoted honestly and is refused (Windows file names cannot contain one,
/// so a live path never does — only an injected name could).
fn quote_cmd(path: &str) -> Result<String, String> {
    if path.contains('"') {
        return Err("That name contains a quote Sprout cannot bind for cmd — pick another target.".into());
    }
    Ok(format!("\"{}\"", path.replace('%', "%%")))
}

/// Builds the shell command that opens a validated target. Pure text
/// building — it never runs anything (ADR-0029, ADR-0030).
fn command_for(shell: QuickActionShell, kind: TargetKind, path: &str) -> Result<String, String> {
    match (shell, kind) {
        (QuickActionShell::Powershell, TargetKind::App)
        | (QuickActionShell::Powershell, TargetKind::File) => {
            Ok(format!("Start-Process -FilePath {}", quote_powershell(path)))
        }
        (QuickActionShell::Powershell, TargetKind::Folder) => {
            Ok(format!("Invoke-Item {}", quote_powershell(path)))
        }
        (QuickActionShell::Cmd, TargetKind::App) | (QuickActionShell::Cmd, TargetKind::File) => {
            Ok(format!("start \"\" {}", quote_cmd(path)?))
        }
        (QuickActionShell::Cmd, TargetKind::Folder) => Ok(format!("explorer {}", quote_cmd(path)?)),
    }
}

/// The locally bound command for one validated reference.
#[derive(Debug, Clone, Serialize)]
pub struct BoundCommand {
    pub ref_id: String,
    pub shell: QuickActionShell,
    pub command: String,
    /// The actual target shown to the user before saving — review decides.
    pub target: String,
    /// The output-check verdict on the bound text, when it is anything but
    /// allow: the command still returns (the user picked a real file), but
    /// the warning travels with it for review.
    pub warning: Option<String>,
}

/// Binds one reference to a shell-quoted command. Every validation runs in
/// trusted local code: the reference must be live, the target must still
/// exist in its approved scope, and the final text passes the same output
/// checks as a generated draft. Binding never executes the target — not even
/// to identify it (ADR-0030).
pub fn bind_target(
    conn: &Connection,
    ref_id: &str,
    shell: QuickActionShell,
) -> Result<BoundCommand, String> {
    let (_, stored) = resolve_ref(ref_id)?;
    match stored.kind {
        TargetKind::App => {
            if !Path::new(&stored.path).exists() {
                return Err("That target is no longer there — run Find again and pick a fresh match.".into());
            }
        }
        TargetKind::File | TargetKind::Folder => {
            revalidate_file(conn, &stored)?;
        }
    }
    let command = command_for(shell, stored.kind, &stored.path)?;
    let warning = match crate::ai_assist::check_output(shell, &command) {
        crate::ai_assist::OutputVerdict::Allow => None,
        crate::ai_assist::OutputVerdict::Refuse { reason } => Some(reason),
        crate::ai_assist::OutputVerdict::Clarify { message } => Some(message),
    };
    Ok(BoundCommand {
        ref_id: ref_id.to_string(),
        shell,
        command,
        target: stored.path,
        warning,
    })
}

/// A separately requested file preview: bounded, labeled untrusted, and only
/// for live file targets inside still-approved roots. Finding never calls
/// this on anyone's behalf.
#[derive(Debug, Clone, Serialize)]
pub struct FileContent {
    pub ref_id: String,
    pub path: String,
    pub content: String,
    pub truncated: bool,
    pub bytes: u64,
    /// Found content stays untrusted input even after an explicit read
    /// (ADR-0031): previewing never authorizes disclosure or execution.
    pub untrusted: bool,
}

pub fn read_target_file(conn: &Connection, ref_id: &str) -> Result<FileContent, String> {
    let (_, stored) = resolve_ref(ref_id)?;
    if stored.kind != TargetKind::File {
        return Err("Only file matches can be previewed — folders open in Explorer instead.".into());
    }
    let path = revalidate_file(conn, &stored)?;
    let file = std::fs::File::open(&path).map_err(|_| {
        "That file could not be opened — run Find again and pick a fresh match.".to_string()
    })?;
    let bytes = file.metadata().map(|m| m.len()).unwrap_or(0);
    let mut capped = std::io::Read::take(file, MAX_READ_BYTES as u64 + 1);
    let mut buf = Vec::new();
    std::io::Read::read_to_end(&mut capped, &mut buf).map_err(|_| {
        "That file could not be read — it was left untouched.".to_string()
    })?;
    let truncated = buf.len() as u64 > MAX_READ_BYTES as u64;
    buf.truncate(MAX_READ_BYTES);
    Ok(FileContent {
        ref_id: ref_id.to_string(),
        path: stored.path,
        content: String::from_utf8_lossy(&buf).into_owned(),
        truncated,
        bytes,
        untrusted: true,
    })
}

/// Records that the user approved disclosing raw fields of one reference to
/// a provider. Discovery never implies this — only this call grants it — and
/// the grant dies with its find-request session (ADR-0031).
pub fn approve_disclosure(ref_id: &str, fields: &[String]) -> Result<DisclosureGrant, String> {
    if fields.is_empty() {
        return Err("Choose what may be disclosed — name, path, or contents.".into());
    }
    for field in fields {
        if !DISCLOSURE_FIELDS.contains(&field.as_str()) {
            return Err("Disclosure covers name, path, or contents only.".into());
        }
    }
    let (session_id, _) = resolve_ref(ref_id)?;
    let mut map = locked();
    let Some(session) = map.get_mut(&session_id) else {
        return Err("That find request expired — run Find again.".into());
    };
    let mut ordered: Vec<String> = DISCLOSURE_FIELDS
        .iter()
        .map(|s| s.to_string())
        .filter(|f| fields.iter().any(|g| g == f))
        .collect();
    if let Some(existing) = session.disclosed.iter_mut().find(|g| g.ref_id == ref_id) {
        for field in ordered.drain(..) {
            if !existing.fields.contains(&field) {
                existing.fields.push(field);
            }
        }
        existing.fields.sort_by_key(|f| DISCLOSURE_FIELDS.iter().position(|d| d == f));
        return Ok(existing.clone());
    }
    let grant = DisclosureGrant { session_id, ref_id: ref_id.to_string(), fields: ordered };
    session.disclosed.push(grant.clone());
    Ok(grant)
}

/// The raw-field approvals recorded against one find request: what a later
/// cloud path may upload, and nothing more (ADR-0031).
pub fn disclosure_grants(session_id: u64) -> Result<Vec<DisclosureGrant>, String> {
    let map = locked();
    map.get(&session_id)
        .map(|session| session.disclosed.clone())
        .ok_or_else(|| "That find request expired — run Find again.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        tempfile::tempdir().unwrap().into_path()
    }

    fn conn_in(dir: &Path) -> Connection {
        crate::db::init_at(&dir.to_path_buf()).unwrap()
    }

    fn app_candidate(name: &str, target: &str, exe: Option<&str>) -> crate::walker::Candidate {
        crate::walker::Candidate {
            name: name.to_string(),
            publisher: None,
            target: target.to_string(),
            exe_path: exe.map(str::to_string),
        }
    }

    fn write_file(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, format!("body of {name}")).unwrap();
        path
    }

    #[test]
    fn duplicate_app_names_keep_distinct_references() {
        let candidates = vec![
            app_candidate("Terminal", r"C:\A\term.lnk", Some(r"C:\Apps\TermA\term.exe")),
            app_candidate("Terminal", r"C:\B\term.lnk", Some(r"C:\Apps\TermB\term.exe")),
        ];
        let hits = search_apps(&candidates, "terminal");
        assert_eq!(hits.len(), 2);
        let id = register(hits);
        let first = resolve_ref(&ref_id(id, 0)).unwrap().1;
        let second = resolve_ref(&ref_id(id, 1)).unwrap().1;
        assert_ne!(first.path, second.path);
        assert_ne!(ref_id(id, 0), ref_id(id, 1));
    }

    #[test]
    fn app_search_ranks_exact_before_substring() {
        let candidates = vec![
            app_candidate("My Terminal Emulator", r"C:\x\a.lnk", None),
            app_candidate("Terminal", r"C:\x\b.lnk", None),
        ];
        let hits = search_apps(&candidates, "terminal");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].name, "Terminal");
    }

    #[test]
    fn quoting_covers_spaces_quotes_metacharacters_and_unicode() {
        assert_eq!(quote_powershell(r"C:\My Apps\run me.exe"), r"'C:\My Apps\run me.exe'");
        assert_eq!(quote_powershell("C:\\it's\\app.exe"), "'C:\\it''s\\app.exe'");
        assert_eq!(
            quote_powershell("C:\\桌面\\файл & co | test ` $ ^.exe"),
            "'C:\\桌面\\файл & co | test ` $ ^.exe'"
        );
        assert_eq!(quote_cmd(r"C:\My Apps\run me.exe").unwrap(), r#""C:\My Apps\run me.exe""#);
        assert_eq!(quote_cmd("C:\\100%\\app & co.exe").unwrap(), r#""C:\100%%\app & co.exe""#);
        assert_eq!(
            quote_cmd("C:\\桌面\\файл.exe").unwrap(),
            "\"C:\\桌面\\файл.exe\""
        );
        assert!(quote_cmd("C:\\evil\"quote\\app.exe").is_err());
    }

    #[test]
    fn bound_shapes_open_targets_without_executing_them() {
        let ps = command_for(QuickActionShell::Powershell, TargetKind::App, r"C:\A\app.exe").unwrap();
        assert_eq!(ps, "Start-Process -FilePath 'C:\\A\\app.exe'");
        let ps_dir = command_for(QuickActionShell::Powershell, TargetKind::Folder, r"D:\Work").unwrap();
        assert_eq!(ps_dir, "Invoke-Item 'D:\\Work'");
        let cmd = command_for(QuickActionShell::Cmd, TargetKind::File, r"C:\My Docs\n.exe").unwrap();
        assert_eq!(cmd, "start \"\" \"C:\\My Docs\\n.exe\"");
        let cmd_dir = command_for(QuickActionShell::Cmd, TargetKind::Folder, r"D:\Work").unwrap();
        assert_eq!(cmd_dir, "explorer \"D:\\Work\"");
    }

    #[test]
    fn dotdot_escape_never_counts_as_contained() {
        let root = temp_dir();
        let root_canon = std::fs::canonicalize(&root).unwrap().to_string_lossy().into_owned();
        let escaped = std::fs::canonicalize(root.join(".."))
            .unwrap()
            .join("evil")
            .to_string_lossy()
            .into_owned();
        assert!(!is_within(&escaped, &root_canon));
        assert!(is_within(&root_canon, &root_canon));
        assert!(!is_within(&format!("{root_canon}2"), &root_canon));
    }

    #[test]
    fn plain_files_are_not_reparse_points() {
        let root = temp_dir();
        let file = write_file(&root, "plain.txt");
        assert!(!is_reparse(&file));
        assert!(!is_reparse(&root));
    }

    #[test]
    fn symlink_escape_is_skipped_when_creatable() {
        let root = temp_dir();
        let outside = temp_dir();
        write_file(&outside, "secret-match.txt");
        write_file(&root, "local-match.txt");
        let _ = &outside;
        #[cfg(windows)]
        {
            let link = root.join("link-out");
            if std::os::windows::fs::symlink_dir(&outside, &link).is_err() {
                return;
            }
            assert!(is_reparse(&link));
            let roots = vec![root.to_string_lossy().into_owned()];
            let (hits, _, _) = walk_roots(&roots, "match", 25);
            assert!(hits.iter().any(|h| h.name == "local-match.txt"));
            assert!(!hits.iter().any(|h| h.name == "secret-match.txt"));
        }
    }

    #[test]
    fn approve_validates_before_recording() {
        let dir = temp_dir();
        let conn = conn_in(&dir);
        assert!(approve_root(&conn, "relative\\path").is_err());
        assert!(approve_root(&conn, "   ").is_err());
        let missing = dir.join("gone");
        assert!(approve_root(&conn, missing.to_str().unwrap()).is_err());
        let file = write_file(&dir, "note.txt");
        assert!(approve_root(&conn, file.to_str().unwrap()).is_err());
        let approved = approve_root(&conn, dir.to_str().unwrap()).unwrap();
        assert_eq!(list_roots(&conn).unwrap().len(), 1);
        assert!(revoke_root(&conn, &approved.path).unwrap());
        assert!(!revoke_root(&conn, &approved.path).unwrap());
    }

    #[test]
    fn revoked_roots_block_binding() {
        let dir = temp_dir();
        let conn = conn_in(&dir);
        let root = temp_dir();
        write_file(&root, "target-app.exe");
        approve_root(&conn, root.to_str().unwrap()).unwrap();
        let found = find_targets(&conn, "target-app", "files").unwrap();
        assert_eq!(found.matches.len(), 1);
        let reference = found.matches[0].ref_id.clone();
        assert!(revoke_root(&conn, root.to_str().unwrap()).unwrap());
        let err = bind_target(&conn, &reference, QuickActionShell::Powershell).unwrap_err();
        assert!(err.contains("no longer approved"), "{err}");
    }

    #[test]
    fn truncated_result_sets_stay_bounded() {
        let dir = temp_dir();
        let conn = conn_in(&dir);
        let root = temp_dir();
        for i in 0..40 {
            write_file(&root, &format!("match-{i:02}.txt"));
        }
        approve_root(&conn, root.to_str().unwrap()).unwrap();
        let found = find_targets(&conn, "match-", "files").unwrap();
        assert_eq!(found.matches.len(), MAX_RESULTS);
        assert!(found.truncated);
        assert!(found.notice.is_none() || !found.notice.unwrap().contains("No match"));
    }

    #[test]
    fn stale_targets_fail_honestly() {
        let dir = temp_dir();
        let conn = conn_in(&dir);
        let root = temp_dir();
        write_file(&root, "here-today.txt");
        approve_root(&conn, root.to_str().unwrap()).unwrap();
        let found = find_targets(&conn, "here-today", "files").unwrap();
        let reference = found.matches[0].ref_id.clone();
        std::fs::remove_file(root.join("here-today.txt")).unwrap();
        let err = bind_target(&conn, &reference, QuickActionShell::Powershell).unwrap_err();
        assert!(err.contains("no longer there"), "{err}");
        assert!(read_target_file(&conn, &reference).is_err());
        force_expire(found.session_id);
        let err = bind_target(&conn, &reference, QuickActionShell::Powershell).unwrap_err();
        assert!(err.contains("expired"), "{err}");
    }

    #[test]
    fn forged_foreign_and_cross_session_references_are_refused() {
        let id = register(vec![StoredTarget {
            kind: TargetKind::File,
            name: "a.txt".into(),
            path: r"C:\Roots\a.txt".into(),
            publisher: None,
        }]);
        assert!(resolve_ref("not-a-reference").is_err());
        assert!(resolve_ref(&format!("ait-{id}-0-deadbeefdeadbeef")).is_err());
        let valid_other_session = ref_tag(999_999, 0);
        assert!(resolve_ref(&format!("ait-999999-0-{valid_other_session}")).is_err());
        let valid_tag_wrong_index = ref_tag(id, 7);
        assert!(resolve_ref(&format!("ait-{id}-7-{valid_tag_wrong_index}")).is_err());
        assert!(resolve_ref(&ref_id(id, 0)).is_ok());
    }

    #[test]
    fn queries_are_literal_text_never_paths() {
        let dir = temp_dir();
        let conn = conn_in(&dir);
        let root = temp_dir();
        write_file(&root, "notes.txt");
        approve_root(&conn, root.to_str().unwrap()).unwrap();
        let found = find_targets(&conn, "..\\..", "files").unwrap();
        assert!(found.matches.is_empty());
        assert!(find_targets(&conn, "   ", "files").is_err());
        assert!(find_targets(&conn, "x", "everywhere").is_err());
    }

    #[test]
    fn no_match_is_honest_not_fabricated() {
        let dir = temp_dir();
        let conn = conn_in(&dir);
        let found = find_targets(&conn, "no-such-app-xyz", "apps").unwrap();
        assert!(found.matches.is_empty());
        assert!(!found.truncated);
        let notice = found.notice.expect("a no-match notice");
        assert!(notice.contains("No match"), "{notice}");
    }

    #[test]
    fn disclosure_needs_its_own_grant_and_leaves_binding_alone() {
        let dir = temp_dir();
        let conn = conn_in(&dir);
        let root = temp_dir();
        write_file(&root, "report.txt");
        approve_root(&conn, root.to_str().unwrap()).unwrap();
        let found = find_targets(&conn, "report", "files").unwrap();
        let reference = found.matches[0].ref_id.clone();
        assert!(disclosure_grants(found.session_id).unwrap().is_empty());
        assert!(approve_disclosure(&reference, &[]).is_err());
        assert!(approve_disclosure(&reference, &["everything".to_string()]).is_err());
        let grant = approve_disclosure(&reference, &["contents".to_string(), "path".to_string()]).unwrap();
        assert_eq!(grant.fields, vec!["path".to_string(), "contents".to_string()]);
        assert_eq!(disclosure_grants(found.session_id).unwrap().len(), 1);
        assert!(bind_target(&conn, &reference, QuickActionShell::Cmd).is_ok());
    }

    #[test]
    fn reading_contents_is_separate_bounded_and_untrusted() {
        let dir = temp_dir();
        let conn = conn_in(&dir);
        let root = temp_dir();
        std::fs::write(root.join("big.txt"), "z".repeat(MAX_READ_BYTES + 100)).unwrap();
        approve_root(&conn, root.to_str().unwrap()).unwrap();
        let found = find_targets(&conn, "big", "files").unwrap();
        let preview = read_target_file(&conn, &found.matches[0].ref_id).unwrap();
        assert!(preview.truncated);
        assert!(preview.untrusted);
        assert_eq!(preview.content.len(), MAX_READ_BYTES);
        let sub = root.join("docs-box");
        std::fs::create_dir_all(&sub).unwrap();
        let folder = find_targets(&conn, "docs-box", "files").unwrap();
        let folder_ref = folder
            .matches
            .iter()
            .find(|m| m.kind == TargetKind::Folder)
            .expect("the subfolder matches by name");
        assert!(read_target_file(&conn, &folder_ref.ref_id).is_err());
    }

    #[test]
    fn bound_commands_validate_with_warnings_not_fabrication() {
        let dir = temp_dir();
        let conn = conn_in(&dir);
        let root = temp_dir();
        let tricky = root.join("passwords");
        std::fs::create_dir_all(&tricky).unwrap();
        std::fs::write(tricky.join("vault.exe"), "binary").unwrap();
        write_file(&root, "plain.exe");
        approve_root(&conn, root.to_str().unwrap()).unwrap();
        let found = find_targets(&conn, ".exe", "files").unwrap();
        assert_eq!(found.matches.len(), 2);
        for matched in &found.matches {
            let bound = bind_target(&conn, &matched.ref_id, QuickActionShell::Powershell).unwrap();
            assert_eq!(bound.target, matched.path);
            assert!(bound.command.contains(&quote_powershell(&matched.path)));
            if matched.path.contains("passwords") {
                assert!(bound.warning.is_some(), "the output check flags the sensitive word");
            } else {
                assert!(bound.warning.is_none(), "plain paths bind cleanly");
            }
        }
    }

    #[test]
    fn injected_names_cannot_become_code() {
        // Single-quote doubling keeps a hostile name inside one PowerShell
        // string: the exact doubled form, still wrapped in one pair.
        let hostile = "x'; Remove-Item C:\\* -Recurse -Force; '";
        assert_eq!(quote_powershell(hostile), format!("'{}'", hostile.replace('\'', "''")));
        // CMD metacharacters stay inside the double quotes.
        assert_eq!(
            command_for(QuickActionShell::Cmd, TargetKind::File, "a & del C:\\*").unwrap(),
            "start \"\" \"a & del C:\\*\""
        );
        assert!(resolve_ref("ait-1-0-'; Remove-Item C:\\*").is_err());
    }
}
