//! Read-only Plan computation (spec decision 5; CONTEXT.md "Plan").
//!
//! Given selected Presets and the engine's Detections, produce the expected
//! per-Requirement actions — will install / will upgrade / already OK /
//! satisfies-by-newer / unmanaged-skip — and surface overlapping Products
//! across Presets as explicit conflicts, never resolved silently.
//!
//! This module is pure: nothing touches the machine. Detection comes in as
//! data (fake in tests), so every VersionPolicy × installed-state combination
//! is exercised without winget.

use std::cmp::Ordering;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::domain::{Preset, Requirement, VersionPolicy};
use crate::engine::Detection;

/// What the run phase would do for one Requirement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlannedAction {
    Install,
    Upgrade { from: String, to: String },
    AlreadyOk,
    SatisfiedByNewer { installed: String, pinned: String },
    UnmanagedSkip,
}

/// One way a selected Preset declares a Product, with the action that
/// declaration would produce on this machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    /// Name of the selected Preset that declares this.
    pub preset: String,
    pub requirement: Requirement,
    pub action: PlannedAction,
    pub detail: String,
}

/// One row of the Plan: a Product as declared by the selected Presets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanEntry {
    pub product_id: String,
    pub product_name: String,
    /// Declarations disagree (different policy or step across Presets) and
    /// the user must pick a candidate or opt out — never resolved silently.
    pub conflict: bool,
    /// A declaration references a Product that is no longer in the Library
    /// (ADR-0007): the entry shows "product removed from library" and is
    /// excluded from runs until the Product is re-added or the requirement
    /// is re-linked. Resolvable declarations of the same Product still run.
    #[serde(default)]
    pub unresolved: bool,
    /// Every declaration, in selection order; `candidates[0]` is the display
    /// default when there is no conflict. Dangling references never produce
    /// a candidate.
    pub candidates: Vec<Candidate>,
    /// Names of the selected Presets that declare this Product.
    pub sources: Vec<String>,
    /// The merged declaration (union of env wiring and verify commands) used
    /// when identical declarations are composed and saved as a new Preset.
    pub merged: Requirement,
}

/// The full read-only Plan for a selection of Presets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Composition {
    pub preset_names: Vec<String>,
    pub entries: Vec<PlanEntry>,
}

/// Builds the Plan for the selected Presets against the given Detections
/// (keyed by Product id). Presets targeting a platform other than this
/// machine's are rejected — a macOS Preset must never be planned on Windows.
/// Requirements whose live reference is dangling (ADR-0007) are marked
/// `unresolved` on their entry and never become runnable candidates.
pub fn compose(presets: &[Preset], detections: &HashMap<String, Detection>) -> Result<Composition, String> {
    for preset in presets {
        if !preset.platform.eq_ignore_ascii_case("windows") {
            return Err(format!(
                "Preset '{}' targets platform '{}' — only Windows presets can be planned on this machine",
                preset.name, preset.platform
            ));
        }
    }

    let mut entries: Vec<PlanEntry> = Vec::new();
    let mut index: HashMap<&str, usize> = HashMap::new();

    for preset in presets {
        for req in &preset.requirements {
            let product_id = req.product.id.as_str();
            if req.unresolved {
                // A dangling live reference (ADR-0007): mark the entry and
                // never let it become a runnable candidate.
                if let Some(&i) = index.get(product_id) {
                    let entry = &mut entries[i];
                    entry.unresolved = true;
                    entry.sources.push(preset.name.clone());
                } else {
                    index.insert(product_id, entries.len());
                    entries.push(PlanEntry {
                        product_id: req.product.id.clone(),
                        product_name: if req.product.name.trim().is_empty() {
                            req.product.id.clone()
                        } else {
                            req.product.name.clone()
                        },
                        conflict: false,
                        unresolved: true,
                        candidates: vec![],
                        sources: vec![preset.name.clone()],
                        merged: req.clone(),
                    });
                }
                continue;
            }
            let Some(&i) = index.get(product_id) else {
                index.insert(product_id, entries.len());
                entries.push(PlanEntry {
                    product_id: req.product.id.clone(),
                    product_name: req.product.name.clone(),
                    conflict: false,
                    unresolved: false,
                    candidates: vec![],
                    sources: vec![preset.name.clone()],
                    merged: req.clone(),
                });
                let entry = entries.last_mut().unwrap();
                let detection = detections.get(product_id).cloned().unwrap_or_default();
                let (action, detail) = plan_requirement(&detection, req);
                entry.candidates.push(Candidate {
                    preset: preset.name.clone(),
                    requirement: req.clone(),
                    action,
                    detail,
                });
                continue;
            };

            let entry = &mut entries[i];
            entry.sources.push(preset.name.clone());
            let detection = detections.get(product_id).cloned().unwrap_or_default();
            let (action, detail) = plan_requirement(&detection, req);
            let candidate = Candidate {
                preset: preset.name.clone(),
                requirement: req.clone(),
                action,
                detail,
            };

            if same_declaration(&entry.merged, req) {
                // Identical declaration — merge silently (union of env wiring
                // and verify commands; a product declared identically twice
                // is not an ambiguity).
                entry.merged = merge_requirement(&entry.merged, req);
                entry.candidates.push(candidate);
            } else {
                // Differing policy or step — an explicit conflict.
                entry.conflict = true;
                entry.candidates.push(candidate);
            }
        }
    }

    Ok(Composition {
        preset_names: presets.iter().map(|p| p.name.clone()).collect(),
        entries,
    })
}

/// The expected action for one Requirement given this machine's Detection.
/// The never-downgrade invariant lives here: pinned with a newer version
/// installed reports "satisfied by newer" and never plans a downgrade.
pub fn plan_requirement(detection: &Detection, requirement: &Requirement) -> (PlannedAction, String) {
    if !detection.installed {
        return (
            PlannedAction::Install,
            "not installed — will install".to_string(),
        );
    }
    if !detection.winget_managed {
        return (
            PlannedAction::UnmanagedSkip,
            "installed outside winget — update manually, winget cannot upgrade it".to_string(),
        );
    }
    let installed = detection.installed_version.as_deref().unwrap_or("?");
    match &requirement.version_policy {
        VersionPolicy::Present => (
            PlannedAction::AlreadyOk,
            format!("installed {installed} — present, never upgraded"),
        ),
        VersionPolicy::Latest => match detection.available_version.as_deref() {
            Some(available) if compare_versions(available, installed) == Ordering::Greater => (
                PlannedAction::Upgrade {
                    from: installed.to_string(),
                    to: available.to_string(),
                },
                format!("installed {installed} — upgrade to {available} available"),
            ),
            _ => (
                PlannedAction::AlreadyOk,
                format!("installed {installed} — already the newest winget knows"),
            ),
        },
        VersionPolicy::Pinned { version } => {
            if installed == version {
                (
                    PlannedAction::AlreadyOk,
                    format!("installed {installed} — pinned version is present"),
                )
            } else if compare_versions(installed, version) == Ordering::Greater {
                (
                    PlannedAction::SatisfiedByNewer {
                        installed: installed.to_string(),
                        pinned: version.clone(),
                    },
                    format!("installed {installed} is newer than pinned {version} — never downgrading"),
                )
            } else {
                (
                    PlannedAction::Upgrade {
                        from: installed.to_string(),
                        to: version.clone(),
                    },
                    format!("installed {installed} — will upgrade to pinned {version}"),
                )
            }
        }
    }
}

/// Do two declarations of the same Product agree (same version policy and
/// same step)? Env wiring and verify commands may differ — those merge.
fn same_declaration(a: &Requirement, b: &Requirement) -> bool {
    a.version_policy == b.version_policy && a.step == b.step
}

/// Merges two identical-policy declarations: env wiring and verify commands
/// are unioned (first occurrence wins per entry, duplicates dropped).
fn merge_requirement(a: &Requirement, b: &Requirement) -> Requirement {
    let mut merged = a.clone();
    for item in &b.env {
        if !a.env.iter().any(|e| e.action == item.action && e.name == item.name) {
            merged.env.push(item.clone());
        }
    }
    for check in &b.verify {
        if !a.verify.contains(check) {
            merged.verify.push(check.clone());
        }
    }
    merged
}

/// Compares two version strings with winget-ish semantics: a leading `v` is
/// ignored, segments split on `.`/`-`/`_`, numeric segments compare
/// numerically, other segments compare case-insensitively, a longer run wins
/// when every shared segment ties (trailing `.0`s do not count). Strings that
/// cannot be parsed fall back to case-insensitive lexical comparison.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let a = a.trim().trim_start_matches(['v', 'V']);
    let b = b.trim().trim_start_matches(['v', 'V']);
    if a == b {
        return Ordering::Equal;
    }
    let a_parts: Vec<&str> = a.split(['.', '-', '_']).collect();
    let b_parts: Vec<&str> = b.split(['.', '-', '_']).collect();

    for (x, y) in a_parts.iter().zip(b_parts.iter()) {
        let ordering = match (x.parse::<u64>(), y.parse::<u64>()) {
            (Ok(m), Ok(n)) => m.cmp(&n),
            _ => x.to_lowercase().cmp(&y.to_lowercase()),
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }

    // Every shared segment ties: the longer run wins unless the extra
    // segments are all zeros ("1.2.3" == "1.2.3.0").
    let a_len = significant_len(&a_parts);
    let b_len = significant_len(&b_parts);
    a_len.cmp(&b_len)
}

/// Length up to the last segment that is not a numeric zero.
fn significant_len(parts: &[&str]) -> usize {
    let mut len = parts.len();
    while len > 0 && parts[len - 1].parse::<u64>().map(|n| n == 0).unwrap_or(false) {
        len -= 1;
    }
    len
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EnvAction, EnvWiring, Product, Step, VerifyCommand};

    fn winget_req(id: &str, policy: VersionPolicy) -> Requirement {
        Requirement {
            product: Product {
                id: id.into(),
                name: id.into(),
                winget_id: Some(format!("Vendor.{id}")),
                install_location_hint: None,
                install_dir: None,
                default_env: vec![],
            },
            step: Step::Winget {
                id: format!("Vendor.{id}"),
                scope: "machine".into(),
            },
            version_policy: policy,
            depends_on: vec![],
            timeout_minutes: 10,
            env: vec![],
            verify: vec![],
            unresolved: false,
        }
    }

    fn detection(installed: bool, managed: bool, version: Option<&str>, available: Option<&str>) -> Detection {
        Detection {
            installed,
            winget_managed: managed,
            installed_version: version.map(str::to_string),
            available_version: available.map(str::to_string),
        }
    }

    fn preset(name: &str, requirements: Vec<Requirement>) -> Preset {
        Preset {
            schema_version: 1,
            platform: "windows".into(),
            name: name.into(),
            description: "test".into(),
            author: "tester".into(),
            version: "1".into(),
            requirements,
        }
    }

    #[test]
    fn not_installed_always_installs() {
        for policy in [
            VersionPolicy::Latest,
            VersionPolicy::Pinned { version: "21.0.5".into() },
            VersionPolicy::Present,
        ] {
            let (action, _) = plan_requirement(&Detection::absent(), &winget_req("x", policy));
            assert_eq!(action, PlannedAction::Install);
        }
    }

    #[test]
    fn present_never_upgrades() {
        let d = detection(true, true, Some("21.0.5"), Some("21.0.6"));
        let (action, detail) = plan_requirement(&d, &winget_req("x", VersionPolicy::Present));
        assert_eq!(action, PlannedAction::AlreadyOk);
        assert!(detail.contains("present"));
    }

    #[test]
    fn latest_upgrades_only_when_newer_available() {
        let newer = detection(true, true, Some("21.0.5"), Some("21.0.6"));
        let (action, _) = plan_requirement(&newer, &winget_req("x", VersionPolicy::Latest));
        assert_eq!(
            action,
            PlannedAction::Upgrade { from: "21.0.5".into(), to: "21.0.6".into() }
        );

        let current = detection(true, true, Some("21.0.6"), None);
        let (action, _) = plan_requirement(&current, &winget_req("x", VersionPolicy::Latest));
        assert_eq!(action, PlannedAction::AlreadyOk);

        let same = detection(true, true, Some("21.0.6"), Some("21.0.6"));
        let (action, _) = plan_requirement(&same, &winget_req("x", VersionPolicy::Latest));
        assert_eq!(action, PlannedAction::AlreadyOk);
    }

    #[test]
    fn pinned_equal_is_ok() {
        let d = detection(true, true, Some("21.0.5"), Some("21.0.6"));
        let (action, _) = plan_requirement(
            &d,
            &winget_req("x", VersionPolicy::Pinned { version: "21.0.5".into() }),
        );
        assert_eq!(action, PlannedAction::AlreadyOk);
    }

    #[test]
    fn pinned_older_upgrades_to_pinned() {
        let d = detection(true, true, Some("21.0.3"), None);
        let (action, _) = plan_requirement(
            &d,
            &winget_req("x", VersionPolicy::Pinned { version: "21.0.5".into() }),
        );
        assert_eq!(
            action,
            PlannedAction::Upgrade { from: "21.0.3".into(), to: "21.0.5".into() }
        );
    }

    #[test]
    fn pinned_newer_satisfies_by_newer_never_downgrades() {
        let d = detection(true, true, Some("21.0.6"), None);
        let (action, detail) = plan_requirement(
            &d,
            &winget_req("x", VersionPolicy::Pinned { version: "21.0.5".into() }),
        );
        assert_eq!(
            action,
            PlannedAction::SatisfiedByNewer { installed: "21.0.6".into(), pinned: "21.0.5".into() }
        );
        assert!(detail.contains("never downgrading"));
    }

    #[test]
    fn unmanaged_skips_regardless_of_policy() {
        let d = detection(true, false, None, None);
        for policy in [
            VersionPolicy::Latest,
            VersionPolicy::Pinned { version: "21.0.5".into() },
            VersionPolicy::Present,
        ] {
            let (action, detail) = plan_requirement(&d, &winget_req("x", policy));
            assert_eq!(action, PlannedAction::UnmanagedSkip);
            assert!(detail.contains("outside winget"), "{detail}");
            assert!(detail.contains("update manually"), "{detail}");
        }
    }

    #[test]
    fn compare_versions_handles_common_shapes() {
        assert_eq!(compare_versions("21.0.5", "21.0.5"), Ordering::Equal);
        assert_eq!(compare_versions("21.0.5", "21.0.6"), Ordering::Less);
        assert_eq!(compare_versions("21.0.10", "21.0.9"), Ordering::Greater);
        assert_eq!(compare_versions("v21.0.5", "21.0.5"), Ordering::Equal);
        assert_eq!(compare_versions("1.0.0", "1.0.0.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.0.0", "1.0.0.1"), Ordering::Less);
        assert_eq!(compare_versions("1.2", "1.2.3"), Ordering::Less);
        assert_eq!(compare_versions("1.0.0-beta", "1.0.0-alpha"), Ordering::Greater);
        assert_eq!(compare_versions("2.47.0", "2.47.0"), Ordering::Equal);
    }

    #[test]
    fn compare_versions_falls_back_lexically() {
        // Neither string parses numerically — case-insensitive lexical wins.
        assert_eq!(compare_versions("nightly", "stable"), Ordering::Less);
        assert_eq!(compare_versions("NIGHTLY", "nightly"), Ordering::Equal);
    }

    #[test]
    fn identical_declarations_merge_across_presets() {
        let mut req = winget_req("git", VersionPolicy::Latest);
        req.env = vec![EnvWiring {
            action: EnvAction::Prepend,
            name: "PATH".into(),
            value: "C:\\Git\\bin".into(),
        }];
        let mut other = req.clone();
        other.verify = vec![VerifyCommand {
            command: "git --version".into(),
            args: vec![],
            match_text: Some("git version".into()),
        }];
        let presets = vec![preset("A", vec![req]), preset("B", vec![other])];
        let detections = HashMap::new();
        let composition = compose(&presets, &detections).unwrap();

        assert_eq!(composition.entries.len(), 1);
        let entry = &composition.entries[0];
        assert!(!entry.conflict);
        assert_eq!(entry.sources, vec!["A", "B"]);
        assert_eq!(entry.candidates.len(), 2);
        assert_eq!(entry.merged.env.len(), 1);
        assert_eq!(entry.merged.verify.len(), 1);
    }

    #[test]
    fn overlapping_products_with_different_policies_conflict() {
        let a = winget_req("openjdk21", VersionPolicy::Latest);
        let b = winget_req(
            "openjdk21",
            VersionPolicy::Pinned { version: "21.0.5".into() },
        );
        let c = winget_req("git", VersionPolicy::Latest);
        let presets = vec![
            preset("Backend", vec![a.clone(), c.clone()]),
            preset("JDK Pin", vec![b.clone()]),
        ];
        let detections = HashMap::new();
        let composition = compose(&presets, &detections).unwrap();

        assert_eq!(composition.entries.len(), 2);
        let jdk = composition.entries.iter().find(|e| e.product_id == "openjdk21").unwrap();
        assert!(jdk.conflict);
        assert_eq!(jdk.candidates.len(), 2);
        assert_eq!(jdk.candidates[0].preset, "Backend");
        assert_eq!(jdk.candidates[1].preset, "JDK Pin");
        assert_eq!(jdk.sources, vec!["Backend", "JDK Pin"]);

        let git = composition.entries.iter().find(|e| e.product_id == "git").unwrap();
        assert!(!git.conflict);
        assert_eq!(git.candidates.len(), 1);
        assert_eq!(git.candidates[0].action, PlannedAction::Install);
    }

    #[test]
    fn same_policy_across_presets_is_not_a_conflict() {
        let presets = vec![
            preset("A", vec![winget_req("git", VersionPolicy::Latest)]),
            preset("B", vec![winget_req("git", VersionPolicy::Latest)]),
        ];
        let composition = compose(&presets, &HashMap::new()).unwrap();
        assert_eq!(composition.entries.len(), 1);
        assert!(!composition.entries[0].conflict);
        assert_eq!(composition.entries[0].sources, vec!["A", "B"]);
    }

    fn unresolved_req(id: &str) -> Requirement {
        let mut req = winget_req(id, VersionPolicy::Latest);
        req.unresolved = true;
        req.product.name = String::new();
        req
    }

    #[test]
    fn unresolved_requirements_are_marked_and_excluded_from_runs() {
        let presets = vec![preset(
            "Box",
            vec![
                unresolved_req("ghost"),
                winget_req("git", VersionPolicy::Latest),
            ],
        )];
        let composition = compose(&presets, &HashMap::new()).unwrap();
        assert_eq!(composition.entries.len(), 2);

        let ghost = composition.entries.iter().find(|e| e.product_id == "ghost").unwrap();
        assert!(ghost.unresolved);
        assert!(ghost.candidates.is_empty(), "a dangling reference must never run");
        assert_eq!(ghost.product_name, "ghost", "the id stands in for the missing name");
        assert_eq!(ghost.sources, vec!["Box"]);
        assert!(!ghost.conflict);

        let git = composition.entries.iter().find(|e| e.product_id == "git").unwrap();
        assert!(!git.unresolved);
        assert_eq!(git.candidates.len(), 1);
    }

    #[test]
    fn unresolved_and_resolvable_declarations_of_one_product_merge() {
        let presets = vec![
            preset("A", vec![winget_req("git", VersionPolicy::Latest)]),
            preset("B", vec![unresolved_req("git")]),
        ];
        let composition = compose(&presets, &HashMap::new()).unwrap();
        assert_eq!(composition.entries.len(), 1);
        let entry = &composition.entries[0];
        // The resolvable declaration still runs; the dangling one is flagged.
        assert!(entry.unresolved);
        assert_eq!(entry.candidates.len(), 1);
        assert_eq!(entry.candidates[0].preset, "A");
        assert_eq!(entry.sources, vec!["A", "B"]);
    }

    #[test]
    fn action_reflects_detection_in_composition() {
        let presets = vec![preset(
            "Box",
            vec![winget_req("git", VersionPolicy::Latest)],
        )];
        let detections = HashMap::from([(
            "git".to_string(),
            detection(true, true, Some("2.46.0"), Some("2.47.0")),
        )]);
        let composition = compose(&presets, &detections).unwrap();
        assert_eq!(
            composition.entries[0].candidates[0].action,
            PlannedAction::Upgrade { from: "2.46.0".into(), to: "2.47.0".into() }
        );
    }

    #[test]
    fn rejects_non_windows_presets() {
        let mut mac = preset("Mac box", vec![winget_req("git", VersionPolicy::Latest)]);
        mac.platform = "macos".into();
        let err = compose(&[mac], &HashMap::new()).unwrap_err();
        assert!(err.contains("macos"));
        assert!(err.contains("only Windows"));
    }

    #[test]
    fn command_step_requirements_plan_via_registry_only() {
        let req = Requirement {
            product: Product {
                id: "node-lts".into(),
                name: "Node.js LTS (via NVM)".into(),
                winget_id: None,
                install_location_hint: None,
                install_dir: None,
                default_env: vec![],
            },
            step: Step::Command {
                exe: "nvm.cmd".into(),
                args: vec!["install".into(), "lts".into()],
                success_codes: vec![0],
            },
            version_policy: VersionPolicy::Latest,
            depends_on: vec![],
            timeout_minutes: 10,
            env: vec![],
            verify: vec![],
            unresolved: false,
        };
        let installed = detection(true, false, None, None);
        let (action, _) = plan_requirement(&installed, &req);
        assert_eq!(action, PlannedAction::UnmanagedSkip);

        let absent = Detection::absent();
        let (action, _) = plan_requirement(&absent, &req);
        assert_eq!(action, PlannedAction::Install);
    }
}
