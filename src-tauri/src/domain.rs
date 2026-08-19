//! Core domain types (see docs/CONTEXT.md for the language glossary).
//!
//! Product is persisted in the local Library; the rest of the glossary
//! (VersionPolicy, Step, Requirement, Preset, EnvWiring, VerifyCommand) is
//! defined here in preset-file shape (spec decision 11) so later tickets can
//! serialize/validate without a model rewrite.
//! Dead-code allowance: these types are consumed by tickets 02/04/05+.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// An environment-variable operation a Requirement applies after a
/// successful install. User scope only; never overwrites existing values;
/// `<InstallLocation>` / `<InstallLocation:hint>` is resolved from the
/// uninstall registry at apply time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnvWiring {
    #[serde(rename = "action")]
    pub action: EnvAction,
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EnvAction {
    Set,
    Prepend,
}

/// A thing installable on this machine — winget ID, display name,
/// install-location hint, default env suggestions, and an optional install
/// directory that overrides the global default (ticket 36, ADR-0009).
///
/// `name` is defaulted on read (ADR-0007): a local Preset's stored
/// Requirement keeps only the product id as a live reference, so the stored
/// payload serializes `{ "id": ... }` and deserializes back with an empty
/// name that resolution fills in from the Library.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Product {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub winget_id: Option<String>,
    #[serde(default)]
    pub install_location_hint: Option<String>,
    /// Machine-local override of the default install directory (ADR-0009):
    /// `None` means "use the global default from Settings". Never shared —
    /// preset files and exports never carry it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_dir: Option<String>,
    #[serde(default)]
    pub default_env: Vec<EnvWiring>,
}

/// A Product as stored in the Library: the file-shape [`Product`] plus the
/// Library-only create/update times (ticket 13). Mirrors [`PresetRecord`] —
/// the embedded value stays export-clean, the metadata stays local.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductRecord {
    #[serde(flatten)]
    pub product: Product,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
}

/// How the machine must relate to a Product's version: `latest` (upgrade to
/// newest), `pinned` (exact version), or `present` (installed, never upgraded).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum VersionPolicy {
    Latest,
    Pinned { version: String },
    Present,
}

/// The mechanism by which a Requirement is executed — `winget` or `command` —
/// described as data with executor-specific parameters.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Step {
    Winget {
        id: String,
        #[serde(default = "default_machine_scope")]
        scope: String,
    },
    Command {
        exe: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        success_codes: Vec<i32>,
    },
}

pub(crate) fn default_machine_scope() -> String {
    "machine".to_string()
}

/// A command declared on a Requirement and run after install; a non-zero exit
/// or non-matching output fails the Requirement.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerifyCommand {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Required text in the command's output; absent means "exit code only".
    #[serde(default)]
    pub match_text: Option<String>,
}

/// A declaration that the machine must have a specific Product in a specific
/// state: a VersionPolicy, an optional Step, optional Env wiring, and optional
/// verify commands.
///
/// In a local Preset the Product is a live reference resolved from the
/// Library on every read (ADR-0007): editing the Product propagates to every
/// Requirement that references it. `unresolved` flags a reference whose
/// Product is no longer in the Library — shown as "product removed from
/// library" and excluded from runs until re-linked. Imported Presets are
/// snapshots and never resolve.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Requirement {
    /// Embedded, self-contained product definition (spec decision 11); the
    /// stored form of a local Preset keeps only the id (ADR-0007).
    pub product: Product,
    pub step: Step,
    pub version_policy: VersionPolicy,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default = "default_timeout_minutes")]
    pub timeout_minutes: u32,
    #[serde(default)]
    pub env: Vec<EnvWiring>,
    #[serde(default)]
    pub verify: Vec<VerifyCommand>,
    /// Set by the resolver, never persisted and never exported (ADR-0007):
    /// the referenced Product is not in the Library right now.
    #[serde(default, skip_serializing_if = "is_false")]
    pub unresolved: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

pub(crate) fn default_timeout_minutes() -> u32 {
    10
}

/// A named, versioned, exportable set of Requirements targeting a platform.
/// The unit of sharing; immutable once imported, edited only by forking.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Preset {
    pub schema_version: u32,
    pub platform: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub version: String,
    pub requirements: Vec<Requirement>,
}

/// A Preset stored in the Library: a local id plus the preset in file shape
/// (spec decision 11), so export/import round-trips stay clean. `imported`
/// flags presets that came from a `.sprout.json` file — they are immutable
/// and must be forked before they can be edited.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PresetRecord {
    pub id: String,
    #[serde(flatten)]
    pub preset: Preset,
    #[serde(default)]
    pub imported: bool,
}

impl Preset {
    /// Validates a Preset before it is saved to the Library: a broken preset
    /// must never reach another machine. Rejects malformed metadata, malformed
    /// steps/policies/env entries/verify commands, duplicate Products within
    /// one Preset, and dependencies that are unknown or self-referential.
    pub fn validate(&self) -> std::result::Result<(), String> {
        if self.schema_version != 1 {
            return Err(format!(
                "Unsupported schema version {} — only version 1 is supported",
                self.schema_version
            ));
        }
        if self.name.trim().is_empty() {
            return Err("Preset name must not be empty".into());
        }
        if self.description.trim().is_empty() {
            return Err("Preset description must not be empty".into());
        }
        if self.platform.trim().is_empty() {
            return Err("Preset platform must not be empty".into());
        }
        if self.version.trim().is_empty() {
            return Err("Preset version must not be empty".into());
        }

        for (i, req) in self.requirements.iter().enumerate() {
            let label = format!("Requirement {} ({})", i + 1, req.product.id);
            if req.product.id.trim().is_empty() {
                return Err("Every requirement needs a product id".into());
            }
            if req.unresolved {
                // A dangling live reference (ADR-0007): the Product is not in
                // the Library, so there is no current name or step to check.
                // The requirement stays valid as a reference and is shown as
                // "product removed from library" until re-linked or edited.
                continue;
            }
            if req.product.name.trim().is_empty() {
                return Err(format!(
                    "Product '{}' needs a display name",
                    req.product.id
                ));
            }
            match &req.step {
                Step::Winget { id, .. } if id.trim().is_empty() => {
                    return Err(format!("{label} has a winget step without a winget id"));
                }
                Step::Command { exe, .. } if exe.trim().is_empty() => {
                    return Err(format!("{label} has a command step without an executable"));
                }
                _ => {}
            }
            if let VersionPolicy::Pinned { version } = &req.version_policy {
                if version.trim().is_empty() {
                    return Err(format!("{label} is pinned to a version, but no version is given"));
                }
            }
            if req.timeout_minutes == 0 {
                return Err(format!("{label} has a timeout of 0 — use at least 1 minute"));
            }
            for item in &req.env {
                if item.name.trim().is_empty() {
                    return Err(format!("{label} has an env wiring entry without a variable name"));
                }
                if item.value.trim().is_empty() {
                    return Err(format!(
                        "Env wiring '{}' on {label} needs a value",
                        item.name
                    ));
                }
            }
            for check in &req.verify {
                if check.command.trim().is_empty() {
                    return Err(format!("{label} has a verify command without a command"));
                }
            }
        }

        let mut seen = std::collections::HashSet::new();
        for req in &self.requirements {
            if !seen.insert(req.product.id.as_str()) {
                return Err(format!(
                    "Product '{}' appears more than once in this preset — each product can be declared only once",
                    req.product.id
                ));
            }
        }
        for req in &self.requirements {
            for dep in &req.depends_on {
                if dep == &req.product.id {
                    return Err(format!(
                        "Requirement '{}' depends on itself",
                        req.product.id
                    ));
                }
                if !seen.contains(dep.as_str()) {
                    return Err(format!(
                        "Requirement '{}' depends on '{}', which is not part of this preset",
                        req.product.id, dep
                    ));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn winget_product(id: &str, name: &str) -> Product {
        Product {
            id: id.into(),
            name: name.into(),
            winget_id: Some(format!("Vendor.{id}")),
            install_location_hint: None,
            install_dir: None,
            default_env: vec![],
        }
    }

    fn winget_req(id: &str) -> Requirement {
        Requirement {
            product: winget_product(id, id),
            step: Step::Winget {
                id: format!("Vendor.{id}"),
                scope: "machine".into(),
            },
            version_policy: VersionPolicy::Latest,
            depends_on: vec![],
            timeout_minutes: 10,
            env: vec![],
            verify: vec![],
            unresolved: false,
        }
    }

    fn valid_preset() -> Preset {
        Preset {
            schema_version: 1,
            platform: "windows".into(),
            name: "Backend dev box".into(),
            description: "Java 21, VSCode, DBeaver".into(),
            author: "User A".into(),
            version: "3".into(),
            requirements: vec![winget_req("openjdk21"), winget_req("vscode")],
        }
    }

    #[test]
    fn valid_preset_passes() {
        assert!(valid_preset().validate().is_ok());
    }

    #[test]
    fn rejects_blank_metadata() {
        let mut preset = valid_preset();
        preset.schema_version = 2;
        assert!(preset.validate().is_err());

        let mut preset = valid_preset();
        preset.name = "  ".into();
        assert!(preset.validate().is_err());

        let mut preset = valid_preset();
        preset.description = "".into();
        assert!(preset.validate().is_err());

        let mut preset = valid_preset();
        preset.platform = "".into();
        assert!(preset.validate().is_err());

        let mut preset = valid_preset();
        preset.version = "  ".into();
        assert!(preset.validate().is_err());
    }

    #[test]
    fn rejects_duplicate_product_within_preset() {
        let mut preset = valid_preset();
        preset.requirements.push(winget_req("openjdk21"));
        let err = preset.validate().unwrap_err();
        assert!(err.contains("openjdk21"), "got: {err}");
        assert!(err.contains("more than once"));
    }

    #[test]
    fn rejects_unknown_dependency() {
        let mut preset = valid_preset();
        preset.requirements[0].depends_on = vec!["not-in-preset".into()];
        let err = preset.validate().unwrap_err();
        assert!(err.contains("not-in-preset"));
    }

    #[test]
    fn rejects_self_dependency() {
        let mut preset = valid_preset();
        preset.requirements[0].depends_on = vec!["openjdk21".into()];
        let err = preset.validate().unwrap_err();
        assert!(err.contains("depends on itself"));
    }

    #[test]
    fn valid_dependency_passes() {
        let mut preset = valid_preset();
        preset.requirements[0].depends_on = vec!["vscode".into()];
        assert!(preset.validate().is_ok());
    }

    #[test]
    fn unresolved_requirements_validate_as_references() {
        // A dangling live reference (ADR-0007) has no current name or step to
        // check — it stays valid so an edited preset can be saved as-is.
        let mut preset = valid_preset();
        preset.requirements[0].unresolved = true;
        preset.requirements[0].product.name = String::new();
        preset.requirements[0].product.winget_id = None;
        assert!(preset.validate().is_ok());

        // But it must still carry an id.
        preset.requirements[0].product.id = String::new();
        assert!(preset.validate().is_err());

        // A healthy requirement serializes without the flag — files and
        // stored payloads never carry it.
        let json = serde_json::to_string(&valid_preset()).unwrap();
        assert!(!json.contains("unresolved"), "got: {json}");
    }

    #[test]
    fn rejects_malformed_policy() {
        let mut preset = valid_preset();
        preset.requirements[0].version_policy = VersionPolicy::Pinned {
            version: " ".into(),
        };
        let err = preset.validate().unwrap_err();
        assert!(err.contains("no version is given"));

        preset.requirements[0].version_policy = VersionPolicy::Pinned {
            version: "21.0.5".into(),
        };
        assert!(preset.validate().is_ok());
    }

    #[test]
    fn rejects_malformed_env_entries() {
        let mut preset = valid_preset();
        preset.requirements[0].env = vec![EnvWiring {
            action: EnvAction::Set,
            name: "".into(),
            value: "v".into(),
        }];
        assert!(preset.validate().is_err());

        preset.requirements[0].env = vec![EnvWiring {
            action: EnvAction::Prepend,
            name: "PATH".into(),
            value: " ".into(),
        }];
        assert!(preset.validate().is_err());

        preset.requirements[0].env = vec![EnvWiring {
            action: EnvAction::Set,
            name: "JAVA_HOME".into(),
            value: "<InstallLocation:Eclipse Temurin>".into(),
        }];
        assert!(preset.validate().is_ok());
    }

    #[test]
    fn rejects_blank_verify_and_zero_timeout() {
        let mut preset = valid_preset();
        preset.requirements[0].verify = vec![VerifyCommand {
            command: " ".into(),
            args: vec![],
            match_text: None,
        }];
        assert!(preset.validate().is_err());

        let mut preset = valid_preset();
        preset.requirements[0].timeout_minutes = 0;
        assert!(preset.validate().is_err());
    }

    #[test]
    fn rejects_blank_step_fields() {
        let mut preset = valid_preset();
        preset.requirements[0].step = Step::Winget {
            id: "".into(),
            scope: "machine".into(),
        };
        let err = preset.validate().unwrap_err();
        assert!(err.contains("winget step without a winget id"));

        let mut preset = valid_preset();
        preset.requirements[0].step = Step::Command {
            exe: " ".into(),
            args: vec![],
            success_codes: vec![],
        };
        let err = preset.validate().unwrap_err();
        assert!(err.contains("command step without an executable"));

        preset.requirements[0].step = Step::Command {
            exe: "nvm.cmd".into(),
            args: vec!["install".into(), "lts".into()],
            success_codes: vec![0],
        };
        assert!(preset.validate().is_ok());
    }

    #[test]
    fn preset_serde_roundtrip_matches_file_shape() {
        let preset = valid_preset();
        let json = serde_json::to_string(&preset).unwrap();
        let back: Preset = serde_json::from_str(&json).unwrap();
        assert_eq!(back, preset);
        assert!(json.contains("\"schema_version\""));
        assert!(json.contains("\"version_policy\""));
        assert!(json.contains("\"depends_on\""));
        assert!(json.contains("\"timeout_minutes\""));
    }

    #[test]
    fn preset_record_roundtrips_with_id() {
        let record = PresetRecord {
            id: "backend-dev-box".into(),
            preset: valid_preset(),
            imported: false,
        };
        let json = serde_json::to_string(&record).unwrap();
        let back: PresetRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back, record);
    }
}
