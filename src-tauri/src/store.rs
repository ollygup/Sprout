//! Store/MSIX scan for the app picker (ticket 122): the additive source behind
//! the Quick Launch installed-app search that surfaces Microsoft Store / MSIX
//! apps alongside Win32 `.lnk`/`.exe` without changing existing entry
//! validation.
//!
//! Filtering is the same on every path: `IsFramework`, `IsResourcePackage`,
//! `IsBundle` and `IsDevelopmentMode` are all excluded, no elevation is
//! needed (per-user + provisioned packages), and dedup rides on AUMID exact.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::walker::Candidate;

/// One Store/MSIX app as the picker sees it — the AUMID is the launch key
/// (`shell:AppsFolder\<AUMID>`), the display name is the tile label, the
/// publisher is the package publisher, and the logo is the best package logo
/// path when one is determinable (used by `icons::candidate_icon` as a
/// fallback for UWP).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StoreApp {
    pub aumid: String,
    pub display_name: String,
    pub publisher: Option<String>,
    /// Absolute path to the package logo file when determinable (e.g.
    /// `C:\Program Files\WindowsApps\...\Assets\...png`). `None` means the
    /// generic `rocket` glyph will be shown.
    pub logo: Option<String>,
}

/// The seam for the enumeration so tests can script Store results without
/// touching the real PackageManager.
pub trait UwpEnumerator: Send + Sync {
    fn enumerate(&self) -> Vec<StoreApp>;
}

/// The real enumerator — delegates to the OS.
pub struct PackageManagerEnumerator;

impl UwpEnumerator for PackageManagerEnumerator {
    fn enumerate(&self) -> Vec<StoreApp> {
        enumerate_uwp()
    }
}

/// The live enumeration: `PackageManager::FindPackages()` → `GetAppListEntries()`
/// per package, filtered as above. Any failure (missing API, not on Windows,
/// COM not initialized) is an empty list — the Win32 sources still surface.
/// Tries the fast WinRT path first (PackageManager, in-process), then falls
/// back to PowerShell `Get-AppxPackage` (slower, ~5 s) so the feature works
/// even when the WinRT feature is not compiled or the OS is older.
pub fn enumerate_uwp() -> Vec<StoreApp> {
    if let Some(apps) = try_enumerate_via_package_manager() {
        return apps;
    }
    enumerate_via_powershell().unwrap_or_default()
}

fn try_enumerate_via_package_manager() -> Option<Vec<StoreApp>> {
    use windows::Management::Deployment::PackageManager;
    let manager = PackageManager::new().ok()?;
    let packages = manager.FindPackages().ok()?;
    let mut out = Vec::new();
    for package in packages {
        if package.IsFramework().unwrap_or(false) {
            continue;
        }
        if package.IsResourcePackage().unwrap_or(false) {
            continue;
        }
        if package.IsBundle().unwrap_or(false) {
            continue;
        }
        if package.IsDevelopmentMode().unwrap_or(false) {
            continue;
        }
        let entries = package.GetAppListEntries().ok()?;
        let publisher = package
            .PublisherDisplayName()
            .ok()
            .map(|s| s.to_string())
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                // Fallback: Publisher DN via Id.Publisher, e.g. CN=Microsoft Corporation...
                package
                    .Id()
                    .ok()
                    .and_then(|id| id.Publisher().ok())
                    .map(|s| s.to_string())
                    .and_then(|dn| {
                        for part in dn.split(',') {
                            let p = part.trim();
                            if let Some(v) = p.strip_prefix("O=") {
                                return Some(v.trim().to_string());
                            }
                        }
                        for part in dn.split(',') {
                            let p = part.trim();
                            if let Some(v) = p.strip_prefix("CN=") {
                                return Some(v.trim().to_string());
                            }
                        }
                        None
                    })
            });
        let installed_path = package
            .InstalledLocation()
            .ok()
            .and_then(|f| f.Path().ok())
            .map(|s| s.to_string());
        let package_name = package
            .Id()
            .ok()
            .and_then(|id| id.Name().ok())
            .map(|s| s.to_string())
            .unwrap_or_default();
        for app in entries {
            let aumid = app.AppUserModelId().ok()?.to_string();
            if aumid.trim().is_empty() {
                continue;
            }
            let raw_display = app
                .DisplayInfo()
                .and_then(|info| info.DisplayName())
                .map(|s| s.to_string())
                .unwrap_or_else(|_| aumid.clone());
            let raw_display = raw_display.trim();
            if raw_display.is_empty() {
                continue;
            }
            let display_name = if raw_display.starts_with("ms-resource:") {
                resolve_ms_resource(raw_display, installed_path.as_deref(), &package_name)
                    .unwrap_or_else(|| fallback_display_name(&raw_display, &package_name, &aumid))
            } else {
                raw_display.to_string()
            };
            if display_name.trim().is_empty() {
                continue;
            }
            // 44×44 AppList logo — the row asset (Square44x44Logo) rendered as 32×32
            // per current design (icons.rs:48). No new token, same cache shape.
            let logo = installed_path
                .as_deref()
                .and_then(|p| find_44_logo(p, &raw_display));
            out.push(StoreApp {
                aumid: aumid.trim().to_string(),
                display_name,
                publisher: publisher.clone(),
                logo,
            });
        }
    }
    Some(out)
}

/// Finds the 44×44 AppList logo for a package (ticket 122, 44×44 decision).
/// Reads `AppxManifest.xml` at `installed_path` for `Square44x44Logo="Assets\..."`
/// (fallback `Square150x150Logo` / `Logo`), joins with `installed_path` and
/// returns the absolute path if the file exists. Handles `scale-*` / `targetsize-*`
/// variants and `Assets\Retail` subfolders, then falls back to a recursive
/// Assets search. Same 44 source is rendered as 32 `data:image/png` and displayed
/// as 24 (picker) / 16 (rack/dock) via CSS — identical display size to Win32
/// `SHGetFileInfoW` 32 icons, no new token.
fn find_44_logo(installed_path: &str, _raw_display: &str) -> Option<String> {
    use std::path::{Path, PathBuf};
    let manifest_path = Path::new(installed_path).join("AppxManifest.xml");
    let content = std::fs::read_to_string(&manifest_path).ok()?;
    for attr in ["Square44x44Logo", "Square150x150Logo", "Logo"] {
        let needle = format!(r#"{}=""#, attr);
        if let Some(start) = content.find(&needle) {
            let after = &content[start + needle.len()..];
            if let Some(end) = after.find('"') {
                let rel = after[..end].trim();
                if !rel.is_empty() {
                    // Exact join first
                    for cand in [
                        Path::new(installed_path).join(rel.replace('/', "\\")),
                        Path::new(installed_path).join(rel),
                    ] {
                        if cand.is_file() {
                            return Some(cand.to_string_lossy().into_owned());
                        }
                        // Scaled variant: e.g. Assets\PaintAppList.png -> Assets\PaintAppList.scale-200.png
                        // and targetsize variants: AppList.targetsize-44.png
                        if let Some(scaled) = find_scaled_variant(&cand) {
                            return Some(scaled);
                        }
                        // Retail subfolder case: manifest says Assets\Retail\PhotosAppList.png but file may be scaled
                        if let Some(parent) = cand.parent() {
                            if let Some(stem) = cand.file_stem().and_then(|s| s.to_str()) {
                                if let Ok(entries) = std::fs::read_dir(parent) {
                                    for e in entries.flatten() {
                                        let p = e.path();
                                        let n = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                                        if n.to_lowercase().starts_with(&stem.to_lowercase())
                                            && p.extension().map(|e| e.to_string_lossy().to_lowercase() == "png").unwrap_or(false)
                                            && p.is_file()
                                        {
                                            return Some(p.to_string_lossy().into_owned());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    // Last resort: recursive Assets search for 44×44 candidates — covers Retail subfolders
    let assets = Path::new(installed_path).join("Assets");
    if let Some(found) = find_logo_recursive(&assets) {
        return Some(found);
    }
    None
}

fn find_scaled_variant(path: &PathBuf) -> Option<String> {
    let parent = path.parent()?;
    let stem = path.file_stem()?.to_str()?;
    let ext = path.extension()?.to_str()?;
    if let Ok(entries) = std::fs::read_dir(parent) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|e| e.to_str()) != Some(ext) {
                continue;
            }
            let name = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if name.to_lowercase().starts_with(&stem.to_lowercase()) && p.is_file() {
                return Some(p.to_string_lossy().into_owned());
            }
        }
    }
    None
}

fn find_logo_recursive(dir: &Path) -> Option<String> {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(cur) = stack.pop() {
        let entries = std::fs::read_dir(&cur).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let lower = name.to_lowercase();
            if (lower.contains("applist") || lower.contains("44") || lower == "storelogo.png")
                && lower.ends_with(".png")
                && path.is_file()
            {
                return Some(path.to_string_lossy().into_owned());
            }
        }
    }
    None
}

fn resolve_ms_resource(
    ms_resource: &str,
    installed_path: Option<&str>,
    package_name: &str,
) -> Option<String> {
    // Try Win32 SHLoadIndirectString first — it resolves ms-resource: via the PRI.
    // Format per docs: @{<pri_path>? ms-resource://<package>/<resource>}
    // Example: ms-resource:Resources/AppDisplayName with package Microsoft.Paint
    // and pri C:\Program Files\WindowsApps\Microsoft.Paint_...\resources.pri
    // -> ms-resource://Microsoft.Paint/Resources/AppDisplayName
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::SHLoadIndirectString;
    let resource = ms_resource.trim();
    let bare = resource
        .strip_prefix("ms-resource:")
        .unwrap_or(resource)
        .trim_start_matches('/');
    // Build candidate indirect strings to try
    let mut candidates = Vec::new();
    if let Some(path) = installed_path {
        let pri = format!("{}\\resources.pri", path);
        // With package name
        if !package_name.is_empty() {
            candidates.push(format!("@{{{}}}? ms-resource://{}/{}", pri, package_name, bare));
            candidates.push(format!("@{{{}}}? ms-resource:{}", pri, bare));
        } else {
            candidates.push(format!("@{{{}}}? {}", pri, resource));
        }
    }
    // Also try the raw ms-resource string directly (some OS builds resolve without PRI)
    candidates.push(resource.to_string());
    candidates.push(format!("ms-resource://{}/{}", package_name, bare));

    for cand in candidates {
        let wide: Vec<u16> = cand.encode_utf16().chain(std::iter::once(0)).collect();
        let mut buf = vec![0u16; 512];
        let hr = unsafe { SHLoadIndirectString(PCWSTR(wide.as_ptr()), &mut buf, None) };
        if hr.is_ok() {
            let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            let resolved = String::from_utf16_lossy(&buf[..end]).trim().to_string();
            if !resolved.is_empty() && !resolved.starts_with("ms-resource:") {
                return Some(resolved);
            }
        }
    }
    None
}

fn fallback_display_name(raw: &str, package_name: &str, aumid: &str) -> String {
    // raw is ms-resource:..., package_name is like Microsoft.Paint, aumid is like Microsoft.Paint_...!App
    // Prefer package DisplayName fallback, else prettify Id.Name
    if !package_name.is_empty() {
        // Turn Microsoft.Paint -> Paint, Microsoft.Windows.Photos -> Photos,
        // Microsoft.WindowsCalculator -> Calculator (Windows prefix without dot)
        let stripped = package_name
            .strip_prefix("Microsoft.")
            .unwrap_or(package_name);
        let mut name = stripped.strip_prefix("Windows.").unwrap_or(stripped);
        if name.starts_with("Windows") {
            name = &name["Windows".len()..];
        }
        if let Some(last) = name.rsplit('.').next() {
            name = last;
        }
        if !name.is_empty() {
            return name.to_string();
        }
    }
    // Fallback to AUMID's family part before ! or _
    let family = aumid.split('!').next().unwrap_or(aumid);
    let family = family.split('_').next().unwrap_or(family);
    let last = family.rsplit('.').next().unwrap_or(family);
    if !last.is_empty() {
        return last.to_string();
    }
    raw.to_string()
}

fn enumerate_via_powershell() -> Option<Vec<StoreApp>> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    // PowerShell one-liner: emit JSON with AUMID, DisplayName, Publisher.
    // We call Get-AppxPackage then Get-AppxPackageManifest to get App Ids.
    let script = r#"
$pkgs = Get-AppxPackage | Where-Object { -not $_.IsFramework -and -not $_.IsResourcePackage -and -not $_.IsBundle -and -not $_.IsDevelopmentMode }
$out = @()
foreach ($pkg in $pkgs) {
  try {
    $manifest = Get-AppxPackageManifest $pkg.PackageFullName
    $apps = $manifest.package.applications.application
    if ($null -eq $apps) { continue }
    foreach ($app in @($apps)) {
      $aumid = "$($pkg.PackageFamilyName)!$($app.Id)"
      $display = $app.VisualElements.DisplayName
      if ([string]::IsNullOrWhiteSpace($display)) { $display = $aumid }
      $out += [PSCustomObject]@{ aumid = $aumid; display_name = $display; publisher = $pkg.PublisherDisplayName; package_name = $pkg.Name; install_location = $pkg.InstallLocation; }
    }
  } catch {}
}
$out | ConvertTo-Json -Compress
"#;
    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW);
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() || text == "null" {
        return Some(Vec::new());
    }
    let json = if text.starts_with('[') {
        text
    } else {
        format!("[{text}]")
    };
    let value: serde_json::Value = serde_json::from_str(&json).ok()?;
    let arr = value.as_array()?;
    let mut out = Vec::new();
    for item in arr {
        let aumid = item.get("aumid")?.as_str()?.trim().to_string();
        let raw_display = item
            .get("display_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let package_name = item
            .get("package_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let install_location = item
            .get("install_location")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let mut display_name = raw_display.clone();
        if display_name.starts_with("ms-resource:") || display_name.is_empty() {
            display_name = fallback_display_name(&raw_display, &package_name, &aumid);
            if let Some(resolved) =
                resolve_ms_resource(&raw_display, install_location.as_deref(), &package_name)
            {
                if !resolved.starts_with("ms-resource:") && !resolved.is_empty() {
                    display_name = resolved;
                }
            }
        }
        let logo = install_location
            .as_deref()
            .and_then(|p| find_44_logo(p, &raw_display));
        let publisher = item
            .get("publisher")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        if aumid.is_empty() || display_name.is_empty() {
            continue;
        }
        out.push(StoreApp {
            aumid,
            display_name,
            publisher,
            logo,
        });
    }
    Some(out)
}

/// Converts Store apps to the walker's `Candidate` shape — what
/// `walker::snapshot` merges before sort/dedup. The `target` is the launch
/// key `shell:AppsFolder\<AUMID>` (ticket 122 storage contract), `kind=app`
/// with `LaunchShell` null, and `exe_path` is the logo path when known so
/// `icons::candidate_icon` can try the package logo.
pub fn to_candidates(apps: Vec<StoreApp>) -> Vec<Candidate> {
    apps.into_iter()
        .map(|app| Candidate {
            name: app.display_name,
            publisher: app.publisher,
            target: format!("shell:AppsFolder\\{}", app.aumid),
            exe_path: app.logo,
        })
        .collect()
}

/// Direct helper for the walker: the live Store candidates as `Candidate`s.
pub fn store_candidates() -> Vec<Candidate> {
    to_candidates(enumerate_uwp())
}

/// Logo for a UWP `AUMID` — the 44×44 AppList file path when determinable
/// (ticket 122, 44×44 decision). Used by `icons::candidate_icon` for
/// `shell:AppsFolder\` targets. Memory-only cache in `lazyIcon.svelte` means
/// a poisoned entry auto-fixes on next `walker::snapshot` (no disk cache).
pub fn logo_for_aumid(aumid: &str) -> Option<String> {
    let needle = aumid.trim().to_lowercase();
    // Fast path: enumerate once per distinct AUMID; lazyIcon caches the data URL
    // so this is called at most once per UWP row per picker open.
    for app in enumerate_uwp() {
        if app.aumid.to_lowercase() == needle {
            return app.logo;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_candidates_maps_aumid_to_shell_target() {
        let apps = vec![StoreApp {
            aumid: "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App".into(),
            display_name: "Calculator".into(),
            publisher: Some("Microsoft Corporation".into()),
            logo: None,
        }];
        let candidates = to_candidates(apps);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].name, "Calculator");
        assert_eq!(
            candidates[0].target,
            "shell:AppsFolder\\Microsoft.WindowsCalculator_8wekyb3d8bbwe!App"
        );
        assert_eq!(
            candidates[0].publisher.as_deref(),
            Some("Microsoft Corporation")
        );
    }

    #[test]
    fn powershell_enumeration_does_not_panic_when_missing() {
        let _ = enumerate_via_powershell();
    }

    #[test]
    #[ignore = "local diagnostic — run explicitly to enumerate Store apps"]
    fn debug_enumerate_uwp() {
        let apps = enumerate_uwp();
        eprintln!("enumerate_uwp returned {} apps", apps.len());
        for app in apps.iter().take(20) {
            eprintln!("  {} -> {} ({:?})", app.aumid, app.display_name, app.publisher);
        }
        let paint = apps.iter().find(|a| a.aumid == "Microsoft.Paint_8wekyb3d8bbwe!App");
        eprintln!("paint exact: {:?}", paint);
        let photos_exact = apps.iter().find(|a| a.aumid == "Microsoft.Windows.Photos_8wekyb3d8bbwe!App");
        eprintln!("photos exact: {:?}", photos_exact);
        let calc = apps.iter().find(|a| a.aumid.to_lowercase().contains("calculator"));
        eprintln!("calc found: {:?}", calc);
    }
}
