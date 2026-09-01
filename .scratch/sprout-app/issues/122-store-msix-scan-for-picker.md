# 122 — Store/MSIX scan for app picker (additive, launch via AUMID)

**What to build:** The Quick Launch app picker surfaces Microsoft Store / MSIX apps alongside Win32 `.lnk`/`.exe` without changing existing entry validation.

**Blocked by:** none.

**Status:** ready-for-agent

## Scope

- New `src-tauri/src/store.rs` (or `launch/store.rs`) `enumerate_uwp()`: `Windows.Management.Deployment.PackageManager::FindPackages()` → `IPackage::GetAppListEntries()` / `AppUserModelId` + display name + logo, filter `IsFramework`/`IsResourcePackage` etc., no elevation needed (per-user + provisioned packages).
- Merge into `launch::list_launch_candidates` / file-picker add path and installed-app search (`39-installed-app-search.md` seam) before sort/dedup; dedup on `AUMID` exact, then on display-name+publisher if needed. Icon extraction via existing `icons.rs` fallback for UWP (package logo or generic `rocket`).
- Storage: `launch_entries.target = "shell:AppsFolder\\<AUMID>"` `kind=app` subtype `uwp` (no schema change — `launch_entries` `target TEXT NOT NULL` `db.rs:108` already holds it; `LaunchEntryKind::App` reused, `LaunchShell` null). No `validate_launch_entry` (`launch.rs:120`) length/payload change; `colliding_entry` (`launch.rs:170`) payload identity gains AUMID branch (trimmed, case-insensitive).
- Launch: branching seam in `launch::run_launch_queue_until` (`launch.rs:468`): `if target starts_with "shell:AppsFolder\\"` → `IApplicationActivationManager::ActivateApplication(AUMID, null, AO_NONE, &pid)` instead of `ShellExecuteExW`; desktop assignment and skip still apply via post-activation `EnumWindows` snapshot on target desktop.

## ACs

- [x] Picker search shows `Calculator`, `Store`-installed Spotify, Win32 apps together; framework packages absent.
- [x] Picking a Store Calculator entry creates `kind=app` row with `target=shell:AppsFolder\Microsoft.WindowsCalculator_8wekyb3d8bbwe!App`; launching activates Calculator (no custom command).
- [x] Existing Win32 flow byte-for-byte unchanged: picker ordering, `validate_launch_entry` messages, ordered list `ordered_list.rs` position discipline, backup `import_export.rs` payload identity all stay green.
- [x] Launch of UWP on assigned dead desktop still frees at spawn per ticket 99, reporting dead-desktop note.

## Verification

- `cargo test` store enumeration filtered-count + picker merge dedup + `FakeLauncher::activate_uwp` mocked launch branch.
- Manual: search `calc` → Store Calculator appears; pick and launch; search `spotify` with Store variant installed → appears.

## Update 2026-08-31 — Paint/Photos fix

- `store.rs:151` `resolve_ms_resource` via `SHLoadIndirectString` + `fallback_display_name` ensures `ms-resource:Resources/AppDisplayName` (Paint) / `ms-resource:AppName` (Calculator) resolve to `Paint`/`Calculator`/`Photos` instead of raw `ms-resource:...`; verified `enumerate_uwp` returns 202 apps with `Paint`/`Photos`/`Calculator` correctly named and `walker::snapshot()` merges them. Picker search `paint`/`photo` now finds them; `shell:AppsFolder` launch via `ActivateApplication` still applies. Framework packages remain absent via `IsFramework`/`IsResourcePackage` filter.

