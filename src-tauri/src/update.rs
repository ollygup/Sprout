//! Self-update from GitHub Releases (ADR-0012): ask the repo's Releases API
//! for the latest tag, compare it against the Cargo.toml version, and — when
//! newer and the user confirms — download the NSIS setup exe to %TEMP% and
//! run its passive `/UPDATE /P /R` path, exiting so the installer replaces
//! the exe in place. All networking lives here in Rust (ureq on rustls), so
//! the CSP stays untouched.
//!
//! The silent-failure contract: offline, private-repo 403/404 (rate-limit
//! responses look the same), and malformed payloads all count as "up to
//! date" — startup checks and the manual re-check never surface errors.
//! Only the user-initiated apply step reports failures. The check is inert
//! while the repo is private and activates when it goes public; no auth
//! machinery either way.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

/// The repo origin (ADR-0012). Inert while the repo is private — the API
/// answers 404 and the silent-failure contract swallows it.
const RELEASES_LATEST_URL: &str = "https://api.github.com/repos/ollygup/Sprout/releases/latest";

/// GitHub's API rejects requests without a User-Agent.
const USER_AGENT: &str = concat!("Sprout/", env!("CARGO_PKG_VERSION"));

/// The event the startup check emits exactly once per launch when a newer
/// release exists — payload `{version, url}`.
pub const UPDATE_AVAILABLE_EVENT: &str = "update-available";

/// The installer asset name pattern release CI publishes (`release.yml`);
/// both the update pick and the download target must match it exactly.
const SETUP_ASSET_PREFIX: &str = "Sprout_";
const SETUP_ASSET_SUFFIX: &str = "_x64-setup.exe";

/// The whole request/response of a check, timeboxed so a stalled connection
/// can never hang a startup thread or the Settings screen for long.
const CHECK_TIMEOUT: Duration = Duration::from_secs(15);
/// Per-read ceiling for the asset download — guards stalls without capping
/// a large file's total transfer time.
const DOWNLOAD_READ_TIMEOUT: Duration = Duration::from_secs(60);

/// The installed build's version — Cargo.toml is the single source of truth,
/// and `release.yml` refuses to publish unless the pushed tag equals it,
/// which is what makes the comparison below sound (ADR-0012).
pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// A newer release worth telling the UI about: the display version (tag
/// stripped of its `v`) and the setup-exe download URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AvailableUpdate {
    pub version: String,
    pub url: String,
}

/// One release reduced to what the update decision needs: the tag, the
/// human-readable notes, and the published assets.
pub struct ParsedRelease {
    pub tag: String,
    /// Not read by the update decision itself — carried for the update
    /// affordance's release-notes display.
    #[allow(dead_code)]
    pub notes: String,
    pub assets: Vec<ReleaseAsset>,
}

/// One published asset: file name and browser-download URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseAsset {
    pub name: String,
    pub url: String,
}

#[derive(Deserialize)]
struct ReleaseJson {
    #[serde(default)]
    tag_name: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    assets: Vec<AssetJson>,
}

#[derive(Deserialize)]
struct AssetJson {
    #[serde(default)]
    name: String,
    #[serde(default)]
    browser_download_url: String,
}

// ---------------------------------------------------------------------------
// Pure functions over data — everything below this line is network-free and
// carries the fixture tests at the bottom of this file.

/// Parses one `/releases/latest` JSON payload into [`ParsedRelease`].
/// `None` for anything malformed — an unreadable payload reads as "no
/// update", never as an error (the silent-failure contract).
pub fn parse_release(payload: &str) -> Option<ParsedRelease> {
    let json: ReleaseJson = serde_json::from_str(payload).ok()?;
    let assets = json
        .assets
        .into_iter()
        .filter(|a| !a.name.is_empty() && !a.browser_download_url.is_empty())
        .map(|a| ReleaseAsset {
            name: a.name,
            url: a.browser_download_url,
        })
        .collect();
    Some(ParsedRelease {
        tag: json.tag_name,
        notes: json.body,
        assets,
    })
}

/// Parses a release tag into its `X.Y.Z` triple after stripping the `v`.
/// Prerelease-suffixed tags (`v0.5.0-rc.1`) and anything else unparseable
/// are rejected — a tag we cannot read cleanly must never read as "newer".
fn parse_semver(tag: &str) -> Option<(u64, u64, u64)> {
    let core = tag.trim().strip_prefix('v').unwrap_or(tag.trim());
    if core.is_empty() || core.contains('-') || core.contains('+') {
        return None;
    }
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/// Whether `candidate_tag` is strictly newer than `current_version` —
/// semver-triple comparison over tags with their optional `v` stripped.
fn is_newer(candidate_tag: &str, current_version: &str) -> bool {
    match (
        parse_semver(candidate_tag),
        parse_semver(current_version),
    ) {
        (Some(candidate), Some(current)) => candidate > current,
        _ => false,
    }
}

/// Whether `name` is the installer asset pattern release CI publishes
/// (`Sprout_*_x64-setup.exe`).
fn is_setup_asset_name(name: &str) -> bool {
    name.starts_with(SETUP_ASSET_PREFIX) && name.ends_with(SETUP_ASSET_SUFFIX)
}

/// Picks the installer asset among a release's assets: the first whose name
/// matches the pattern. `None` when the release has no usable installer —
/// which also reads as "no update".
fn pick_setup_asset<'a>(assets: &'a [ReleaseAsset]) -> Option<&'a ReleaseAsset> {
    assets.iter().find(|a| is_setup_asset_name(&a.name))
}

/// The full update decision over raw API JSON: parse, compare against the
/// running version, pick the installer asset. `Some` only for a genuinely
/// newer release with a downloadable Sprout installer; every other outcome
/// is `None` ("up to date").
pub fn evaluate(payload: &str, current_version: &str) -> Option<AvailableUpdate> {
    let release = parse_release(payload)?;
    if !is_newer(&release.tag, current_version) {
        return None;
    }
    let asset = pick_setup_asset(&release.assets)?;
    Some(AvailableUpdate {
        version: release.tag.trim().strip_prefix('v').unwrap_or(release.tag.trim()).to_string(),
        url: asset.url.clone(),
    })
}

// ---------------------------------------------------------------------------
// Network paths

/// The check agent: whole-request timebox (connect through body).
fn check_agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(CHECK_TIMEOUT).build()
}

/// The download agent: connect + per-read ceilings, no total cap.
fn download_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(CHECK_TIMEOUT)
        .timeout_read(DOWNLOAD_READ_TIMEOUT)
        .build()
}

/// One update check under the silent-failure contract: any transport error,
/// non-success status, or unreadable payload yields `None` —
/// indistinguishable from up-to-date, never an error surface (ADR-0012).
pub fn check_for_update_silent() -> Option<AvailableUpdate> {
    let response = check_agent()
        .get(RELEASES_LATEST_URL)
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .call();
    let payload = match response {
        Ok(response) => response.into_string().ok()?,
        // Non-2xx (private-repo 404, rate-limit 403) and transport failures
        // land here together.
        Err(_) => return None,
    };
    evaluate(&payload, current_version())
}

/// The once-per-launch background check: runs off the setup path and emits
/// a single `update-available` event when a newer release exists. Every
/// failure stays silent — no event, no log noise, nothing to nag about.
pub fn start_background_check(app: AppHandle) {
    std::thread::spawn(move || {
        if let Some(update) = check_for_update_silent() {
            let _ = app.emit(UPDATE_AVAILABLE_EVENT, &update);
        }
    });
}

/// Streams `url` into `target`, replacing whatever was there.
fn download_to_file(url: &str, target: &Path) -> Result<(), String> {
    let mut reader = download_agent()
        .get(url)
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| format!("the update could not be downloaded: {e}"))?
        .into_reader();
    let mut file = std::fs::File::create(target)
        .map_err(|e| format!("cannot create {}: {e}", target.display()))?;
    std::io::copy(&mut reader, &mut file)
        .map_err(|e| format!("the download failed mid-way: {e}"))?;
    Ok(())
}

/// Spawns the NSIS installer detached from this process with its passive
/// update flags: `/UPDATE` skips the uninstall-first page, `/P` keeps it to
/// a progress bar, and `/R` relaunches Sprout once the new files land (the
/// template's running-app macro closes any instance that survives until
/// then). Dropping the child handle leaves it running on its own.
fn spawn_installer_detached(installer: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use windows_sys::Win32::System::Threading::{
        CREATE_BREAKAWAY_FROM_JOB, CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS,
    };
    Command::new(installer)
        .args(["/UPDATE", "/P", "/R"])
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB)
        .spawn()
        .map_err(|e| format!("the installer could not be started: {e}"))?;
    Ok(())
}

/// The user-confirmed apply step behind the `install_update` command:
/// downloads the setup asset to `%TEMP%\<asset-name>` (the URL's last path
/// segment, refused unless it names a real Sprout installer), spawns the
/// installer detached, and exits the app shortly after so NSIS can replace
/// the exe. Unlike the checks, failures here are reported — the user asked
/// for this action explicitly.
pub fn apply_update(app: &AppHandle, url: &str) -> Result<(), String> {
    let name = url.rsplit('/').next().unwrap_or("");
    if !is_setup_asset_name(name) {
        return Err("That download is not a Sprout installer — refusing to run it.".into());
    }
    let target = std::env::temp_dir().join(name);
    download_to_file(url, &target)?;
    spawn_installer_detached(&target)?;
    // Give the installer a moment to take the stage, then leave quietly.
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(800));
        app.exit(0);
    });
    Ok(())
}

// ---------------------------------------------------------------------------
// Fixture tests — recorded GitHub-API responses, no network anywhere.

#[cfg(test)]
mod tests {
    use super::*;

    /// A recorded `/releases/latest` response for v0.5.0 carrying the setup
    /// exe plus an unrelated asset, in GitHub's field order and shape.
    const RELEASE_V050: &str =
        include_str!("update/fixtures/release-v050.json");
    /// A prerelease-tagged response — `/releases/latest` never returns one,
    /// but a stable release may still carry a prerelease-suffixed tag.
    const RELEASE_V060_RC1: &str =
        include_str!("update/fixtures/release-v060-rc1.json");

    // -- strip-v semver comparison --------------------------------------

    #[test]
    fn strictly_newer_tags_are_newer() {
        assert!(is_newer("v0.5.0", "0.4.1"));
        assert!(is_newer("0.5.0", "0.4.9"));
        assert!(is_newer("v1.0.0", "0.99.99"));
        assert!(is_newer("0.4.2", "0.4.1"));
    }

    #[test]
    fn same_or_older_tags_and_v_prefixes_compare_cleanly() {
        assert!(!is_newer("v0.4.1", "0.4.1"));
        assert!(!is_newer("0.5.0", "v0.5.0"));
        assert!(!is_newer("v0.3.9", "0.4.1"));
        assert!(!is_newer("v0.4.0", "0.4.1"));
    }

    #[test]
    fn unreadable_current_versions_never_read_as_newer() {
        assert!(!is_newer("v999.0.0", ""));
        assert!(!is_newer("v999.0.0", "dev"));
    }

    #[test]
    fn malformed_candidate_tags_are_rejected() {
        assert!(!is_newer("", "0.4.1"));
        assert!(!is_newer("not-a-version", "0.4.1"));
        assert!(!is_newer("v0.5", "0.4.1"));
        assert!(!is_newer("v0.5.0.1", "0.4.1"));
        assert!(!is_newer("vx.y.z", "0.4.1"));
    }

    #[test]
    fn prerelease_looking_tags_are_ignored() {
        // Bigger numbers behind a prerelease suffix never win.
        assert!(!is_newer("v0.6.0-rc.1", "0.4.1"));
        assert!(!is_newer("0.6.0-beta", "0.4.1"));
        assert!(!is_newer("v0.5.0-rc.1+build.7", "0.4.1"));
    }

    // -- release-JSON parsing -------------------------------------------

    #[test]
    fn recorded_fixture_parses_into_tag_notes_assets() {
        let release = parse_release(RELEASE_V050).expect("fixture should parse");
        assert_eq!(release.tag, "v0.5.0");
        assert!(release.notes.contains("Quick Clips"));
        assert_eq!(release.assets.len(), 2);
        assert!(release.assets[0].url.starts_with("https://github.com/ollygup/Sprout/releases/download/"));
    }

    #[test]
    fn missing_optional_fields_still_parse() {
        let release = parse_release(r#"{"tag_name": "v1.2.3"}"#).expect("parses");
        assert_eq!(release.tag, "v1.2.3");
        assert_eq!(release.notes, "");
        assert!(release.assets.is_empty());
    }

    #[test]
    fn malformed_payloads_yield_none() {
        assert!(parse_release("").is_none());
        assert!(parse_release("<html>rate limited</html>").is_none());
        assert!(parse_release("{\"tag_name\": 12}").is_none());
        // An empty-tag release parses but can never be an update.
        let release = parse_release("{\"tag_name\": \"\"}").unwrap();
        assert!(!is_newer(&release.tag, "0.4.1"));
    }

    // -- asset selection --------------------------------------------------

    #[test]
    fn picks_the_setup_exe_from_the_recorded_fixture() {
        let release = parse_release(RELEASE_V050).unwrap();
        let asset = pick_setup_asset(&release.assets).expect("setup asset present");
        assert_eq!(asset.name, "Sprout_0.5.0_x64-setup.exe");
        assert_eq!(
            asset.url,
            "https://github.com/ollygup/Sprout/releases/download/v0.5.0/Sprout_0.5.0_x64-setup.exe"
        );
    }

    #[test]
    fn first_matching_asset_wins() {
        let assets = vec![
            ReleaseAsset {
                name: "latest.json".into(),
                url: "https://example.com/latest.json".into(),
            },
            ReleaseAsset {
                name: "Sprout_0.7.0_x64-setup.exe".into(),
                url: "https://example.com/first".into(),
            },
            ReleaseAsset {
                name: "Sprout_0.8.0_x64-setup.exe".into(),
                url: "https://example.com/second".into(),
            },
        ];
        assert_eq!(pick_setup_asset(&assets).unwrap().url, "https://example.com/first");
    }

    #[test]
    fn releases_without_a_setup_asset_pick_nothing() {
        let zip_only = vec![ReleaseAsset {
            name: "Sprout_0.5.0_x64.zip".into(),
            url: "https://example.com/a.zip".into(),
        }];
        assert!(pick_setup_asset(&zip_only).is_none());
        assert!(pick_setup_asset(&[]).is_none());
    }

    #[test]
    fn near_miss_names_do_not_match_the_pattern() {
        assert!(is_setup_asset_name("Sprout_0.5.0_x64-setup.exe"));
        assert!(!is_setup_asset_name("sprout_0.5.0_x64-setup.exe"));
        assert!(!is_setup_asset_name("Sprout_0.5.0_x86-setup.exe"));
        assert!(!is_setup_asset_name("Sprout_0.5.0_x64-setup.exe.bak"));
        assert!(!is_setup_asset_name("MyApp_0.5.0_x64-setup.exe"));
    }

    // -- the full decision ------------------------------------------------

    #[test]
    fn recorded_fixture_reads_as_an_update_for_an_older_build() {
        // Pinned to a fixed older build so the fixture keeps working after
        // v0.5.0 actually becomes the running version.
        let update = evaluate(RELEASE_V050, "0.4.1").expect("update available");
        assert_eq!(update.version, "0.5.0");
        assert_eq!(update.url, "https://github.com/ollygup/Sprout/releases/download/v0.5.0/Sprout_0.5.0_x64-setup.exe");
    }

    #[test]
    fn same_version_and_prerelease_fixtures_read_as_up_to_date() {
        // The fixture's v0.4.1 matches this working copy's Cargo.toml while
        // the round is in flight; equal versions are not updates.
        assert!(evaluate(RELEASE_V050, "0.5.0").is_none());
        assert!(evaluate(RELEASE_V050, "0.6.0").is_none());
        // Prerelease-looking tag: ignored even though 0.6 > 0.4.
        assert!(evaluate(RELEASE_V060_RC1, "0.4.1").is_none());
    }

    #[test]
    fn a_release_without_a_usable_installer_is_not_an_update() {
        let payload = r#"{
            "tag_name": "v9.0.0",
            "body": "",
            "assets": [
                {"name": "latest.json",
                 "browser_download_url": "https://example.com/latest.json"}
            ]
        }"#;
        assert!(evaluate(payload, "0.4.1").is_none());
    }
}
