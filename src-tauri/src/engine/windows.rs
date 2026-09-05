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

mod inspection;

use std::collections::HashMap;
use std::time::Duration;

use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_SET_VALUE, REG_EXPAND_SZ, REG_SZ};
use winreg::RegKey;

use crate::domain::{EnvAction, EnvWiring, Product, Requirement, Step, VerifyCommand};
use crate::engine::{Detection, PlatformEngine, StepOutcome, VerifyOutcome};
use crate::winget;
use crate::windows_execution::{run_timed_process, spawn_user_command};
use inspection::{app_windows, target_exists, wait_for_new_window};

/// How long a verify command may run before it is killed like a hung
/// installer: verifies must be quick (e.g. `java -version`), and a hung one
/// must never wedge the whole Run.
const VERIFY_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// The minimum Windows build the virtual-desktop surface runs on — Windows
/// 11 24H2 (26100), where the IVirtualDesktopManager service the winvd crate
/// drives actually exists. Below this gate `desktops()` is empty,
/// `create_desktop()` is `None`, and every move is an error: the whole
/// assignment surface hides itself (ticket 44).
const MIN_VIRTUAL_DESKTOP_BUILD: u32 = 26100;

/// Whether a launch target is a Store/MSIX AUMID launch key (ticket 122):
/// `shell:AppsFolder\<AUMID>` (case-insensitive, trimmed).
fn is_uwp_target(target: &str) -> bool {
    target.trim().to_ascii_lowercase().starts_with("shell:appsfolder\\")
}

/// The AUMID inside a `shell:AppsFolder\` target, trimmed. `None` for Win32
/// targets.
fn uwp_aumid(target: &str) -> Option<String> {
    if !is_uwp_target(target) {
        return None;
    }
    let trimmed = target.trim();
    let prefix_len = "shell:AppsFolder\\".len();
    Some(trimmed[prefix_len..].trim().to_string())
}

/// A command step's exit code when its author declared none: 0 only.
const DEFAULT_SUCCESS_CODE: i32 = 0;




/// The placeholder prefix for the installed product's location. The full
/// forms are `<InstallLocation>` (bare) and `<InstallLocation:hint>` (inline
/// hint) — the closing `>` follows the hint in the inline form, so matching
/// on the prefix (not the whole `<InstallLocation>`) is what catches both.
const INSTALL_LOCATION_PREFIX: &str = "<InstallLocation";

/// Registry hives the uninstall scan reads, in the legacy runner's order.
/// The `HKEY` type is windows-sys's; winreg re-exports the predef constants.
type HKEY = windows_sys::Win32::System::Registry::HKEY;

pub struct WindowsWingetEngine;

impl WindowsWingetEngine {


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


    /// Every DisplayName found under the three uninstall-registry hives the
    /// legacy runner scanned (HKLM, WOW6432Node, HKCU).
    fn registry_display_names() -> Vec<String> {
        crate::walker::uninstall_subkeys()
            .iter()
            .filter_map(|sub| sub.get_value::<String, _>("DisplayName").ok())
            .collect()
    }

    /// The installed location of a product whose uninstall key's DisplayName
    /// contains `hint` — the legacy `Resolve-InstallLocation`, same three
    /// hives and same rules (first key that matches and carries a non-blank
    /// InstallLocation; trailing backslash trimmed).
    fn resolve_install_location(hint: &str) -> Option<String> {
        for sub in crate::walker::uninstall_subkeys() {
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




/// The Windows build number (19045 for 10 22H2, 26100 for 11 24H2), read
/// from the `Windows NT\CurrentVersion` registry key — the same source the
/// legacy `Get-CimInstance Win32_OperatingSystem` reported.
pub(crate) fn windows_build_number() -> Option<u32> {
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

    fn foreground_window(&self, hwnd: usize) -> bool {
        foreground_window(hwnd)
    }
}

/// Foregrounds a window (ticket 121): restores it if minimized and brings
/// it to the foreground at normal Z (no HWND_TOPMOST) so a Fixed dock
/// (AppBar ABM_SETPOS work-area squeeze) stays as-is — overlapping single
/// foreground appears above it per Q10. No ShellExecute on hit.
fn foreground_window(hwnd: usize) -> bool {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE,
    };
    let hwnd = hwnd as HWND;
    unsafe {
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        SetForegroundWindow(hwnd) != 0
    }
}

/// Launches an app entry as-is through the shell: ShellExecuteExW on the .lnk
/// or exe path, so shortcut semantics, association handling, and the shell's
/// own environment all come along. Returns the new process's id — or none
/// when the shell hands the launch to an already-running process (ticket 47):
/// Explorer and other single-instance shells report success with no process
/// handle, and that is a *started* launch, never a failure. The window
/// resolution's image fallback finds the window in that case. Ticket 122:
/// `shell:AppsFolder\<AUMID>` (Store/MSIX) goes through
/// `IApplicationActivationManager::ActivateApplication` instead.
fn spawn_app(target: &str) -> Result<crate::engine::Spawned, String> {
    if let Some(aumid) = uwp_aumid(target) {
        return activate_uwp(&aumid);
    }
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

/// Activates a Store/MSIX app via `IApplicationActivationManager::ActivateApplication`
/// (ticket 122). `AO_NONE` is the normal activation — no flags. On success the
/// pid is the new app's; on failure the error bubbles as a failed Launch entry
/// (never a silent stall). The call needs COM MTA, like `ShellLinkLnkResolver`.
fn activate_uwp(aumid: &str) -> Result<crate::engine::Spawned, String> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Com::{CoCreateInstance, CoIncrementMTAUsage, CLSCTX_INPROC_SERVER};
    use windows::Win32::UI::Shell::{ApplicationActivationManager, IApplicationActivationManager, AO_NONE};

    let _mta = unsafe { CoIncrementMTAUsage() }
        .map_err(|e| format!("Windows could not activate '{aumid}' (COM init failed: {e:?})"))?;
    let manager: IApplicationActivationManager = unsafe {
        CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_INPROC_SERVER)
    }
    .map_err(|e| format!("Windows could not activate '{aumid}' ({e:?})"))?;
    let wide: Vec<u16> = aumid.encode_utf16().chain(std::iter::once(0)).collect();
    let pid = unsafe { manager.ActivateApplication(PCWSTR(wide.as_ptr()), PCWSTR::null(), AO_NONE) }
        .map_err(|e| format!("Windows could not activate '{aumid}' ({e:?})"))?;
    Ok(crate::engine::Spawned {
        pid: Some(pid),
        target: format!("shell:AppsFolder\\{aumid}"),
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
    let child = spawn_user_command(&exe, &args, entry.show_window)?;
    Ok(crate::engine::Spawned {
        pid: Some(child.id()),
        target: exe,
    })
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
    let current = winvd::get_current_desktop()
        .ok()
        .and_then(|desktop| desktop.get_id().ok())
        .map(|id| guid_to_id(&id));
    desktops
        .iter()
        .map(|desktop| {
            let id = desktop.get_id().map(|id| guid_to_id(&id)).unwrap_or_default();
            crate::engine::DesktopInfo {
                current: current.as_deref() == Some(id.as_str()),
                id,
                name: match desktop.get_name() {
                    Ok(name) if !name.trim().is_empty() => name,
                    _ => format!("Desktop {}", desktop.get_index().unwrap_or(0) + 1),
                },
            }
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
/// crate's `GUID::from` panics on a malformed string, so the shared loose
/// shape check (`launch::looks_like_guid` — the same predicate
/// `validate_launch_entry` gates ids with) runs first: ids come from the
/// database, which validates on write, but a stale hand-edited value must be
/// an error, never a panic.
fn parse_guid_id(id: &str) -> Option<windows::core::GUID> {
    if !crate::launch::looks_like_guid(id) {
        return None;
    }
    Some(windows::core::GUID::from(id))
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


impl PlatformEngine for WindowsWingetEngine {
    fn prepare(&self, requirements: &[&Requirement]) -> Result<(), String> {
        winget::prepare(requirements)
    }

    fn detect(&self, product: &Product, step: &Step) -> Detection {
        let snapshot = winget::snapshot().unwrap_or_default();
        let registry = Self::registry_display_names();
        detection_for(product, step, &snapshot, &registry)
    }

    fn detect_many(&self, requirements: &[&Requirement]) -> HashMap<String, Detection> {
        let snapshot = winget::snapshot().unwrap_or_default();
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
            Step::Winget { id, .. } => winget::install(id, timeout_minutes, install_dir),
            Step::Command { .. } => command_step("install", step, timeout_minutes),
        }
    }

    fn upgrade(&self, step: &Step, timeout_minutes: u32, install_dir: Option<&str>) -> StepOutcome {
        match step {
            Step::Winget { id, .. } => winget::upgrade(id, timeout_minutes, install_dir),
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
    fn reads_a_real_windows_build_number() {
        let build = windows_build_number().expect("this machine has a build number");
        // Anything below 19041 predates winget support — this box must be newer.
        assert!(build >= 19041, "unexpected build {build}");
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
