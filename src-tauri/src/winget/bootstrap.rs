use std::{path::PathBuf, time::Duration};
use serde::Deserialize;
use crate::engine::windows::windows_build_number;
use crate::windows_execution::{powershell_argv, powershell_output, run_timed_process};
const BOOTSTRAP_TIMEOUT: Duration = Duration::from_secs(20 * 60);
const RELEASE_FETCH_TIMEOUT: Duration = Duration::from_secs(60);
const MIN_WINGET_BUILD: u32 = 19041;

/// Bootstraps the App Installer (winget) when it is missing — the port
/// of the legacy `Install-WingetIfMissing`: only on supported builds
/// (10.0.19041+), downloads the official winget-cli release's
/// `.msixbundle` from GitHub, and installs it via `Add-AppxPackage`
/// (which is why this only ever runs in the elevated worker).
pub(super) fn bootstrap_winget() -> Result<(), String> {
    let build = windows_build_number()
        .ok_or_else(|| "cannot determine the Windows build number".to_string())?;
    if build < MIN_WINGET_BUILD {
        return Err(format!(
            "winget is missing and this Windows build ({build}) is unsupported — Sprout needs \
             Windows 10 build 19041 or later. Install the App Installer manually and run this \
             plan again: https://learn.microsoft.com/windows/package-manager/winget/"
        ));
    }

    let url = fetch_msixbundle_url()?;
    let bundle = download_msixbundle(&url)?;
    let (exe, args) = powershell_argv(&format!(
        "Add-AppxPackage -Path '{}' -ForceApplicationShutdown",
        bundle.display()
    ));
    let install = run_timed_process(&exe, &args, BOOTSTRAP_TIMEOUT);
    let _ = std::fs::remove_file(&bundle);
    if install.timed_out || install.exit_code != Some(0) {
        return Err(format!(
            "winget bootstrap failed: installing the App Installer did not finish cleanly. \
             Install it manually and run this plan again: \
             https://learn.microsoft.com/windows/package-manager/winget/ ({})",
            install.output.trim()
        ));
    }

    if !super::available(&super::NativeWinget) {
        return Err(
            "winget was installed but is not on PATH yet — close Sprout and run this plan again"
                .to_string(),
        );
    }
    Ok(())
}

/// Fetches the download URL of the latest winget-cli `.msixbundle` from
/// the GitHub releases API — the same source the legacy bootstrap used.
fn fetch_msixbundle_url() -> Result<String, String> {
    let script = "(Invoke-RestMethod -Uri 'https://api.github.com/repos/microsoft/winget-cli/releases/latest' -Headers @{ 'User-Agent' = 'Sprout-bootstrap' }).assets | ConvertTo-Json -Depth 3";
    let output = powershell_output(script, RELEASE_FETCH_TIMEOUT)
        .map_err(|e| format!("winget bootstrap failed: {e}"))?;
    let assets: Vec<GitHubAsset> = serde_json::from_str(output.trim())
        .map_err(|e| format!("winget bootstrap failed: cannot read the winget release info ({e})"))?;
    pick_msixbundle(&assets).ok_or_else(|| {
        "winget bootstrap failed: no .msixbundle asset in the latest winget-cli release"
            .to_string()
    })
}

/// Downloads the bundle to the system temp folder; the caller removes it
/// after the install.
fn download_msixbundle(url: &str) -> Result<PathBuf, String> {
    let target = std::env::temp_dir().join(format!(
        "sprout-winget-{}.msixbundle",
        std::process::id()
    ));
    let result = powershell_output(
        &format!(
            "Invoke-WebRequest -Uri '{}' -OutFile '{}' -UseBasicParsing",
            url,
            target.display()
        ),
        BOOTSTRAP_TIMEOUT,
    );
    match result {
        Ok(_) => Ok(target),
        Err(e) => {
            let _ = std::fs::remove_file(&target);
            Err(format!(
                "winget bootstrap failed: downloading the App Installer failed ({e})"
            ))
        }
    }
}

/// One asset of a GitHub release — enough of the API shape to pick the
/// `.msixbundle` download URL.
#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    #[serde(rename = "browser_download_url")]
    browser_download_url: String,
}

/// The download URL of the first `.msixbundle` asset, when a release has
/// one — pure, so the bootstrap's release parsing is testable offline.
fn pick_msixbundle(assets: &[GitHubAsset]) -> Option<String> {
    assets
        .iter()
        .find(|asset| asset.name.ends_with(".msixbundle"))
        .map(|asset| asset.browser_download_url.clone())
}

#[cfg(test)]
mod tests { use super::*;
#[test]
fn picks_the_msixbundle_asset_from_a_release() {
    let assets = vec![
        GitHubAsset {
            name: "msixbundle.cer".into(),
            browser_download_url: "https://x/msixbundle.cer".into(),
        },
        GitHubAsset {
            name: "Microsoft.DesktopAppInstaller_8wekyb3d8bbwe.msixbundle".into(),
            browser_download_url: "https://x/bundle.msixbundle".into(),
        },
        GitHubAsset {
            name: "Microsoft.DesktopAppInstaller_8wekyb3d8bbwe.msix".into(),
            browser_download_url: "https://x/not-this.msix".into(),
        },
    ];
    assert_eq!(
        pick_msixbundle(&assets).as_deref(),
        Some("https://x/bundle.msixbundle")
    );
    assert_eq!(pick_msixbundle(&assets[..1]), None);
}
}
