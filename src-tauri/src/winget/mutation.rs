use std::time::Duration;
use crate::engine::StepOutcome;

/// A whitelisted winget exit — `ok` with the legacy wording (reboot flag for
/// the 3010 / 1641 / INSTALL_REBOOT_REQUIRED_TO_FINISH family).
#[derive(Debug, Clone, PartialEq, Eq)]
struct WingetReason {
    pub reboot: bool,
    pub detail: String,
}

/// Maps a winget exit code (and output) to a result — the port of the legacy
/// `Get-WingetResultReason`. winget returns nonzero codes for benign "nothing
/// to do" outcomes (already installed / already up to date), so those are
/// whitelisted per action or every re-run would look broken. `None` is a
/// genuine failure. Exit codes from the `0x8A1500xx` family exceed i32 range,
/// so the comparison uses the unsigned re-interpretation of the code.
fn classify_winget_result(action: &str, exit_code: i32, output: &str) -> Option<WingetReason> {
    // Message guard first (ticket 27): winget reuses NO_APPLICATIONS_FOUND
    // (0x8A150014) for the install-of-missing-package case, where the
    // whitelist's "already up to date" semantic would report a fake success.
    // "No package found" is always a genuine failure, whatever the code says.
    if no_package_found(output) {
        return None;
    }
    let joined = output.to_lowercase();
    let is_no_update = [
        "no applicable upgrade",
        "no available upgrade",
        "no newer version",
        "no newer package",
        "up to date",
    ]
    .iter()
    .any(|needle| joined.contains(needle));
    let is_installed = joined.contains("already installed");

    let reason = match exit_code as u32 {
        0 => Some(WingetReason {
            reboot: false,
            detail: match action {
                "upgrade" if is_no_update => "already up to date".into(),
                "upgrade" => "upgraded".into(),
                _ => "installed".into(),
            },
        }),
        3010 => Some(WingetReason {
            reboot: true,
            detail: "installed - reboot required to finish".into(),
        }),
        1641 => Some(WingetReason {
            reboot: true,
            detail: "installed - reboot initiated".into(),
        }),
        // INSTALL_REBOOT_REQUIRED_TO_FINISH
        0x8A15_0109 => Some(WingetReason {
            reboot: true,
            detail: "installed - reboot required to finish".into(),
        }),
        // UPDATE_NOT_APPLICABLE
        0x8A15_002B => Some(WingetReason {
            reboot: false,
            detail: "already up to date".into(),
        }),
        // NO_APPLICATIONS_FOUND (no newer package in source) — the upgrade
        // semantic only: on install the same code means the id resolves to
        // nothing, which must fail honestly (ticket 27).
        0x8A15_0014 if action == "upgrade" => Some(WingetReason {
            reboot: false,
            detail: "already up to date".into(),
        }),
        // UPGRADE_VERSION_NOT_NEWER
        0x8A15_004F => Some(WingetReason {
            reboot: false,
            detail: "already up to date".into(),
        }),
        // PACKAGE_ALREADY_INSTALLED
        0x8A15_0061 => Some(WingetReason {
            reboot: false,
            detail: "already installed".into(),
        }),
        // INSTALL_ALREADY_INSTALLED
        0x8A15_010D => Some(WingetReason {
            reboot: false,
            detail: "already installed".into(),
        }),
        // INSTALL_DOWNGRADE — winget refused a downgrade
        0x8A15_010E => Some(WingetReason {
            reboot: false,
            detail: "already installed (newer version present)".into(),
        }),
        _ => None,
    };

    // Message backstop: winget version differences drift exit codes, but the
    // on-screen reason is stable enough to classify the same outcomes.
    reason.or_else(|| {
        if action == "upgrade" && is_no_update {
            Some(WingetReason {
                reboot: false,
                detail: "already up to date".into(),
            })
        } else if action == "install" && is_installed {
            Some(WingetReason {
                reboot: false,
                detail: "already installed".into(),
            })
        } else {
            None
        }
    })
}

/// winget's phrasing when the id or source resolves to nothing — the wrong or
/// stale id case (ticket 16) deserves its own honest wording, not the generic
/// exit-code message.
fn no_package_found(output: &str) -> bool {
    output
        .to_lowercase()
        .contains("no package found matching")
}

/// The honest failure wording for a genuinely failed winget step (ticket 16):
/// a "no package found" result names the most common cause — a wrong or
/// stale id — before falling back to the generic per-action text.
fn winget_failure_detail(action: &str, exit: i32, output: &str) -> String {
    if no_package_found(output) {
        format!(
            "can't find this app in the winget registry (exit {exit}) — check its ID is correct, then re-run this plan"
        )
    } else if action == "upgrade" {
        format!(
            "upgrade failed (exit {exit}) — the previous version is still installed; re-run this plan later to retry"
        )
    } else {
        format!("install failed (exit {exit}) — not installed")
    }
}

/// Runs one winget step under the Requirement's timebox and classifies the
/// outcome with the whitelist. Genuine failures carry the honest wording.
pub(super) fn apply(process: &impl super::WingetProcess, action: &str, id: &str, timeout_minutes: u32, install_dir: Option<&str>) -> StepOutcome {
    let timeout = Duration::from_secs(u64::from(timeout_minutes) * 60);
    let run = process.timed(&winget_args(action, id, install_dir), timeout);
    let log = run.output.clone();
    if run.timed_out {
        return StepOutcome {
            ok: false,
            reboot_required: false,
            timed_out: true,
            detail: format!(
                "{action} did not finish in {timeout_minutes} min — its processes were killed"
            ),
            log,
        };
    }
    let Some(exit) = run.exit_code else {
        return StepOutcome {
            ok: false,
            reboot_required: false,
            timed_out: false,
            detail: format!("{action} failed to start: {}", run.output.trim()),
            log,
        };
    };
    match classify_winget_result(action, exit, &run.output) {
        Some(reason) => StepOutcome {
            ok: true,
            reboot_required: reason.reboot,
            timed_out: false,
            detail: reason.detail,
            log,
        },
        None => StepOutcome {
            ok: false,
            reboot_required: false,
            timed_out: false,
            detail: winget_failure_detail(action, exit, &run.output),
            log,
        },
    }
}

/// Every winget install/upgrade invocation, mirroring the legacy runner's
/// flags exactly (`-e`, `--source winget`, both accept agreements). A
/// machine-local install directory (ticket 34, ADR-0009) rides as
/// `--location`, so winget is told where the product should land — many
/// installers ignore it, which the run loop's post-install honesty check
/// catches.
fn winget_args(verb: &str, id: &str, install_dir: Option<&str>) -> Vec<String> {
    let mut args = vec![
        verb.to_string(),
        "--id".to_string(),
        id.to_string(),
        "-e".to_string(),
        "--source".to_string(),
        "winget".to_string(),
    ];
    if let Some(dir) = install_dir {
        args.push("--location".to_string());
        args.push(dir.to_string());
    }
    args.push("--accept-source-agreements".to_string());
    args.push("--accept-package-agreements".to_string());
    args
}

#[cfg(test)]
mod tests { use super::*;
#[test]
fn winget_args_carry_the_location_flag_when_one_applies() {
    // No directory: the legacy flag set, unchanged.
    let plain = winget_args("install", "Git.Git", None);
    assert_eq!(
        plain,
        vec![
            "install",
            "--id",
            "Git.Git",
            "-e",
            "--source",
            "winget",
            "--accept-source-agreements",
            "--accept-package-agreements",
        ]
    );
    // A directory rides as `--location` on both verbs.
    let install = winget_args("install", "Git.Git", Some(r"D:\Apps"));
    assert_eq!(
        install,
        vec![
            "install",
            "--id",
            "Git.Git",
            "-e",
            "--source",
            "winget",
            "--location",
            r"D:\Apps",
            "--accept-source-agreements",
            "--accept-package-agreements",
        ]
    );
    let upgrade = winget_args("upgrade", "Git.Git", Some(r"D:\Apps"));
    assert_eq!(
        upgrade,
        vec![
            "upgrade",
            "--id",
            "Git.Git",
            "-e",
            "--source",
            "winget",
            "--location",
            r"D:\Apps",
            "--accept-source-agreements",
            "--accept-package-agreements",
        ]
    );
}

#[test]
fn whitelists_benign_winget_exit_codes() {
    // The whitelist table from the legacy Get-WingetResultReason, 1:1.
    let ok = |action: &str, code: i32, output: &str| {
        classify_winget_result(action, code, output).expect("whitelisted")
    };

    let r = ok("install", 0, "");
    assert!(!r.reboot);
    assert_eq!(r.detail, "installed");

    let r = ok("upgrade", 0, "");
    assert_eq!(r.detail, "upgraded");

    let r = ok("upgrade", 0, "no available upgrade");
    assert_eq!(r.detail, "already up to date");

    let r = ok("install", 3010, "");
    assert!(r.reboot);
    assert_eq!(r.detail, "installed - reboot required to finish");

    let r = ok("install", 1641, "");
    assert!(r.reboot);
    assert_eq!(r.detail, "installed - reboot initiated");

    let r = ok("install", 0x8A15_0109u32 as i32, "");
    assert!(r.reboot);
    assert_eq!(r.detail, "installed - reboot required to finish");

    for code in [0x8A15_002Bu32 as i32, 0x8A15_0014u32 as i32, 0x8A15_004Fu32 as i32] {
        let r = ok("upgrade", code, "");
        assert!(!r.reboot);
        assert_eq!(r.detail, "already up to date");
    }

    for code in [0x8A15_0061u32 as i32, 0x8A15_010Du32 as i32, 0x8A15_010Eu32 as i32] {
        let r = ok("install", code, "");
        assert!(!r.reboot);
        assert!(r.detail.starts_with("already installed"), "{}", r.detail);
    }

    // The "newer version present" wording survives for INSTALL_DOWNGRADE.
    let r = ok("install", 0x8A15_010Eu32 as i32, "");
    assert_eq!(r.detail, "already installed (newer version present)");
}

#[test]
fn classifies_by_message_when_exit_code_drifts() {
    assert_eq!(
        classify_winget_result("upgrade", -1, "everything is up to date already")
            .expect("backstop")
            .detail,
        "already up to date"
    );
    assert_eq!(
        classify_winget_result("install", -1, "x is already installed")
            .expect("backstop")
            .detail,
        "already installed"
    );
    // Exit 0 is always a success for install, whatever the message says.
    let r = classify_winget_result("install", 0, "no upgrade available for the installer")
        .expect("exit 0 is whitelisted");
    assert_eq!(r.detail, "installed");

    // A non-whitelisted code with an install message that is not "already
    // installed" stays a genuine failure.
    assert_eq!(
        classify_winget_result("install", -1, "no upgrade available for the installer"),
        None
    );
}

#[test]
fn genuine_failures_return_none() {
    assert_eq!(classify_winget_result("install", 1, ""), None);
    assert_eq!(classify_winget_result("install", 5, "some error"), None);
    assert_eq!(classify_winget_result("upgrade", 42, ""), None);
}

#[test]
fn no_package_found_names_the_stale_id_cause() {
    // winget's phrasing when the id or source resolves to nothing — this
    // is the wrong-or-stale id case, not a generic failure.
    assert!(no_package_found("No package found matching input criteria."));
    assert!(no_package_found("No package found matching the query."));
    assert!(!no_package_found("install failed (exit 5) — not installed"));
    assert!(!no_package_found("no newer package"));
    assert_eq!(classify_winget_result("install", 5, "No package found matching input criteria."), None);

    let detail = winget_failure_detail("install", 5, "No package found matching input criteria.");
    assert!(detail.contains("can't find this app in the winget registry"), "{detail}");
    assert!(detail.contains("check its ID"), "{detail}");
    // The upgrade path shares the same cause and the same wording.
    let detail = winget_failure_detail("upgrade", 5, "No package found matching the query.");
    assert!(detail.contains("check its ID"), "{detail}");
}

#[test]
fn no_package_found_never_surfaces_as_success() {
    // The real-world probe (ticket 27): installing a nonexistent ID makes
    // winget exit -1978335212 (0x8A150014) with this message — previously
    // the whitelist swallowed it as "already up to date".
    assert_eq!(
        classify_winget_result(
            "install",
            0x8A15_0014u32 as i32,
            "No package found matching input criteria."
        ),
        None
    );
    assert_eq!(
        classify_winget_result(
            "upgrade",
            0x8A15_0014u32 as i32,
            "No package found matching input criteria."
        ),
        None
    );
    // The message guard runs before any whitelist match — even a benign
    // code must not mask the missing package.
    assert_eq!(
        classify_winget_result("install", 0, "No package found matching input criteria."),
        None
    );
    assert_eq!(
        classify_winget_result("upgrade", -1, "No package found matching the query."),
        None
    );
    // The honest wording is what surfaces for the real exit code.
    let detail = winget_failure_detail(
        "install",
        0x8A15_0014u32 as i32,
        "No package found matching input criteria.",
    );
    assert!(detail.contains("can't find this app in the winget registry"), "{detail}");
}

#[test]
fn no_package_found_exit_is_whitelisted_for_outputless_upgrade_only() {
    // Defense in depth: an output-less upgrade that hits
    // NO_APPLICATIONS_FOUND still means "already up to date".
    let r = classify_winget_result("upgrade", 0x8A15_0014u32 as i32, "")
        .expect("output-less upgrade is whitelisted");
    assert!(!r.reboot);
    assert_eq!(r.detail, "already up to date");
    // The same code without the message on install is a genuine failure —
    // winget found nothing to install.
    assert_eq!(
        classify_winget_result("install", 0x8A15_0014u32 as i32, ""),
        None
    );
}

#[test]
fn winget_failure_detail_keeps_the_generic_wording_for_other_failures() {
    assert_eq!(
        winget_failure_detail("install", 5, "some error"),
        "install failed (exit 5) — not installed"
    );
    assert_eq!(
        winget_failure_detail("upgrade", 42, ""),
        "upgrade failed (exit 42) — the previous version is still installed; re-run this plan later to retry"
    );
}
}
