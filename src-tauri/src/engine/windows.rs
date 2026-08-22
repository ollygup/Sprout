//! The v1 platform engine: winget on Windows.
//!
//! Porting source: legacy/scripts/runner.ps1 (two-tier detection, exit-code
//! whitelist, timeboxing, env wiring, registry heuristics, winget bootstrap,
//! the nvm custom step). Detection is implemented here (ticket 04);
//! install/upgrade with the timeboxed runner and the ported exit-code
//! whitelist arrived with the run phase (ticket 05); verify commands and env
//! wiring landed in ticket 07; command steps, the winget bootstrap, and the
//! PATH refresh for custom steps came with ticket 08.
//!
//! Detection is read-only and needs no elevation: one `winget list` snapshot
//! (installed + available versions per id) plus one uninstall-registry scan
//! (DisplayName heuristics, the same three hives the legacy runner used).
//! Env wiring writes User scope only (`HKCU\Environment`), resolved via the
//! same uninstall-registry scan, and never overwrites anything that exists.
//! The winget bootstrap runs only in the elevated worker, where
//! `Add-AppxPackage` is allowed.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Read};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Deserialize;
use windows_sys::Win32::Foundation::HWND;
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_SET_VALUE, REG_EXPAND_SZ, REG_SZ};
use winreg::RegKey;

use crate::domain::{EnvAction, EnvWiring, Product, Requirement, Step, VerifyCommand};
use crate::engine::{Detection, PlatformEngine, StepOutcome, VerifyOutcome};

/// How long a verify command may run before it is killed like a hung
/// installer: verifies must be quick (e.g. `java -version`), and a hung one
/// must never wedge the whole Run.
const VERIFY_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// The whole winget bootstrap may take a while: the msixbundle is tens of MB
/// and the Appx install itself is not instant.
const BOOTSTRAP_TIMEOUT: Duration = Duration::from_secs(20 * 60);

/// The release-info fetch is small and must not hang the bootstrap.
const RELEASE_FETCH_TIMEOUT: Duration = Duration::from_secs(60);

/// The minimum Windows build winget runs on — the legacy runner's check
/// (Windows 10 2004 / 10.0.19041 and later).
const MIN_WINGET_BUILD: u32 = 19041;

/// The minimum Windows build the virtual-desktop surface runs on — Windows
/// 11 24H2 (26100), where the IVirtualDesktopManager service the winvd crate
/// drives actually exists. Below this gate `desktops()` is empty,
/// `create_desktop()` is `None`, and every move is an error: the whole
/// assignment surface hides itself (ticket 44).
const MIN_VIRTUAL_DESKTOP_BUILD: u32 = 26100;

/// A command step's exit code when its author declared none: 0 only.
const DEFAULT_SUCCESS_CODE: i32 = 0;

/// The creation flag that suppresses the console window a spawned console
/// app would otherwise create (ticket 18): every subprocess in the engine —
/// winget, PowerShell, command steps, taskkill — carries it, so a run never
/// flashes cmd/powershell windows. The run logs are the record.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Applies `CREATE_NO_WINDOW` to a Command builder before it is spawned.
/// Shared with the Quick Actions runner (ticket 50) — every subprocess in the
/// app carries the flag, so a run never flashes a console window.
pub(crate) fn hidden(mut command: Command) -> Command {
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

/// The placeholder prefix for the installed product's location. The full
/// forms are `<InstallLocation>` (bare) and `<InstallLocation:hint>` (inline
/// hint) — the closing `>` follows the hint in the inline form, so matching
/// on the prefix (not the whole `<InstallLocation>`) is what catches both.
const INSTALL_LOCATION_PREFIX: &str = "<InstallLocation";

/// Registry hives the uninstall scan reads, in the legacy runner's order.
/// The `HKEY` type is windows-sys's; winreg re-exports the predef constants.
type HKEY = windows_sys::Win32::System::Registry::HKEY;
const UNINSTALL_HIVES: [(HKEY, &str); 3] = [
    (
        HKEY_LOCAL_MACHINE,
        r"Software\Microsoft\Windows\CurrentVersion\Uninstall",
    ),
    (
        HKEY_LOCAL_MACHINE,
        r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
    ),
    (
        HKEY_CURRENT_USER,
        r"Software\Microsoft\Windows\CurrentVersion\Uninstall",
    ),
];

pub struct WindowsWingetEngine;

impl WindowsWingetEngine {
    /// Is `winget` on PATH at all? Absent winget means nothing is
    /// winget-managed; bootstrap is a run-phase concern (ticket 08).
    fn winget_available() -> bool {
        hidden(Command::new("winget"))
            .arg("--version")
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    /// Does this Run need winget on PATH? Command-only runs (e.g. node-lts
    /// via nvm) detect against the registry and never touch winget.
    fn requires_winget(requirements: &[&Requirement]) -> bool {
        requirements.iter().any(|r| matches!(r.step, Step::Winget { .. }))
    }

    /// Bootstraps the App Installer (winget) when it is missing — the port
    /// of the legacy `Install-WingetIfMissing`: only on supported builds
    /// (10.0.19041+), downloads the official winget-cli release's
    /// `.msixbundle` from GitHub, and installs it via `Add-AppxPackage`
    /// (which is why this only ever runs in the elevated worker).
    fn bootstrap_winget() -> Result<(), String> {
        let build = windows_build_number()
            .ok_or_else(|| "cannot determine the Windows build number".to_string())?;
        if build < MIN_WINGET_BUILD {
            return Err(format!(
                "winget is missing and this Windows build ({build}) is unsupported — Sprout needs \
                 Windows 10 build 19041 or later. Install the App Installer manually and run this \
                 plan again: https://learn.microsoft.com/windows/package-manager/winget/"
            ));
        }

        let url = Self::fetch_msixbundle_url()?;
        let bundle = Self::download_msixbundle(&url)?;
        let install = run_timed_process(
            "powershell",
            &[
                "-NoProfile".to_string(),
                "-NonInteractive".to_string(),
                "-Command".to_string(),
                format!(
                    "Add-AppxPackage -Path '{}' -ForceApplicationShutdown",
                    bundle.display()
                ),
            ],
            BOOTSTRAP_TIMEOUT,
        );
        let _ = std::fs::remove_file(&bundle);
        if install.timed_out || install.exit_code != Some(0) {
            return Err(format!(
                "winget bootstrap failed: installing the App Installer did not finish cleanly. \
                 Install it manually and run this plan again: \
                 https://learn.microsoft.com/windows/package-manager/winget/ ({})",
                install.output.trim()
            ));
        }

        if !Self::winget_available() {
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

    /// Merges the Machine and User `Path` from the registry into this
    /// process's PATH (the legacy `Update-ProcessPath`), with `%VAR%`
    /// references expanded. Command steps like `nvm.cmd` need entries that
    /// were added after this process started — the registry is the one
    /// source that always has them.
    fn refresh_process_path() {
        let machine = Self::machine_env("Path").unwrap_or_default();
        let user = Self::user_env("Path").unwrap_or_default();
        let merged = join_path_lists(&machine, &user);
        if merged.is_empty() {
            return;
        }
        let expanded = expand_env(&merged);
        let current = std::env::var("Path").unwrap_or_default();
        if current.eq_ignore_ascii_case(&expanded) {
            return;
        }
        Self::set_process_env("Path", &expanded);
    }

    /// One `winget list --source winget` dump of everything installed:
    /// winget id (lowercased) -> (installed version, available version).
    /// `None` when winget is missing or the source cannot be read.
    fn winget_snapshot() -> Option<HashMap<String, (String, Option<String>)>> {
        if !Self::winget_available() {
            return None;
        }
        let out = hidden(Command::new("winget"))
            .args([
                "list",
                "--source",
                "winget",
                "--accept-source-agreements",
                "--disable-interactivity",
            ])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        Some(parse_winget_list(&String::from_utf8_lossy(&out.stdout)))
    }

    /// Every DisplayName found under the three uninstall-registry hives the
    /// legacy runner scanned (HKLM, WOW6432Node, HKCU).
    fn registry_display_names() -> Vec<String> {
        let mut names = Vec::new();
        for (root, path) in UNINSTALL_HIVES {
            let Ok(uninstall) = RegKey::predef(root).open_subkey(path) else {
                continue;
            };
            for name in uninstall.enum_keys().flatten() {
                let Ok(sub) = uninstall.open_subkey(&name) else {
                    continue;
                };
                if let Ok(display) = sub.get_value::<String, _>("DisplayName") {
                    names.push(display);
                }
            }
        }
        names
    }

    /// The installed location of a product whose uninstall key's DisplayName
    /// contains `hint` — the legacy `Resolve-InstallLocation`, same three
    /// hives and same rules (first key that matches and carries a non-blank
    /// InstallLocation; trailing backslash trimmed).
    fn resolve_install_location(hint: &str) -> Option<String> {
        for (root, path) in UNINSTALL_HIVES {
            let Ok(uninstall) = RegKey::predef(root).open_subkey_with_flags(path, KEY_READ) else {
                continue;
            };
            for name in uninstall.enum_keys().flatten() {
                let Ok(sub) = uninstall.open_subkey_with_flags(&name, KEY_READ) else {
                    continue;
                };
                let Ok(display) = sub.get_value::<String, _>("DisplayName") else {
                    continue;
                };
                if !display.to_lowercase().contains(&hint.to_lowercase()) {
                    continue;
                }
                let Ok(location) = sub.get_value::<String, _>("InstallLocation") else {
                    continue;
                };
                let trimmed = location.trim().trim_end_matches('\\');
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
        None
    }

    /// The product's env value with the `<InstallLocation>` placeholder
    /// resolved from the uninstall registry. `<InstallLocation:hint>` uses
    /// the inline hint; the bare tag falls back to the Product's
    /// `install_location_hint`. Values without a placeholder are literal and
    /// returned unchanged. `None` when the hint cannot be resolved — the
    /// caller skips the entry with a note.
    fn resolve_env_value(value: &str, product: &Product) -> Option<String> {
        if !value.contains(INSTALL_LOCATION_PREFIX) {
            return Some(value.to_string());
        }
        let (hint, before, after) = split_placeholder(value, product)?;
        let location = Self::resolve_install_location(&hint)?;
        Some(format!("{before}{location}{after}"))
    }

    /// Current value of a User-scope environment variable (HKCU\Environment,
    /// raw — the same source .NET's User scope reads).
    fn user_env(name: &str) -> Option<String> {
        env_value_at(
            (HKEY_CURRENT_USER, r"Environment"),
            name,
        )
    }

    /// Current value of a Machine-scope environment variable (the system
    /// environment key — .NET's Machine scope).
    fn machine_env(name: &str) -> Option<String> {
        env_value_at(
            (HKEY_LOCAL_MACHINE, r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment"),
            name,
        )
    }

    /// Writes a User-scope environment variable (REG_EXPAND_SZ when the value
    /// contains `%`, REG_SZ otherwise — what .NET writes for User scope).
    /// Registry string values are UTF-16LE with a null terminator; writing
    /// anything else corrupts the value on read.
    fn set_user_env(name: &str, value: &str) {
        let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(
            r"Environment",
            KEY_SET_VALUE,
        ) else {
            return;
        };
        let value_type = if value.contains('%') { REG_EXPAND_SZ } else { REG_SZ };
        let bytes = utf16_with_nul(value);
        let _ = key.set_raw_value(name, &winreg::RegValue {
            vtype: value_type,
            bytes: bytes.into(),
        });
    }

    /// Makes the wiring visible to this process and its children (the verify
    /// commands and later steps in the same Run), as the legacy runner's
    /// `Set-Item Env:` did — the registry write alone only reaches new
    /// processes.
    fn set_process_env(name: &str, value: &str) {
        use windows_sys::Win32::System::Environment::SetEnvironmentVariableW;
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let value_wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            SetEnvironmentVariableW(wide.as_ptr(), value_wide.as_ptr());
        }
    }

    /// One `set` wiring: applied only when both User and Machine scopes are
    /// unset, never overwriting either.
    fn apply_set(product: &Product, wiring: &EnvWiring) -> String {
        let user = Self::user_env(&wiring.name);
        let machine = Self::machine_env(&wiring.name);
        let Some(value) = Self::resolve_env_value(&wiring.value, product) else {
            return format!(
                "env: cannot resolve install location for {} (hint: {}) - skipped",
                wiring.name,
                hint_of(&wiring.value, product).unwrap_or("none")
            );
        };
        let (note, to_write) = decide_set(wiring, &value, user.as_deref(), machine.as_deref());
        if let Some(to_write) = to_write {
            Self::set_user_env(&wiring.name, &to_write);
            Self::set_process_env(&wiring.name, &to_write);
        }
        note
    }

    /// One `prepend` wiring: applied only when the value is absent from both
    /// User and Machine scopes (case-insensitive, on the `;` list boundary).
    fn apply_prepend(product: &Product, wiring: &EnvWiring) -> String {
        let user = Self::user_env(&wiring.name).unwrap_or_default();
        let machine = Self::machine_env(&wiring.name).unwrap_or_default();
        let Some(value) = Self::resolve_env_value(&wiring.value, product) else {
            return format!(
                "env: cannot resolve install location for {} (hint: {}) - skipped",
                wiring.name,
                hint_of(&wiring.value, product).unwrap_or("none")
            );
        };
        let (note, to_write) = decide_prepend(wiring, &value, &user, &machine);
        if let Some(to_write) = to_write {
            Self::set_user_env(&wiring.name, &to_write);
            // For this process, prepend onto the live value (which already
            // carries the machine scope) rather than replacing it with the
            // user-scope value — later steps and verify commands in this Run
            // must keep finding System32 and friends.
            let live = std::env::var(&wiring.name).unwrap_or_default();
            if live.is_empty() {
                Self::set_process_env(&wiring.name, &value);
            } else if !live.split(';').any(|part| part.eq_ignore_ascii_case(&value)) {
                Self::set_process_env(&wiring.name, &format!("{value};{live}"));
            }
        }
        note
    }
}

/// The decision for one `set` wiring, given the current User and Machine
/// values: `(note, value_to_write)`, where `value_to_write` is `Some` exactly
/// when the wiring should be applied. Pure — the write stays in the caller,
/// so the no-overwrite rules are testable without touching the registry.
fn decide_set(
    wiring: &EnvWiring,
    value: &str,
    user: Option<&str>,
    machine: Option<&str>,
) -> (String, Option<String>) {
    if user.is_some() || machine.is_some() {
        return (
            format!("env: {} already set - leaving it as-is", wiring.name),
            None,
        );
    }
    (
        format!("env: set {} = {value} (User)", wiring.name),
        Some(value.to_string()),
    )
}

/// The decision for one `prepend` wiring, given the current User and Machine
/// values: `(note, value_to_write)`, where `value_to_write` is the new User
/// value exactly when the entry should be applied. Pure, like [`decide_set`].
fn decide_prepend(
    wiring: &EnvWiring,
    value: &str,
    user: &str,
    machine: &str,
) -> (String, Option<String>) {
    let list = format!("{machine};{user}");
    if list.split(';').any(|part| part.eq_ignore_ascii_case(value)) {
        return (
            format!("env: {} already contains {value} - skipped", wiring.name),
            None,
        );
    }
    let new_user = if user.is_empty() {
        value.to_string()
    } else {
        format!("{value};{user}")
    };
    (
        format!("env: prepend {} = {value} (User)", wiring.name),
        Some(new_user),
    )
}

/// The pieces of a value that carries an `<InstallLocation>` placeholder:
/// the hint to resolve with (inline one, else the Product's), the text
/// before the placeholder, and the text after it. `None` when there is no
/// placeholder or no hint to resolve with. Pure — the registry lookup stays
/// in [`WindowsWingetEngine::resolve_env_value`].
fn split_placeholder(value: &str, product: &Product) -> Option<(String, String, String)> {
    let marker = value.find(INSTALL_LOCATION_PREFIX)?;
    let before = value[..marker].to_string();
    let rest = &value[marker + INSTALL_LOCATION_PREFIX.len()..];
    if let Some(after_colon) = rest.strip_prefix(':') {
        if let Some(end) = after_colon.find('>') {
            let hint = after_colon[..end].trim();
            if !hint.is_empty() {
                return Some((
                    hint.to_string(),
                    before,
                    after_colon[end + 1..].to_string(),
                ));
            }
        }
    } else if let Some(after) = rest.strip_prefix('>') {
        let hint = product.install_location_hint.clone()?;
        return Some((hint, before, after.to_string()));
    }
    None
}

/// Runs one verify command under a timebox and reports `ok` for exit 0 with
/// the declared text (when any) present in the output. Every other outcome —
/// timeout, failed start, non-zero exit, missing text — is a loud failure.
fn verify_with_timeout(command: &VerifyCommand, timeout: Duration) -> VerifyOutcome {
    let run = run_timed_process(&command.command, &command.args, timeout);
    if run.timed_out {
        return VerifyOutcome::failed(
            format!(
                "'{}' did not finish in {} — its processes were killed",
                command.command,
                describe_duration(timeout)
            ),
            run.output,
        );
    }
    let Some(exit) = run.exit_code else {
        return VerifyOutcome::failed(
            format!("'{}' failed to start: {}", command.command, run.output.trim()),
            run.output,
        );
    };
    if exit != 0 {
        return VerifyOutcome::failed(
            format!(
                "'{}' exited {exit} — the product is not behaving as declared",
                command.command
            ),
            run.output,
        );
    }
    match &command.match_text {
        Some(needle) if !run.output.contains(needle) => VerifyOutcome::failed(
            format!(
                "'{}' exited 0 but its output did not contain '{needle}'",
                command.command
            ),
            run.output,
        ),
        Some(needle) => VerifyOutcome::passed(
            format!("'{}' exited 0 and reported '{needle}'", command.command),
            run.output,
        ),
        None => VerifyOutcome::passed(format!("'{}' exited 0", command.command), run.output),
    }
}

fn describe_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs % 60 == 0 {
        format!("{} min", secs / 60)
    } else {
        format!("{secs} s")
    }
}

/// Reads one value from a registry key (raw, no `%` expansion).
fn env_value_at((root, path): (HKEY, &str), name: &str) -> Option<String> {
    let key = RegKey::predef(root)
        .open_subkey_with_flags(path, KEY_READ)
        .ok()?;
    key.get_value::<String, _>(name).ok()
}

/// The hint the given env value would resolve with: the inline
/// `<InstallLocation:hint>` one when present, else the Product's
/// `install_location_hint`. `None` when the value carries no placeholder and
/// the Product has no hint — used for the "cannot resolve" note.
fn hint_of<'a>(value: &'a str, product: &'a Product) -> Option<&'a str> {
    let marker = value.find(INSTALL_LOCATION_PREFIX)?;
    let rest = &value[marker + INSTALL_LOCATION_PREFIX.len()..];
    if let Some(after_colon) = rest.strip_prefix(':') {
        if let Some(end) = after_colon.find('>') {
            let hint = after_colon[..end].trim();
            if !hint.is_empty() {
                return Some(hint);
            }
        }
    } else if rest.strip_prefix('>').is_some() {
        return product.install_location_hint.as_deref();
    }
    None
}

/// The value as a null-terminated UTF-16LE byte string, the only encoding
/// registry string values accept.
fn utf16_with_nul(text: &str) -> Vec<u8> {
    let mut bytes: Vec<u8> = text
        .encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect();
    bytes.extend_from_slice(&[0, 0]);
    bytes
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

/// Runs one PowerShell one-liner under a timebox and returns its stdout.
/// Non-zero exits fail loudly with the raw output attached.
fn powershell_output(script: &str, timeout: Duration) -> Result<String, String> {
    let run = run_timed_process(
        "powershell",
        &[
            "-NoProfile".to_string(),
            "-NonInteractive".to_string(),
            "-Command".to_string(),
            script.to_string(),
        ],
        timeout,
    );
    if run.timed_out {
        return Err("PowerShell did not finish in time — its processes were killed".to_string());
    }
    match run.exit_code {
        Some(0) => Ok(run.output),
        Some(code) => Err(format!(
            "PowerShell exited {code}: {}",
            run.output.trim()
        )),
        None => Err(format!("PowerShell failed to start: {}", run.output.trim())),
    }
}

/// The Windows build number (19045 for 10 22H2, 26100 for 11 24H2), read
/// from the `Windows NT\CurrentVersion` registry key — the same source the
/// legacy `Get-CimInstance Win32_OperatingSystem` reported.
fn windows_build_number() -> Option<u32> {
    let key = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion", KEY_READ)
        .ok()?;
    let build: String = key
        .get_value("CurrentBuild")
        .or_else(|_| key.get_value("CurrentBuildNumber"))
        .ok()?;
    build.trim().parse().ok()
}

/// Joins two `;`-separated lists the way the legacy runner did (Machine
/// first, then User; empty sides collapse).
fn join_path_lists(first: &str, second: &str) -> String {
    match (first.is_empty(), second.is_empty()) {
        (true, true) => String::new(),
        (true, false) => second.to_string(),
        (false, true) => first.to_string(),
        (false, false) => format!("{first};{second}"),
    }
}

/// Expands `%VAR%` references in a value (e.g. the registry's raw Path)
/// against the system environment — a raw registry Path carries
/// `%SystemRoot%` and would be useless to child processes unexpanded.
fn expand_env(value: &str) -> String {
    use windows_sys::Win32::System::Environment::ExpandEnvironmentStringsW;
    let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
    let mut buf = vec![0u16; 32768];
    let len = unsafe {
        ExpandEnvironmentStringsW(wide.as_ptr(), buf.as_mut_ptr(), buf.len() as u32)
    };
    if len == 0 {
        return value.to_string();
    }
    String::from_utf16_lossy(&buf[..(len - 1) as usize])
}

/// Result of a timeboxed external step (the port of the legacy
/// `Start-TimedProcess`). stdout and stderr are merged, stderr lines prefixed
/// with `ERR ` exactly like the legacy per-entry logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessRun {
    pub timed_out: bool,
    pub exit_code: Option<i32>,
    pub output: String,
}

/// A whitelisted winget exit — `ok` with the legacy wording (reboot flag for
/// the 3010 / 1641 / INSTALL_REBOOT_REQUIRED_TO_FINISH family).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WingetReason {
    pub reboot: bool,
    pub detail: String,
}

/// Runs `exe` with `args` under a per-Requirement timebox. If the process
/// outlives the box its whole tree is killed via `taskkill /T /F` (the legacy
/// runner's behavior) and the run is recorded as timed out — a hung installer
/// must never wedge the machine.
pub fn run_timed_process(exe: &str, args: &[String], timeout: Duration) -> ProcessRun {
    run_timed_process_in(None, exe, args, timeout)
}

/// The same as [`run_timed_process`] with an explicit working directory —
/// `cwd` `None` inherits the caller's. Shared with the Quick Actions Test
/// (ticket 50), whose commands honor their configured directory.
pub fn run_timed_process_in(
    cwd: Option<&str>,
    exe: &str,
    args: &[String],
    timeout: Duration,
) -> ProcessRun {
    let mut command = hidden(Command::new(exe));
    command.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            return ProcessRun {
                timed_out: false,
                exit_code: None,
                output: format!("failed to start: {e}"),
            }
        }
    };

    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");
    let output = Arc::new(Mutex::new(String::new()));

    let out = Arc::clone(&output);
    let reader = std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        let mut reader = stdout;
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => out
                    .lock()
                    .expect("output lock")
                    .push_str(&String::from_utf8_lossy(&buf[..n])),
            }
        }
    });
    let out = Arc::clone(&output);
    let err_reader = std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            out.lock()
                .expect("output lock")
                .push_str(&format!("ERR {line}\n"));
        }
    });

    let deadline = Instant::now() + timeout;
    let mut timed_out = false;
    let exit_code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) => {}
            Err(_) => break None,
        }
        if Instant::now() >= deadline {
            timed_out = true;
            kill_tree(child.id());
            let _ = child.wait();
            output
                .lock()
                .expect("output lock")
                .push_str(&format!(
                    "\n[TIMED OUT after {}s - killed]\n",
                    timeout.as_secs()
                ));
            break None;
        }
        std::thread::sleep(Duration::from_millis(200));
    };

    // Drain the reader threads before reading the merged output.
    let _ = reader.join();
    let _ = err_reader.join();
    let output = Arc::into_inner(output)
        .expect("reader threads joined")
        .into_inner()
        .expect("output lock");

    ProcessRun {
        timed_out,
        exit_code,
        output,
    }
}

/// Kills a process and its whole tree (`taskkill /T`), as the legacy runner
/// did on timebox expiry. Shared with the Quick Action Stop (ticket 62),
/// whose no-stop-command actions die the same way.
pub(crate) fn kill_tree(pid: u32) {
    let _ = hidden(Command::new("taskkill"))
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status();
}

// ---------------------------------------------------------------------------
// The launcher engine (ticket 42)
// ---------------------------------------------------------------------------

/// The v1 launcher engine (ticket 42): ShellExecuteExW for app entries (the
/// .lnk/.exe as-is), the engine's CREATE_NO_WINDOW convention for command
/// entries (or visible per the entry's show-window toggle), EnumWindows +
/// image-basename matching for the window surface (ticket 48), and the
/// winvd-backed virtual-desktop surface (ticket 44) — runtime-gated on
/// Windows 11 24H2+ like the winget build-number check. The skip decision
/// comes from per-window desktop queries, never the process table: an
/// assigned entry skips only when a window of its image already sits on the
/// assigned desktop. New-window resolution prefers the spawned pid's own
/// window, then a window of the app's image that appeared after the launch
/// (the snapshot preference), then a direct child's (wrapper launchers) —
/// so a launch the shell handed to a running instance (Edge) still gets its
/// fresh window waited on and moved, never one the user already has open.
pub struct WindowsLauncherEngine;

impl crate::engine::LauncherEngine for WindowsLauncherEngine {
    fn spawn(&self, entry: &crate::launch::LaunchEntryInput) -> Result<crate::engine::Spawned, String> {
        match entry.kind {
            crate::launch::LaunchEntryKind::App => spawn_app(&entry.target),
            crate::launch::LaunchEntryKind::Command => spawn_command(entry),
        }
    }

    fn wait_for_new_window(
        &self,
        spawned: &crate::engine::Spawned,
        before: &[usize],
        timeout: Duration,
    ) -> Option<usize> {
        wait_for_new_window(spawned, before, timeout)
    }

    fn move_window_to_desktop(&self, hwnd: usize, guid: &str) -> Result<(), String> {
        move_window_to_desktop(hwnd, guid)
    }

    fn app_windows(&self, target: &str) -> Vec<crate::engine::AppWindow> {
        app_windows(target)
    }

    fn target_exists(&self, target: &str) -> bool {
        target_exists(target)
    }

    fn desktops(&self) -> Vec<crate::engine::DesktopInfo> {
        virtual_desktops()
    }

    fn create_desktop(&self) -> Option<String> {
        create_virtual_desktop()
    }
}

/// Launches an app entry as-is through the shell: ShellExecuteExW on the .lnk
/// or exe path, so shortcut semantics, association handling, and the shell's
/// own environment all come along. Returns the new process's id — or none
/// when the shell hands the launch to an already-running process (ticket 47):
/// Explorer and other single-instance shells report success with no process
/// handle, and that is a *started* launch, never a failure. The window
/// resolution's image fallback finds the window in that case.
fn spawn_app(target: &str) -> Result<crate::engine::Spawned, String> {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError};
    use windows_sys::Win32::System::Threading::GetProcessId;
    use windows_sys::Win32::UI::Shell::{
        ShellExecuteExW, SHELLEXECUTEINFOW, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOCLOSEPROCESS,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let mut file: Vec<u16> = target.encode_utf16().chain(std::iter::once(0)).collect();
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        // NOCLOSEPROCESS hands back a real process handle so the pid can be
        // read; NO_UI keeps a failed launch a quiet error, never a modal
        // dialog popping up over the user's work.
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_FLAG_NO_UI,
        lpFile: file.as_mut_ptr(),
        nShow: SW_SHOWNORMAL,
        ..Default::default()
    };
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        return Err(format!(
            "Windows could not launch '{target}' (error {})",
            unsafe { GetLastError() }
        ));
    }
    // A launch handed to an existing process comes back with no handle — the
    // window resolution's image match finds the window it opened. Success
    // without a pid is a started launch, never an error.
    let pid = if info.hProcess.is_null() {
        None
    } else {
        let pid = unsafe { GetProcessId(info.hProcess) };
        let _ = unsafe { CloseHandle(info.hProcess) };
        (pid != 0).then_some(pid)
    };
    Ok(crate::engine::Spawned {
        pid,
        target: target.to_string(),
    })
}

/// Spawns a command entry under its shell: PowerShell/cmd get the engine's
/// non-interactive one-liner convention, "none" launches the command line
/// as-is, and `show_window` trades the CREATE_NO_WINDOW convention for a
/// visible window (the debugging toggle, ticket 41). The process outlives the
/// dropped `Child` — Windows does not kill children when a handle closes.
fn spawn_command(entry: &crate::launch::LaunchEntryInput) -> Result<crate::engine::Spawned, String> {
    let (exe, args) = crate::launch::command_argv(
        entry.shell.unwrap_or(crate::launch::LaunchShell::None),
        &entry.target,
    );
    if exe.is_empty() {
        return Err("the command line has no executable".into());
    }
    let mut command = Command::new(&exe);
    command.args(&args);
    if !entry.show_window {
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let child = command
        .spawn()
        .map_err(|e| format!("failed to start '{exe}': {e}"))?;
    Ok(crate::engine::Spawned {
        pid: Some(child.id()),
        target: exe,
    })
}

/// Polls for the spawn's NEW main window — a visible window of the app's
/// image that was not there when the orchestrator snapshotted `before` —
/// up to `timeout` (ticket 48). The snapshot preference is what keeps a
/// launch the shell handed to a running instance (Edge) from resolving an
/// old window: the window that appeared after the launch is the one that
/// gets waited on and moved, never one the user already has open. Aborts
/// early when a process with a real pid died and left no child to carry the
/// app (its window will never come). `None` is a timeout or a dead process;
/// the orchestrator counts that as started anyway, so the queue never
/// stalls on an app that shows no window (ticket 42).
fn wait_for_new_window(
    spawned: &crate::engine::Spawned,
    before: &[usize],
    timeout: Duration,
) -> Option<usize> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(hwnd) = new_window_for_spawned(spawned, before) {
            return Some(hwnd);
        }
        // A handed-off launch (no pid) has nothing to watch die — poll the
        // new window until the deadline. With a real pid the old abort rule
        // holds unless the process handed the app to a direct child
        // (wrapper launchers): only give up when no child is left alive to
        // show a window.
        if let Some(pid) = spawned.pid {
            if !process_alive(pid) && !children_alive(pid) {
                return None;
            }
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The new-window resolution chain (ticket 48): the spawned pid's own
/// visible top-level window first (a fresh pid's window is new by
/// construction), then any visible window of the app's image that is not in
/// the pre-launch snapshot `before` (a launch the shell handed to a running
/// instance — the running Edge's new window, the fresh Explorer window),
/// then a direct child's window that is not in the snapshot (wrapper
/// launchers — Discord's updater, installer shims). The queue waits on and
/// moves exactly the window this finds — never one the user already has
/// open.
fn new_window_for_spawned(spawned: &crate::engine::Spawned, before: &[usize]) -> Option<usize> {
    if let Some(pid) = spawned.pid {
        if let Some(hwnd) = window_for_pid(pid) {
            return Some(hwnd as usize);
        }
    }
    let image = window_image_basename(&spawned.target);
    let windows = visible_app_windows();
    if let Some(image) = &image {
        for (hwnd, pid) in &windows {
            if !before.contains(hwnd) && process_matches_image(*pid, image) {
                return Some(*hwnd);
            }
        }
    }
    if let Some(pid) = spawned.pid {
        let children: HashSet<u32> = all_processes()
            .into_iter()
            .filter(|(_, parent)| *parent == pid)
            .map(|(child, _)| child)
            .collect();
        for (hwnd, window_pid) in &windows {
            if !before.contains(hwnd) && children.contains(window_pid) {
                return Some(*hwnd);
            }
        }
    }
    None
}

/// Every visible, ownerless, top-level window with a real title that is not
/// shell chrome, as (hwnd, pid) pairs — the shared source for the skip
/// decision, the pre-launch snapshot, and the new-window resolution
/// (ticket 48). Shell chrome never counts (the taskbar, the desktop-icons
/// host, the Start menu are all explorer.exe windows — moving one would
/// wreck the shell).
fn visible_app_windows() -> Vec<(usize, u32)> {
    use windows_sys::core::BOOL;
    use windows_sys::Win32::Foundation::LPARAM;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindow, GetWindowThreadProcessId, IsWindowVisible, GW_OWNER,
    };

    struct Probe {
        found: Vec<(usize, u32)>,
    }
    unsafe extern "system" fn probe_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let probe = lparam as *mut Probe;
        if probe.is_null() {
            return 0;
        }
        let probe = &mut *probe;
        let mut pid: u32 = 0;
        let _ = GetWindowThreadProcessId(hwnd, &mut pid);
        if IsWindowVisible(hwnd) != 0
            && GetWindow(hwnd, GW_OWNER).is_null()
            && !is_shell_chrome(hwnd)
            && !window_title(hwnd).trim().is_empty()
        {
            probe.found.push((hwnd as usize, pid));
        }
        1 // continue
    }

    let mut probe = Probe { found: Vec::new() };
    let _ = unsafe { EnumWindows(Some(probe_window), &mut probe as *mut Probe as LPARAM) };
    probe.found
}

/// The app's visible windows for the skip decision and the pre-launch
/// snapshot (ticket 48): every visible window whose process image's
/// basename equals the entry's target basename — versioned install
/// directories (Edge, Slack, Discord) match the running instance's
/// unversioned image. Each window carries the desktop answers the skip rule
/// is decided from; windows whose desktop cannot be resolved never match an
/// assigned-desktop skip, so a machine that cannot answer the question
/// launches instead of wrongly skipping.
fn app_windows(target: &str) -> Vec<crate::engine::AppWindow> {
    let Some(image) = window_image_basename(target) else {
        return Vec::new();
    };
    visible_app_windows()
        .into_iter()
        .filter(|(_, pid)| process_matches_image(*pid, &image))
        .map(|(hwnd, _)| crate::engine::AppWindow {
            hwnd,
            desktop: window_desktop(hwnd),
            on_current_desktop: window_on_current_desktop(hwnd),
        })
        .collect()
}

/// The desktop GUID a window is on (winvd) — `None` below the 24H2 gate
/// (where the queries cannot work) or on a refused query (a dying window):
/// the assigned-desktop skip then never matches, and the entry launches
/// instead of being wrongly skipped over a window whose desktop cannot be
/// verified.
fn window_desktop(hwnd: usize) -> Option<String> {
    if !virtual_desktops_supported_on_this_machine() {
        return None;
    }
    let desktop = winvd::get_desktop_by_window(winvd_hwnd(hwnd)).ok()?;
    desktop.get_id().ok().map(|id| guid_to_id(&id))
}

/// Whether the window is on the current desktop (winvd). Below the gate or
/// on a refused query every visible window counts as current — the closest
/// available approximation of "open on this desktop", which keeps the skip
/// check meaningful on machines without desktop support instead of silently
/// launching duplicates.
fn window_on_current_desktop(hwnd: usize) -> bool {
    if !virtual_desktops_supported_on_this_machine() {
        return true;
    }
    winvd::is_window_on_current_desktop(winvd_hwnd(hwnd)).unwrap_or(true)
}

/// Whether the process's image file name (basename, case-insensitive)
/// equals `image` (ticket 48): the basename comparison is what makes a
/// versioned install directory like Edge's
/// `...\Application\151.0.4129.86\msedge.exe` match the running instance's
/// unversioned image. An unreadable image is never a match here — the skip
/// decision comes from windows, and a window whose image cannot be read
/// must not count as this app's (unlike the old process-table check, where
/// the safe direction was the opposite).
fn process_matches_image(pid: u32, image: &str) -> bool {
    process_image_path(pid)
        .and_then(|path| image_basename(&path))
        .is_some_and(|basename| basename.eq_ignore_ascii_case(image))
}

/// The lowercase basename of a path — the image key window matching uses
/// (ticket 48): versioned install directories share their basename with the
/// running instance's unversioned image.
fn image_basename(path: &str) -> Option<String> {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
}

/// The image key an entry's target matches windows against: the resolved
/// exe path's lowercase basename (ticket 48). A .lnk resolves through the
/// same IShellLink/raw-bytes resolution as the window fallback.
fn window_image_basename(target: &str) -> Option<String> {
    window_target_exe(target).and_then(|exe| image_basename(&exe))
}

/// The exe path the image matching resolves an entry's target to (ticket
/// 47): IShellLink resolution first, then the raw-bytes extraction for
/// shortcuts the shell link API refuses — File Explorer's shortcut reports
/// no target through IShellLink but carries `%windir%\explorer.exe` in its
/// bytes. Only its basename is compared, so versioned install directories
/// match the running instance's unversioned image (ticket 48).
fn window_target_exe(path: &str) -> Option<String> {
    use crate::walker::LnkResolver;
    let path = Path::new(path);
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("lnk")) {
        if let Some(resolved) = crate::walker::ShellLinkLnkResolver.resolve(path) {
            return Some(resolved);
        }
        return lnk_target_from_bytes(path).map(|target| expand_env(&target));
    }
    Some(path.to_string_lossy().into_owned())
}

/// The target path embedded in a .lnk's raw bytes, for shortcuts that never
/// resolve through the shell link API (ticket 47): the StringData section is
/// plain UTF-16LE, so the heuristic scans the file for the longest printable
/// string that ends in `.exe` — `%windir%\explorer.exe` for File Explorer's
/// shortcut. A wrong pick simply fails the image match (no window moves),
/// never a panic; a path without a real target is no worse than today.
fn lnk_target_from_bytes(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let mut best: Option<String> = None;
    let mut current = String::new();
    let mut index = 0;
    while index + 1 < bytes.len() {
        let (lo, hi) = (bytes[index], bytes[index + 1]);
        if hi == 0 && (0x20..0x7f).contains(&lo) {
            current.push(lo as char);
            index += 2;
        } else {
            if current.len() >= 4 && current.to_lowercase().ends_with(".exe") {
                if best.as_ref().is_none_or(|picked| current.len() > picked.len()) {
                    best = Some(std::mem::take(&mut current));
                } else {
                    current.clear();
                }
            } else {
                current.clear();
            }
            index += 1;
        }
    }
    if current.len() >= 4 && current.to_lowercase().ends_with(".exe") {
        if best.as_ref().is_none_or(|picked| current.len() > picked.len()) {
            best = Some(current);
        }
    }
    best
}

/// The window class name of `hwnd` ("" on failure — never a match for the
/// chrome list).
fn window_class(hwnd: HWND) -> String {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetClassNameW;
    let mut buf = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    if len <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..len as usize])
}

/// The window's title text ("" when the window has none).
fn window_title(hwnd: HWND) -> String {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextW;
    let mut buf = [0u16; 1024];
    let len = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    if len <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..len as usize])
}

/// The shell-chrome window classes the image-match fallback must never pick
/// (ticket 47): the taskbar, the desktop-icons host, and the Start menu
/// islands are all explorer.exe windows with visible ownerless top-level
/// handles — moving one to a virtual desktop would wreck the user's shell.
fn is_shell_chrome(hwnd: HWND) -> bool {
    const CHROME: [&str; 5] = [
        "Shell_TrayWnd",
        "Shell_SecondaryTrayWnd",
        "Progman",
        "WorkerW",
        "XamlExplorerHostIslandWindow",
    ];
    let class = window_class(hwnd);
    CHROME.iter().any(|chrome| class.eq_ignore_ascii_case(chrome))
}

/// Whether any direct child of `pid` is still alive — the wait's abort
/// companion to the child-window step of the new-window resolution: a dead
/// wrapper with a living child still has a window to wait for.
fn children_alive(pid: u32) -> bool {
    all_processes()
        .into_iter()
        .any(|(child, parent)| parent == pid && process_alive(child))
}

/// The first visible, ownerless top-level window owned by `pid` — the
/// handle the desktop move hands to winvd (ticket 44), and the first step
/// of the new-window resolution (ticket 48): a fresh pid's window is new by
/// construction, so it never needs the snapshot check. `None` when the
/// process owns no such window yet.
fn window_for_pid(pid: u32) -> Option<HWND> {
    use windows_sys::core::BOOL;
    use windows_sys::Win32::Foundation::LPARAM;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindow, GetWindowThreadProcessId, IsWindowVisible, GW_OWNER,
    };

    struct Probe {
        pid: u32,
        found: Option<HWND>,
    }
    unsafe extern "system" fn probe_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let probe = lparam as *mut Probe;
        if probe.is_null() {
            return 0;
        }
        let probe = &mut *probe;
        let mut window_pid: u32 = 0;
        let _ = GetWindowThreadProcessId(hwnd, &mut window_pid);
        if window_pid == probe.pid
            && IsWindowVisible(hwnd) != 0
            && GetWindow(hwnd, GW_OWNER).is_null()
        {
            probe.found = Some(hwnd);
            return 0; // stop at the first match
        }
        1 // continue
    }

    let mut probe = Probe { pid, found: None };
    let _ = unsafe { EnumWindows(Some(probe_window), &mut probe as *mut Probe as LPARAM) };
    probe.found
}

/// Whether the process is still alive (the Windows STILL_ACTIVE check).
fn process_alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return false;
    }
    let mut code: u32 = 0;
    let ok = unsafe { GetExitCodeProcess(handle, &mut code) };
    let _ = unsafe { CloseHandle(handle) };
    ok != 0 && code as i32 == STILL_ACTIVE
}

/// Whether the entry's target still exists on disk (ticket 48): a .lnk
/// resolves to its target exe through IShellLink; a bare executable name is
/// PATH-resolvable and never a false failure; and an unresolvable shortcut
/// counts as existing — the shell may still launch it. An app that updated
/// its version folder fails the entry fast with "target no longer exists"
/// instead of the silent 15 s window stall.
fn target_exists(path: &str) -> bool {
    use crate::walker::LnkResolver;
    let path = Path::new(path);
    // A bare executable name resolves through PATH — never a false failure.
    if path.parent().is_none_or(|parent| parent.as_os_str().is_empty()) {
        return true;
    }
    if !path.exists() {
        return false;
    }
    if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("lnk")) {
        return true;
    }
    // A shortcut is broken only when its target positively resolves to a
    // missing file; an unresolvable shortcut counts as existing.
    match crate::walker::ShellLinkLnkResolver.resolve(path) {
        Some(target) => {
            let target = Path::new(&target);
            target.parent().is_none_or(|parent| parent.as_os_str().is_empty())
                || target.exists()
        }
        None => true,
    }
}

/// Every (pid, parent pid) pair on the machine, from one Toolhelp32 snapshot
/// — the shared source for the image matching, the direct-children walk,
/// and the wait's abort rule (tickets 47 & 48). Empty when the snapshot
/// cannot be taken — every caller degrades to "nothing found".
fn all_processes() -> Vec<(u32, u32)> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Vec::new();
    }
    let mut pairs = Vec::new();
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    if unsafe { Process32FirstW(snapshot, &mut entry) } != 0 {
        loop {
            pairs.push((entry.th32ProcessID, entry.th32ParentProcessID));
            if unsafe { Process32NextW(snapshot, &mut entry) } == 0 {
                break;
            }
        }
    }
    let _ = unsafe { CloseHandle(snapshot) };
    pairs
}

/// The full image path of a process, via `QueryFullProcessImageNameW`.
/// `None` when the handle or the query fails — the caller treats that as
/// not matching.
fn process_image_path(pid: u32) -> Option<String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }
    let mut buf = [0u16; 32_768];
    let mut len = buf.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len) };
    let _ = unsafe { CloseHandle(handle) };
    if ok == 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buf[..len as usize]))
}

// ---------------------------------------------------------------------------
// The virtual-desktop surface (ticket 44)
// ---------------------------------------------------------------------------

/// Whether virtual-desktop support is available on a Windows build number:
/// the 24H2 gate (26100), the same registry build-number check the winget
/// bootstrap uses. Pure — the gating rule is testable without a machine.
fn virtual_desktops_supported(build: u32) -> bool {
    build >= MIN_VIRTUAL_DESKTOP_BUILD
}

/// The gate against the live machine: unknown build number means no.
fn virtual_desktops_supported_on_this_machine() -> bool {
    virtual_desktops_supported(windows_build_number().unwrap_or(0))
}

/// The winvd/windows-crate handle for a window: the crate-internal handle
/// and the windows-crate one are the same raw pointer under the hood.
fn winvd_hwnd(hwnd: usize) -> windows::Win32::Foundation::HWND {
    windows::Win32::Foundation::HWND(hwnd as *mut core::ffi::c_void)
}

/// The machine's virtual desktops in Task View order (winvd), each with its
/// id — the GUID, lowercase and unbraced, the same shape the database stores
/// — and its label: the Windows name when the desktop has one, "Desktop N"
/// (1-based index) otherwise. Empty below the 24H2 gate and on any winvd
/// failure, which degrades the whole surface to "hidden" instead of
/// half-working.
fn virtual_desktops() -> Vec<crate::engine::DesktopInfo> {
    if !virtual_desktops_supported_on_this_machine() {
        return Vec::new();
    }
    let Ok(desktops) = winvd::get_desktops() else {
        return Vec::new();
    };
    desktops
        .iter()
        .map(|desktop| crate::engine::DesktopInfo {
            id: desktop.get_id().map(|id| guid_to_id(&id)).unwrap_or_default(),
            name: match desktop.get_name() {
                Ok(name) if !name.trim().is_empty() => name,
                _ => format!("Desktop {}", desktop.get_index().unwrap_or(0) + 1),
            },
        })
        .collect()
}

/// Creates a virtual desktop on the user's behalf (winvd) and returns its id;
/// `None` below the gate or when the OS refused.
fn create_virtual_desktop() -> Option<String> {
    if !virtual_desktops_supported_on_this_machine() {
        return None;
    }
    let desktop = winvd::create_desktop().ok()?;
    desktop.get_id().ok().map(|id| guid_to_id(&id))
}

/// How many times a desktop move is attempted, and the pause between tries
/// (ticket 47): the shell registers a fresh window's desktop view
/// asynchronously, so an immediate move can be refused. Four tries with a
/// half-second pause cover the race in ~1.5 s before the failure surfaces —
/// and a failed move is a note, never a silent drop.
const MOVE_RETRY_ATTEMPTS: u32 = 4;
const MOVE_RETRY_DELAY: Duration = Duration::from_millis(500);

/// Moves the window `hwnd` — the NEW window the queue's wait resolved,
/// never one the user already had open (ticket 48) — to the virtual desktop
/// `guid` (winvd), retrying over ~1.5 s before giving up (ticket 47). Below
/// the gate or on a malformed id it is a clean error — the caller treats
/// any failure as "the entry launched anyway" and notes it.
fn move_window_to_desktop(hwnd: usize, guid: &str) -> Result<(), String> {
    if !virtual_desktops_supported_on_this_machine() {
        return Err(
            "virtual desktops are not supported on this Windows version".into(),
        );
    }
    let guid = parse_guid_id(guid)
        .ok_or_else(|| format!("'{guid}' is not a valid virtual desktop id"))?;
    move_with_retries(MOVE_RETRY_ATTEMPTS, MOVE_RETRY_DELAY, || {
        attempt_move(hwnd, guid)
    })
}

/// One move attempt: hand the window to winvd.
fn attempt_move(hwnd: usize, guid: windows::core::GUID) -> Result<(), String> {
    let windows_hwnd = winvd_hwnd(hwnd);
    winvd::move_window_to_desktop(guid, &windows_hwnd).map_err(|e| {
        format!("Windows could not move the window to its desktop: {e:?}")
    })
}

/// The retry shell around a move attempt (ticket 47): `attempt` is tried up
/// to `attempts` times with `retry_delay` between tries; the first success
/// wins, the last error is what the caller sees. Parameterized so tests drive
/// it without sleeping the real 1.5 s budget. Pure.
fn move_with_retries(
    attempts: u32,
    retry_delay: Duration,
    mut attempt: impl FnMut() -> Result<(), String>,
) -> Result<(), String> {
    let mut last_error = String::from("the move was never attempted");
    for tries in 0..attempts {
        match attempt() {
            Ok(()) => return Ok(()),
            Err(error) => last_error = error,
        }
        if tries + 1 < attempts {
            std::thread::sleep(retry_delay);
        }
    }
    Err(last_error)
}

/// The id shape the database and the API use: the GUID, lowercase, without
/// braces — the same 8-4-4-4-12 shape `launch::validate_launch_entry` checks.
fn guid_to_id(guid: &windows::core::GUID) -> String {
    format!("{:?}", guid).to_lowercase()
}

/// Parses a stored desktop id back into the GUID winvd expects. The windows
/// crate's `GUID::from` panics on a malformed string, so the loose shape
/// check runs first — ids come from the database, which validates on write,
/// but a stale hand-edited value must be an error, never a panic.
fn parse_guid_id(id: &str) -> Option<windows::core::GUID> {
    let parts: Vec<&str> = id.split('-').collect();
    if parts.len() != 5 {
        return None;
    }
    let lengths = [8usize, 4, 4, 4, 12];
    if !parts
        .iter()
        .zip(lengths)
        .all(|(part, len)| part.len() == len && part.chars().all(|c| c.is_ascii_hexdigit()))
    {
        return None;
    }
    Some(windows::core::GUID::from(id))
}

/// Maps a winget exit code (and output) to a result — the port of the legacy
/// `Get-WingetResultReason`. winget returns nonzero codes for benign "nothing
/// to do" outcomes (already installed / already up to date), so those are
/// whitelisted per action or every re-run would look broken. `None` is a
/// genuine failure. Exit codes from the `0x8A1500xx` family exceed i32 range,
/// so the comparison uses the unsigned re-interpretation of the code.
pub fn classify_winget_result(action: &str, exit_code: i32, output: &str) -> Option<WingetReason> {
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
fn winget_step(action: &str, args: &[String], timeout_minutes: u32) -> StepOutcome {
    let timeout = Duration::from_secs(u64::from(timeout_minutes) * 60);
    let run = run_timed_process("winget", args, timeout);
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

/// Runs one command step (executable + args) under the Requirement's
/// timebox, refreshing PATH first (the legacy `Update-ProcessPath` — the
/// `nvm.cmd` flow depends on it). Success is an exit code the step declared
/// in `success_codes` (an empty declaration means "0 only"); a missing
/// executable, a timeout, or an undeclared exit code is a loud failure. A
/// command step is a custom flow, so "upgrade" runs the very same command —
/// the step decides what to do about the installed version itself.
fn command_step(action: &str, step: &Step, timeout_minutes: u32) -> StepOutcome {
    command_step_with_timeout(
        action,
        step,
        Duration::from_secs(u64::from(timeout_minutes) * 60),
    )
}

/// The timeboxed core of [`command_step`], split out so the timeout path is
/// testable without waiting out a full minute.
fn command_step_with_timeout(action: &str, step: &Step, timeout: Duration) -> StepOutcome {
    let Step::Command {
        exe,
        args,
        success_codes,
    } = step
    else {
        unreachable!("command_step only ever receives command steps");
    };
    WindowsWingetEngine::refresh_process_path();

    let run = run_timed_process(exe, args, timeout);
    let log = run.output.clone();
    if run.timed_out {
        return StepOutcome {
            ok: false,
            reboot_required: false,
            timed_out: true,
            detail: format!(
                "{action} did not finish in {} — its processes were killed",
                describe_duration(timeout)
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
    let declared: Vec<i32> = if success_codes.is_empty() {
        vec![DEFAULT_SUCCESS_CODE]
    } else {
        success_codes.clone()
    };
    if declared.contains(&exit) {
        StepOutcome {
            ok: true,
            reboot_required: false,
            timed_out: false,
            detail: if action == "upgrade" {
                "upgraded".into()
            } else {
                "installed".into()
            },
            log,
        }
    } else {
        StepOutcome {
            ok: false,
            reboot_required: false,
            timed_out: false,
            detail: format!(
                "{action} exited {exit} — not in the step's declared success codes {declared:?}"
            ),
            log,
        }
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

/// Combines the winget snapshot and the registry scan into one Detection for
/// a Product, mirroring the legacy two-tier rule: winget wins for version and
/// manageability; the registry catches installs winget does not know about.
fn detection_for(
    product: &Product,
    step: &Step,
    snapshot: &HashMap<String, (String, Option<String>)>,
    registry: &[String],
) -> Detection {
    // Command steps are never winget-managed (e.g. node-lts via nvm).
    let winget = match step {
        Step::Winget { id, .. } => snapshot.get(&id.to_lowercase()).cloned(),
        Step::Command { .. } => None,
    };
    let needle = product.name.to_lowercase();
    let registry_known = registry
        .iter()
        .any(|display| display.to_lowercase().contains(&needle));

    match winget {
        Some((version, available)) => Detection {
            installed: true,
            winget_managed: true,
            installed_version: Some(version),
            available_version: available,
        },
        None => Detection {
            installed: registry_known,
            winget_managed: false,
            installed_version: None,
            available_version: None,
        },
    }
}

/// Parses `winget list` stdout (English column layout, as the legacy runner
/// assumed). Columns are aligned by the header words, so product names that
/// contain spaces parse correctly: the id column starts at "Id", the version
/// at "Version", and the optional available version at "Available".
fn parse_winget_list(text: &str) -> HashMap<String, (String, Option<String>)> {
    #[derive(Clone, Copy)]
    struct Columns {
        id_start: usize,
        version_start: usize,
        available_start: Option<usize>,
    }

    fn find_word(line: &str, word: &str) -> Option<usize> {
        let bytes = line.as_bytes();
        let needle = word.as_bytes();
        let mut i = 0;
        while i + needle.len() <= bytes.len() {
            if &bytes[i..i + needle.len()] == needle {
                let before_ok = i == 0 || bytes[i - 1] == b' ';
                let after_ok =
                    i + needle.len() == bytes.len() || bytes[i + needle.len()] == b' ';
                if before_ok && after_ok {
                    return Some(i);
                }
            }
            i += 1;
        }
        None
    }

    let mut map = HashMap::new();
    let mut columns: Option<Columns> = None;

    for line in text.lines() {
        // Note: no trim_end here — rows end with column padding that the
        // column slices rely on.
        if columns.is_none() {
            if let (Some(id_start), Some(version_start)) =
                (find_word(line, "Id"), find_word(line, "Version"))
            {
                columns = Some(Columns {
                    id_start,
                    version_start,
                    available_start: find_word(line, "Available"),
                });
            }
            continue;
        }
        let cols = columns.unwrap();
        if line.trim().is_empty() || line.trim_start().starts_with("---") {
            continue;
        }
        let id = line.get(cols.id_start..cols.version_start).unwrap_or("").trim();
        if id.is_empty() {
            continue;
        }
        let version = match cols.available_start {
            Some(available) => line.get(cols.version_start..available).unwrap_or("").trim(),
            None => line.get(cols.version_start..).unwrap_or("").trim(),
        };
        if version.is_empty() {
            continue;
        }
        let available = cols
            .available_start
            .and_then(|start| line.get(start..))
            .map(|s| s.trim())
            .filter(|s| !s.is_empty() && *s != "-")
            .map(str::to_string);
        map.insert(id.to_lowercase(), (version.to_string(), available));
    }
    map
}

impl PlatformEngine for WindowsWingetEngine {
    fn prepare(&self, requirements: &[&Requirement]) -> Result<(), String> {
        if Self::winget_available() || !Self::requires_winget(requirements) {
            return Ok(());
        }
        Self::bootstrap_winget()
    }

    fn detect(&self, product: &Product, step: &Step) -> Detection {
        let snapshot = Self::winget_snapshot().unwrap_or_default();
        let registry = Self::registry_display_names();
        detection_for(product, step, &snapshot, &registry)
    }

    fn detect_many(&self, requirements: &[&Requirement]) -> HashMap<String, Detection> {
        let snapshot = Self::winget_snapshot().unwrap_or_default();
        let registry = Self::registry_display_names();
        requirements
            .iter()
            .map(|r| {
                (
                    r.product.id.clone(),
                    detection_for(&r.product, &r.step, &snapshot, &registry),
                )
            })
            .collect()
    }

    fn install(&self, step: &Step, timeout_minutes: u32, install_dir: Option<&str>) -> StepOutcome {
        match step {
            Step::Winget { id, .. } => winget_step(
                "install",
                &winget_args("install", id, install_dir),
                timeout_minutes,
            ),
            Step::Command { .. } => command_step("install", step, timeout_minutes),
        }
    }

    fn upgrade(&self, step: &Step, timeout_minutes: u32, install_dir: Option<&str>) -> StepOutcome {
        match step {
            Step::Winget { id, .. } => winget_step(
                "upgrade",
                &winget_args("upgrade", id, install_dir),
                timeout_minutes,
            ),
            Step::Command { .. } => command_step("upgrade", step, timeout_minutes),
        }
    }

    /// The product's actual install location, resolved from the uninstall
    /// registry via its install-location hint — the post-install honesty
    /// check (ticket 34). `None` when there is no hint to resolve, so the run
    /// never fabricates a note about a location it cannot verify.
    fn actual_install_location(&self, product: &Product) -> Option<String> {
        product
            .install_location_hint
            .as_deref()
            .and_then(Self::resolve_install_location)
    }

    fn verify(&self, command: &VerifyCommand) -> VerifyOutcome {
        verify_with_timeout(command, VERIFY_TIMEOUT)
    }

    fn apply_env_wiring(&self, product: &Product, env: &[EnvWiring]) -> Vec<String> {
        env.iter()
            .map(|wiring| match wiring.action {
                EnvAction::Set => Self::apply_set(product, wiring),
                EnvAction::Prepend => Self::apply_prepend(product, wiring),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn parses_winget_list_rows_with_available_column() {
        // Real winget output pads every column to the header width; rows are
        // built here the same way so alignment is exact.
        let row = |name: &str, id: &str, version: &str, available: &str| {
            format!("{:<40}{:<32}{:<12}{}", name, id, version, available)
        };
        let text = format!(
            "{}\n{}\n{}\n{}\n{}",
            row("Name", "Id", "Version", "Available"),
            "-".repeat(96),
            row("7-Zip 24.09", "7zip.7zip", "24.09", "24.10"),
            row(
                "Eclipse Temurin 21.0.5",
                "EclipseAdoptium.Temurin.21.JDK",
                "21.0.5",
                ""
            ),
            row("Git 2.47.0", "Git.Git", "2.47.0", ""),
        );
        let map = parse_winget_list(&text);
        assert_eq!(map.len(), 3);
        let (version, available) = &map["7zip.7zip"];
        assert_eq!(version, "24.09");
        assert_eq!(available.as_deref(), Some("24.10"));
        let (version, available) = &map["eclipseadoptium.temurin.21.jdk"];
        assert_eq!(version, "21.0.5");
        assert_eq!(available, &None);
        let (version, available) = &map["git.git"];
        assert_eq!(version, "2.47.0");
        assert_eq!(available, &None);
    }

    #[test]
    fn ignores_separator_and_empty_lines() {
        let map = parse_winget_list("\n-----\n\n");
        assert!(map.is_empty());
    }

    #[test]
    fn detection_prefers_winget_when_known() {
        let product = Product {
            id: "openjdk21".into(),
            name: "Eclipse Temurin OpenJDK 21 (LTS)".into(),
            winget_id: Some("EclipseAdoptium.Temurin.21.JDK".into()),
            install_location_hint: None,
            install_dir: None,
            default_env: vec![],
        };
        let step = Step::Winget {
            id: "EclipseAdoptium.Temurin.21.JDK".into(),
            scope: "machine".into(),
        };
        let snapshot = HashMap::from([(
            "eclipseadoptium.temurin.21.jdk".to_string(),
            ("21.0.5".to_string(), Some("21.0.6".to_string())),
        )]);
        let detection = detection_for(&product, &step, &snapshot, &[]);
        assert!(detection.installed);
        assert!(detection.winget_managed);
        assert_eq!(detection.installed_version.as_deref(), Some("21.0.5"));
        assert_eq!(detection.available_version.as_deref(), Some("21.0.6"));
    }

    #[test]
    fn registry_catches_installs_winget_does_not_know() {
        let product = Product {
            id: "corp-tool".into(),
            name: "Corp Tool".into(),
            winget_id: None,
            install_location_hint: None,
            install_dir: None,
            default_env: vec![],
        };
        let step = Step::Command {
            exe: "corp-tool.exe".into(),
            args: vec![],
            success_codes: vec![0],
        };
        let snapshot = HashMap::new();
        let registry = vec!["Corp Tool 3.2".to_string()];
        let detection = detection_for(&product, &step, &snapshot, &registry);
        assert!(detection.installed);
        assert!(!detection.winget_managed);
    }

    #[test]
    fn command_steps_are_never_winget_managed() {
        let product = Product {
            id: "node-lts".into(),
            name: "Node.js LTS (via NVM)".into(),
            winget_id: None,
            install_location_hint: None,
            install_dir: None,
            default_env: vec![],
        };
        let step = Step::Command {
            exe: "nvm.cmd".into(),
            args: vec!["install".into(), "lts".into()],
            success_codes: vec![0],
        };
        let snapshot = HashMap::from([(
            "coreybutler.nvmforwindows".to_string(),
            ("1.2.3".to_string(), None),
        )]);
let detection = detection_for(&product, &step, &snapshot, &[]);
        assert!(!detection.installed);
        assert!(!detection.winget_managed);
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

    #[test]
    fn timebox_kills_a_runaway_process() {
        let run = run_timed_process(
            "powershell",
            &[
                "-NoProfile".to_string(),
                "-Command".to_string(),
                "Start-Sleep -Seconds 30".to_string(),
            ],
            Duration::from_secs(2),
        );
        assert!(run.timed_out);
        assert_eq!(run.exit_code, None);
        assert!(run.output.contains("TIMED OUT"), "{}", run.output);
    }

    #[test]
    fn completed_process_reports_exit_code_and_output() {
        let run = run_timed_process(
            "cmd",
            &["/c".to_string(), "echo".to_string(), "hello-sprout".to_string()],
            Duration::from_secs(30),
        );
        assert!(!run.timed_out);
        assert_eq!(run.exit_code, Some(0));
        assert!(run.output.contains("hello-sprout"), "{}", run.output);
    }

    #[test]
    fn missing_executable_is_a_clean_failure() {
        let run = run_timed_process(
            "no-such-binary-sprout-test",
            &[],
            Duration::from_secs(5),
        );
        assert!(!run.timed_out);
        assert_eq!(run.exit_code, None);
        assert!(run.output.contains("failed to start"));
    }

    fn cmd(args: &[&str]) -> VerifyCommand {
        VerifyCommand {
            command: "cmd".into(),
            args: args.iter().map(|s| s.to_string()).collect(),
            match_text: None,
        }
    }

    #[test]
    fn verify_passes_on_exit_zero() {
        let report = verify_with_timeout(&cmd(&["/c", "exit", "0"]), Duration::from_secs(30));
        assert!(report.ok, "{}", report.detail);
        assert_eq!(report.detail, "'cmd' exited 0");
    }

    #[test]
    fn verify_fails_loudly_on_nonzero_exit() {
        let report = verify_with_timeout(&cmd(&["/c", "exit", "3"]), Duration::from_secs(30));
        assert!(!report.ok);
        assert!(
            report.detail.contains("exited 3") && report.detail.contains("not behaving"),
            "{}",
            report.detail
        );
    }

    #[test]
    fn verify_checks_the_declared_match_text() {
        let mut check = cmd(&["/c", "echo", "java 21"]);
        check.match_text = Some("21".into());
        let report = verify_with_timeout(&check, Duration::from_secs(30));
        assert!(report.ok, "{}", report.detail);
        assert!(report.detail.contains("reported '21'"), "{}", report.detail);

        check.match_text = Some("22".into());
        let report = verify_with_timeout(&check, Duration::from_secs(30));
        assert!(!report.ok);
        assert!(report.detail.contains("did not contain '22'"), "{}", report.detail);
    }

    #[test]
    fn verify_fails_when_the_command_cannot_start() {
        let check = VerifyCommand {
            command: "no-such-binary-sprout-test".into(),
            args: vec![],
            match_text: None,
        };
        let report = verify_with_timeout(&check, Duration::from_secs(30));
        assert!(!report.ok);
        assert!(report.detail.contains("failed to start"), "{}", report.detail);
    }

    #[test]
    fn verify_times_out_like_a_hung_installer() {
        let check = VerifyCommand {
            command: "powershell".into(),
            args: vec![
                "-NoProfile".into(),
                "-Command".into(),
                "Start-Sleep -Seconds 30".into(),
            ],
            match_text: None,
        };
        let report = verify_with_timeout(&check, Duration::from_secs(2));
        assert!(!report.ok);
        assert!(report.detail.contains("did not finish in 2 s"), "{}", report.detail);
    }

    #[test]
    fn set_applies_only_when_both_scopes_are_unset() {
        let wiring = EnvWiring {
            action: EnvAction::Set,
            name: "JAVA_HOME".into(),
            value: "C:\\jdk".into(),
        };
        let (note, write) = decide_set(&wiring, "C:\\jdk", None, None);
        assert_eq!(write.as_deref(), Some("C:\\jdk"));
        assert!(note.contains("set JAVA_HOME = C:\\jdk (User)"), "{note}");

        // A User value already exists — never overwritten.
        let (note, write) = decide_set(&wiring, "C:\\jdk", Some("C:\\existing"), None);
        assert_eq!(write, None);
        assert!(note.contains("already set - leaving it as-is"), "{note}");

        // A Machine value alone is enough to hold off.
        let (note, write) = decide_set(&wiring, "C:\\jdk", None, Some("C:\\machine"));
        assert_eq!(write, None);
        assert!(note.contains("already set"), "{note}");

        // Both set — still untouched.
        let (note, write) = decide_set(&wiring, "C:\\jdk", Some(""), Some(""));
        assert_eq!(write, None);
        assert!(note.contains("already set"), "{note}");
    }

    #[test]
    fn prepend_applies_only_when_absent_from_both_scopes() {
        let wiring = EnvWiring {
            action: EnvAction::Prepend,
            name: "PATH".into(),
            value: "C:\\jdk\\bin".into(),
        };

        // Absent everywhere: prepended to the existing User list…
        let (note, write) = decide_prepend(&wiring, "C:\\jdk\\bin", "C:\\tools", "");
        assert_eq!(write.as_deref(), Some("C:\\jdk\\bin;C:\\tools"));
        assert!(note.contains("prepend PATH = C:\\jdk\\bin (User)"), "{note}");

        // …or the whole value when the User list is empty.
        let (_, write) = decide_prepend(&wiring, "C:\\jdk\\bin", "", "");
        assert_eq!(write.as_deref(), Some("C:\\jdk\\bin"));

        // Present in the User list (mid-list, case-insensitive): skipped.
        let (note, write) = decide_prepend(&wiring, "C:\\jdk\\bin", "A;C:\\JDK\\BIN;D", "");
        assert_eq!(write, None);
        assert!(note.contains("already contains C:\\jdk\\bin - skipped"), "{note}");

        // Present in the Machine list: skipped.
        let (note, write) = decide_prepend(&wiring, "C:\\jdk\\bin", "", "C:\\jdk\\bin;Z");
        assert_eq!(write, None);
        assert!(note.contains("already contains"), "{note}");

        // Machine scope is never rewritten — the note carries the value.
        let (note, _) = decide_prepend(&wiring, "C:\\jdk\\bin", "C:\\tools", "C:\\machine");
        assert!(!note.contains("C:\\machine"), "{note}");
    }

    #[test]
    fn literal_env_values_pass_through_unchanged() {
        let product = Product {
            id: "tool".into(),
            name: "Tool".into(),
            winget_id: None,
            install_location_hint: Some("Tool".into()),
            install_dir: None,
            default_env: vec![],
        };
        assert_eq!(
            WindowsWingetEngine::resolve_env_value("C:\\Tools\\bin", &product),
            Some("C:\\Tools\\bin".to_string())
        );
    }

    #[test]
    fn placeholder_parsing_uses_inline_then_product_hint() {
        let product = Product {
            id: "openjdk21".into(),
            name: "Eclipse Temurin OpenJDK 21 (LTS)".into(),
            winget_id: None,
            install_location_hint: Some("Eclipse Temurin".into()),
            install_dir: None,
            default_env: vec![],
        };
        let (hint, before, after) =
            split_placeholder("<InstallLocation:Eclipse Temurin>\\bin", &product).unwrap();
        assert_eq!(hint, "Eclipse Temurin");
        assert_eq!(before, "");
        assert_eq!(after, "\\bin");

        // The bare tag falls back to the Product's hint.
        let (hint, before, after) = split_placeholder("X<InstallLocation>Y", &product).unwrap();
        assert_eq!(hint, "Eclipse Temurin");
        assert_eq!(before, "X");
        assert_eq!(after, "Y");

        // No hint anywhere — unresolved.
        let product = Product {
            install_location_hint: None,
            ..product
        };
        assert!(split_placeholder("X<InstallLocation>Y", &product).is_none());
        assert!(split_placeholder("plain path", &product).is_none());
    }

    #[test]
    fn resolves_a_real_install_location_from_the_registry() {
        let location = WindowsWingetEngine::resolve_install_location("VMware Tools");
        assert!(location.is_some(), "VMware Tools should be resolvable on this machine");
        let location = location.unwrap();
        assert!(location.contains("VMware"), "{location}");
        assert!(!location.ends_with('\\'), "{location}");
    }

    #[test]
    fn registry_wiring_round_trips_with_cleanup() {
        // A unique name per process: this test writes and reads the real
        // HKCU\Environment (the only honest way to prove the write glue), and
        // a Drop guard removes the value and the process copy afterwards.
        let name = format!("SPROUT_TEST_ENV_{}", std::process::id());
        struct Cleanup(String);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                use windows_sys::Win32::System::Environment::SetEnvironmentVariableW;
                let wide: Vec<u16> = self.0.encode_utf16().chain(std::iter::once(0)).collect();
                unsafe {
                    SetEnvironmentVariableW(wide.as_ptr(), std::ptr::null());
                }
                let _ = RegKey::predef(HKEY_CURRENT_USER)
                    .open_subkey_with_flags(r"Environment", KEY_SET_VALUE)
                    .and_then(|key| key.delete_value(&self.0));
            }
        }
        let _guard = Cleanup(name.clone());
        let product = Product {
            id: "t".into(),
            name: "T".into(),
            winget_id: None,
            install_location_hint: None,
            install_dir: None,
            default_env: vec![],
        };

        // set: applied, visible in the User scope and the live process.
        let set = EnvWiring {
            action: EnvAction::Set,
            name: name.clone(),
            value: r"C:\sprout-test\jdk".into(),
        };
        let note = WindowsWingetEngine::apply_set(&product, &set);
        assert!(note.contains(&format!("set {name}")), "{note}");
        assert_eq!(
            WindowsWingetEngine::user_env(&name).as_deref(),
            Some(r"C:\sprout-test\jdk")
        );
        assert_eq!(
            std::env::var(&name).ok().as_deref(),
            Some(r"C:\sprout-test\jdk")
        );

        // set again: never overwrites.
        let note = WindowsWingetEngine::apply_set(&product, &set);
        assert!(note.contains("already set - leaving it as-is"), "{note}");
        assert_eq!(
            WindowsWingetEngine::user_env(&name).as_deref(),
            Some(r"C:\sprout-test\jdk")
        );

        // prepend: new value goes in front of the existing one.
        let prepend = EnvWiring {
            action: EnvAction::Prepend,
            name: name.clone(),
            value: r"C:\sprout-test\bin".into(),
        };
        let note = WindowsWingetEngine::apply_prepend(&product, &prepend);
        assert!(note.contains("prepend"), "{note}");
        assert_eq!(
            WindowsWingetEngine::user_env(&name).as_deref(),
            Some(r"C:\sprout-test\bin;C:\sprout-test\jdk")
        );

        // prepend again: already present — skipped, list unchanged.
        let note = WindowsWingetEngine::apply_prepend(&product, &prepend);
        assert!(note.contains("already contains"), "{note}");
        assert_eq!(
            WindowsWingetEngine::user_env(&name).as_deref(),
            Some(r"C:\sprout-test\bin;C:\sprout-test\jdk")
        );
    }

    #[test]
    fn unresolvable_placeholder_skips_with_a_note() {
        let product = Product {
            id: "tool".into(),
            name: "Tool".into(),
            winget_id: None,
            install_location_hint: None,
            install_dir: None,
            default_env: vec![],
        };
        // A unique name: nothing else in the suite (or on this machine) can
        // have set it, so the "nothing was written" assertion is reliable.
        let name = format!("SPROUT_TEST_UNRESOLVED_{}", std::process::id());
        let wiring = EnvWiring {
            action: EnvAction::Set,
            name: name.clone(),
            value: "<InstallLocation:sprout-test-does-not-exist-xyz>".into(),
        };
        let note = WindowsWingetEngine::apply_set(&product, &wiring);
        assert!(note.contains("cannot resolve install location"), "{note}");
        assert!(note.contains("skipped"), "{note}");
        // Nothing was written anywhere.
        assert_eq!(WindowsWingetEngine::user_env(&name), None);
    }

    fn command_step_req(exe: &str, args: &[&str], success_codes: &[i32]) -> Step {
        Step::Command {
            exe: exe.into(),
            args: args.iter().map(|s| s.to_string()).collect(),
            success_codes: success_codes.to_vec(),
        }
    }

    #[test]
    fn command_step_succeeds_on_a_declared_exit_code() {
        let step = command_step_req("cmd", &["/c", "exit", "0"], &[0]);
        let outcome = command_step("install", &step, 10);
        assert!(outcome.ok, "{}", outcome.detail);
        assert_eq!(outcome.detail, "installed");
        assert!(!outcome.reboot_required);
        assert!(!outcome.timed_out);

        // Any declared code counts — the winget reboot family is declared
        // where the step's author wants it.
        let step = command_step_req("cmd", &["/c", "exit", "3010"], &[0, 3010]);
        let outcome = command_step("install", &step, 10);
        assert!(outcome.ok, "{}", outcome.detail);

        let step = command_step_req("cmd", &["/c", "exit", "0"], &[0]);
        let outcome = command_step("upgrade", &step, 10);
        assert!(outcome.ok, "{}", outcome.detail);
        assert_eq!(outcome.detail, "upgraded");
    }

    #[test]
    fn command_step_fails_loudly_on_an_undeclared_exit_code() {
        let step = command_step_req("cmd", &["/c", "exit", "7"], &[0]);
        let outcome = command_step("install", &step, 10);
        assert!(!outcome.ok);
        assert!(outcome.detail.contains("exited 7"), "{}", outcome.detail);
        assert!(outcome.detail.contains("declared success codes [0]"), "{}", outcome.detail);
    }

    #[test]
    fn command_step_defaults_to_exit_zero_only() {
        let step = command_step_req("cmd", &["/c", "exit", "0"], &[]);
        let outcome = command_step("install", &step, 10);
        assert!(outcome.ok, "{}", outcome.detail);

        let step = command_step_req("cmd", &["/c", "exit", "2"], &[]);
        let outcome = command_step("install", &step, 10);
        assert!(!outcome.ok);
        assert!(outcome.detail.contains("exited 2"), "{}", outcome.detail);
    }

    #[test]
    fn command_step_runs_batch_files() {
        // The node-lts flow is `nvm.cmd install lts` — a batch file. std
        // resolves .cmd through cmd.exe, so a batch that exits nonzero must
        // be reported as the step failing, not as a start error.
        let dir = tempfile::tempdir().unwrap();
        let batch = dir.path().join("sprout-step.cmd");
        std::fs::write(&batch, "@echo off\r\nexit /b 5\r\n").unwrap();
        let step = command_step_req(batch.to_str().unwrap(), &[], &[0]);
        let outcome = command_step("install", &step, 10);
        assert!(!outcome.ok);
        assert!(outcome.detail.contains("exited 5"), "{}", outcome.detail);
    }

    #[test]
    fn command_step_times_out_like_a_hung_installer() {
        let step = command_step_req(
            "powershell",
            &["-NoProfile", "-Command", "Start-Sleep -Seconds 30"],
            &[0],
        );
        let outcome = command_step_with_timeout("install", &step, Duration::from_secs(2));
        assert!(!outcome.ok);
        assert!(outcome.timed_out);
        assert!(outcome.detail.contains("did not finish in 2 s"), "{}", outcome.detail);
    }

    #[test]
    fn command_step_missing_executable_is_a_clean_failure() {
        let step = command_step_req("no-such-binary-sprout-test", &[], &[0]);
        let outcome = command_step("install", &step, 10);
        assert!(!outcome.ok);
        assert!(outcome.detail.contains("failed to start"), "{}", outcome.detail);
    }

    #[test]
    fn command_steps_keep_the_raw_output_in_the_log() {
        let step = command_step_req("cmd", &["/c", "echo", "sprout-command-log"], &[0]);
        let outcome = command_step("install", &step, 10);
        assert!(outcome.ok, "{}", outcome.detail);
        assert!(outcome.log.contains("sprout-command-log"), "{}", outcome.log);
    }

    #[test]
    fn requires_winget_is_false_for_command_only_runs() {
        let cmd_req = Requirement {
            product: Product {
                id: "node-lts".into(),
                name: "Node.js LTS (via NVM)".into(),
                winget_id: None,
                install_location_hint: None,
                install_dir: None,
                default_env: vec![],
            },
            step: command_step_req("nvm.cmd", &["install", "lts"], &[0]),
            version_policy: crate::domain::VersionPolicy::Latest,
            depends_on: vec![],
            timeout_minutes: 10,
            env: vec![],
            verify: vec![],
            unresolved: false,
        };
        assert!(!WindowsWingetEngine::requires_winget(&[&cmd_req]));

        let winget_req = Requirement {
            step: Step::Winget {
                id: "Git.Git".into(),
                scope: "machine".into(),
            },
            ..cmd_req.clone()
        };
        assert!(WindowsWingetEngine::requires_winget(&[&winget_req]));
        assert!(WindowsWingetEngine::requires_winget(&[&cmd_req, &winget_req]));
    }

    #[test]
    fn prepare_is_ok_when_winget_is_present_or_unneeded() {
        let engine = WindowsWingetEngine;
        let cmd_req = Requirement {
            product: Product {
                id: "node-lts".into(),
                name: "Node.js LTS (via NVM)".into(),
                winget_id: None,
                install_location_hint: None,
                install_dir: None,
                default_env: vec![],
            },
            step: command_step_req("nvm.cmd", &["install", "lts"], &[0]),
            version_policy: crate::domain::VersionPolicy::Latest,
            depends_on: vec![],
            timeout_minutes: 10,
            env: vec![],
            verify: vec![],
            unresolved: false,
        };
        // This machine has winget; regardless, command-only runs never need
        // the bootstrap to be attempted.
        assert!(engine.prepare(&[&cmd_req]).is_ok());
    }

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

    #[test]
    fn reads_a_real_windows_build_number() {
        let build = windows_build_number().expect("this machine has a build number");
        // Anything below 19041 predates winget support — this box must be newer.
        assert!(build >= MIN_WINGET_BUILD, "unexpected build {build}");
    }

    #[test]
    fn expand_env_resolves_system_variables() {
        let expanded = expand_env("%SystemRoot%\\System32");
        assert!(!expanded.contains('%'), "{expanded}");
        assert!(expanded.to_lowercase().contains("windows"), "{expanded}");
        // No variables: unchanged.
        assert_eq!(expand_env(r"C:\Tools\bin"), r"C:\Tools\bin");
    }

    #[test]
    fn join_path_lists_merges_machine_then_user() {
        assert_eq!(join_path_lists("A;B", "C"), "A;B;C");
        assert_eq!(join_path_lists("", "C"), "C");
        assert_eq!(join_path_lists("A", ""), "A");
        assert_eq!(join_path_lists("", ""), "");
    }

    // ------------------- the virtual-desktop surface (ticket 44) ----------

    #[test]
    fn desktop_move_retries_before_giving_up() {
        // A success on a later attempt wins — the shell's view-registration
        // race makes the first try fail.
        let mut attempts = 0;
        let result = move_with_retries(4, Duration::ZERO, || {
            attempts += 1;
            if attempts < 4 {
                Err(format!("attempt {attempts} refused"))
            } else {
                Ok(())
            }
        });
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(attempts, 4);

        // Every attempt refused: the last error is what surfaces.
        let mut attempts = 0;
        let result = move_with_retries(3, Duration::ZERO, || {
            attempts += 1;
            Err(format!("always refused {attempts}"))
        });
        assert!(result.is_err());
        assert_eq!(attempts, 3);
        let error = result.unwrap_err();
        assert!(error.contains("always refused 3"), "last error: {error}");
    }

    #[test]
    fn lnk_bytes_yield_the_exe_path_when_ishelllink_refuses() {
        // The real File Explorer shortcut on this machine: IShellLink
        // GetPath returns an empty string, but the raw bytes carry
        // `%windir%\explorer.exe` as UTF-16LE.
        let dir = tempfile::tempdir().unwrap();
        let lnk = dir.path().join("File Explorer.lnk");
        let mut bytes = vec![0x00u8, 0x11, 0x22, 0x33];
        for unit in r"%windir%\explorer.exe".encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        bytes.extend_from_slice(&[0x44, 0x55]);
        std::fs::write(&lnk, &bytes).unwrap();
        assert_eq!(
            lnk_target_from_bytes(&lnk).as_deref(),
            Some(r"%windir%\explorer.exe")
        );
        // The env expansion makes the image match comparable: the extracted
        // path resolves to the real explorer.exe image path.
        let expanded = expand_env(r"%windir%\explorer.exe");
        assert!(
            expanded.to_lowercase().ends_with(r"\windows\explorer.exe"),
            "expanded: {expanded}"
        );

        // A shortcut with no exe string has no target — a wrong pick must be
        // "no match", never a panic.
        let empty = dir.path().join("Empty.lnk");
        std::fs::write(&empty, b"\x00\x01\x02\x03").unwrap();
        assert_eq!(lnk_target_from_bytes(&empty), None);

        // A shorter non-exe string never wins over the exe path (real
        // StringData entries are NUL-terminated, so each sits in its own
        // run).
        let mixed = dir.path().join("Mixed.lnk");
        let mut bytes = Vec::new();
        let mut push = |text: &str| {
            for unit in text.encode_utf16() {
                bytes.extend_from_slice(&unit.to_le_bytes());
            }
            bytes.extend_from_slice(&[0x00, 0x00]);
        };
        push("not-an-exe");
        push(r"C:\Apps\real.exe");
        std::fs::write(&mixed, &bytes).unwrap();
        assert_eq!(
            lnk_target_from_bytes(&mixed).as_deref(),
            Some(r"C:\Apps\real.exe")
        );
    }

    #[test]
    fn virtual_desktop_gate_opens_on_windows_11_24h2() {
        // Windows 10 2004-22H2 and Windows 11 21H2-23H2: closed.
        assert!(!virtual_desktops_supported(19041));
        assert!(!virtual_desktops_supported(19045));
        assert!(!virtual_desktops_supported(22000));
        assert!(!virtual_desktops_supported(22631));
        // Windows 11 24H2 and later: open.
        assert!(virtual_desktops_supported(26100));
        assert!(virtual_desktops_supported(26200));
    }

    #[test]
    fn desktop_id_roundtrips_through_the_guid_shape() {
        // The id shape the database stores: lowercase, unbraced.
        let id = "550fe0a1-3d41-4e5f-9a2b-c8d0e1f2a3b4";
        let guid = parse_guid_id(id).expect("a valid id parses");
        assert_eq!(guid_to_id(&guid), id, "the canonical form is lowercase");
        // Uppercase hex parses and normalizes to lowercase.
        let upper = "550FE0A1-3D41-4E5F-9A2B-C8D0E1F2A3B4";
        assert_eq!(guid_to_id(&parse_guid_id(upper).unwrap()), id);
        // Malformed ids are a clean error, never a panic.
        assert!(parse_guid_id("not-a-guid").is_none());
        assert!(parse_guid_id("550fe0a1-3d41-4e5f-9a2b").is_none());
        assert!(parse_guid_id("550fe0a1-3d41-4e5f-9a2b-c8d0e1f2a3bzz").is_none());
    }
}
