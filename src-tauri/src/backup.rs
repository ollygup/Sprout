//! Whole-app backup: Settings → Backup (Export… / Restore…).
//!
//! One versioned, kind-tagged JSON document carries every content
//! collection — Products, Presets (with their requirement snapshots),
//! Launch entries, Quick Actions, and Clips — so a machine can be moved or
//! restored without re-authoring anything. The restore is a merging import:
//! records whose identity already exists locally are skipped and counted,
//! never overwritten, and the whole merge lands in one transaction (a
//! failure halfway leaves nothing behind).
//!
//! Exports may be selective (ticket 87): the user picks the collections in
//! the export dialog, and unchecked ones are written as empty arrays in the
//! SAME document format — there is no separate partial file type, because
//! exported files circulate and a format split is irreversible once users
//! hold files (ADR-0014). A partial file therefore restores through the
//! ordinary flow with true counts.
//!
//! Machine-scoped state stays local by design: run history, logs, the
//! Settings knobs, and the dock's per-monitor memory are never read into the
//! document, and every install directory is stripped on the way out AND on
//! the way in (ADR-0009) — a shared file never carries another machine's
//! paths.
//!
//! Record identities used by the merge: Products and Presets keep their
//! Library ids; Launch entries and Quick Actions are identified by name
//! (their rowids are storage internals, meaningless across machines); a
//! text Clip is identified by its text — the content is what a copy
//! restores — while an image Clip (ticket 178) is identified by the FNV-1a
//! hash over its bytes, its name display-only like the other payload-backed
//! lists.

use std::collections::{HashMap, HashSet};
use std::fs;

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::clips::{self, ClipInput};
use crate::db;
use crate::domain::{PresetRecord, Product};
use crate::launch::{self, LaunchEntryInput};
use crate::quick_actions::{self, QuickActionInput};

/// Which content collections an export includes (ticket 87). Unchecked
/// collections are written as empty arrays, so a partial export is the same
/// document as a whole-app one and restores through the same flow
/// (ADR-0014).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct BackupSelection {
    pub products: bool,
    pub presets: bool,
    pub launch_entries: bool,
    pub quick_actions: bool,
    pub clips: bool,
}

impl BackupSelection {
    /// Every collection included — what Export produced before ticket 87.
    /// Test-only: production always receives the frontend's explicit picks.
    #[cfg(test)]
    pub fn all() -> Self {
        Self {
            products: true,
            presets: true,
            launch_entries: true,
            quick_actions: true,
            clips: true,
        }
    }

    fn any(&self) -> bool {
        self.products
            || self.presets
            || self.launch_entries
            || self.quick_actions
            || self.clips
    }
}

/// The document's kind tag — a file without it is not a Sprout backup (a
/// `.sprout.json` Preset, for one, is rejected here with its own message).
pub const BACKUP_KIND: &str = "sprout-backup";

/// The document version this build writes. Reads accept version 1 as legacy
/// PowerShell actions plus version 2 (ADR-0014 shell-aware evolution).
pub const BACKUP_VERSION: u32 = 2;
/// The last legacy version: its Quick Actions carry no shell and read back as
/// PowerShell. A version-1 record that declares CMD is inconsistent and
/// rejected rather than silently reinterpreted.
pub const BACKUP_LEGACY_VERSION: u32 = 1;

/// The whole-app backup document. One array per content collection; the
/// collections reuse the stored domain types, so Preset requirement
/// snapshots survive intact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupDocument {
    pub kind: String,
    pub version: u32,
    /// Unix seconds when the file was written — provenance only, never read
    /// back as data.
    pub exported_at: i64,
    pub products: Vec<Product>,
    pub presets: Vec<PresetRecord>,
    pub launch_entries: Vec<LaunchEntryInput>,
    pub quick_actions: Vec<QuickActionInput>,
    /// Attached action files, additive since the files round (ADR-0014: one
    /// evolving document, never a second format). `action` indexes the
    /// `quick_actions` array of the same document; legacy files without this
    /// array read back as fileless. Whole-app JSON carries the bytes inline;
    /// a single-action zip carries only the metadata here with the raw bytes
    /// under `files/` in the same bundle.
    #[serde(default)]
    pub quick_action_files: Vec<QuickActionFileBackup>,
    pub clips: Vec<ClipInput>,
}

/// One attached file as backed up: which action of the same document owns it,
/// under which name, with the content. `bytes_b64` is `None` only in a zip
/// bundle's `action.json`, where the same bytes ride as raw `files/`
/// entries; everywhere else it carries the base64 content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickActionFileBackup {
    pub action: usize,
    pub filename: String,
    pub size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes_b64: Option<String>,
}

/// Per-collection item counts — what an export wrote, what a file contains,
/// and (as two of these) what a restore inserted versus skipped.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct BackupCounts {
    pub products: usize,
    pub presets: usize,
    pub launch_entries: usize,
    pub quick_actions: usize,
    pub clips: usize,
}

/// A restore's outcome: how many items each collection gained and how many
/// were skipped because their identity already exists locally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ImportSummary {
    pub inserted: BackupCounts,
    pub skipped: BackupCounts,
}

impl BackupDocument {
    fn counts(&self) -> BackupCounts {
        BackupCounts {
            products: self.products.len(),
            presets: self.presets.len(),
            launch_entries: self.launch_entries.len(),
            quick_actions: self.quick_actions.len(),
            clips: self.clips.len(),
        }
    }
}

/// Writes the backup of `conn`'s content to `path`, limited to the
/// collections `selection` includes, and returns the per-collection counts
/// for the success notice. The document is the unchanged whole-app shape —
/// unselected collections are simply empty arrays (ADR-0014).
pub fn export_backup(
    conn: &Connection,
    path: &str,
    selection: &BackupSelection,
) -> Result<BackupCounts, String> {
    if !selection.any() {
        return Err("Pick at least one collection to export.".into());
    }

    let products = if selection.products {
        db::list_products(conn, None)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|record| record.product)
            .collect()
    } else {
        Vec::new()
    };
    let presets = if selection.presets {
        db::list_presets(conn).map_err(|e| e.to_string())?
    } else {
        Vec::new()
    };
    let launch_entries = if selection.launch_entries {
        launch::list_launch_entries(conn)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|entry| entry.entry)
            .collect()
    } else {
        Vec::new()
    };
    let stored_actions = if selection.quick_actions {
        quick_actions::list_quick_actions(conn).map_err(|e| e.to_string())?
    } else {
        Vec::new()
    };
    let mut quick_action_files = Vec::new();
    for (index, stored) in stored_actions.iter().enumerate() {
        for (filename, bytes) in
            quick_actions::list_quick_action_file_blobs(conn, stored.id).map_err(|e| e.to_string())?
        {
            quick_action_files.push(QuickActionFileBackup {
                action: index,
                filename,
                size: bytes.len() as u64,
                bytes_b64: Some(B64.encode(&bytes)),
            });
        }
    }
    let quick_actions = stored_actions
        .into_iter()
        .map(|action| action.action)
        .collect();
    let clips = if selection.clips {
        clips::list_clips(conn)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|clip| clip.clip)
            .collect()
    } else {
        Vec::new()
    };

    let mut doc = BackupDocument {
        kind: BACKUP_KIND.into(),
        version: BACKUP_VERSION,
        exported_at: db::now_ts(),
        products,
        presets,
        launch_entries,
        quick_actions,
        quick_action_files,
        clips,
    };
    // The portable form is applied before anything touches disk: machine-
    // local install directories never leave this PC (ADR-0009).
    normalize(&mut doc);
    let json = serde_json::to_string_pretty(&doc)
        .map_err(|e| format!("Could not serialize the backup: {e}"))?;
    fs::write(path, json).map_err(|e| format!("Could not write '{path}': {e}"))?;
    Ok(doc.counts())
}

/// Writes one Quick Action to `path` as the unchanged whole-app document —
/// a one-element `quick_actions` array with four empty siblings — so the
/// file restores through the ordinary merge with honest counts (ADR-0014
/// one-format rule). Identity stays shell+command+cwd plus the attached file
/// names (ADR-0026): restoring skips when the same payload already exists
/// under any name. Fileless actions write the same JSON as always; an action
/// with files writes a zip bundle holding that same JSON as `action.json`
/// (file metadata only) plus the raw bytes under `files/`.
pub fn export_quick_action(conn: &Connection, path: &str, id: i64) -> Result<BackupCounts, String> {
    let stored = quick_actions::get_quick_action(conn, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "That quick action is gone — refresh and try again.".to_string())?;
    let blobs =
        quick_actions::list_quick_action_file_blobs(conn, id).map_err(|e| e.to_string())?;
    if blobs.is_empty() {
        let mut doc = BackupDocument {
            kind: BACKUP_KIND.into(),
            version: BACKUP_VERSION,
            exported_at: db::now_ts(),
            products: Vec::new(),
            presets: Vec::new(),
            launch_entries: Vec::new(),
            quick_actions: vec![stored.action],
            quick_action_files: Vec::new(),
            clips: Vec::new(),
        };
        // Same portable form as the whole-app path, so a shared file never
        // carries another machine's paths.
        normalize(&mut doc);
        let json = serde_json::to_string_pretty(&doc)
            .map_err(|e| format!("Could not serialize the backup: {e}"))?;
        fs::write(path, json).map_err(|e| format!("Could not write '{path}': {e}"))?;
        return Ok(doc.counts());
    }
    let mut files_meta = Vec::new();
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    for (filename, bytes) in &blobs {
        files_meta.push(QuickActionFileBackup {
            action: 0,
            filename: filename.clone(),
            size: bytes.len() as u64,
            bytes_b64: None,
        });
        entries.push((format!("{ZIP_FILES_PREFIX}{filename}"), bytes.clone()));
    }
    let mut doc = BackupDocument {
        kind: BACKUP_KIND.into(),
        version: BACKUP_VERSION,
        exported_at: db::now_ts(),
        products: Vec::new(),
        presets: Vec::new(),
        launch_entries: Vec::new(),
        quick_actions: vec![stored.action],
        quick_action_files: files_meta,
        clips: Vec::new(),
    };
    normalize(&mut doc);
    let json = serde_json::to_vec_pretty(&doc)
        .map_err(|e| format!("Could not serialize the backup: {e}"))?;
    entries.insert(0, (ZIP_ENTRY_ACTION_JSON.into(), json));
    let zip = write_zip(&entries);
    fs::write(path, zip).map_err(|e| format!("Could not write '{path}': {e}"))?;
    Ok(doc.counts())
}

/// Parses `path` and reports what a restore would write — the parsed counts
/// behind the confirmation dialog. Nothing is read from or written to the
/// database.
pub fn inspect_backup(path: &str) -> Result<BackupCounts, String> {
    let doc = read_document(path)?;
    validate_records(&doc)?;
    Ok(doc.counts())
}

/// Restores `path` into `conn`: parse → validate → transactional merge that
/// skips identities which already exist. Returns {inserted, skipped} per
/// collection for the summary notice.
pub fn import_backup(conn: &Connection, path: &str) -> Result<ImportSummary, String> {
    let doc = read_document(path)?;
    validate_records(&doc)?;
    merge(conn, &doc)
}

/// Reads and shape-checks a backup file, returning it in portable form:
/// wrong files (junk, `.sprout.json` presets, future versions) are rejected
/// with authored messages mirroring the preset-import behavior. Version 1
/// reads as legacy PowerShell actions; a version-1 CMD declaration is
/// rejected rather than silently run as PowerShell (ADR-0014). Version 2
/// requires an explicit valid shell on every Quick Action (ADR-0017). A zip
/// bundle (single-action export with files) is detected by its magic bytes
/// regardless of extension and restores through the same merge.
fn read_document(path: &str) -> Result<BackupDocument, String> {
    let raw = fs::read(path).map_err(|e| format!("Could not read '{path}': {e}"))?;
    if is_zip_magic(&raw) {
        return document_from_zip(path, &raw);
    }
    let text =
        String::from_utf8(raw).map_err(|e| format!("Could not read '{path}': {e}"))?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|_| format!("'{path}' is not a Sprout backup file"))?;
    document_from_json_value(path, value)
}

/// The shared JSON shape check behind plain files and zip bundles: kind tag,
/// version evolution, then the typed document in portable form.
fn document_from_json_value(
    path: &str,
    mut value: serde_json::Value,
) -> Result<BackupDocument, String> {
    if value.get("kind").and_then(|k| k.as_str()) != Some(BACKUP_KIND) {
        return Err(format!(
            "'{path}' is not a Sprout backup file — a .sprout.json preset is not a whole-app backup"
        ));
    }
    match value.get("version").and_then(|v| v.as_u64()) {
        Some(v) if v == BACKUP_VERSION as u64 => {}
        Some(v) if v == BACKUP_LEGACY_VERSION as u64 => {
            apply_legacy_shell_defaults(&mut value, path)?;
        }
        Some(other) => {
            return Err(format!(
                "Unsupported backup version {other} — only versions {BACKUP_LEGACY_VERSION} and {BACKUP_VERSION} are supported"
            ))
        }
        None => return Err(format!("'{path}' is not a valid Sprout backup")),
    }
    let mut doc: BackupDocument = serde_json::from_value(value)
        .map_err(|e| format!("'{path}' is not a valid Sprout backup: {e}"))?;
    normalize(&mut doc);
    Ok(doc)
}

/// Reads a single-action zip bundle: `action.json` carries the same envelope
/// with file metadata, `files/` carries the raw bytes. Entry names are
/// sanitized — anything outside `action.json` and plain `files/` basenames
/// (traversal, absolute paths, nested folders) fails before the merge writes.
fn document_from_zip(path: &str, raw: &[u8]) -> Result<BackupDocument, String> {
    let entries = read_zip(raw)
        .map_err(|e| format!("'{path}' is not a valid action-files bundle: {e}"))?;
    let json_bytes = entries
        .iter()
        .find(|(name, _)| name == ZIP_ENTRY_ACTION_JSON)
        .map(|(_, bytes)| bytes)
        .ok_or_else(|| format!("'{path}' is not a valid action-files bundle: missing action.json"))?;
    let text = String::from_utf8(json_bytes.clone())
        .map_err(|_| format!("'{path}' is not a valid action-files bundle: action.json is not text"))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|_| format!("'{path}' is not a Sprout backup file"))?;
    let mut doc = document_from_json_value(path, value)?;
    let mut raw_by_name: HashMap<String, Vec<u8>> = HashMap::new();
    for (name, bytes) in &entries {
        if name == ZIP_ENTRY_ACTION_JSON {
            continue;
        }
        let Some(basename) = name.strip_prefix(ZIP_FILES_PREFIX) else {
            return Err(format!(
                "'{path}' is not a valid action-files bundle: unexpected entry '{name}'"
            ));
        };
        let clean = quick_actions::validate_action_filename(basename).map_err(|_| {
            format!("'{path}' is not a valid action-files bundle: unsafe entry '{name}'")
        })?;
        if raw_by_name.insert(clean, bytes.clone()).is_some() {
            return Err(format!(
                "'{path}' is not a valid action-files bundle: duplicate entry '{name}'"
            ));
        }
    }
    for file in &mut doc.quick_action_files {
        if file.bytes_b64.is_some() {
            continue;
        }
        let bytes = raw_by_name.remove(&file.filename).ok_or_else(|| {
            format!(
                "'{path}' is not a valid action-files bundle: missing file '{}'",
                file.filename
            )
        })?;
        file.bytes_b64 = Some(B64.encode(&bytes));
    }
    if !raw_by_name.is_empty() {
        return Err(format!(
            "'{path}' is not a valid action-files bundle: files without an action entry"
        ));
    }
    normalize(&mut doc);
    Ok(doc)
}

/// Fills legacy version-1 Quick Actions without a shell as PowerShell so the
/// required version-2 field can deserialize. A version-1 record that declares
/// any shell other than PowerShell is inconsistent — accepting it would
/// silently reinterpret CMD text as PowerShell — so it fails before any merge
/// writes (ADR-0014).
fn apply_legacy_shell_defaults(value: &mut serde_json::Value, path: &str) -> Result<(), String> {
    let actions = value
        .get_mut("quick_actions")
        .and_then(|v| v.as_array_mut());
    let Some(actions) = actions else {
        return Ok(());
    };
    for action in actions.iter_mut() {
        match action.get("shell") {
            None => {
                action["shell"] = serde_json::Value::String("powershell".into());
            }
            Some(serde_json::Value::String(shell)) if shell == "powershell" => {}
            Some(serde_json::Value::String(shell)) => {
                return Err(format!(
                    "'{path}' is not a valid Sprout backup: version 1 Quick Action declares shell '{shell}' — version 1 actions are PowerShell-only"
                ));
            }
            Some(_) => {
                return Err(format!(
                    "'{path}' is not a valid Sprout backup: version 1 Quick Action has a malformed shell"
                ));
            }
        }
    }
    Ok(())
}

/// The portable form of every record: install directories are stripped in
/// both directions (ADR-0009) — export so they never leave this PC, import
/// so a hand-edited file can never smuggle one in — and resolver flags don't
/// travel: the restored library re-resolves live references on read.
fn normalize(doc: &mut BackupDocument) {
    for product in &mut doc.products {
        product.install_dir = None;
    }
    for record in &mut doc.presets {
        for req in &mut record.preset.requirements {
            req.product.install_dir = None;
            req.unresolved = false;
        }
    }
    // Staged paths never persist: the stored command keeps its `<FilesDir>`
    // placeholder, so there is nothing to strip here — this is the assertion
    // point, not a scrubber. Staging is runtime-only; export reads the stored
    // command verbatim.
}

// ---------------------------------------------------------------------------
// Single-action zip bundles: a minimal stored (uncompressed) zip writer and
// reader with no new dependencies. Only this module's single-action export
// writes them and only its import reads them; whole-app backups stay JSON.
// ---------------------------------------------------------------------------

/// `action.json` inside a bundle: the same backup envelope with file metadata.
pub const ZIP_ENTRY_ACTION_JSON: &str = "action.json";
/// Raw file bytes live under this prefix, one entry per attached file.
pub const ZIP_FILES_PREFIX: &str = "files/";
/// Rejects absurd bundles before they allocate.
const MAX_ZIP_TOTAL_BYTES: usize = 64 * 1024 * 1024;

fn is_zip_magic(raw: &[u8]) -> bool {
    raw.len() >= 4 && raw[0] == b'P' && raw[1] == b'K' && raw[2] == 3 && raw[3] == 4
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for byte in data {
        crc ^= *byte as u32;
        for _ in 0..8 {
            let lsb = crc & 1;
            crc >>= 1;
            if lsb != 0 {
                crc ^= 0xEDB8_8320;
            }
        }
    }
    !crc
}

fn put_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn get_u16(raw: &[u8], at: usize) -> Result<u16, String> {
    raw.get(at..at + 2)
        .and_then(|s| <[u8; 2]>::try_from(s).ok())
        .map(u16::from_le_bytes)
        .ok_or_else(|| "truncated entry".to_string())
}

fn get_u32(raw: &[u8], at: usize) -> Result<u32, String> {
    raw.get(at..at + 4)
        .and_then(|s| <[u8; 4]>::try_from(s).ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| "truncated entry".to_string())
}

/// Writes stored (method 0) entries: local headers, central directory, EOCD.
fn write_zip(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in entries {
        let name_bytes = name.as_bytes();
        let crc = crc32(data);
        let offset = out.len() as u32;
        put_u32(&mut out, 0x0403_4B50);
        put_u16(&mut out, 20);
        put_u16(&mut out, 0x0800);
        put_u16(&mut out, 0);
        put_u16(&mut out, 0);
        put_u16(&mut out, 0x21);
        put_u32(&mut out, crc);
        put_u32(&mut out, data.len() as u32);
        put_u32(&mut out, data.len() as u32);
        put_u16(&mut out, name_bytes.len() as u16);
        put_u16(&mut out, 0);
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(data);
        put_u32(&mut central, 0x0201_4B50);
        put_u16(&mut central, 20);
        put_u16(&mut central, 20);
        put_u16(&mut central, 0x0800);
        put_u16(&mut central, 0);
        put_u16(&mut central, 0);
        put_u16(&mut central, 0x21);
        put_u32(&mut central, crc);
        put_u32(&mut central, data.len() as u32);
        put_u32(&mut central, data.len() as u32);
        put_u16(&mut central, name_bytes.len() as u16);
        put_u16(&mut central, 0);
        put_u16(&mut central, 0);
        put_u16(&mut central, 0);
        put_u16(&mut central, 0);
        put_u32(&mut central, 0);
        put_u32(&mut central, offset);
        central.extend_from_slice(name_bytes);
    }
    let central_offset = out.len() as u32;
    let central_size = central.len() as u32;
    out.extend_from_slice(&central);
    put_u32(&mut out, 0x0605_4B50);
    put_u16(&mut out, 0);
    put_u16(&mut out, 0);
    put_u16(&mut out, entries.len() as u16);
    put_u16(&mut out, entries.len() as u16);
    put_u32(&mut out, central_size);
    put_u32(&mut out, central_offset);
    put_u16(&mut out, 0);
    out
}

/// Reads the local-header sequence back: name/data pairs for stored entries.
/// Anything compressed, encrypted, or malformed fails instead of guessing.
fn read_zip(raw: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    if raw.len() > MAX_ZIP_TOTAL_BYTES {
        return Err("bundle is larger than 64 MB".into());
    }
    let mut entries = Vec::new();
    let mut at = 0usize;
    loop {
        if raw.len() - at < 4 {
            return Err("truncated bundle".into());
        }
        match get_u32(raw, at)? {
            0x0403_4B50 => {}
            0x0201_4B50 | 0x0605_4B50 => break,
            other => return Err(format!("unexpected zip signature {other:#x}")),
        }
        let method = get_u16(raw, at + 8)?;
        if method != 0 {
            return Err("compressed entries are not supported".into());
        }
        let crc = get_u32(raw, at + 14)?;
        let size = get_u32(raw, at + 18)? as usize;
        let name_len = get_u16(raw, at + 26)? as usize;
        let extra_len = get_u16(raw, at + 28)? as usize;
        let name_at = at + 30;
        let data_at = name_at + name_len + extra_len;
        if raw.len() < data_at || raw.len() - data_at < size {
            return Err("truncated entry".into());
        }
        let name = String::from_utf8(raw[name_at..name_at + name_len].to_vec())
            .map_err(|_| "entry name is not UTF-8".to_string())?;
        let data = raw[data_at..data_at + size].to_vec();
        if crc32(&data) != crc {
            return Err(format!("entry '{name}' failed its checksum"));
        }
        if entries.iter().any(|(n, _)| n == &name) {
            return Err(format!("duplicate entry '{name}'"));
        }
        entries.push((name, data));
        at = data_at + size;
    }
    if entries.is_empty() {
        return Err("bundle holds no entries".into());
    }
    Ok(entries)
}

/// Every record must be storable: the same validators the authoring dialogs
/// enforce, so a broken file fails before the merge touches anything. Preset
/// payloads are exempt — local Presets store their Requirements as id-only
/// live references (empty display names by design, ADR-0007), and they were
/// validated when authored or imported.
fn validate_records(doc: &BackupDocument) -> Result<(), String> {
    for product in &doc.products {
        db::validate_product(product)?;
    }
    for entry in &doc.launch_entries {
        launch::validate_launch_entry(entry)?;
    }
    for action in &doc.quick_actions {
        quick_actions::validate_quick_action(action)?;
    }
    validate_action_files(doc)?;
    for clip in &doc.clips {
        clips::validate_clip(clip)?;
    }
    Ok(())
}

/// Every attached file must be storable: its action index lands inside the
/// same document, its name follows the plain file-name rule, its content
/// decodes and matches its size, and the v1 caps hold per file and per
/// action. A broken bundle fails before the merge touches anything.
fn validate_action_files(doc: &BackupDocument) -> Result<(), String> {
    let mut seen: HashSet<(usize, String)> = HashSet::new();
    let mut totals: HashMap<usize, u64> = HashMap::new();
    for file in &doc.quick_action_files {
        if file.action >= doc.quick_actions.len() {
            return Err(format!(
                "Backup file '{}' refers to a missing action.",
                file.filename
            ));
        }
        let clean = quick_actions::validate_action_filename(&file.filename)?;
        if clean != file.filename {
            return Err(format!(
                "Backup file '{}' is not a plain file name.",
                file.filename
            ));
        }
        if !seen.insert((file.action, clean.to_lowercase())) {
            return Err(format!(
                "Backup file '{clean}' is attached twice to the same action."
            ));
        }
        let b64 = file.bytes_b64.as_deref().ok_or_else(|| {
            format!("Backup file '{clean}' carries no content.")
        })?;
        let bytes = B64.decode(b64).map_err(|_| {
            format!("Backup file '{clean}' is not valid base64 content.")
        })?;
        if bytes.len() as u64 != file.size {
            return Err(format!(
                "Backup file '{clean}' declares {} bytes but carries {}.",
                file.size,
                bytes.len()
            ));
        }
        if bytes.len() > quick_actions::MAX_ACTION_FILE_BYTES {
            return Err(format!(
                "Backup file '{clean}' exceeds the 5 MB per-file limit."
            ));
        }
        let total = totals.entry(file.action).or_insert(0);
        *total += bytes.len() as u64;
        if *total > quick_actions::MAX_ACTION_FILES_BYTES as u64 {
            return Err(
                "One action's files exceed the 20 MB per-action limit.".into(),
            );
        }
    }
    Ok(())
}

/// Decodes one validated backup file's content. Validation ran first, so the
/// content is present and well-formed here.
fn decode_action_file(file: &QuickActionFileBackup) -> Vec<u8> {
    file.bytes_b64
        .as_deref()
        .and_then(|b64| B64.decode(b64).ok())
        .unwrap_or_default()
}

/// Reads one text column into a list — every collection's existing-identity
/// set in the merge starts here.
fn column_strings(conn: &Connection, sql: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

/// Reads rows through `map` into a set — the composite payload keys the merge
/// compares against (ticket 103). `\u{1f}` joins key parts; it is a control
/// character, so a target or command can never forge a cross-part collision.
fn column_set<T, F>(conn: &Connection, sql: &str, map: F) -> Result<HashSet<T>, String>
where
    T: std::hash::Hash + Eq,
    F: Fn(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
{
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], map).map_err(|e| e.to_string())?;
    let all = rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    Ok(all.into_iter().collect())
}

/// The merging restore, all five collections under ONE transaction: any
/// failure rolls back everything, so a half-restored library is impossible.
fn merge(conn: &Connection, doc: &BackupDocument) -> Result<ImportSummary, String> {
    let mut inserted = BackupCounts::default();
    let mut skipped = BackupCounts::default();
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

    // Products: skip an id or a case-insensitively matching name that the
    // Library already has — the same duplicate rule create_product enforces.
    let mut product_ids: HashSet<String> =
        column_strings(&tx, "SELECT id FROM products")?.into_iter().collect();
    let mut product_names: HashSet<String> = column_strings(&tx, "SELECT name FROM products")?
        .into_iter()
        .map(|name| name.trim().to_lowercase())
        .collect();
    for product in &doc.products {
        let name_key = product.name.trim().to_lowercase();
        let exists =
            product_ids.contains(&product.id) || product_names.contains(&name_key);
        if exists {
            skipped.products += 1;
            continue;
        }
        db::insert_product(&tx, product).map_err(|e| e.to_string())?;
        product_ids.insert(product.id.clone());
        product_names.insert(name_key);
        inserted.products += 1;
    }

    // Presets: the Library id is the identity — same id means kept, whatever
    // its contents.
    let mut preset_ids: HashSet<String> =
        column_strings(&tx, "SELECT id FROM presets")?.into_iter().collect();
    for record in &doc.presets {
        if preset_ids.contains(&record.id) {
            skipped.presets += 1;
            continue;
        }
        db::create_preset(&tx, record).map_err(|e| e.to_string())?;
        preset_ids.insert(record.id.clone());
        inserted.presets += 1;
    }

    // Launch entries and Quick Actions skip on PAYLOAD identity — the same
    // rule the create/update commands enforce: kind + target for entries,
    // shell + command + working directory for actions (ADR-0026 shell-aware
    // identity), all case-folded (Windows paths) and trimmed. Names are
    // display-only for both lists; a same-name-different-target entry from a
    // backup is a distinct item and must land.
    let mut entry_keys: HashSet<String> = column_set(
        &tx,
        "SELECT kind, target FROM launch_entries",
        |row| {
            Ok(format!(
                "{0}\u{1f}{1}",
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?.trim().to_lowercase()
            ))
        },
    )?;
    for entry in &doc.launch_entries {
        let key = format!(
            "{0}\u{1f}{1}",
            launch::kind_to_str(entry.kind),
            entry.target.trim().to_lowercase()
        );
        if entry_keys.contains(&key) {
            skipped.launch_entries += 1;
            continue;
        }
        launch::append_entry(&tx, entry).map_err(|e| e.to_string())?;
        entry_keys.insert(key);
        inserted.launch_entries += 1;
    }

    // Quick Actions skip on extended payload identity: shell + command +
    // working directory (ADR-0026) plus the attached file-name set — the same
    // text with different files is a different action, while content bytes
    // never decide identity. All parts fold case and trim; names are
    // display-only, as before.
    let action_rows: Vec<(i64, String)> = {
        let mut stmt = tx
            .prepare("SELECT id, shell, command, cwd FROM quick_actions")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let key = format!(
                    "{0}\u{1f}{1}\u{1f}{2}",
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?.trim().to_lowercase(),
                    row.get::<_, Option<String>>(3)?
                        .map(|cwd| cwd.to_lowercase())
                        .unwrap_or_default()
                );
                Ok((id, key))
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?
    };
    let mut files_by_action: HashMap<i64, Vec<String>> = HashMap::new();
    {
        let mut stmt = tx
            .prepare("SELECT action_id, filename FROM quick_action_files")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (action_id, filename): (i64, String) = row.map_err(|e| e.to_string())?;
            files_by_action.entry(action_id).or_default().push(filename);
        }
    }
    let mut action_keys: HashSet<String> = action_rows
        .iter()
        .map(|(id, base)| {
            let mut names = files_by_action
                .get(id)
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|n| n.to_lowercase())
                .collect::<Vec<_>>();
            names.sort();
            format!("{base}\u{1f}{}", names.join("\u{1f}"))
        })
        .collect();
    let mut incoming_names: HashMap<usize, Vec<&QuickActionFileBackup>> = HashMap::new();
    for file in &doc.quick_action_files {
        incoming_names.entry(file.action).or_default().push(file);
    }
    for (index, action) in doc.quick_actions.iter().enumerate() {
        let base = format!(
            "{0}\u{1f}{1}\u{1f}{2}",
            action.shell.as_str(),
            action.command.trim().to_lowercase(),
            quick_actions::normalized_cwd(action)
                .map(|cwd| cwd.to_lowercase())
                .unwrap_or_default()
        );
        let mut names = incoming_names
            .get(&index)
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|f| f.filename.to_lowercase())
            .collect::<Vec<_>>();
        names.sort();
        let key = format!("{base}\u{1f}{}", names.join("\u{1f}"));
        if action_keys.contains(&key) {
            skipped.quick_actions += 1;
            continue;
        }
        let new_id =
            quick_actions::append_action(&tx, action).map_err(|e| e.to_string())?;
        if let Some(files) = incoming_names.get(&index) {
            for file in files {
                quick_actions::insert_action_file(
                    &tx,
                    new_id,
                    &file.filename,
                    &decode_action_file(file),
                )
                .map_err(|e| e.to_string())?;
            }
        }
        action_keys.insert(key);
        inserted.quick_actions += 1;
    }

    // Clips: a text Clip IS its text — identical content means kept,
    // whatever the optional title says. An image Clip (ticket 178) IS its
    // bytes — the FNV-1a hash over them is the identity and the name is
    // display-only. Image rows carry blank content, so the text set excludes
    // them: without the exclusion every image would collide on "".
    let mut clip_texts: HashSet<String> = column_strings(
        &tx,
        "SELECT content FROM clips WHERE id NOT IN (SELECT clip_id FROM clip_images)",
    )?
    .into_iter()
    .map(|content| content.trim().to_string())
    .collect();
    let mut clip_image_hashes: HashSet<String> = column_set(
        &tx,
        "SELECT bytes FROM clip_images",
        |row| Ok(clips::image_hash(&row.get::<_, Vec<u8>>(0)?)),
    )?;
    for clip in &doc.clips {
        if let Some(image) = &clip.image {
            let bytes = B64.decode(image.bytes_base64.trim()).map_err(|e| {
                format!("That backup's clip image couldn't be read: {e}")
            })?;
            let image_key = clips::image_hash(&bytes);
            if clip_image_hashes.contains(&image_key) {
                skipped.clips += 1;
                continue;
            }
            clips::append_clip_image(&tx, &clip.name, clip.show_in_dock, &image.mime, &bytes)
                .map_err(|e| e.to_string())?;
            clip_image_hashes.insert(image_key);
            inserted.clips += 1;
            continue;
        }
        let text_key = clip.content.trim().to_string();
        if clip_texts.contains(&text_key) {
            skipped.clips += 1;
            continue;
        }
        clips::append_clip(&tx, clip).map_err(|e| e.to_string())?;
        clip_texts.insert(text_key);
        inserted.clips += 1;
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(ImportSummary { inserted, skipped })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        EnvAction, EnvWiring, Requirement, Step, VersionPolicy,
    };
    use crate::run::{RequirementOutcome, RunOutcome, RunRecord, RunStatus};

    fn conn() -> Connection {
        crate::db::init_at(&tempfile::tempdir().unwrap().into_path()).unwrap()
    }

    fn write_file(dir: &std::path::Path, name: &str, contents: &str) -> String {
        let path = dir.join(name);
        fs::write(&path, contents).unwrap();
        path.to_str().unwrap().to_string()
    }

    fn product(id: &str) -> Product {
        Product {
            id: id.into(),
            name: format!("{id} display"),
            winget_id: Some(format!("Vendor.{id}")),
            install_location_hint: None,
            install_dir: None,
            default_env: vec![],
        }
    }

    fn requirement(product_id: &str) -> Requirement {
        Requirement {
            product: Product {
                id: product_id.into(),
                ..product(product_id)
            },
            step: Step::Winget {
                id: format!("Vendor.{product_id}"),
                scope: "machine".into(),
            },
            version_policy: VersionPolicy::Latest,
            depends_on: vec![],
            timeout_minutes: 10,
            env: vec![EnvWiring {
                action: EnvAction::Set,
                name: "JAVA_HOME".into(),
                value: "<InstallLocation>".into(),
            }],
            verify: vec![],
            unresolved: false,
        }
    }

    fn app_entry(name: &str) -> LaunchEntryInput {
        LaunchEntryInput {
            name: name.into(),
            kind: launch::LaunchEntryKind::App,
            target: format!(r"C:\Apps\{name}.lnk"),
            shell: None,
            show_window: false,
            desktop_id: None,
            show_in_dock: true,
        }
    }

    fn command_entry(name: &str) -> LaunchEntryInput {
        LaunchEntryInput {
            name: name.into(),
            kind: launch::LaunchEntryKind::Command,
            target: "Get-Process".into(),
            shell: Some(launch::LaunchShell::Powershell),
            show_window: false,
            desktop_id: None,
            show_in_dock: true,
        }
    }

    fn action(name: &str) -> QuickActionInput {
        QuickActionInput {
            name: name.into(),
            shell: quick_actions::QuickActionShell::Powershell,
            command: "docker compose up -d".into(),
            cwd: None,
            stoppable: false,
            stop_command: None,
            note: None,
            auto_run: false,
            show_in_dock: true,
            pre_check: None,
            pre_fix: None,
        }
    }

    /// Populates every content collection and returns the expected shapes.
    fn seed_all(conn: &Connection) {
        let mut git = product("git");
        git.default_env.push(EnvWiring {
            action: EnvAction::Prepend,
            name: "PATH".into(),
            value: "%JAVA_HOME%\\bin".into(),
        });
        db::create_product(conn, &git).unwrap();
        let mut vscode = product("vscode");
        vscode.install_dir = Some(r"E:\Tools\VSCode".into());
        db::create_product(conn, &vscode).unwrap();

        db::create_preset(
            conn,
            &PresetRecord {
                id: "dev-box".into(),
                preset: crate::domain::Preset {
                    schema_version: 1,
                    platform: "windows".into(),
                    name: "Dev box".into(),
                    description: "The daily drivers".into(),
                    author: "Tester".into(),
                    version: "2".into(),
                    requirements: vec![requirement("git")],
                },
                imported: false,
            },
        )
        .unwrap();
        let mut snapshot = requirement("ghost");
        snapshot.product.name = "Ghost tools".into();
        snapshot.product.winget_id = Some("Vendor.Ghost".into());
        db::create_preset(
            conn,
            &PresetRecord {
                id: "shared-snap".into(),
                preset: crate::domain::Preset {
                    schema_version: 1,
                    platform: "windows".into(),
                    name: "Shared snap".into(),
                    description: "Imported once, stored as authored".into(),
                    author: "Someone else".into(),
                    version: "1".into(),
                    requirements: vec![snapshot],
                },
                imported: true,
            },
        )
        .unwrap();

        launch::create_launch_entry(conn, &app_entry("Spotify")).unwrap();
        launch::create_launch_entry(conn, &command_entry("Ports")).unwrap();
        quick_actions::create_quick_action(conn, &action("Build")).unwrap();
        clips::create_clip(conn, &ClipInput {
            name: "reply".into(),
            content: "Thanks for the report!".into(),
            show_in_dock: true,
            image: None,
        })
        .unwrap();
        clips::create_clip(conn, &ClipInput {
            name: "".into(),
            content: "git status --short".into(),
            show_in_dock: true,
            image: None,
        })
        .unwrap();
    }

    #[test]
    fn export_import_roundtrip_restores_every_collection() {
        let source = conn();
        seed_all(&source);

        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(&dir, "backup.json", "");
        let counts = export_backup(&source, &file, &BackupSelection::all()).unwrap();
        assert_eq!(
            counts,
            BackupCounts {
                products: 2,
                presets: 2,
                launch_entries: 2,
                quick_actions: 1,
                clips: 2,
            }
        );

        // The document shape: kind-tagged, versioned, stamped, one array per
        // collection — and no machine-local install directory anywhere.
        let on_disk: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(on_disk["kind"], BACKUP_KIND);
        assert_eq!(on_disk["version"], BACKUP_VERSION);
        assert!(on_disk["exported_at"].is_i64());
        assert_eq!(on_disk["products"].as_array().unwrap().len(), 2);
        assert_eq!(on_disk["presets"].as_array().unwrap().len(), 2);
        assert_eq!(on_disk["launch_entries"].as_array().unwrap().len(), 2);
        assert_eq!(on_disk["quick_actions"].as_array().unwrap().len(), 1);
        assert_eq!(on_disk["clips"].as_array().unwrap().len(), 2);
        assert!(
            !fs::read_to_string(&file).unwrap().contains(r"E:\Tools"),
            "the backup must never carry an install directory"
        );

        // Restore into a fresh library.
        let target = conn();
        let summary = import_backup(&target, &file).unwrap();
        assert_eq!(summary.skipped, BackupCounts::default());
        assert_eq!(
            summary.inserted,
            BackupCounts {
                products: 2,
                presets: 2,
                launch_entries: 2,
                quick_actions: 1,
                clips: 2,
            }
        );

        // Products: content equal (timestamps regenerate), env wiring intact,
        // install directory stripped.
        let restored = db::list_products(&target, None).unwrap();
        assert_eq!(restored.len(), 2);
        let git = restored.iter().find(|p| p.product.id == "git").unwrap();
        assert_eq!(git.product.name, "git display");
        assert_eq!(git.product.default_env.len(), 1);
        assert_eq!(git.product.default_env[0].action, EnvAction::Prepend);
        assert!(git.created_at.unwrap_or(0) > 0, "fresh stamps are written");
        assert_eq!(
            restored.iter().find(|p| p.product.id == "vscode").unwrap().product.install_dir,
            None
        );

        // Presets: ids, names, and the immutable imported flag survive; the
        // local preset's live reference resolves against the RESTORED
        // products; the imported snapshot keeps its embedded payload.
        let presets = db::list_presets(&target).unwrap();
        assert_eq!(presets.len(), 2);
        let dev_box = presets.iter().find(|p| p.id == "dev-box").unwrap();
        assert!(!dev_box.imported);
        assert_eq!(dev_box.preset.name, "Dev box");
        assert_eq!(dev_box.preset.requirements[0].product.name, "git display");
        assert!(!dev_box.preset.requirements[0].unresolved);
        let snap = presets.iter().find(|p| p.id == "shared-snap").unwrap();
        assert!(snap.imported);
        assert_eq!(snap.preset.requirements[0].product.name, "Ghost tools");

        // The ordered lists come back in order with their fields intact.
        let entries = launch::list_launch_entries(&target).unwrap();
        assert_eq!(
            entries.iter().map(|e| e.entry.clone()).collect::<Vec<_>>(),
            vec![app_entry("Spotify"), command_entry("Ports")]
        );
        let actions = quick_actions::list_quick_actions(&target).unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action, action("Build"));
        let clips = clips::list_clips(&target).unwrap();
        assert_eq!(
            clips.iter().map(|c| c.clip.clone()).collect::<Vec<_>>(),
            vec![
                ClipInput { name: "reply".into(), content: "Thanks for the report!".into(), show_in_dock: true, image: None },
                ClipInput { name: "".into(), content: "git status --short".into(), show_in_dock: true, image: None },
            ]
        );

        // Re-importing the same file changes nothing — every identity exists.
        let again = import_backup(&target, &file).unwrap();
        assert_eq!(again.inserted, BackupCounts::default());
        assert_eq!(again.skipped.products, 2);
        assert_eq!(again.skipped.presets, 2);
        assert_eq!(again.skipped.launch_entries, 2);
        assert_eq!(again.skipped.quick_actions, 1);
        assert_eq!(again.skipped.clips, 2);
        assert_eq!(db::list_products(&target, None).unwrap().len(), 2);
        assert_eq!(db::list_presets(&target).unwrap().len(), 2);
        assert_eq!(launch::list_launch_entries(&target).unwrap().len(), 2);
    }

    #[test]
    fn foreign_files_are_rejected_cleanly_and_change_nothing() {
        let c = conn();
        let dir = tempfile::tempdir().unwrap().into_path();

        let junk = write_file(&dir, "junk.json", "this is not json");
        let err = import_backup(&c, &junk).unwrap_err();
        assert!(err.contains("not a Sprout backup"), "got: {err}");

        // A real .sprout.json preset file is not a whole-app backup.
        let preset_file = write_file(
            &dir,
            "preset.sprout.json",
            r#"{"schema_version":1,"platform":"windows","name":"X","description":"d","author":"a","version":"1","requirements":[]}"#,
        );
        let err = import_backup(&c, &preset_file).unwrap_err();
        assert!(
            err.contains("not a Sprout backup") && err.contains(".sprout.json"),
            "got: {err}"
        );

        // Wrong kind, right shape otherwise.
        let wrong_kind = write_file(
            &dir,
            "kind.json",
            r#"{"kind":"something-else","version":1,"exported_at":0,"products":[],"presets":[],"launch_entries":[],"quick_actions":[],"clips":[]}"#,
        );
        assert!(import_backup(&c, &wrong_kind)
            .unwrap_err()
            .contains("not a Sprout backup"));

        // A future version is named honestly, like the preset importer does.
        let future = write_file(
            &dir,
            "future.json",
            r#"{"kind":"sprout-backup","version":99,"exported_at":0,"products":[],"presets":[],"launch_entries":[],"quick_actions":[],"clips":[]}"#,
        );
        let err = inspect_backup(&future).unwrap_err();
        assert!(err.contains("Unsupported backup version 99"), "got: {err}");

        // Nothing above may have written anything.
        assert!(db::list_products(&c, None).unwrap().is_empty());
        assert!(db::list_presets(&c).unwrap().is_empty());
    }

    #[test]
    fn partial_duplicates_merge_without_duplication() {
        // The target already holds SOME of what the file carries: the merge
        // adds only the missing identities.
        let target = conn();
        db::create_product(&target, &product("git")).unwrap();
        launch::create_launch_entry(&target, &app_entry("Spotify")).unwrap();
        clips::create_clip(&target, &ClipInput {
            name: "different title".into(),
            content: "Thanks for the report!".into(),
            show_in_dock: true,
            image: None,
        })
        .unwrap();

        let source = conn();
        seed_all(&source);
        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(&dir, "backup.json", "");
        export_backup(&source, &file, &BackupSelection::all()).unwrap();

        let summary = import_backup(&target, &file).unwrap();
        assert_eq!(summary.inserted.products, 1, "only vscode is new");
        assert_eq!(summary.skipped.products, 1);
        assert_eq!(summary.inserted.launch_entries, 1, "only Ports is new");
        assert_eq!(summary.skipped.launch_entries, 1);
        assert_eq!(summary.inserted.clips, 1, "only the untitled clip is new");
        assert_eq!(summary.skipped.clips, 1);

        // And no collection grew a duplicate.
        assert_eq!(db::list_products(&target, None).unwrap().len(), 2);
        assert_eq!(launch::list_launch_entries(&target).unwrap().len(), 2);
        assert_eq!(clips::list_clips(&target).unwrap().len(), 2);
    }

    #[test]
    fn import_identity_is_payload_not_name() {
        // Ticket 103's rule holds at import: names never decide — payloads do.
        // Same name + different target lands; same payload under any name and
        // case folds away.
        let target = conn();
        launch::create_launch_entry(&target, &app_entry("Spotify")).unwrap();
        quick_actions::create_quick_action(&target, &action("Builder")).unwrap();

        let doc = BackupDocument {
            kind: BACKUP_KIND.into(),
            version: BACKUP_VERSION,
            exported_at: 0,
            products: vec![],
            presets: vec![],
            launch_entries: vec![
                LaunchEntryInput {
                    name: "Spotify".into(),
                    kind: launch::LaunchEntryKind::App,
                    target: r"D:\Apps\Spotify.lnk".into(),
                    shell: None,
                    show_window: false,
                    desktop_id: None,
                    show_in_dock: true,
                },
                LaunchEntryInput {
                    name: "Music".into(),
                    kind: launch::LaunchEntryKind::App,
                    target: r"c:\APPS\SPOTIFY.LNK".into(),
                    shell: None,
                    show_window: false,
                    desktop_id: None,
                    show_in_dock: true,
                },
            ],
            quick_actions: vec![action("Build")],
            quick_action_files: vec![],
            clips: vec![],
        };

        let summary = merge(&target, &doc).unwrap();
        assert_eq!(summary.inserted.launch_entries, 1, "different target = distinct");
        assert_eq!(summary.skipped.launch_entries, 1, "same target folds");
        assert_eq!(summary.inserted.quick_actions, 0, "same shell+command+cwd skips");
        assert_eq!(summary.skipped.quick_actions, 1);

        let entries = launch::list_launch_entries(&target).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(quick_actions::list_quick_actions(&target).unwrap().len(), 1);
    }

    #[test]
    fn machine_scoped_state_never_travels() {
        let source = conn();
        seed_all(&source);
        // Run history, settings knobs, and dock memory exist on this machine…
        db::create_run(
            &source,
            &RunRecord {
                id: "run-1".into(),
                started_at: 100,
                finished_at: 200,
                preset_names: vec!["Dev box".into()],
                outcome: RunOutcome::Ok,
                results: vec![RequirementOutcome {
                    product_id: "git".into(),
                    product_name: "git display".into(),
                    status: RunStatus::Installed,
                    detail: "done".into(),
                    reboot_required: false,
                    log_path: r"C:\logs\x.log".into(),
                }],
            },
        )
        .unwrap();
        crate::settings::save(
            &source,
            &crate::settings::Settings {
                install_dir: r"D:\Apps".into(),
                theme: "dark".into(),
                ..crate::settings::Settings::default()
            },
        )
        .unwrap();
        db::save_dock_edge(&source, "MONITOR-1", "right").unwrap();
        db::save_dock_mode(&source, "MONITOR-1", "fixed").unwrap();

        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(&dir, "backup.json", "");
        let on_disk = export_backup(&source, &file, &BackupSelection::all()).unwrap();
        assert!(on_disk.products > 0);

        let target = conn();
        import_backup(&target, &file).unwrap();
        // …and none of it arrives anywhere else.
        assert!(db::list_runs(&target).unwrap().is_empty());
        assert_eq!(crate::settings::load(&target), crate::settings::Settings::default());
        assert_eq!(db::load_dock_edge(&target, "MONITOR-1"), None);
        assert_eq!(db::load_dock_mode(&target, "MONITOR-1"), None);
    }

    #[test]
    fn crafted_install_dirs_are_stripped_on_import() {
        let c = conn();
        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(
            &dir,
            "carry.json",
            r#"{
              "kind":"sprout-backup","version":1,"exported_at":0,
              "products":[{"id":"p1","name":"P1","winget_id":null,"install_location_hint":null,"install_dir":"E:\\Smuggled","default_env":[]}],
              "presets":[{"id":"snap","schema_version":1,"platform":"windows","name":"Snap","description":"d","author":"a","version":"1","imported":true,
                "requirements":[{"product":{"id":"p1","name":"P1","winget_id":null,"install_location_hint":null,"install_dir":"E:\\Smuggled","default_env":[]},
                  "step":{"type":"winget","id":"Vendor.P1","scope":"machine"},"version_policy":{"kind":"latest"},"depends_on":[],"timeout_minutes":10,"env":[],"verify":[]}],
                "_note":"requirement arrays"}],
              "launch_entries":[],"quick_actions":[],"clips":[]
            }"#,
        );
        import_backup(&c, &file).unwrap();
        let stored = db::get_product(&c, "p1").unwrap().unwrap();
        assert_eq!(stored.product.install_dir, None);
        let payload: serde_json::Value = serde_json::from_str(
            &c.query_row("SELECT data FROM presets WHERE id='snap'", [], |r| r.get::<_, String>(0))
                .unwrap(),
        )
        .unwrap();
        assert!(
            payload["requirements"][0]["product"].get("install_dir").is_none(),
            "an imported snapshot must never carry another machine's install directory: {payload}"
        );
    }

    #[test]
    fn empty_backup_roundtrips_as_a_no_op() {
        let source = conn();
        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(&dir, "empty.json", "");
        let counts = export_backup(&source, &file, &BackupSelection::all()).unwrap();
        assert_eq!(counts, BackupCounts::default());

        let target = conn();
        let summary = import_backup(&target, &file).unwrap();
        assert_eq!(summary.inserted, BackupCounts::default());
        assert_eq!(summary.skipped, BackupCounts::default());
        assert_eq!(inspect_backup(&file).unwrap(), BackupCounts::default());
    }

    #[test]
    fn partial_export_writes_only_selected_collections() {
        // Ticket 87: launch entries and clips ride; everything else is an
        // empty array in the SAME document — kind tag, version, shape.
        let source = conn();
        seed_all(&source);

        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(&dir, "partial.json", "");
        let counts = export_backup(
            &source,
            &file,
            &BackupSelection {
                products: false,
                presets: false,
                launch_entries: true,
                quick_actions: false,
                clips: true,
            },
        )
        .unwrap();
        assert_eq!(
            counts,
            BackupCounts {
                launch_entries: 2,
                clips: 2,
                ..BackupCounts::default()
            }
        );

        let on_disk: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(on_disk["kind"], BACKUP_KIND);
        assert_eq!(on_disk["version"], BACKUP_VERSION);
        assert_eq!(on_disk["products"].as_array().unwrap().len(), 0);
        assert_eq!(on_disk["presets"].as_array().unwrap().len(), 0);
        assert_eq!(on_disk["launch_entries"].as_array().unwrap().len(), 2);
        assert_eq!(on_disk["quick_actions"].as_array().unwrap().len(), 0);
        assert_eq!(on_disk["clips"].as_array().unwrap().len(), 2);

        // An ordinary file: inspect and restore run the unchanged flow.
        assert_eq!(
            inspect_backup(&file).unwrap(),
            BackupCounts {
                launch_entries: 2,
                clips: 2,
                ..BackupCounts::default()
            }
        );
        let target = conn();
        let summary = import_backup(&target, &file).unwrap();
        assert_eq!(
            summary.inserted,
            BackupCounts {
                launch_entries: 2,
                clips: 2,
                ..BackupCounts::default()
            }
        );
        assert_eq!(summary.skipped, BackupCounts::default());
        assert!(db::list_products(&target, None).unwrap().is_empty());
        assert!(db::list_presets(&target).unwrap().is_empty());
        assert_eq!(launch::list_launch_entries(&target).unwrap().len(), 2);
        assert_eq!(clips::list_clips(&target).unwrap().len(), 2);
    }

    #[test]
    fn partial_restore_into_a_populated_database_reports_true_counts() {
        // The target already holds one entry the partial file carries: the
        // merge must split inserted/skipped exactly there.
        let target = conn();
        launch::create_launch_entry(&target, &app_entry("Spotify")).unwrap();

        let source = conn();
        seed_all(&source);
        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(&dir, "partial.json", "");
        export_backup(
            &source,
            &file,
            &BackupSelection {
                products: false,
                presets: false,
                launch_entries: true,
                quick_actions: true,
                clips: true,
            },
        )
        .unwrap();

        let summary = import_backup(&target, &file).unwrap();
        assert_eq!(summary.inserted.launch_entries, 1, "only Ports is new");
        assert_eq!(summary.skipped.launch_entries, 1);
        assert_eq!(summary.inserted.quick_actions, 1);
        assert_eq!(summary.skipped.quick_actions, 0);
        assert_eq!(summary.inserted.clips, 2);
        assert_eq!(summary.inserted.products, 0);
        assert_eq!(summary.inserted.presets, 0);
        assert_eq!(summary.skipped.products, 0);
        assert_eq!(summary.skipped.presets, 0);

        // No duplicates anywhere; the excluded collections never arrived.
        assert!(db::list_products(&target, None).unwrap().is_empty());
        assert!(db::list_presets(&target).unwrap().is_empty());
        assert_eq!(launch::list_launch_entries(&target).unwrap().len(), 2);
        assert_eq!(quick_actions::list_quick_actions(&target).unwrap().len(), 1);
        assert_eq!(clips::list_clips(&target).unwrap().len(), 2);
    }

    #[test]
    fn zero_selection_is_refused_before_anything_is_written() {
        let source = conn();
        seed_all(&source);
        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(&dir, "nothing.json", "");
        let err = export_backup(
            &source,
            &file,
            &BackupSelection {
                products: false,
                presets: false,
                launch_entries: false,
                quick_actions: false,
                clips: false,
            },
        )
        .unwrap_err();
        assert!(err.contains("at least one"), "got: {err}");
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "",
            "the refusal happens before serialization"
        );
    }

    #[test]
    fn invalid_records_fail_before_anything_is_written() {
        let source = conn();
        seed_all(&source);
        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(&dir, "backup.json", "");
        export_backup(&source, &file, &BackupSelection::all()).unwrap();

        // Corrupt one record past validation: a blank clip text can never
        // serve a copy.
        let mut doc: BackupDocument =
            serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        doc.clips.push(ClipInput {
            name: "broken".into(),
            content: "   ".into(),
            show_in_dock: true,
            image: None,
        });
        let bad = write_file(&dir, "bad.json", &serde_json::to_string(&doc).unwrap());

        let target = conn();
        let err = import_backup(&target, &bad).unwrap_err();
        assert!(err.contains("clip"), "got: {err}");
        assert!(
            db::list_products(&target, None).unwrap().is_empty(),
            "validation failures must leave the library untouched"
        );
    }

    #[test]
    fn quick_action_notes_survive_backup_roundtrip_and_alias() {
        // Ticket 117: notes are machine-local backup data — exported, imported,
        // and preserved alongside the action's other fields. Both `note` and its
        // alias `notes` deserialize.
        let source = conn();
        let mut with_note = action("Noted");
        with_note.note = Some("  hello note\nsecond line  ".into());
        quick_actions::create_quick_action(&source, &with_note).unwrap();
        let mut plain = action("Plain");
        plain.command = "echo plain".into();
        plain.note = None;
        quick_actions::create_quick_action(&source, &plain).unwrap();

        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(&dir, "backup.json", "");
        export_backup(&source, &file, &BackupSelection::all()).unwrap();

        // The file carries the trimmed note.
        let on_disk: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        let actions = on_disk["quick_actions"].as_array().unwrap();
        assert_eq!(actions.len(), 2);
        // The trimmed note is stored.
        assert_eq!(actions[0]["note"], "hello note\nsecond line");
        // Alias `notes` is accepted on import — a hand-edited backup using the
        // plural key must still restore.
        let mut doc: BackupDocument =
            serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        doc.quick_actions[1].note = Some("via alias".into());
        let mut value = serde_json::to_value(&doc).unwrap();
        // Rewrite the second action to use `notes` instead of `note`.
        if let Some(arr) = value.get_mut("quick_actions").and_then(|v| v.as_array_mut()) {
            if let Some(obj) = arr[1].as_object_mut() {
                let note = obj.remove("note").unwrap();
                obj.insert("notes".into(), note);
            }
        }
        let alias_file = write_file(&dir, "alias.json", &value.to_string());
        let target = conn();
        import_backup(&target, &alias_file).unwrap();
        let listed = quick_actions::list_quick_actions(&target).unwrap();
        assert_eq!(listed.len(), 2);
        // First action kept its trimmed note; second came via `notes` alias.
        let first = listed.iter().find(|a| a.action.name == "Noted").unwrap();
        assert_eq!(first.action.note.as_deref(), Some("hello note\nsecond line"));
        let second = listed.iter().find(|a| a.action.name == "Plain").unwrap();
        assert_eq!(second.action.note.as_deref(), Some("via alias"));

        // Re-export and re-import is idempotent for notes: empty notes stay None.
        let target2 = conn();
        import_backup(&target2, &file).unwrap();
        let listed2 = quick_actions::list_quick_actions(&target2).unwrap();
        let plain2 = listed2.iter().find(|a| a.action.name == "Plain").unwrap();
        assert_eq!(plain2.action.note, None);
        let noted2 = listed2.iter().find(|a| a.action.name == "Noted").unwrap();
        assert_eq!(noted2.action.note.as_deref(), Some("hello note\nsecond line"));
    }

    #[test]
    fn quick_action_auto_run_survives_backup_roundtrip() {
        // The flag is machine-local backup data: exported, imported, and
        // preserved alongside the action's other fields — and never part of
        // Preset documents, which carry requirements only.
        let source = conn();
        let mut flagged = action("Starter");
        flagged.command = "echo starter".into();
        flagged.auto_run = true;
        quick_actions::create_quick_action(&source, &flagged).unwrap();
        let mut manual = action("Manual");
        manual.command = "echo manual".into();
        quick_actions::create_quick_action(&source, &manual).unwrap();

        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(&dir, "backup.json", "");
        export_backup(&source, &file, &BackupSelection::all()).unwrap();

        // The file carries the flag.
        let on_disk: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        let actions = on_disk["quick_actions"].as_array().unwrap();
        assert_eq!(actions.len(), 2);
        let starter = actions.iter().find(|a| a["name"] == "Starter").unwrap();
        assert_eq!(starter["auto_run"], true);
        let manual = actions.iter().find(|a| a["name"] == "Manual").unwrap();
        assert_eq!(manual["auto_run"], false);

        let target = conn();
        import_backup(&target, &file).unwrap();
        let listed = quick_actions::list_quick_actions(&target).unwrap();
        assert_eq!(listed.len(), 2);
        assert!(listed.iter().find(|a| a.action.name == "Starter").unwrap().action.auto_run);
        assert!(!listed.iter().find(|a| a.action.name == "Manual").unwrap().action.auto_run);
    }

    #[test]
    fn show_in_dock_survives_backup_roundtrip() {
        // Per-item dock visibility travels in the whole-app backup for all
        // three collections — hidden stays hidden, visible stays visible.
        let source = conn();
        let mut hidden_entry = app_entry("HiddenApp");
        hidden_entry.show_in_dock = false;
        launch::create_launch_entry(&source, &hidden_entry).unwrap();
        launch::create_launch_entry(&source, &app_entry("ShownApp")).unwrap();
        let mut hidden_action = action("HiddenAction");
        hidden_action.command = "echo hidden".into();
        hidden_action.show_in_dock = false;
        quick_actions::create_quick_action(&source, &hidden_action).unwrap();
        let mut shown_action = action("ShownAction");
        shown_action.command = "echo shown".into();
        quick_actions::create_quick_action(&source, &shown_action).unwrap();
        clips::create_clip(&source, &ClipInput {
            name: "hidden".into(),
            content: "hidden text".into(),
            show_in_dock: false,
            image: None,
        })
        .unwrap();
        clips::create_clip(&source, &ClipInput {
            name: "shown".into(),
            content: "shown text".into(),
            show_in_dock: true,
            image: None,
        })
        .unwrap();

        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(&dir, "backup.json", "");
        export_backup(&source, &file, &BackupSelection::all()).unwrap();

        let on_disk: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(on_disk["launch_entries"][0]["show_in_dock"], false);
        assert_eq!(on_disk["quick_actions"][0]["show_in_dock"], false);
        assert_eq!(on_disk["clips"][0]["show_in_dock"], false);

        let target = conn();
        import_backup(&target, &file).unwrap();
        let entries = launch::list_launch_entries(&target).unwrap();
        assert!(!entries.iter().find(|e| e.entry.name == "HiddenApp").unwrap().entry.show_in_dock);
        assert!(entries.iter().find(|e| e.entry.name == "ShownApp").unwrap().entry.show_in_dock);
        let actions = quick_actions::list_quick_actions(&target).unwrap();
        assert!(!actions.iter().find(|a| a.action.name == "HiddenAction").unwrap().action.show_in_dock);
        assert!(actions.iter().find(|a| a.action.name == "ShownAction").unwrap().action.show_in_dock);
        let clips = clips::list_clips(&target).unwrap();
        assert!(!clips.iter().find(|c| c.clip.content == "hidden text").unwrap().clip.show_in_dock);
        assert!(clips.iter().find(|c| c.clip.content == "shown text").unwrap().clip.show_in_dock);
    }

    #[test]
    fn legacy_backup_without_show_in_dock_reads_as_visible() {
        // Files written before the flag carry no `show_in_dock` key — they
        // must restore as visible, never hidden.
        let c = conn();
        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(
            &dir,
            "legacy.json",
            r#"{
              "kind":"sprout-backup","version":1,"exported_at":0,
              "products":[],"presets":[],
              "launch_entries":[{"name":"OldApp","kind":"app","target":"C:\\Apps\\OldApp.lnk","shell":null,"show_window":false,"desktop_id":null}],
              "quick_actions":[{"name":"OldAction","command":"echo old","cwd":null,"stoppable":false,"stop_command":null,"note":null,"auto_run":false}],
              "clips":[{"name":"old","content":"old text"}]
            }"#,
        );
        import_backup(&c, &file).unwrap();
        let entries = launch::list_launch_entries(&c).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].entry.show_in_dock);
        let actions = quick_actions::list_quick_actions(&c).unwrap();
        assert_eq!(actions.len(), 1);
        assert!(actions[0].action.show_in_dock);
        let clips = clips::list_clips(&c).unwrap();
        assert_eq!(clips.len(), 1);
        assert!(clips[0].clip.show_in_dock);
    }

    #[test]
    fn single_quick_action_export_restores_through_the_ordinary_flow() {
        // One row's export is the same versioned envelope with a one-element
        // array and four empty siblings, so inspect/import treat it like any
        // selective export that happened to hold one action.
        let source = conn();
        let mut first = action("Build");
        first.command = "docker compose up -d".into();
        first.cwd = Some(r"D:\Stack".into());
        let stored_first = quick_actions::create_quick_action(&source, &first).unwrap();
        let mut second = action("Other");
        second.command = "echo other".into();
        quick_actions::create_quick_action(&source, &second).unwrap();

        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(&dir, "single.json", "");
        let counts = export_quick_action(&source, &file, stored_first.id).unwrap();
        assert_eq!(
            counts,
            BackupCounts {
                quick_actions: 1,
                ..BackupCounts::default()
            }
        );

        let on_disk: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(on_disk["kind"], BACKUP_KIND);
        assert_eq!(on_disk["version"], BACKUP_VERSION);
        assert_eq!(on_disk["products"].as_array().unwrap().len(), 0);
        assert_eq!(on_disk["presets"].as_array().unwrap().len(), 0);
        assert_eq!(on_disk["launch_entries"].as_array().unwrap().len(), 0);
        assert_eq!(on_disk["quick_actions"].as_array().unwrap().len(), 1);
        assert_eq!(on_disk["clips"].as_array().unwrap().len(), 0);
        assert_eq!(on_disk["quick_actions"][0]["name"], "Build");
        assert_eq!(on_disk["quick_actions"][0]["command"], "docker compose up -d");
        assert_eq!(on_disk["quick_actions"][0]["shell"], "powershell");

        assert_eq!(
            inspect_backup(&file).unwrap(),
            BackupCounts {
                quick_actions: 1,
                ..BackupCounts::default()
            }
        );
        let target = conn();
        let summary = import_backup(&target, &file).unwrap();
        assert_eq!(summary.inserted.quick_actions, 1);
        assert_eq!(summary.skipped, BackupCounts::default());
        let listed = quick_actions::list_quick_actions(&target).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].action.name, "Build");
        assert_eq!(listed[0].action.command, "docker compose up -d");
        assert_eq!(listed[0].action.cwd.as_deref(), Some(r"D:\Stack"));

        // Restoring the same file again skips — the identity already exists.
        let again = import_backup(&target, &file).unwrap();
        assert_eq!(again.inserted, BackupCounts::default());
        assert_eq!(again.skipped.quick_actions, 1);
    }

    #[test]
    fn single_quick_action_export_identity_is_payload_not_name() {
        // Same shell+command+cwd under a different name skips; same name with
        // a different payload lands — the merge never consults the name.
        let source = conn();
        let mut exported = action("Build");
        exported.command = "docker compose up -d".into();
        let stored = quick_actions::create_quick_action(&source, &exported).unwrap();
        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(&dir, "single.json", "");
        export_quick_action(&source, &file, stored.id).unwrap();

        let target = conn();
        let mut twin = action("Renamed twin");
        twin.command = "DOCKER COMPOSE UP -D".into();
        quick_actions::create_quick_action(&target, &twin).unwrap();
        let summary = import_backup(&target, &file).unwrap();
        assert_eq!(summary.inserted.quick_actions, 0);
        assert_eq!(summary.skipped.quick_actions, 1);
        assert_eq!(quick_actions::list_quick_actions(&target).unwrap().len(), 1);

        let other = conn();
        let mut namesake = action("Build");
        namesake.command = "echo something else".into();
        quick_actions::create_quick_action(&other, &namesake).unwrap();
        let summary = import_backup(&other, &file).unwrap();
        assert_eq!(summary.inserted.quick_actions, 1);
        assert_eq!(summary.skipped.quick_actions, 0);
        assert_eq!(quick_actions::list_quick_actions(&other).unwrap().len(), 2);
    }

    #[test]
    fn single_quick_action_export_of_a_missing_row_fails_cleanly() {
        let source = conn();
        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(&dir, "single.json", "");
        let err = export_quick_action(&source, &file, 999).unwrap_err();
        assert!(err.contains("gone"), "got: {err}");
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "",
            "the refusal happens before serialization"
        );
    }

    #[test]
    fn same_command_under_a_different_shell_is_a_distinct_action() {
        let source = conn();
        let mut exported = action("PsBuild");
        exported.shell = quick_actions::QuickActionShell::Powershell;
        exported.command = "echo same".into();
        let stored = quick_actions::create_quick_action(&source, &exported).unwrap();
        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(&dir, "single.json", "");
        export_quick_action(&source, &file, stored.id).unwrap();

        let target = conn();
        let mut cmd_twin = action("CmdBuild");
        cmd_twin.shell = quick_actions::QuickActionShell::Cmd;
        cmd_twin.command = "echo same".into();
        quick_actions::create_quick_action(&target, &cmd_twin).unwrap();
        let summary = import_backup(&target, &file).unwrap();
        assert_eq!(summary.inserted.quick_actions, 1);
        assert_eq!(summary.skipped.quick_actions, 0);
        assert_eq!(quick_actions::list_quick_actions(&target).unwrap().len(), 2);
    }

    #[test]
    fn version1_backup_without_shell_reads_as_powershell() {
        let c = conn();
        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(
            &dir,
            "v1.json",
            r#"{
              "kind":"sprout-backup","version":1,"exported_at":0,
              "products":[],"presets":[],"launch_entries":[],
              "quick_actions":[{"name":"Legacy","command":"echo legacy","cwd":null,"stoppable":false,"stop_command":null,"note":null,"auto_run":false,"show_in_dock":true}],
              "clips":[]
            }"#,
        );
        let summary = import_backup(&c, &file).unwrap();
        assert_eq!(summary.inserted.quick_actions, 1);
        let listed = quick_actions::list_quick_actions(&c).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].action.shell, quick_actions::QuickActionShell::Powershell);
    }

    #[test]
    fn version1_backup_declaring_cmd_is_rejected_before_any_write() {
        let c = conn();
        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(
            &dir,
            "v1cmd.json",
            r#"{
              "kind":"sprout-backup","version":1,"exported_at":0,
              "products":[],"presets":[],"launch_entries":[],
              "quick_actions":[{"name":"Sneaky","shell":"cmd","command":"echo hi","cwd":null,"stoppable":false,"stop_command":null}],
              "clips":[]
            }"#,
        );
        let err = import_backup(&c, &file).unwrap_err();
        assert!(err.contains("version 1") && err.contains("cmd"), "got: {err}");
        assert!(quick_actions::list_quick_actions(&c).unwrap().is_empty());
    }

    #[test]
    fn version2_backup_without_shell_is_rejected_before_any_write() {
        let c = conn();
        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(
            &dir,
            "v2noshell.json",
            r#"{
              "kind":"sprout-backup","version":2,"exported_at":0,
              "products":[],"presets":[],"launch_entries":[],
              "quick_actions":[{"name":"NoShell","command":"echo hi","cwd":null,"stoppable":false,"stop_command":null}],
              "clips":[]
            }"#,
        );
        let err = import_backup(&c, &file).unwrap_err();
        assert!(err.contains("not a valid Sprout backup"), "got: {err}");
        assert!(quick_actions::list_quick_actions(&c).unwrap().is_empty());
    }

    #[test]
    fn version2_backup_with_unknown_shell_is_rejected_before_any_write() {
        let c = conn();
        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(
            &dir,
            "v2bad.json",
            r#"{
              "kind":"sprout-backup","version":2,"exported_at":0,
              "products":[],"presets":[],"launch_entries":[],
              "quick_actions":[{"name":"Weird","shell":"none","command":"echo hi","cwd":null,"stoppable":false,"stop_command":null}],
              "clips":[]
            }"#,
        );
        let err = import_backup(&c, &file).unwrap_err();
        assert!(err.contains("not a valid Sprout backup"), "got: {err}");
        assert!(quick_actions::list_quick_actions(&c).unwrap().is_empty());
    }

    #[test]
    fn legacy_version1_reader_rejects_version2_rather_than_misreading_cmd() {
        let source = conn();
        let mut cmd_action = action("CmdOnly");
        cmd_action.shell = quick_actions::QuickActionShell::Cmd;
        cmd_action.command = "echo cmd-text".into();
        let stored = quick_actions::create_quick_action(&source, &cmd_action).unwrap();
        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(&dir, "v2.json", "");
        export_quick_action(&source, &file, stored.id).unwrap();
        let on_disk: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(on_disk["version"], BACKUP_VERSION);
        assert_eq!(on_disk["quick_actions"][0]["shell"], "cmd");
        let version = on_disk["version"].as_u64().unwrap();
        assert!(
            version != BACKUP_LEGACY_VERSION as u64,
            "a strict version-1 reader accepts only version 1 and must refuse this file"
        );
    }

    #[test]
    fn failed_restore_leaves_nothing_behind() {
        let c = conn();
        db::create_product(&c, &product("git")).unwrap();
        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(
            &dir,
            "bad.json",
            r#"{
              "kind":"sprout-backup","version":2,"exported_at":0,
              "products":[{"id":"git","name":"git display","winget_id":"Vendor.git","install_location_hint":null,"install_dir":null,"default_env":[]}],
              "presets":[],"launch_entries":[],
              "quick_actions":[{"name":"Bad","shell":"none","command":"echo hi","cwd":null,"stoppable":false,"stop_command":null}],
              "clips":[]
            }"#,
        );
        assert!(import_backup(&c, &file).is_err());
        assert_eq!(db::list_products(&c, None).unwrap().len(), 1);
        assert!(quick_actions::list_quick_actions(&c).unwrap().is_empty());
    }

    fn action_with_files(conn: &Connection, name: &str) -> i64 {
        let mut input = action(name);
        input.command = "process <FilesDir>\\in.txt".into();
        let stored = quick_actions::create_quick_action(conn, &input).unwrap();
        quick_actions::attach_quick_action_file(conn, stored.id, "in.txt", b"input-bytes")
            .unwrap();
        quick_actions::attach_quick_action_file(conn, stored.id, "cfg.ini", b"[main]\n")
            .unwrap();
        stored.id
    }

    #[test]
    fn whole_app_backup_carries_file_bytes_and_restores_them() {
        let source = conn();
        action_with_files(&source, "WithFiles");
        let mut bare = action("Bare");
        bare.command = "echo bare".into();
        quick_actions::create_quick_action(&source, &bare).unwrap();

        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(&dir, "backup.json", "");
        let counts = export_backup(&source, &file, &BackupSelection::all()).unwrap();
        assert_eq!(counts.quick_actions, 2);

        let on_disk: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        let files = on_disk["quick_action_files"].as_array().unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|f| f["bytes_b64"].as_str().unwrap().len() > 0));

        let target = conn();
        let summary = import_backup(&target, &file).unwrap();
        assert_eq!(summary.inserted.quick_actions, 2);
        assert_eq!(summary.skipped, BackupCounts::default());
        let listed = quick_actions::list_quick_actions(&target).unwrap();
        let restored = listed
            .iter()
            .find(|a| a.action.name == "WithFiles")
            .unwrap();
        let blobs =
            quick_actions::list_quick_action_file_blobs(&target, restored.id).unwrap();
        assert_eq!(blobs.len(), 2);
        assert_eq!(
            blobs.iter().find(|(n, _)| n == "in.txt").unwrap().1,
            b"input-bytes"
        );

        let again = import_backup(&target, &file).unwrap();
        assert_eq!(again.inserted, BackupCounts::default());
        assert_eq!(again.skipped.quick_actions, 2);
    }

    #[test]
    fn fileless_single_export_stays_plain_json() {
        let source = conn();
        let stored = quick_actions::create_quick_action(&source, &action("Bare")).unwrap();
        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(&dir, "single.json", "");
        export_quick_action(&source, &file, stored.id).unwrap();
        let raw = fs::read(&file).unwrap();
        assert_eq!(raw[0], b'{', "a fileless export stays the same JSON shape");
        let target = conn();
        let summary = import_backup(&target, &file).unwrap();
        assert_eq!(summary.inserted.quick_actions, 1);
        let listed = quick_actions::list_quick_actions(&target).unwrap();
        assert!(quick_actions::list_quick_action_files(&target, listed[0].id).unwrap().is_empty());
    }

    #[test]
    fn single_export_with_files_is_a_zip_bundle_with_honest_counts() {
        let source = conn();
        let id = action_with_files(&source, "Bundled");
        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(&dir, "single.zip", "");
        let counts = export_quick_action(&source, &file, id).unwrap();
        assert_eq!(
            counts,
            BackupCounts {
                quick_actions: 1,
                ..BackupCounts::default()
            }
        );
        let raw = fs::read(&file).unwrap();
        assert!(raw.starts_with(b"PK\x03\x04"), "a files export is a zip bundle");

        let target = conn();
        assert_eq!(
            inspect_backup(&file).unwrap(),
            BackupCounts {
                quick_actions: 1,
                ..BackupCounts::default()
            }
        );
        let summary = import_backup(&target, &file).unwrap();
        assert_eq!(summary.inserted.quick_actions, 1);
        assert_eq!(summary.skipped, BackupCounts::default());
        let listed = quick_actions::list_quick_actions(&target).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].action.command, "process <FilesDir>\\in.txt");
        let blobs =
            quick_actions::list_quick_action_file_blobs(&target, listed[0].id).unwrap();
        assert_eq!(blobs.len(), 2);

        // Same names with different bytes still skip: bytes never decide identity.
        let other = conn();
        let twin = quick_actions::create_quick_action(&other, &action("Twin")).unwrap();
        {
            let mut updated = quick_actions::get_quick_action(&other, twin.id)
                .unwrap()
                .unwrap();
            updated.action.command = "process <FilesDir>\\in.txt".into();
            quick_actions::update_quick_action(&other, &updated).unwrap();
        }
        quick_actions::attach_quick_action_file(&other, twin.id, "in.txt", b"other-bytes")
            .unwrap();
        quick_actions::attach_quick_action_file(&other, twin.id, "cfg.ini", b"other")
            .unwrap();
        let summary = import_backup(&other, &file).unwrap();
        assert_eq!(summary.inserted.quick_actions, 0);
        assert_eq!(summary.skipped.quick_actions, 1);
        let blobs = quick_actions::list_quick_action_file_blobs(&other, twin.id).unwrap();
        assert_eq!(
            blobs.iter().find(|(n, _)| n == "in.txt").unwrap().1,
            b"other-bytes",
            "a skipped restore never overwrites stored bytes"
        );
    }

    #[test]
    fn same_command_with_different_files_is_a_distinct_action() {
        let source = conn();
        let id = action_with_files(&source, "Bundled");
        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(&dir, "single.zip", "");
        export_quick_action(&source, &file, id).unwrap();

        let target = conn();
        let mut input = action("SameCommandNoFiles");
        input.command = "process <FilesDir>\\in.txt".into();
        quick_actions::create_quick_action(&target, &input).unwrap();
        let summary = import_backup(&target, &file).unwrap();
        assert_eq!(summary.inserted.quick_actions, 1);
        assert_eq!(summary.skipped.quick_actions, 0);
        assert_eq!(quick_actions::list_quick_actions(&target).unwrap().len(), 2);
    }

    #[test]
    fn crafted_file_entries_fail_before_anything_is_written() {
        let c = conn();
        let dir = tempfile::tempdir().unwrap().into_path();
        // Traversal name inside the files array.
        let bad = write_file(
            &dir,
            "traversal.json",
            r#"{
              "kind":"sprout-backup","version":2,"exported_at":0,
              "products":[],"presets":[],"launch_entries":[],
              "quick_actions":[{"name":"A","shell":"powershell","command":"echo hi","cwd":null,"stoppable":false,"stop_command":null,"note":null,"auto_run":false,"show_in_dock":true}],
              "quick_action_files":[{"action":0,"filename":"../evil.txt","size":3,"bytes_b64":"aGk="}],
              "clips":[]
            }"#,
        );
        let err = import_backup(&c, &bad).unwrap_err();
        assert!(err.contains("plain file name") || err.contains("usable file name"), "got: {err}");
        assert!(quick_actions::list_quick_actions(&c).unwrap().is_empty());

        // Oversized content.
        let big = B64.encode(vec![0u8; quick_actions::MAX_ACTION_FILE_BYTES + 1]);
        let doc = serde_json::json!({
           "kind":"sprout-backup","version":2,"exported_at":0,
           "products":[],"presets":[],"launch_entries":[],
           "quick_actions":[{"name":"A","shell":"powershell","command":"echo hi","cwd":null,"stoppable":false,"stop_command":null,"note":null,"auto_run":false,"show_in_dock":true}],
           "quick_action_files":[{"action":0,"filename":"big.bin","size": quick_actions::MAX_ACTION_FILE_BYTES as u64 + 1,"bytes_b64": big}],
           "clips":[]
        });
        let big_file = write_file(&dir, "big.json", &doc.to_string());
        let err = import_backup(&c, &big_file).unwrap_err();
        assert!(err.contains("5 MB"), "got: {err}");
        assert!(quick_actions::list_quick_actions(&c).unwrap().is_empty());

        // Zip bundle with an escaping entry.
        let zip = write_zip(&[
            (
                "action.json".into(),
                br#"{"kind":"sprout-backup","version":2,"exported_at":0,"products":[],"presets":[],"launch_entries":[],"quick_actions":[{"name":"A","shell":"powershell","command":"echo hi","cwd":null,"stoppable":false,"stop_command":null,"note":null,"auto_run":false,"show_in_dock":true}],"quick_action_files":[],"clips":[]}"#.to_vec(),
            ),
            ("files/../../evil.txt".into(), b"hi".to_vec()),
        ]);
        let zip_file = dir.join("evil.zip");
        fs::write(&zip_file, zip).unwrap();
        let err = import_backup(&c, zip_file.to_str().unwrap()).unwrap_err();
        assert!(err.contains("unsafe entry") || err.contains("unexpected entry"), "got: {err}");
        assert!(quick_actions::list_quick_actions(&c).unwrap().is_empty());
    }

    #[test]
    fn zip_writer_roundtrips_through_the_reader() {
        let entries = vec![
            ("action.json".to_string(), b"{}".to_vec()),
            ("files/a.txt".to_string(), b"hello".to_vec()),
        ];
        let raw = write_zip(&entries);
        assert!(is_zip_magic(&raw));
        assert_eq!(read_zip(&raw).unwrap(), entries);
    }

    #[test]
    fn quick_action_pre_action_survives_backup_roundtrip() {
        let source = conn();
        let mut guarded = action("Guarded");
        guarded.command = "docker compose up -d".into();
        guarded.pre_check = Some("docker info".into());
        guarded.pre_fix = Some("docker compose pull".into());
        quick_actions::create_quick_action(&source, &guarded).unwrap();
        let mut plain = action("Plain");
        plain.command = "echo plain-hi".into();
        quick_actions::create_quick_action(&source, &plain).unwrap();

        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(&dir, "backup.json", "");
        export_backup(&source, &file, &BackupSelection::all()).unwrap();

        let on_disk: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        let actions = on_disk["quick_actions"].as_array().unwrap();
        assert_eq!(actions.len(), 2);
        let guarded_json = actions.iter().find(|a| a["name"] == "Guarded").unwrap();
        assert_eq!(guarded_json["pre_check"], "docker info");
        assert_eq!(guarded_json["pre_fix"], "docker compose pull");

        let target = conn();
        let summary = import_backup(&target, &file).unwrap();
        assert_eq!(summary.inserted.quick_actions, 2);
        let listed = quick_actions::list_quick_actions(&target).unwrap();
        let restored = listed.iter().find(|a| a.action.name == "Guarded").unwrap();
        assert_eq!(restored.action.pre_check.as_deref(), Some("docker info"));
        assert_eq!(restored.action.pre_fix.as_deref(), Some("docker compose pull"));
        let restored_plain = listed.iter().find(|a| a.action.name == "Plain").unwrap();
        assert_eq!(restored_plain.action.pre_check, None);
        assert_eq!(restored_plain.action.pre_fix, None);

        // Re-importing skips on the extended identity with honest counts.
        let again = import_backup(&target, &file).unwrap();
        assert_eq!(again.inserted.quick_actions, 0);
        assert_eq!(again.skipped.quick_actions, 2);
    }

    #[test]
    fn quick_action_pre_action_absent_in_older_files_reads_as_none() {
        // Files written before the section carry no such keys — they restore
        // as actions with no pre-action, like every additive field before.
        let c = conn();
        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(
            &dir,
            "nopre.json",
            r#"{
              "kind":"sprout-backup","version":2,"exported_at":0,
              "products":[],"presets":[],"launch_entries":[],
              "quick_actions":[{"name":"OldAction","shell":"powershell","command":"echo old","cwd":null,"stoppable":false,"stop_command":null,"note":null,"auto_run":false,"show_in_dock":true}],
              "clips":[]
            }"#,
        );
        let summary = import_backup(&c, &file).unwrap();
        assert_eq!(summary.inserted.quick_actions, 1);
        let listed = quick_actions::list_quick_actions(&c).unwrap();
        assert_eq!(listed[0].action.pre_check, None);
        assert_eq!(listed[0].action.pre_fix, None);
    }

    #[test]
    fn merge_identity_ignores_pre_action_fields() {
        // Same shell+command+cwd with different pre-action text is the same
        // action for merge purposes: skipped, never duplicated.
        let target = conn();
        let mut existing = action("Builder");
        existing.pre_check = Some("exit 0".into());
        quick_actions::create_quick_action(&target, &existing).unwrap();

        let source = conn();
        let mut incoming = action("Build");
        incoming.pre_check = Some("different check".into());
        incoming.pre_fix = Some("some fix".into());
        quick_actions::create_quick_action(&source, &incoming).unwrap();
        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(&dir, "backup.json", "");
        export_backup(&source, &file, &BackupSelection::all()).unwrap();

        let summary = import_backup(&target, &file).unwrap();
        assert_eq!(summary.inserted.quick_actions, 0);
        assert_eq!(summary.skipped.quick_actions, 1);
        assert_eq!(quick_actions::list_quick_actions(&target).unwrap().len(), 1);
    }

    // ---- ticket 178: image Clips ride the same document additively ----

    /// Encodes a solid RGBA PNG — the backup tests' image fixture (no
    /// checked-in binary blobs; the `png` dependency is already direct).
    fn png_fixture(width: u32, height: u32, fill: u8) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer
                .write_image_data(&vec![fill; (width * height * 4) as usize])
                .unwrap();
        }
        out
    }

    #[test]
    fn image_clips_roundtrip_mixed_with_text() {
        // Text + image Clips export together and restore with true counts;
        // the image bytes survive byte-for-byte with their metadata.
        let source = conn();
        clips::create_clip(
            &source,
            &ClipInput { name: "reply".into(), content: "Thanks!".into(), show_in_dock: true, image: None },
        )
        .unwrap();
        let shot = png_fixture(2, 2, 0xAB);
        clips::create_clip_image(&source, "shot", &shot).unwrap();

        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(&dir, "backup.json", "");
        let counts = export_backup(&source, &file, &BackupSelection::all()).unwrap();
        assert_eq!(counts.clips, 2);

        // The document carries the image inline; text records gain no key.
        let on_disk: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        let record = &on_disk["clips"][1];
        assert_eq!(record["image"]["mime"], "image/png");
        assert_eq!(record["image"]["width"], 2);
        assert_eq!(record["image"]["height"], 2);
        assert!(!record["image"]["hash"].as_str().unwrap().is_empty());
        assert!(on_disk["clips"][0].get("image").is_none());

        let target = conn();
        let summary = import_backup(&target, &file).unwrap();
        assert_eq!(summary.inserted.clips, 2);
        assert_eq!(summary.skipped.clips, 0);
        let restored = clips::list_clips(&target).unwrap();
        assert_eq!(restored.len(), 2);
        let image_clip = restored.iter().find(|c| c.clip.image.is_some()).unwrap();
        let image = image_clip.clip.image.as_ref().unwrap();
        assert_eq!(image.mime, "image/png");
        assert_eq!((image.width, image.height), (2, 2));
        assert_eq!(
            B64.decode(image.bytes_base64.trim()).unwrap(),
            shot
        );
        // Re-import is a no-op with honest skip counts.
        let again = import_backup(&target, &file).unwrap();
        assert_eq!(again.inserted.clips, 0);
        assert_eq!(again.skipped.clips, 2);
    }

    #[test]
    fn image_identity_is_bytes_hash_not_name() {
        // Same bytes under a new name skips; same name with new bytes lands.
        let source = conn();
        let shot = png_fixture(2, 2, 0xAB);
        clips::create_clip_image(&source, "shot", &shot).unwrap();
        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(&dir, "backup.json", "");
        export_backup(&source, &file, &BackupSelection::all()).unwrap();

        let target = conn();
        clips::create_clip_image(&target, "renamed", &shot).unwrap();
        let summary = import_backup(&target, &file).unwrap();
        assert_eq!(summary.inserted.clips, 0, "same bytes = same image");
        assert_eq!(summary.skipped.clips, 1);

        let other = png_fixture(2, 2, 0xCD);
        let source2 = conn();
        clips::create_clip_image(&source2, "shot", &other).unwrap();
        let file2 = write_file(&dir, "backup2.json", "");
        export_backup(&source2, &file2, &BackupSelection::all()).unwrap();
        let summary2 = import_backup(&target, &file2).unwrap();
        assert_eq!(summary2.inserted.clips, 1, "same name, new bytes = new clip");
        assert_eq!(clips::list_clips(&target).unwrap().len(), 2);
    }

    #[test]
    fn image_clips_do_not_collide_with_text_content() {
        // The text merge set excludes image rows: an image's blank content
        // never swallows a text record, and text never swallows an image.
        let source = conn();
        let shot = png_fixture(1, 1, 0xAB);
        clips::create_clip_image(&source, "shot", &shot).unwrap();
        let dir = tempfile::tempdir().unwrap().into_path();
        let file = write_file(&dir, "backup.json", "");
        export_backup(&source, &file, &BackupSelection::all()).unwrap();

        let target = conn();
        clips::create_clip(
            &target,
            &ClipInput { name: "t".into(), content: "hello".into(), show_in_dock: true, image: None },
        )
        .unwrap();
        let summary = import_backup(&target, &file).unwrap();
        assert_eq!(summary.inserted.clips, 1);
        assert_eq!(summary.skipped.clips, 0);
        assert_eq!(clips::list_clips(&target).unwrap().len(), 2);
    }
}
