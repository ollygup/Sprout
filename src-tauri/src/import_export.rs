//! Preset export/import (ticket 03, ADR-0002).
//!
//! Export: any Library Preset serializes to a single self-contained
//! `.sprout.json` — schemaVersion, metadata, and full product definitions
//! embedded (spec decision 11) — so the file is the unit of sharing and needs
//! no knowledge of the recipient's Library or Sprout version. Import: read a
//! file back, validate it strictly, and store it immutably: imported presets
//! are stored exactly as authored and must be forked before they can be
//! edited (ADR-0005).

use std::fs;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::db;
use crate::domain::{Preset, PresetRecord};

/// The platform this build targets. Preset files tagged for another platform
/// are still imported (for reference) but with a warning, so a macOS preset
/// never silently becomes a Windows run.
pub const CURRENT_PLATFORM: &str = "windows";

/// Result of importing a `.sprout.json` into the Library.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportResult {
    pub preset: PresetRecord,
    /// Non-fatal warning (e.g. wrong-platform file), shown to the user.
    #[serde(default)]
    pub warning: Option<String>,
}

/// Serializes a Preset as a pretty-printed, self-contained `.sprout.json`.
/// The install directory is machine-local (ticket 36, ADR-0009) — every
/// requirement's product is stripped of it before serialization, so a shared
/// file never carries another machine's paths.
pub fn export_to_json(preset: &Preset) -> Result<String, String> {
    preset.validate()?;
    let mut clean = preset.clone();
    for req in &mut clean.requirements {
        req.product.install_dir = None;
    }
    serde_json::to_string_pretty(&clean)
        .map_err(|e| format!("Could not serialize the preset: {e}"))
}

/// Writes one Preset from the Library to `path` as a `.sprout.json`.
/// The file is a point-in-time snapshot: a local Preset's live references are
/// resolved first (ADR-0007). Requirements whose Product is gone from the
/// Library cannot be snapshotted — they are left out of the export.
pub fn export_preset(conn: &Connection, path: &str, preset_id: &str) -> Result<(), String> {
    let record = db::get_preset(conn, preset_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Preset '{preset_id}' is not in your library"))?;
    let mut preset = record.preset;
    preset.requirements.retain(|req| !req.unresolved);
    let json = export_to_json(&preset)?;
    fs::write(path, json).map_err(|e| format!("Could not write '{path}': {e}"))?;
    Ok(())
}

/// Reads, validates, and stores a `.sprout.json`. The imported Preset is
/// stored exactly as authored (immutable — editing requires a fork) under a
/// Library id derived from its name; a same-named Preset already in the
/// Library is rejected rather than overwritten. Any install directory a file
/// carries is stripped on the way in — it is machine-local (ticket 36,
/// ADR-0009) and never trusted from a shared file.
pub fn import_preset_file(conn: &Connection, path: &str) -> Result<ImportResult, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("Could not read '{path}': {e}"))?;
    let mut preset: Preset = serde_json::from_str(&text)
        .map_err(|e| format!("'{path}' is not a valid .sprout.json file: {e}"))?;
    for req in &mut preset.requirements {
        req.product.install_dir = None;
    }
    preset.validate()?;

    let warning = if preset.platform != CURRENT_PLATFORM {
        Some(format!(
            "'{}' targets '{}', not this {} machine — it is imported for reference, but applying it here will not work",
            preset.name, preset.platform, CURRENT_PLATFORM
        ))
    } else {
        None
    };

    let record = PresetRecord {
        id: slugify(&preset.name),
        preset,
        imported: true,
    };
    db::create_preset(conn, &record).map_err(|e| match e {
        rusqlite::Error::SqliteFailure(err, _)
            if err.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            format!(
                "A preset named '{}' already exists in your library — imported presets are immutable, so fork it if you want a variant",
                record.preset.name
            )
        }
        other => other.to_string(),
    })?;
    Ok(ImportResult {
        preset: record,
        warning,
    })
}

/// Library id shape: lowercase, `[a-z0-9-]`, at most 40 characters, runs of
/// separators collapsed (mirrors the frontend's fork-id slug).
fn slugify(value: &str) -> String {
    let slug: String = value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let mut collapsed = String::with_capacity(slug.len());
    let mut last_dash = false;
    for c in slug.chars() {
        if c == '-' {
            if last_dash {
                continue;
            }
            last_dash = true;
        } else {
            last_dash = false;
        }
        collapsed.push(c);
    }
    let trimmed = collapsed.trim_matches('-');
    trimmed[..trimmed.len().min(40)].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EnvAction, EnvWiring, Product, Requirement, Step, VersionPolicy};

    fn test_dir(suffix: &str) -> std::path::PathBuf {
        // Unique per call — libtest reuses worker threads across tests, so
        // pid+thread-id dirs collide on re-runs.
        tempfile::tempdir().unwrap().into_path().join(suffix)
    }

    fn sample_preset() -> Preset {
        Preset {
            schema_version: 1,
            platform: "windows".into(),
            name: "Backend dev box".into(),
            description: "Java 21, VSCode, DBeaver".into(),
            author: "User A".into(),
            version: "3".into(),
            requirements: vec![Requirement {
                product: Product {
                    id: "openjdk21".into(),
                    name: "Eclipse Temurin OpenJDK 21 (LTS)".into(),
                    winget_id: Some("EclipseAdoptium.Temurin.21.JDK".into()),
                    install_location_hint: Some("Eclipse Temurin".into()),
                    install_dir: None,
                    default_env: vec![],
                },
                step: Step::Winget {
                    id: "EclipseAdoptium.Temurin.21.JDK".into(),
                    scope: "machine".into(),
                },
                version_policy: VersionPolicy::Pinned {
                    version: "21.0.5".into(),
                },
                depends_on: vec![],
                timeout_minutes: 10,
                env: vec![EnvWiring {
                    action: EnvAction::Set,
                    name: "JAVA_HOME".into(),
                    value: "<InstallLocation:Eclipse Temurin>".into(),
                }],
                verify: vec![],
                unresolved: false,
            }],
        }
    }

    fn write_file(dir: &std::path::Path, name: &str, contents: &str) -> String {
        let path = dir.join(name);
        fs::write(&path, contents).unwrap();
        path.to_str().unwrap().to_string()
    }

    #[test]
    fn export_then_import_roundtrips_exactly() {
        let dir = test_dir("roundtrip");
        let conn = db::init_at(&dir).unwrap();
        // The requirement's live link must resolve at export time, so the
        // product exists in the library (created by the test — nothing is
        // seeded, ADR-0008).
        db::create_product(
            &conn,
            &Product {
                id: "openjdk21".into(),
                name: "Eclipse Temurin OpenJDK 21 (LTS)".into(),
                winget_id: Some("EclipseAdoptium.Temurin.21.JDK".into()),
                install_location_hint: Some("Eclipse Temurin".into()),
                install_dir: None,
                default_env: vec![],
            },
        )
        .unwrap();
        let record = PresetRecord {
            id: "backend-dev-box".into(),
            preset: sample_preset(),
            imported: false,
        };
        db::create_preset(&conn, &record).unwrap();

        let file = write_file(&dir, "backend-dev-box.sprout.json", "");
        export_preset(&conn, &file, "backend-dev-box").unwrap();
        // The exported file is the file shape: no Library id inside.
        let on_disk: serde_json::Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        assert!(on_disk.get("id").is_none());
        assert_eq!(on_disk["schema_version"], 1);

        // Import into a second, empty Library.
        let dir2 = test_dir("roundtrip-b");
        let conn2 = db::init_at(&dir2).unwrap();
        let result = import_preset_file(&conn2, &file).unwrap();
        assert_eq!(result.preset.preset, sample_preset());
        assert_eq!(result.warning, None);
        assert!(result.preset.imported);
        assert_eq!(result.preset.id, "backend-dev-box");
        assert_eq!(db::list_presets(&conn2).unwrap().len(), 1);
    }

    #[test]
    fn rejects_unsupported_schema_version() {
        let dir = test_dir("schema");
        let conn = db::init_at(&dir).unwrap();
        let mut preset = sample_preset();
        preset.schema_version = 2;
        let file = write_file(&dir, "future.sprout.json", &serde_json::to_string(&preset).unwrap());
        let err = import_preset_file(&conn, &file).unwrap_err();
        assert!(err.contains("Unsupported schema version"), "got: {err}");
        assert!(db::list_presets(&conn).unwrap().is_empty());
    }

    #[test]
    fn rejects_invalid_json() {
        let dir = test_dir("badjson");
        let conn = db::init_at(&dir).unwrap();
        let file = write_file(&dir, "junk.sprout.json", "this is not json");
        let err = import_preset_file(&conn, &file).unwrap_err();
        assert!(err.contains("not a valid .sprout.json"), "got: {err}");
    }

    #[test]
    fn rejects_duplicate_product_within_file() {
        let dir = test_dir("dupe");
        let conn = db::init_at(&dir).unwrap();
        let mut preset = sample_preset();
        let dup = preset.requirements[0].clone();
        preset.requirements.push(dup);
        let file = write_file(&dir, "dupe.sprout.json", &serde_json::to_string(&preset).unwrap());
        let err = import_preset_file(&conn, &file).unwrap_err();
        assert!(err.contains("more than once"), "got: {err}");
        assert!(db::list_presets(&conn).unwrap().is_empty());
    }

    #[test]
    fn wrong_platform_imports_with_warning() {
        let dir = test_dir("platform");
        let conn = db::init_at(&dir).unwrap();
        let mut preset = sample_preset();
        preset.platform = "macos".into();
        let file = write_file(&dir, "mac.sprout.json", &serde_json::to_string(&preset).unwrap());
        let result = import_preset_file(&conn, &file).unwrap();
        let warning = result.warning.expect("wrong platform must warn");
        assert!(warning.contains("macos"), "got: {warning}");
        assert!(warning.contains("windows"));
        assert!(result.preset.imported);
    }

    #[test]
    fn same_name_import_is_rejected() {
        let dir = test_dir("twice");
        let conn = db::init_at(&dir).unwrap();
        let file = write_file(&dir, "one.sprout.json", &serde_json::to_string(&sample_preset()).unwrap());
        import_preset_file(&conn, &file).unwrap();
        let err = import_preset_file(&conn, &file).unwrap_err();
        assert!(err.contains("already exists"), "got: {err}");
    }

    #[test]
    fn export_never_contains_the_install_directory() {
        // The default install directory (ticket 34) and any per-product
        // override (ticket 36, ADR-0009) are machine-local values — they
        // must never leak into a shared preset.
        let dir = test_dir("install-dir");
        let conn = db::init_at(&dir).unwrap();
        db::create_product(
            &conn,
            &Product {
                id: "openjdk21".into(),
                name: "Eclipse Temurin OpenJDK 21 (LTS)".into(),
                winget_id: Some("EclipseAdoptium.Temurin.21.JDK".into()),
                install_location_hint: None,
                install_dir: Some(r"E:\Tools".into()),
                default_env: vec![],
            },
        )
        .unwrap();
        let record = PresetRecord {
            id: "backend-dev-box".into(),
            preset: sample_preset(),
            imported: false,
        };
        db::create_preset(&conn, &record).unwrap();
        crate::settings::save(
            &conn,
            &crate::settings::Settings {
                install_dir: r"D:\Apps".into(),
                ..crate::settings::Settings::default()
            },
        )
        .unwrap();

        let file = write_file(&dir, "backend-dev-box.sprout.json", "");
        export_preset(&conn, &file, "backend-dev-box").unwrap();
        let on_disk = fs::read_to_string(&file).unwrap();
        assert!(
            !on_disk.contains(r"D:\Apps"),
            "the exported file must never carry the global install directory: {on_disk}"
        );
        assert!(
            !on_disk.contains(r"E:\Tools"),
            "the exported file must never carry the per-product install directory: {on_disk}"
        );
        assert!(
            !on_disk.contains("install_dir"),
            "the exported file must not mention an install directory at all: {on_disk}"
        );
        // The library values themselves are intact in the database.
        assert_eq!(
            crate::settings::load(&conn).install_dir,
            r"D:\Apps"
        );
        assert_eq!(
            db::get_product(&conn, "openjdk21")
                .unwrap()
                .unwrap()
                .product
                .install_dir
                .as_deref(),
            Some(r"E:\Tools")
        );
    }

    #[test]
    fn import_strips_any_install_directory_the_file_carries() {
        // A shared file might carry a per-product install directory (authored
        // by a buggy or older client) — the import must never keep it.
        let dir = test_dir("import-strip");
        let conn = db::init_at(&dir).unwrap();
        let mut preset = sample_preset();
        preset.requirements[0].product.install_dir = Some(r"E:\Tools".into());
        let file = write_file(&dir, "carrying.sprout.json", &serde_json::to_string(&preset).unwrap());
        let result = import_preset_file(&conn, &file).unwrap();
        assert_eq!(
            result.preset.preset.requirements[0].product.install_dir,
            None,
            "an imported snapshot must never carry another machine's install directory"
        );
        // And it stays stripped in the stored payload.
        let stored: serde_json::Value = serde_json::from_str(
            &conn
                .query_row("SELECT data FROM presets WHERE id = 'backend-dev-box'", [], |row| {
                    row.get::<_, String>(0)
                })
                .unwrap(),
        )
        .unwrap();
        assert!(
            stored["requirements"][0]["product"].get("install_dir").is_none(),
            "the stored payload must not carry install_dir: {stored}"
        );
    }

    #[test]
    fn export_of_missing_preset_fails() {
        let dir = test_dir("missing");
        let conn = db::init_at(&dir).unwrap();
        let err = export_preset(&conn, &dir.join("x.sprout.json").to_str().unwrap(), "no-such").unwrap_err();
        assert!(err.contains("not in your library"), "got: {err}");
    }

    #[test]
    fn export_skips_requirements_whose_product_left_the_library() {
        let dir = test_dir("dangling-export");
        let conn = db::init_at(&dir).unwrap();
        // The live reference must resolve, so the product exists in the
        // library (created by the test — nothing is seeded, ADR-0008).
        db::create_product(
            &conn,
            &Product {
                id: "git".into(),
                name: "Git".into(),
                winget_id: Some("Git.Git".into()),
                install_location_hint: None,
                install_dir: None,
                default_env: vec![],
            },
        )
        .unwrap();
        // A local preset with one live reference and one dangling one.
        let record = PresetRecord {
            id: "mixed-box".into(),
            imported: false,
            preset: Preset {
                schema_version: 1,
                platform: "windows".into(),
                name: "Mixed box".into(),
                description: "one live, one gone".into(),
                author: "Tester".into(),
                version: "1".into(),
                requirements: vec![Requirement {
                    product: Product {
                        id: "git".into(),
                        name: String::new(),
                        winget_id: None,
                        install_location_hint: None,
                        install_dir: None,
                        default_env: vec![],
                    },
                    step: Step::Winget {
                        id: "Git.Git".into(),
                        scope: "machine".into(),
                    },
                    version_policy: VersionPolicy::Latest,
                    depends_on: vec![],
                    timeout_minutes: 10,
                    env: vec![],
                    verify: vec![],
                    unresolved: false,
                }],
            },
        };
        db::create_preset(&conn, &record).unwrap();
        // Simulate legacy data: the stored payload also carries a reference
        // to a product that does not exist in the library.
        conn.execute(
            "UPDATE presets SET data = ?1 WHERE id = 'mixed-box'",
            [
                r#"{"schema_version":1,"platform":"windows","name":"Mixed box","description":"one live, one gone","author":"Tester","version":"1","requirements":[{"product":{"id":"git","name":"","winget_id":null,"install_location_hint":null,"default_env":[]},"step":{"type":"winget","id":"Git.Git","scope":"machine"},"version_policy":{"kind":"latest"},"depends_on":[],"timeout_minutes":10,"env":[],"verify":[]},{"product":{"id":"ghost","name":"","winget_id":null,"install_location_hint":null,"default_env":[]},"step":{"type":"winget","id":"Vendor.Ghost","scope":"machine"},"version_policy":{"kind":"latest"},"depends_on":[],"timeout_minutes":10,"env":[],"verify":[]}]}"#,
            ],
        )
        .unwrap();

        let file = write_file(&dir, "mixed.sprout.json", "");
        export_preset(&conn, &file, "mixed-box").unwrap();
        let on_disk: serde_json::Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        let requirements = on_disk["requirements"].as_array().unwrap();
        assert_eq!(requirements.len(), 1, "the dangling reference must not be exported");
        assert_eq!(requirements[0]["product"]["id"], "git");
        // The exported snapshot carries the live name, resolved at export time.
        assert_eq!(requirements[0]["product"]["name"], "Git");
        // The file is a valid, importable snapshot (into a fresh library).
        let dir2 = test_dir("dangling-export-b");
        let conn2 = db::init_at(&dir2).unwrap();
        assert!(import_preset_file(&conn2, &file).is_ok());
    }

    #[test]
    fn library_id_is_slugged() {
        let dir = test_dir("slug");
        let conn = db::init_at(&dir).unwrap();
        let mut preset = sample_preset();
        preset.name = "Backend Dev Box! (prod)".to_string();
        let file = write_file(&dir, "s.sprout.json", &serde_json::to_string(&preset).unwrap());
        let result = import_preset_file(&conn, &file).unwrap();
        assert_eq!(result.preset.id, "backend-dev-box-prod");
    }
}
