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
//! Clip is identified by its text — the content is what a copy restores.

use std::collections::HashSet;
use std::fs;

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

/// The only document version this build reads and writes.
pub const BACKUP_VERSION: u32 = 1;

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
    pub clips: Vec<ClipInput>,
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
    let quick_actions = if selection.quick_actions {
        quick_actions::list_quick_actions(conn)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|action| action.action)
            .collect()
    } else {
        Vec::new()
    };
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
/// with authored messages mirroring the preset-import behavior.
fn read_document(path: &str) -> Result<BackupDocument, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("Could not read '{path}': {e}"))?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|_| format!("'{path}' is not a Sprout backup file"))?;
    if value.get("kind").and_then(|k| k.as_str()) != Some(BACKUP_KIND) {
        return Err(format!(
            "'{path}' is not a Sprout backup file — a .sprout.json preset is not a whole-app backup"
        ));
    }
    match value.get("version").and_then(|v| v.as_u64()) {
        Some(v) if v == BACKUP_VERSION as u64 => {}
        Some(other) => {
            return Err(format!(
                "Unsupported backup version {other} — only version {BACKUP_VERSION} is supported"
            ))
        }
        None => return Err(format!("'{path}' is not a valid Sprout backup")),
    }
    let mut doc: BackupDocument = serde_json::from_value(value)
        .map_err(|e| format!("'{path}' is not a valid Sprout backup: {e}"))?;
    normalize(&mut doc);
    Ok(doc)
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
    for clip in &doc.clips {
        clips::validate_clip(clip)?;
    }
    Ok(())
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

    // Launch entries and Quick Actions: named things — same trimmed name
    // (case-insensitive) means kept.
    let mut entry_names: HashSet<String> = column_strings(&tx, "SELECT name FROM launch_entries")?
        .into_iter()
        .map(|name| name.trim().to_lowercase())
        .collect();
    for entry in &doc.launch_entries {
        let name_key = entry.name.trim().to_lowercase();
        if entry_names.contains(&name_key) {
            skipped.launch_entries += 1;
            continue;
        }
        launch::append_entry(&tx, entry).map_err(|e| e.to_string())?;
        entry_names.insert(name_key);
        inserted.launch_entries += 1;
    }

    let mut action_names: HashSet<String> =
        column_strings(&tx, "SELECT name FROM quick_actions")?
            .into_iter()
            .map(|name| name.trim().to_lowercase())
            .collect();
    for action in &doc.quick_actions {
        let name_key = action.name.trim().to_lowercase();
        if action_names.contains(&name_key) {
            skipped.quick_actions += 1;
            continue;
        }
        quick_actions::append_action(&tx, action).map_err(|e| e.to_string())?;
        action_names.insert(name_key);
        inserted.quick_actions += 1;
    }

    // Clips: a Clip IS its text — identical content means kept, whatever the
    // optional title says.
    let mut clip_texts: HashSet<String> = column_strings(&tx, "SELECT content FROM clips")?
        .into_iter()
        .map(|content| content.trim().to_string())
        .collect();
    for clip in &doc.clips {
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
        }
    }

    fn action(name: &str) -> QuickActionInput {
        QuickActionInput {
            name: name.into(),
            command: "docker compose up -d".into(),
            cwd: None,
            stoppable: false,
            stop_command: None,
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
        })
        .unwrap();
        clips::create_clip(conn, &ClipInput {
            name: "".into(),
            content: "git status --short".into(),
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
        assert_eq!(on_disk["version"], 1);
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
                ClipInput { name: "reply".into(), content: "Thanks for the report!".into() },
                ClipInput { name: "".into(), content: "git status --short".into() },
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
        assert_eq!(on_disk["version"], 1);
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
}
