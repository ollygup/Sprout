//! Self-update from GitHub Releases (ADR-0012, dual trains per 2026-09-16
//! amendment): ask the repo's Releases list for this platform's train, compare
//! the newest matching tag against the train's Cargo.toml version, and — when
//! newer and the user confirms — download the platform installer to %TEMP% and
//! run its passive `/UPDATE /P /R` path, exiting so the installer replaces
//! the exe in place. All networking lives here in Rust (ureq on rustls), so
//! the CSP stays untouched.
//!
//! Trains: Windows follows `win-v*` tags with `Sprout_*_x64-setup.exe`
//! assets; Mac follows `mac-v*` tags with `Sprout_*.dmg` assets. Each side
//! ignores the other's tags and assets, and bare `v*` tags (retired in 207)
//! are ignored by both.
//!
//! The silent-failure contract: offline, private-repo 403/404 (rate-limit
//! responses look the same), and malformed payloads all count as "up to
//! date" — startup checks and the manual re-check never surface errors.
//! Only the user-initiated apply step reports failures. The check is inert
//! while the repo is private and activates when it goes public; no auth
//! machinery either way.
//!
//! Apply-step integrity (ADR-0012 scheme B): release CI signs every setup
//! exe with an ed25519 minisign key held as Actions secrets, and this module
//! verifies the download against [`UPDATE_PUBKEY`] before the installer ever
//! spawns — TLS alone no longer has to carry trust in the update path.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use minisign_verify::{PublicKey, Signature};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

/// The repo origin (ADR-0012). Inert while the repo is private — the API
/// answers 404 and the silent-failure contract swallows it.
///
/// The list endpoint (not `/releases/latest`): with two trains sharing one
/// repo, "latest overall" may belong to the other platform, so each side
/// lists recent releases and picks the newest of its own train instead.
const RELEASES_LIST_URL: &str =
    "https://api.github.com/repos/ollygup/Sprout/releases?per_page=100";

/// Release-train tag prefixes (spec 206, ticket 207): Windows ships
/// `win-vA.B.C`, Mac ships `mac-vX.Y.Z`. Bare `v*` is retired — matched by
/// neither train and ignored by both updaters.
pub const WIN_TAG_PREFIX: &str = "win-v";
pub const MAC_TAG_PREFIX: &str = "mac-v";

/// This build's train, selected at compile time so each binary only ever
/// follows its own releases. Windows-only today; the Mac adapter (tickets
/// 208–215) compiles this same module with `target_os = "macos"`.
#[cfg(target_os = "macos")]
pub const UPDATE_TAG_PREFIX: &str = MAC_TAG_PREFIX;
#[cfg(not(target_os = "macos"))]
pub const UPDATE_TAG_PREFIX: &str = WIN_TAG_PREFIX;

/// GitHub's API rejects requests without a User-Agent.
const USER_AGENT: &str = concat!("Sprout/", env!("CARGO_PKG_VERSION"));

/// The event the startup check emits exactly once per launch when a newer
/// release exists — payload `{version, url}`.
pub const UPDATE_AVAILABLE_EVENT: &str = "update-available";

/// The installer asset name patterns release CI publishes (`release.yml`);
/// both the update pick and the download target must match the running
/// platform's pattern exactly. Windows keeps `Sprout_*_x64-setup.exe`; the
/// Mac train publishes `Sprout_*.dmg` (Tauri `dmg` bundle, e.g.
/// `Sprout_0.1.0_aarch64.dmg`). Signature sidecars (`*.sig`) never match —
/// they ride beside the asset and are fetched as `<url>.sig`.
const SETUP_ASSET_PREFIX: &str = "Sprout_";
const SETUP_ASSET_SUFFIX: &str = "_x64-setup.exe";
const MAC_ASSET_PREFIX: &str = "Sprout_";
const MAC_ASSET_SUFFIX: &str = ".dmg";

/// CI's update-signing public key (ADR-0012 scheme B): the second line of
/// the `<key>.pub` file produced by `tauri signer generate` — the base64
/// body pasted verbatim (key id 5345CD6883CC4501). The matching private half
/// lives only as GitHub Actions secrets. An empty string means this build
/// cannot verify signatures, so every install refuses — fail-closed until a
/// real key is embedded.
const UPDATE_PUBKEY: &str = "RWQBRcyDaM1FU5hHUVavD5qe+TzSx+y5LY/DHzGH7jZxahgxpyGOcTrb";

/// The whole request/response of a check, timeboxed so a stalled connection
/// can never hang a startup thread or the Settings screen for long.
const CHECK_TIMEOUT: Duration = Duration::from_secs(15);
/// Per-read ceiling for the asset download — guards stalls without capping
/// a large file's total transfer time.
const DOWNLOAD_READ_TIMEOUT: Duration = Duration::from_secs(60);

/// The installed build's version — its train's Cargo.toml is the single
/// source of truth (`src-tauri/Cargo.toml` for `win-v*`,
/// `src-tauri/mac/Cargo.toml` for `mac-v*`), and `release.yml` refuses to
/// publish unless the pushed namespaced tag equals it, which is what makes
/// the comparison below sound (ADR-0012 as amended 2026-09-16).
pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// This build's train prefix — [`UPDATE_TAG_PREFIX`], exposed for the feed
/// filter and its tests.
pub fn update_train() -> &'static str {
    UPDATE_TAG_PREFIX
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

// `Option` fields, not `#[serde(default)]`: GitHub answers `"body": null`
// for releases published without notes (action-gh-release), and `default`
// only covers a missing field — an explicit null would fail the whole
// parse and read as "up to date" (silent-failure contract).

#[derive(Deserialize)]
struct ReleaseJson {
    #[serde(default)]
    tag_name: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    assets: Vec<AssetJson>,
}

#[derive(Deserialize)]
struct AssetJson {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    browser_download_url: Option<String>,
}

// ---------------------------------------------------------------------------
// Pure functions over data — everything below this line is network-free and
// carries the fixture tests at the bottom of this file.

/// Parses one `/releases/latest` JSON payload into [`ParsedRelease`].
/// `None` for anything malformed — an unreadable payload reads as "no
/// update", never as an error (the silent-failure contract). Single-release
/// unit seam for the table tests (production consumes the list path).
#[allow(dead_code)]
pub fn parse_release(payload: &str) -> Option<ParsedRelease> {
    let json: ReleaseJson = serde_json::from_str(payload).ok()?;
    let assets = json
        .assets
        .into_iter()
        .filter_map(|a| match (a.name, a.browser_download_url) {
            (Some(name), Some(url)) if !name.is_empty() && !url.is_empty() => {
                Some(ReleaseAsset { name, url })
            }
            _ => None,
        })
        .collect();
    Some(ParsedRelease {
        tag: json.tag_name.unwrap_or_default(),
        notes: json.body.unwrap_or_default(),
        assets,
    })
}

/// Parses one `/releases` list payload (a JSON array of release objects)
/// into its releases, skipping array entries that do not parse. `None` for
/// anything malformed at the top level — an unreadable payload reads as "no
/// update", never as an error (the silent-failure contract).
pub fn parse_releases(payload: &str) -> Option<Vec<ParsedRelease>> {
    let json: Vec<ReleaseJson> = serde_json::from_str(payload).ok()?;
    let releases = json
        .into_iter()
        .filter_map(|entry| {
            let assets = entry
                .assets
                .into_iter()
                .filter_map(|a| match (a.name, a.browser_download_url) {
                    (Some(name), Some(url)) if !name.is_empty() && !url.is_empty() => {
                        Some(ReleaseAsset { name, url })
                    }
                    _ => None,
                })
                .collect();
            Some(ParsedRelease {
                tag: entry.tag_name.unwrap_or_default(),
                notes: entry.body.unwrap_or_default(),
                assets,
            })
        })
        .collect();
    Some(releases)
}

/// Parses a release tag into its `X.Y.Z` triple after stripping the train
/// prefix (`win-v` / `mac-v`) or the retired bare `v`.
/// Prerelease-suffixed tags (`win-v0.5.0-rc.1`) and anything else unparseable
/// are rejected — a tag we cannot read cleanly must never read as "newer".
fn parse_semver(tag: &str) -> Option<(u64, u64, u64)> {
    let trimmed = tag.trim();
    let core = trimmed
        .strip_prefix(WIN_TAG_PREFIX)
        .or_else(|| trimmed.strip_prefix(MAC_TAG_PREFIX))
        .or_else(|| trimmed.strip_prefix('v'))
        .unwrap_or(trimmed);
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
/// semver-triple comparison over tags with their train prefix (or retired
/// bare `v`) stripped. Semver unit seam for the table tests; the list
/// decision compares triples directly.
#[allow(dead_code)]
fn is_newer(candidate_tag: &str, current_version: &str) -> bool {
    match (
        parse_semver(candidate_tag),
        parse_semver(current_version),
    ) {
        (Some(candidate), Some(current)) => candidate > current,
        _ => false,
    }
}

/// Whether `tag` belongs to the `train_prefix` train (`win-v` / `mac-v`).
/// Bare `v*` tags belong to neither — both updaters ignore them.
fn is_train_tag(tag: &str, train_prefix: &str) -> bool {
    tag.trim().starts_with(train_prefix)
}

/// Whether `name` is the Windows installer asset pattern release CI
/// publishes (`Sprout_*_x64-setup.exe`).
fn is_setup_asset_name(name: &str) -> bool {
    name.starts_with(SETUP_ASSET_PREFIX) && name.ends_with(SETUP_ASSET_SUFFIX)
}

/// Whether `name` is the Mac installer asset pattern release CI publishes
/// (`Sprout_*.dmg`). The `.sig` sidecar (`*.dmg.sig`) never matches — it is
/// fetched as `<url>.sig` after the pick.
fn is_mac_asset_name(name: &str) -> bool {
    name.starts_with(MAC_ASSET_PREFIX) && name.ends_with(MAC_ASSET_SUFFIX)
}

/// Whether `name` is this build's installer asset pattern.
fn is_update_asset_name(name: &str) -> bool {
    is_train_asset_name(name, update_train())
}

/// Whether `name` matches the `train_prefix` train's asset pattern.
/// Table-test seam for the cross-platform rule: each train ignores the
/// other's assets regardless of which OS the test host runs.
fn is_train_asset_name(name: &str, train_prefix: &str) -> bool {
    if train_prefix == MAC_TAG_PREFIX {
        is_mac_asset_name(name)
    } else {
        is_setup_asset_name(name)
    }
}

/// Picks the installer asset among a release's assets for the
/// `train_prefix` train: the first whose name matches the pattern. `None`
/// when the release has no usable installer — which also reads as "no
/// update".
fn pick_train_asset<'a>(
    assets: &'a [ReleaseAsset],
    train_prefix: &str,
) -> Option<&'a ReleaseAsset> {
    assets
        .iter()
        .find(|a| is_train_asset_name(&a.name, train_prefix))
}

/// Picks the installer asset among a release's assets: the first whose name
/// matches this build's pattern. `None` when the release has no usable
/// installer — which also reads as "no update".
#[allow(dead_code)]
fn pick_setup_asset<'a>(assets: &'a [ReleaseAsset]) -> Option<&'a ReleaseAsset> {
    pick_train_asset(assets, update_train())
}

/// Strips the `train_prefix` train prefix (falling back to the retired bare
/// `v`) for display versions.
fn strip_train_prefix<'a>(tag: &'a str, train_prefix: &str) -> &'a str {
    let trimmed = tag.trim();
    trimmed
        .strip_prefix(train_prefix)
        .or_else(|| trimmed.strip_prefix('v'))
        .unwrap_or(trimmed)
}

/// The full update decision over one raw single-release API JSON payload for
/// the `train_prefix` train: parse, require the train's tag, compare against
/// the running version, pick the train's installer asset. `Some` only for a
/// genuinely newer same-train release with a downloadable installer; every
/// other outcome is `None` ("up to date"). Single-release unit seam for the
/// table tests (production consumes the list path); covers both trains on
/// any host, production calls it with [`UPDATE_TAG_PREFIX`] via [`evaluate`].
#[allow(dead_code)]
pub fn evaluate_in_train(
    payload: &str,
    current_version: &str,
    train_prefix: &str,
) -> Option<AvailableUpdate> {
    let release = parse_release(payload)?;
    select_update_in_train(std::slice::from_ref(&release), current_version, train_prefix)
}

/// The full update decision over raw single-release API JSON: parse, require
/// this build's train tag, compare against the running version, pick the
/// installer asset. `Some` only for a genuinely newer same-train release
/// with a downloadable Sprout installer; every other outcome is `None`
/// ("up to date"). Single-release unit seam (see [`evaluate_in_train`]).
#[allow(dead_code)]
pub fn evaluate(payload: &str, current_version: &str) -> Option<AvailableUpdate> {
    evaluate_in_train(payload, current_version, update_train())
}

/// The update decision over a parsed release list for the `train_prefix`
/// train: keep same-train releases newer than `current_version` with a usable
/// installer asset, offer the highest version. Cross-train tags, bare `v*`
/// tags, and releases without a usable asset never surface.
pub fn select_update_in_train(
    releases: &[ParsedRelease],
    current_version: &str,
    train_prefix: &str,
) -> Option<AvailableUpdate> {
    let current = parse_semver(current_version)?;
    releases
        .iter()
        .filter(|release| is_train_tag(&release.tag, train_prefix))
        .filter_map(|release| {
            let candidate = parse_semver(&release.tag)?;
            if candidate <= current {
                return None;
            }
            let asset = pick_train_asset(&release.assets, train_prefix)?;
            Some((candidate, release, asset))
        })
        .max_by(|left, right| left.0.cmp(&right.0))
        .map(|(_, release, asset)| AvailableUpdate {
            version: strip_train_prefix(&release.tag, train_prefix).to_string(),
            url: asset.url.clone(),
        })
}

/// The update decision over a raw `/releases` list payload for the
/// `train_prefix` train. `None` for malformed payloads and for lists with no
/// newer same-train release carrying a usable installer.
pub fn evaluate_list_in_train(
    payload: &str,
    current_version: &str,
    train_prefix: &str,
) -> Option<AvailableUpdate> {
    let releases = parse_releases(payload)?;
    select_update_in_train(&releases, current_version, train_prefix)
}

/// The update decision over a raw `/releases` list payload for this build's
/// train.
pub fn evaluate_list(payload: &str, current_version: &str) -> Option<AvailableUpdate> {
    evaluate_list_in_train(payload, current_version, update_train())
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
/// Lists recent releases and offers only this build's train: the other
/// train's tags and bare `v*` tags never surface, no matter how new.
pub fn check_for_update_silent() -> Option<AvailableUpdate> {
    let response = check_agent()
        .get(RELEASES_LIST_URL)
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .call();
    let payload = match response {
        Ok(response) => response.into_string().ok()?,
        // Non-2xx (private-repo 404, rate-limit 403) and transport failures
        // land here together.
        Err(_) => return None,
    };
    evaluate_list(&payload, current_version())
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

/// Fetches a small text asset — the `.sig` sidecar next to the installer —
/// into memory.
fn download_to_string(url: &str) -> Result<String, String> {
    download_agent()
        .get(url)
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| format!("the update signature could not be downloaded: {e}"))?
        .into_string()
        .map_err(|e| format!("the update signature could not be read: {e}"))
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

/// Decodes a downloaded `.sig` payload into a minisign signature. CI signs
/// with `tauri signer sign`, which emits the minisign text base64-wrapped on
/// a single line (Tauri's updater sidecar format), while a bare minisign file
/// starts with its "untrusted comment:" header — both are accepted so every
/// shipped verifier generation can read current releases (ADR-0012).
/// Everything else refuses, fail-closed.
fn decode_signature(signature_text: &str) -> Result<Signature, ()> {
    use base64::Engine as _;
    let trimmed = signature_text.trim();
    let minisign_text = if trimmed.starts_with("untrusted comment:") {
        trimmed.to_owned()
    } else {
        let packed: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
        let decoded =
            base64::engine::general_purpose::STANDARD.decode(packed.as_bytes()).map_err(|_| ())?;
        String::from_utf8(decoded).map_err(|_| ())?
    };
    Signature::decode(&minisign_text).map_err(|_| ())
}

/// Verifies installer bytes against a minisign signature made by CI's
/// update key (ADR-0012 scheme B). Total and fail-closed: an empty or
/// unreadable embedded key, an unparsable signature, and any mismatch all
/// refuse — an unsigned, tampered, or corrupt setup exe never reaches the
/// spawn step.
fn verify_installer_signature(
    public_key_body: &str,
    installer_bytes: &[u8],
    signature_text: &str,
) -> Result<(), String> {
    if public_key_body.trim().is_empty() {
        return Err(
            "this build has no update-signature key embedded — refusing to run the installer"
                .into(),
        );
    }
    let public_key = PublicKey::from_base64(public_key_body).map_err(|_| {
        "the embedded update-signature key is unreadable — refusing to run the installer".to_string()
    })?;
    let signature = decode_signature(signature_text)
        .map_err(|_| "the downloaded signature could not be read — refusing to run the installer".to_string())?;
    // Legacy mode off: CI's `tauri signer sign` emits pre-hashed minisign
    // signatures, and anything older must never pass by accident.
    public_key
        .verify(installer_bytes, &signature, false)
        .map_err(|_| {
            "the installer failed its signature check — it may be tampered with or corrupt; refusing to run it"
                .to_string()
        })
}

/// The user-confirmed apply step behind the `install_update` command:
/// downloads the setup asset to `%TEMP%\<asset-name>` (the URL's last path
/// segment, refused unless it names a real Sprout installer), spawns the
/// installer detached, and exits the app shortly after so NSIS can replace
/// the exe. Unlike the checks, failures here are reported — the user asked
/// for this action explicitly.
pub fn apply_update(app: &AppHandle, url: &str) -> Result<(), String> {
    let name = url.rsplit('/').next().unwrap_or("");
    if !is_update_asset_name(name) {
        return Err("That download is not a Sprout installer — refusing to run it.".into());
    }
    let target = std::env::temp_dir().join(name);
    download_to_file(url, &target)?;
    let installer_bytes = std::fs::read(&target)
        .map_err(|e| format!("the downloaded installer could not be opened: {e}"))?;
    let signature = download_to_string(&format!("{url}.sig"))?;
    verify_installer_signature(UPDATE_PUBKEY, &installer_bytes, &signature)?;
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

    /// A recorded pre-dual-train `/releases/latest` response for v0.5.0
    /// carrying the setup exe plus an unrelated asset, in GitHub's field
    /// order and shape. Bare `v*` tags are retired in 207 — kept as the
    /// "other/bare tags are ignored" control.
    const RELEASE_V050: &str =
        include_str!("update/fixtures/release-v050.json");
    /// A prerelease-tagged response — `/releases/latest` never returns one,
    /// but a stable release may still carry a prerelease-suffixed tag.
    const RELEASE_V060_RC1: &str =
        include_str!("update/fixtures/release-v060-rc1.json");
    /// The real shape of the first CI-published release: action-gh-release
    /// left `body` explicitly null, which a plain `String` field rejects —
    /// the silent "always up to date" bug this fixture pins (ADR-0012).
    const RELEASE_V042_NULL_BODY: &str =
        include_str!("update/fixtures/release-v042-null-body.json");

    // -- train-prefixed semver comparison ---------------------------------

    #[test]
    fn strictly_newer_tags_are_newer() {
        assert!(is_newer("v0.5.0", "0.4.1"));
        assert!(is_newer("0.5.0", "0.4.9"));
        assert!(is_newer("v1.0.0", "0.99.99"));
        assert!(is_newer("0.4.2", "0.4.1"));
        // Namespaced trains strip their prefix before comparing.
        assert!(is_newer("win-v1.0.5", "1.0.4"));
        assert!(is_newer("mac-v0.2.0", "0.1.0"));
        assert!(is_newer("win-v1.0.5", "win-v1.0.4"));
        assert!(is_newer("mac-v0.1.1", "mac-v0.1.0"));
    }

    #[test]
    fn same_or_older_tags_and_v_prefixes_compare_cleanly() {
        assert!(!is_newer("v0.4.1", "0.4.1"));
        assert!(!is_newer("0.5.0", "v0.5.0"));
        assert!(!is_newer("v0.3.9", "0.4.1"));
        assert!(!is_newer("v0.4.0", "0.4.1"));
        assert!(!is_newer("win-v1.0.4", "1.0.4"));
        assert!(!is_newer("mac-v0.1.0", "0.1.0"));
        assert!(!is_newer("win-v1.0.3", "1.0.4"));
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
        assert!(!is_newer("win-v1.0", "1.0.4"));
        assert!(!is_newer("mac-v0.1.0.1", "0.1.0"));
        assert!(!is_newer("win-vx.y.z", "1.0.4"));
    }

    #[test]
    fn prerelease_looking_tags_are_ignored() {
        // Bigger numbers behind a prerelease suffix never win.
        assert!(!is_newer("v0.6.0-rc.1", "0.4.1"));
        assert!(!is_newer("0.6.0-beta", "0.4.1"));
        assert!(!is_newer("v0.5.0-rc.1+build.7", "0.4.1"));
        assert!(!is_newer("win-v1.0.6-rc.1", "1.0.4"));
        assert!(!is_newer("mac-v0.2.0-beta", "0.1.0"));
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

    #[test]
    fn null_body_and_null_asset_fields_parse() {
        // GitHub's real shape for a notes-less release: explicit nulls, not
        // absent fields — `#[serde(default)]` alone rejects these.
        let release = parse_release(RELEASE_V042_NULL_BODY).expect("null-body release parses");
        assert_eq!(release.tag, "v0.4.2");
        assert_eq!(release.notes, "");
        assert_eq!(release.assets.len(), 1);
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
        // The Windows matcher never claims Mac assets and vice versa.
        assert!(!is_setup_asset_name("Sprout_0.1.0_aarch64.dmg"));
        assert!(!is_mac_asset_name("Sprout_0.5.0_x64-setup.exe"));
    }

    #[test]
    fn mac_asset_pattern_matches_dmgs_only() {
        assert!(is_mac_asset_name("Sprout_0.1.0_aarch64.dmg"));
        assert!(is_mac_asset_name("Sprout_0.2.0_x86_64.dmg"));
        assert!(!is_mac_asset_name("Sprout_0.1.0_aarch64.dmg.sig"));
        assert!(!is_mac_asset_name("sprout_0.1.0_aarch64.dmg"));
        assert!(!is_mac_asset_name("MyApp_0.1.0_aarch64.dmg"));
        assert!(!is_mac_asset_name("Sprout_0.1.0_aarch64.zip"));
    }

    #[test]
    fn train_asset_routing_ignores_the_other_platform() {
        assert!(is_train_asset_name("Sprout_1.0.5_x64-setup.exe", WIN_TAG_PREFIX));
        assert!(!is_train_asset_name("Sprout_0.1.0_aarch64.dmg", WIN_TAG_PREFIX));
        assert!(is_train_asset_name("Sprout_0.1.0_aarch64.dmg", MAC_TAG_PREFIX));
        assert!(!is_train_asset_name("Sprout_1.0.5_x64-setup.exe", MAC_TAG_PREFIX));
    }

    // -- the full decision ------------------------------------------------

    /// One single-release payload for the Windows train.
    fn win_release(tag: &str, asset: &str) -> String {
        format!(
            r#"{{"tag_name": "{tag}", "body": "", "assets": [{{"name": "{asset}", "browser_download_url": "https://example.com/{asset}"}}]}}"#
        )
    }

    /// One single-release payload for the Mac train.
    fn mac_release(tag: &str, asset: &str) -> String {
        format!(
            r#"{{"tag_name": "{tag}", "body": "", "assets": [{{"name": "{asset}", "browser_download_url": "https://example.com/{asset}"}}]}}"#
        )
    }

    #[test]
    fn win_train_release_reads_as_an_update_for_an_older_build() {
        let payload = win_release("win-v1.0.5", "Sprout_1.0.5_x64-setup.exe");
        let update =
            evaluate_in_train(&payload, "1.0.4", WIN_TAG_PREFIX).expect("update available");
        assert_eq!(update.version, "1.0.5");
        assert_eq!(update.url, "https://example.com/Sprout_1.0.5_x64-setup.exe");
    }

    #[test]
    fn mac_train_release_reads_as_an_update_for_an_older_build() {
        let payload = mac_release("mac-v0.1.1", "Sprout_0.1.1_aarch64.dmg");
        let update =
            evaluate_in_train(&payload, "0.1.0", MAC_TAG_PREFIX).expect("update available");
        assert_eq!(update.version, "0.1.1");
        assert_eq!(update.url, "https://example.com/Sprout_0.1.1_aarch64.dmg");
    }

    #[test]
    fn retired_bare_tags_are_ignored_by_both_trains() {
        // The pre-dual-train fixtures carry bare `v*` tags: retired in 207,
        // they must never surface as updates on either train.
        assert!(evaluate_in_train(RELEASE_V050, "0.4.1", WIN_TAG_PREFIX).is_none());
        assert!(evaluate_in_train(RELEASE_V050, "0.4.1", MAC_TAG_PREFIX).is_none());
        assert!(evaluate_in_train(RELEASE_V042_NULL_BODY, "0.4.1", WIN_TAG_PREFIX).is_none());
        assert!(evaluate_in_train(RELEASE_V042_NULL_BODY, "0.4.1", MAC_TAG_PREFIX).is_none());
        // Production entry point follows this build's train — bare tags read
        // as up to date there too.
        assert!(evaluate(RELEASE_V050, "0.4.1").is_none());
    }

    #[test]
    fn each_train_ignores_the_other_trains_tags() {
        let win_payload = win_release("win-v9.0.0", "Sprout_9.0.0_x64-setup.exe");
        let mac_payload = mac_release("mac-v9.0.0", "Sprout_9.0.0_aarch64.dmg");
        assert!(evaluate_in_train(&win_payload, "0.1.0", MAC_TAG_PREFIX).is_none());
        assert!(evaluate_in_train(&mac_payload, "1.0.4", WIN_TAG_PREFIX).is_none());
        // Even a much larger version on the wrong train never wins.
        assert!(evaluate_in_train(&mac_payload, "0.0.1", WIN_TAG_PREFIX).is_none());
    }

    #[test]
    fn each_train_ignores_the_other_trains_assets() {
        // Right tag, wrong asset: a dmg riding a win-v release (and vice
        // versa) is not a usable installer for that train.
        let win_tag_mac_asset = win_release("win-v1.0.5", "Sprout_1.0.5_aarch64.dmg");
        let mac_tag_win_asset = mac_release("mac-v0.1.1", "Sprout_0.1.1_x64-setup.exe");
        assert!(evaluate_in_train(&win_tag_mac_asset, "1.0.4", WIN_TAG_PREFIX).is_none());
        assert!(evaluate_in_train(&mac_tag_win_asset, "0.1.0", MAC_TAG_PREFIX).is_none());
    }

    #[test]
    fn same_version_and_prerelease_fixtures_read_as_up_to_date() {
        // Equal versions are not updates, on either train.
        let win_same = win_release("win-v1.0.4", "Sprout_1.0.4_x64-setup.exe");
        assert!(evaluate_in_train(&win_same, "1.0.4", WIN_TAG_PREFIX).is_none());
        let win_older = win_release("win-v1.0.5", "Sprout_1.0.5_x64-setup.exe");
        assert!(evaluate_in_train(&win_older, "1.0.6", WIN_TAG_PREFIX).is_none());
        // Prerelease-looking tag: ignored even though 0.6 > 0.4.
        assert!(evaluate(RELEASE_V060_RC1, "0.4.1").is_none());
        let win_rc = win_release("win-v1.0.6-rc.1", "Sprout_1.0.6-rc.1_x64-setup.exe");
        assert!(evaluate_in_train(&win_rc, "1.0.4", WIN_TAG_PREFIX).is_none());
    }

    #[test]
    fn a_release_without_a_usable_installer_is_not_an_update() {
        let payload = r#"{
            "tag_name": "win-v9.0.0",
            "body": "",
            "assets": [
                {"name": "latest.json",
                 "browser_download_url": "https://example.com/latest.json"}
            ]
        }"#;
        assert!(evaluate_in_train(payload, "1.0.4", WIN_TAG_PREFIX).is_none());
        assert!(evaluate(payload, "1.0.4").is_none());
    }

    // -- release-list feed filter (dual trains, no network) ---------------

    /// One `/releases` list entry.
    fn list_entry(tag: &str, asset: &str) -> String {
        format!(
            r#"{{"tag_name": "{tag}", "body": "", "assets": [{{"name": "{asset}", "browser_download_url": "https://example.com/{asset}"}}]}}"#
        )
    }

    #[test]
    fn release_lists_parse_into_releases() {
        let payload = format!(
            "[{}, {}]",
            list_entry("win-v1.0.5", "Sprout_1.0.5_x64-setup.exe"),
            list_entry("mac-v0.1.1", "Sprout_0.1.1_aarch64.dmg"),
        );
        let releases = parse_releases(&payload).expect("list parses");
        assert_eq!(releases.len(), 2);
        assert_eq!(releases[0].tag, "win-v1.0.5");
        assert_eq!(releases[1].tag, "mac-v0.1.1");
    }

    #[test]
    fn malformed_list_payloads_yield_none() {
        assert!(parse_releases("").is_none());
        assert!(parse_releases("<html>rate limited</html>").is_none());
        assert!(parse_releases(r#"{"tag_name": "win-v1.0.5"}"#).is_none());
        assert!(evaluate_list("not json", "1.0.4").is_none());
        assert!(evaluate_list("[]", "1.0.4").is_none());
    }

    #[test]
    fn list_picks_the_newest_same_train_release() {
        // Newest-first (GitHub order) with both trains interleaved plus a
        // retired bare tag: Windows must land on win-v1.0.6, Mac on
        // mac-v0.1.1, and neither may surface the other's newer release.
        let payload = format!(
            "[{}, {}, {}, {}, {}]",
            list_entry("mac-v0.2.0", "Sprout_0.2.0_aarch64.dmg"),
            list_entry("win-v1.0.6", "Sprout_1.0.6_x64-setup.exe"),
            list_entry("v9.9.9", "Sprout_9.9.9_x64-setup.exe"),
            list_entry("win-v1.0.5", "Sprout_1.0.5_x64-setup.exe"),
            list_entry("mac-v0.1.1", "Sprout_0.1.1_aarch64.dmg"),
        );
        let win = evaluate_list_in_train(&payload, "1.0.4", WIN_TAG_PREFIX)
            .expect("win update available");
        assert_eq!(win.version, "1.0.6");
        let mac = evaluate_list_in_train(&payload, "0.1.0", MAC_TAG_PREFIX)
            .expect("mac update available");
        assert_eq!(mac.version, "0.2.0");
    }

    #[test]
    fn list_ignores_newer_other_train_releases() {
        // The other train is far ahead, this train has nothing new: up to
        // date, not a cross-train "update".
        let payload = format!(
            "[{}]",
            list_entry("mac-v9.0.0", "Sprout_9.0.0_aarch64.dmg"),
        );
        assert!(evaluate_list_in_train(&payload, "1.0.4", WIN_TAG_PREFIX).is_none());
        let payload = format!(
            "[{}]",
            list_entry("win-v9.0.0", "Sprout_9.0.0_x64-setup.exe"),
        );
        assert!(evaluate_list_in_train(&payload, "0.1.0", MAC_TAG_PREFIX).is_none());
    }

    #[test]
    fn list_skips_same_train_releases_without_a_usable_asset() {
        // win-v1.0.7 carries no installer; the older win-v1.0.6 with one
        // still surfaces.
        let payload = format!(
            "[{}, {}]",
            r#"{"tag_name": "win-v1.0.7", "body": "", "assets": [{"name": "notes.txt", "browser_download_url": "https://example.com/notes.txt"}]}"#,
            list_entry("win-v1.0.6", "Sprout_1.0.6_x64-setup.exe"),
        );
        let update = evaluate_list_in_train(&payload, "1.0.4", WIN_TAG_PREFIX)
            .expect("falls back to the older usable release");
        assert_eq!(update.version, "1.0.6");
    }

    #[test]
    fn list_with_no_usable_same_train_release_is_not_an_update() {
        let payload = format!(
            "[{}, {}]",
            list_entry("mac-v0.1.1", "Sprout_0.1.1_aarch64.dmg"),
            r#"{"tag_name": "v9.0.0", "body": "", "assets": [{"name": "Sprout_9.0.0_x64-setup.exe", "browser_download_url": "https://example.com/a.exe"}]}"#,
        );
        assert!(evaluate_list_in_train(&payload, "1.0.4", WIN_TAG_PREFIX).is_none());
    }

    // -- signature verification -------------------------------------------
    // Generated once during implementation with two THROWAWAY keypairs
    // (`tauri signer generate`); only public keys and signatures are
    // committed here — the private halves were deleted.

    /// Public half of throwaway fixture key A.
    const FIXTURE_PUBKEY_A: &str = "RWTglbqsMsbGN4HSSS/bPaUG0JQe85HXO9QOYvhArOJNdgnoBKwoAZrn";
    /// Public half of throwaway fixture key B — the wrong-key control.
    const FIXTURE_PUBKEY_B: &str = "RWR3mjRnDMqxx7CFRPJUtcIN2R4edLWL4L7p4JPSnqVkvwRgSI4iQvqI";
    /// The exact bytes fixture A signed.
    const FIXTURE_PAYLOAD: &[u8] = b"Sprout update-signature fixture v1";
    /// Fixture A's minisign signature over [`FIXTURE_PAYLOAD`], verbatim
    /// `.sig` text (the trusted comment is part of the signed data and its
    /// separator is a literal tab).
    const FIXTURE_SIG_A: &str = concat!(
        "untrusted comment: signature from tauri secret key\n",
        "RUTglbqsMsbGN42Wh9VrDLz94YEesD3BzWrepMqOz7mergXNkAMM7jeN12fvO+lOJcuGfYn29kUQfScZA1sehOkFtpcZsWf+mAQ=\n",
        "trusted comment: timestamp:1787573500\tfile:payload_a.bin\n",
        "jVyuk/E8gPs8BShFplaXaahDIR2/0C4rE0HXB8MjBGZHRJOf0TKf2UDJSLh1aRFAd0VgnBy1W9sHuOz+drkZDA==\n",
    );

    #[test]
    fn genuine_bytes_verify_against_the_signing_key() {
        assert!(verify_installer_signature(FIXTURE_PUBKEY_A, FIXTURE_PAYLOAD, FIXTURE_SIG_A).is_ok());
    }

    #[test]
    fn the_shipped_pubkey_is_a_valid_minisign_key() {
        // Fail-closed means an empty key refuses installs — but a *garbled*
        // non-empty one would too, silently bricking updates. This keeps the
        // embedded constant itself honest.
        assert!(!UPDATE_PUBKEY.trim().is_empty());
        assert!(PublicKey::from_base64(UPDATE_PUBKEY).is_ok());
    }

    #[test]
    fn one_flipped_byte_rejects() {
        let mut tampered = FIXTURE_PAYLOAD.to_vec();
        let last = tampered.len() - 1;
        tampered[last] ^= 1;
        assert!(verify_installer_signature(FIXTURE_PUBKEY_A, &tampered, FIXTURE_SIG_A).is_err());
    }

    #[test]
    fn wrong_pubkey_rejects() {
        assert!(verify_installer_signature(FIXTURE_PUBKEY_B, FIXTURE_PAYLOAD, FIXTURE_SIG_A).is_err());
    }

    #[test]
    fn empty_key_refuses_closed() {
        let err = verify_installer_signature("", FIXTURE_PAYLOAD, FIXTURE_SIG_A)
            .expect_err("empty key must refuse");
        assert!(err.contains("no update-signature key"));
        assert!(verify_installer_signature("   ", FIXTURE_PAYLOAD, FIXTURE_SIG_A).is_err());
    }

    #[test]
    fn unreadable_signature_text_refuses() {
        assert!(verify_installer_signature(FIXTURE_PUBKEY_A, FIXTURE_PAYLOAD, "not a signature").is_err());
    }

    /// The real `.sig` asset CI published with v0.4.8, captured verbatim from
    /// the GitHub release (public data): one base64 line wrapping the
    /// minisign text — exactly what `tauri signer sign` writes.
    const REAL_CI_SIG_V048: &str =
        include_str!("update/fixtures/ci-signature-v048-wrapped.txt");

    #[test]
    fn the_real_ci_signature_asset_clears_the_decode_stage() {
        // Over a wrong payload the only acceptable refusal is the mismatch
        // message — a "could not be read" here means CI's signature format
        // itself failed to parse.
        let err = verify_installer_signature(UPDATE_PUBKEY, b"not the installer bytes", REAL_CI_SIG_V048)
            .expect_err("wrong payload must not verify");
        assert!(!err.contains("could not be read"), "CI's signature must parse; got: {err}");
    }

    #[test]
    fn tauri_wrapped_signatures_verify_end_to_end() {
        // CI signs with `tauri signer sign`, which base64-wraps the minisign
        // text (Tauri's updater sidecar format); the verifier must accept
        // that shape as well as bare minisign files.
        use base64::Engine as _;
        let wrapped = base64::engine::general_purpose::STANDARD.encode(FIXTURE_SIG_A);
        assert!(verify_installer_signature(FIXTURE_PUBKEY_A, FIXTURE_PAYLOAD, &wrapped).is_ok());
    }

    #[test]
    fn wrapped_garbage_still_refuses_closed() {
        use base64::Engine as _;
        let wrapped = base64::engine::general_purpose::STANDARD.encode(b"not a signature");
        assert!(verify_installer_signature(FIXTURE_PUBKEY_A, FIXTURE_PAYLOAD, &wrapped).is_err());
    }
}
