//! Installed-app search (ticket 39): the fresh snapshot of launchable apps
//! behind the Quick Launch search box.
//!
//! Glossary (docs/CONTEXT.md): a **candidate** is one app the search found —
//! a Start Menu shortcut (per-user + all-users, recursive `*.lnk`) or a
//! registry uninstall entry (HKLM 32/64 + HKCU). Candidates carry a display
//! name, a publisher when the registry knows one, the launchable target (the
//! `.lnk` or exe path), and a resolved exe path where determinable
//! (IShellLink for shortcuts; DisplayIcon/InstallLocation for registry
//! entries).
//!
//! The snapshot is re-walked fresh on every call — no cache, no resync. The
//! registry and shortcut-resolution seams are traits so tests can script
//! them without touching the real registry (or COM); only `snapshot()` uses
//! the real implementations.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
use winreg::RegKey;

/// One app the installed-app search found. `target` is what a Launch entry
/// launches (the `.lnk` or the exe path itself); `exe_path` is the resolved
/// executable where determinable (`None` when a shortcut could not be
/// resolved — the entry still launches the shortcut as-is).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Candidate {
    pub name: String,
    pub publisher: Option<String>,
    pub target: String,
    pub exe_path: Option<String>,
}

/// One raw uninstall-registry entry, as read from a hive. Fields the walker
/// does not care about are simply not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryEntry {
    pub display_name: String,
    pub publisher: Option<String>,
    pub display_icon: Option<String>,
    pub install_location: Option<String>,
    pub uninstall_string: Option<String>,
    pub system_component: bool,
    pub parent_key_name: Option<String>,
}

/// The uninstall-registry seam: `RegistryReader` reads the real hives; tests
/// script their own.
pub trait UninstallRegistry {
    fn entries(&self) -> Vec<RegistryEntry>;
}

/// The shortcut-resolution seam: `ShellLinkLnkResolver` resolves via
/// IShellLink (COM); tests script their own. `None` means the shortcut could
/// not be resolved — the walker then falls back to the filename stem.
pub trait LnkResolver {
    fn resolve(&self, lnk_path: &Path) -> Option<String>;
}

/// Uninstall-registry hives the app scans, in priority order (the legacy
/// runner's): 64-bit HKLM, 32-bit HKLM (WOW6432Node), HKCU. Walked first
/// wins on name collisions. The one copy — shared with the engine's
/// uninstall scan, walked only through [`uninstall_subkeys`].
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

/// Every uninstall subkey under [`UNINSTALL_HIVES`], in priority order —
/// the one enumeration behind both the app-list snapshot and the engine's
/// uninstall heuristics. Hives and subkeys that cannot be opened are
/// skipped; each consumer extracts what it needs (full entry, display name,
/// or install location).
pub(crate) fn uninstall_subkeys() -> Vec<RegKey> {
    let mut subs = Vec::new();
    for (root, path) in UNINSTALL_HIVES {
        let Ok(uninstall) = RegKey::predef(root).open_subkey_with_flags(path, KEY_READ) else {
            continue;
        };
        for name in uninstall.enum_keys().flatten() {
            if let Ok(sub) = uninstall.open_subkey_with_flags(&name, KEY_READ) {
                subs.push(sub);
            }
        }
    }
    subs
}

/// Reads the three uninstall hives for real.
pub struct RegistryReader;

impl UninstallRegistry for RegistryReader {
    fn entries(&self) -> Vec<RegistryEntry> {
        uninstall_subkeys().iter().map(read_entry).collect()
    }
}

fn read_entry(sub: &RegKey) -> RegistryEntry {
    RegistryEntry {
        display_name: sub
            .get_value::<String, _>("DisplayName")
            .unwrap_or_default(),
        publisher: sub.get_value::<String, _>("Publisher").ok(),
        display_icon: sub.get_value::<String, _>("DisplayIcon").ok(),
        install_location: sub.get_value::<String, _>("InstallLocation").ok(),
        uninstall_string: sub.get_value::<String, _>("UninstallString").ok(),
        system_component: sub.get_value::<u32, _>("SystemComponent").unwrap_or(0) != 0,
        parent_key_name: sub.get_value::<String, _>("ParentKeyName").ok(),
    }
}

/// Resolves `.lnk` targets via IShellLink (COM). The MTA is initialized for
/// the process lifetime via `CoIncrementMTAUsage` — the same pattern the
/// winvd dependency uses; the cookie is intentionally not decremented.
pub struct ShellLinkLnkResolver;

impl LnkResolver for ShellLinkLnkResolver {
    fn resolve(&self, lnk_path: &Path) -> Option<String> {
        use windows::core::{Interface, PCWSTR};
        use windows::Win32::Foundation::MAX_PATH;
        use windows::Win32::System::Com::{
            CoCreateInstance, CoIncrementMTAUsage, CLSCTX_INPROC_SERVER, IPersistFile, STGM_READ,
        };
        use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

        let _mta = unsafe { CoIncrementMTAUsage() }.ok()?;
        let shell_link: IShellLinkW =
            unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }.ok()?;
        let persist: IPersistFile = shell_link.cast().ok()?;
        let wide = wide_string(lnk_path);
        unsafe { persist.Load(PCWSTR(wide.as_ptr()), STGM_READ) }.ok()?;
        let mut buffer = [0u16; MAX_PATH as usize];
        unsafe { shell_link.GetPath(&mut buffer, std::ptr::null_mut(), 0) }.ok()?;
        let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        if end == 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buffer[..end]))
    }
}

/// A wide NUL-terminated copy of `path`, for the COM calls.
fn wide_string(path: &Path) -> Vec<u16> {
    path.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect()
}

/// The fresh snapshot: per-user + all-users Start Menu shortcuts, merged with
/// the uninstall-registry entries, deduped and sorted by name. Re-walked on
/// every call — no cache.
pub fn snapshot() -> Vec<Candidate> {
    snapshot_with(&start_menu_roots(), &RegistryReader, &ShellLinkLnkResolver)
}

/// The walk itself with its seams injected (test entry point).
fn snapshot_with(
    roots: &[PathBuf],
    registry: &dyn UninstallRegistry,
    resolver: &dyn LnkResolver,
) -> Vec<Candidate> {
    let start_menu = walk_start_menu(roots, resolver);
    let registry = registry
        .entries()
        .iter()
        .filter_map(candidate_from_registry)
        .collect();
    merge(start_menu, registry)
}

/// The two Start Menu roots: this user's shortcuts (`%APPDATA%\...`) and the
/// all-users shortcuts (`%PROGRAMDATA%\...`). Missing roots are simply not
/// walked.
fn start_menu_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(appdata) = dirs::config_dir() {
        roots.push(appdata.join(r"Microsoft\Windows\Start Menu\Programs"));
    }
    if let Some(programdata) = dirs::data_dir() {
        roots.push(programdata.join(r"Microsoft\Windows\Start Menu\Programs"));
    }
    roots
}

fn walk_start_menu(roots: &[PathBuf], resolver: &dyn LnkResolver) -> Vec<Candidate> {
    let mut out = Vec::new();
    for root in roots {
        if root.is_dir() {
            collect_lnks(root, resolver, &mut out);
        }
    }
    out
}

fn collect_lnks(dir: &Path, resolver: &dyn LnkResolver, out: &mut Vec<Candidate>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_lnks(&path, resolver, out);
        } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("lnk")) {
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let stem = stem.trim();
            if stem.is_empty() {
                continue;
            }
            // A shortcut into WindowsApps is a Store/AppX app — excluded
            // entirely, not just unresolved.
            if let Some(exe) = resolver.resolve(&path) {
                if is_windows_apps(&exe) {
                    continue;
                }
                out.push(Candidate {
                    name: stem.to_string(),
                    publisher: None,
                    target: path.to_string_lossy().into_owned(),
                    exe_path: Some(exe),
                });
            } else {
                out.push(Candidate {
                    name: stem.to_string(),
                    publisher: None,
                    target: path.to_string_lossy().into_owned(),
                    exe_path: None,
                });
            }
        }
    }
}

/// Turns one registry entry into a candidate, or skips it: Store/AppX residue
/// (`SystemComponent`, Appx uninstall strings), update children
/// (`ParentKeyName`), blank names, and entries with no launchable exe at all
/// are not apps you can add.
fn candidate_from_registry(entry: &RegistryEntry) -> Option<Candidate> {
    let name = entry.display_name.trim();
    if name.is_empty() {
        return None;
    }
    if entry.system_component || entry.parent_key_name.is_some() {
        return None;
    }
    if entry
        .uninstall_string
        .as_deref()
        .is_some_and(is_appx_uninstall)
    {
        return None;
    }
    let exe = entry
        .display_icon
        .as_deref()
        .and_then(exe_from_icon)
        .or_else(|| entry.install_location.as_deref().and_then(exe_from_location))?;
    if is_windows_apps(&exe) {
        return None;
    }
    Some(Candidate {
        name: name.to_string(),
        publisher: entry.publisher.clone(),
        target: exe.clone(),
        exe_path: Some(exe),
    })
}

/// An Appx/Store uninstall string starts with `Get-AppxPackage` /
/// `Remove-AppxPackage` (or quotes around them).
fn is_appx_uninstall(value: &str) -> bool {
    let value = value.trim().trim_start_matches('"').to_lowercase();
    value.starts_with("get-appxpackage") || value.starts_with("remove-appxpackage")
}

/// Pulls the exe path out of a registry `DisplayIcon`: strips surrounding
/// quotes, drops a trailing `,N` resource index, and only accepts a `.exe`
/// result — an icon file (.ico/.dll) is not a launchable target.
fn exe_from_icon(icon: &str) -> Option<String> {
    let icon = icon.trim().trim_matches('"').trim();
    let path = match icon.rfind(',') {
        Some(comma) => {
            let (head, tail) = icon.split_at(comma);
            let tail = tail[1..].trim();
            if tail.parse::<u32>().is_ok() {
                // `"C:\path\app.exe",0` — the quote lands in the head.
                head.trim().trim_matches('"').trim()
            } else {
                icon
            }
        }
        None => icon,
    };
    path.to_lowercase()
        .ends_with(".exe")
        .then(|| path.to_string())
}

/// A registry `InstallLocation` is usually a directory; only a bare exe path
/// is a launchable target — nothing is fabricated from the directory.
fn exe_from_location(location: &str) -> Option<String> {
    let location = location.trim().trim_matches('"').trim();
    location
        .to_lowercase()
        .ends_with(".exe")
        .then(|| location.to_string())
}

/// Store/AppX apps live under `%ProgramFiles%\WindowsApps` and are excluded
/// from both sources.
fn is_windows_apps(path: &str) -> bool {
    path.replace('/', "\\").to_lowercase().contains(r"\windowsapps\")
}

/// Merge/dedupe: candidates sharing a resolved exe path collapse to one (the
/// Start Menu one wins — it is walked first — keeping its .lnk target; a
/// registry twin only donates its publisher when the winner lacks one), and
/// within each source same-name duplicates collapse before that (the resolved
/// duplicate beats the unresolved one). Candidates without an exe path are
/// never merged — they are distinct shortcuts.
fn merge(start_menu: Vec<Candidate>, registry: Vec<Candidate>) -> Vec<Candidate> {
    let start_menu = collapse_by_name(start_menu);
    let registry = collapse_by_name(registry);
    let mut by_exe: HashMap<String, Candidate> = HashMap::new();
    let mut unresolved: Vec<Candidate> = Vec::new();
    for candidate in start_menu.into_iter().chain(registry) {
        match &candidate.exe_path {
            Some(exe) => {
                let key = exe.replace('/', "\\").to_lowercase();
                match by_exe.get_mut(&key) {
                    Some(held) => {
                        if held.publisher.is_none() {
                            held.publisher = candidate.publisher.clone();
                        }
                    }
                    None => {
                        by_exe.insert(key, candidate);
                    }
                }
            }
            None => unresolved.push(candidate),
        }
    }
    let mut merged: Vec<Candidate> = by_exe.into_values().chain(unresolved).collect();
    merged.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.target.cmp(&b.target))
    });
    merged
}

fn collapse_by_name(list: Vec<Candidate>) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();
    for candidate in list {
        let key = candidate.name.to_lowercase();
        match seen.get(&key).copied() {
            // First of its name: keep it.
            None => {
                seen.insert(key, out.len());
                out.push(candidate);
            }
            Some(index) => {
                let held = &out[index];
                let same_app = held.exe_path.is_some() && held.exe_path == candidate.exe_path;
                if held.exe_path.is_none() || same_app {
                    // An unresolved twin adds nothing; prefer the resolved one.
                    if held.exe_path.is_none() && candidate.exe_path.is_some() {
                        out[index] = candidate;
                    }
                } else {
                    // Same name but different resolved exes: distinct apps,
                    // both stay.
                    seen.insert(key, out.len());
                    out.push(candidate);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root() -> PathBuf {
        tempfile::tempdir().unwrap().into_path()
    }

    /// A fixture `.lnk` (content does not matter — resolution is scripted).
    fn make_lnk(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, b"fixture").unwrap();
        path
    }

    struct ScriptedRegistry {
        entries: Vec<RegistryEntry>,
    }

    impl UninstallRegistry for ScriptedRegistry {
        fn entries(&self) -> Vec<RegistryEntry> {
            self.entries.clone()
        }
    }

    struct ScriptedLnk {
        map: HashMap<PathBuf, Option<String>>,
    }

    impl LnkResolver for ScriptedLnk {
        fn resolve(&self, lnk_path: &Path) -> Option<String> {
            self.map.get(lnk_path).cloned().flatten()
        }
    }

    fn scripted(map: HashMap<PathBuf, Option<String>>) -> ScriptedLnk {
        ScriptedLnk { map }
    }

    fn walk(
        roots: &[PathBuf],
        registry: &dyn UninstallRegistry,
        resolver: &dyn LnkResolver,
    ) -> Vec<Candidate> {
        snapshot_with(roots, registry, resolver)
    }

    #[test]
    fn shared_exe_merges_with_start_menu_preferred() {
        let root = temp_root();
        let code_lnk = make_lnk(&root, "Visual Studio Code.lnk");
        let mut resolve = HashMap::new();
        resolve.insert(code_lnk.clone(), Some(r"C:\Apps\VSCode\Code.exe".into()));
        let registry = ScriptedRegistry {
            entries: vec![RegistryEntry {
                display_name: "Visual Studio Code".into(),
                publisher: Some("Microsoft Corporation".into()),
                display_icon: Some(r"C:\Apps\VSCode\Code.exe,0".into()),
                install_location: None,
                uninstall_string: None,
                system_component: false,
                parent_key_name: None,
            }],
        };

        let merged = walk(&[root], &registry, &scripted(resolve));
        assert_eq!(merged.len(), 1);
        // The Start Menu candidate wins: the target stays the .lnk.
        assert_eq!(merged[0].name, "Visual Studio Code");
        assert_eq!(merged[0].target, code_lnk.to_string_lossy());
        assert_eq!(merged[0].exe_path.as_deref(), Some(r"C:\Apps\VSCode\Code.exe"));
        // ...but the registry twin donated its publisher.
        assert_eq!(merged[0].publisher.as_deref(), Some("Microsoft Corporation"));
    }

    #[test]
    fn registry_entries_merge_on_shared_exe() {
        let registry = ScriptedRegistry {
            entries: vec![
                RegistryEntry {
                    display_name: "Foo".into(),
                    publisher: None,
                    display_icon: Some(r"C:\Apps\Foo\foo.exe,0".into()),
                    install_location: None,
                    uninstall_string: None,
                    system_component: false,
                    parent_key_name: None,
                },
                RegistryEntry {
                    display_name: "Foo Inc.".into(),
                    publisher: Some("Foo Inc.".into()),
                    display_icon: Some(r"C:\Apps\Foo\foo.exe".into()),
                    install_location: None,
                    uninstall_string: None,
                    system_component: false,
                    parent_key_name: None,
                },
            ],
        };

        let merged = walk(&[], &registry, &scripted(HashMap::new()));
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].name, "Foo");
        assert_eq!(merged[0].exe_path.as_deref(), Some(r"C:\Apps\Foo\foo.exe"));
        assert_eq!(merged[0].publisher.as_deref(), Some("Foo Inc."));
    }

    #[test]
    fn same_name_within_a_source_collapses() {
        let root = temp_root();
        let sub = root.join("Accessories");
        std::fs::create_dir_all(&sub).unwrap();
        make_lnk(&root, "Broken.lnk");
        make_lnk(&sub, "Broken.lnk");
        // Neither resolves: two identical unresolved twins collapse to one
        // (which twin survives depends on read_dir order — only the
        // collapse matters).
        let merged = walk(
            &[root],
            &ScriptedRegistry { entries: vec![] },
            &scripted(HashMap::new()),
        );
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].name, "Broken");
    }

    #[test]
    fn same_name_but_different_exes_stay_distinct() {
        let root = temp_root();
        let sub = root.join("Accessories");
        std::fs::create_dir_all(&sub).unwrap();
        let a = make_lnk(&root, "Terminal.lnk");
        let b = make_lnk(&sub, "Terminal.lnk");
        let mut resolve = HashMap::new();
        resolve.insert(a.clone(), Some(r"C:\Apps\TermA\term.exe".into()));
        resolve.insert(b.clone(), Some(r"C:\Apps\TermB\term.exe".into()));

        let merged = walk(&[root], &ScriptedRegistry { entries: vec![] }, &scripted(resolve));
        assert_eq!(merged.len(), 2, "different resolved exes are distinct apps");
        let targets: Vec<String> = merged.iter().map(|c| c.target.clone()).collect();
        assert!(targets.contains(&a.to_string_lossy().into_owned()));
        assert!(targets.contains(&b.to_string_lossy().into_owned()));
    }

    #[test]
    fn same_name_prefers_the_resolved_duplicate() {
        let root = temp_root();
        let sub = root.join("Tools");
        std::fs::create_dir_all(&sub).unwrap();
        let broken = make_lnk(&root, "Notepad.lnk");
        let working = make_lnk(&sub, "Notepad.lnk");
        let mut resolve = HashMap::new();
        resolve.insert(broken.clone(), None);
        resolve.insert(working.clone(), Some(r"C:\Windows\notepad.exe".into()));

        let merged = walk(&[root], &ScriptedRegistry { entries: vec![] }, &scripted(resolve));
        assert_eq!(merged.len(), 1);
        // The resolved duplicate beats the earlier unresolved one.
        assert_eq!(merged[0].name, "Notepad");
        assert_eq!(merged[0].target, working.to_string_lossy());
        assert_eq!(merged[0].exe_path.as_deref(), Some(r"C:\Windows\notepad.exe"));
    }

    #[test]
    fn unresolved_lnk_falls_back_to_filename_stem() {
        let root = temp_root();
        let broken = make_lnk(&root, "Broken App.lnk");
        let merged = walk(
            &[root],
            &ScriptedRegistry { entries: vec![] },
            &scripted(HashMap::new()),
        );
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].name, "Broken App");
        assert_eq!(merged[0].target, broken.to_string_lossy());
        assert_eq!(merged[0].exe_path, None);
        assert_eq!(merged[0].publisher, None);
    }

    #[test]
    fn store_and_appx_entries_are_excluded() {
        let root = temp_root();
        let store_lnk = make_lnk(&root, "Store App.lnk");
        let mut resolve = HashMap::new();
        resolve.insert(
            store_lnk.clone(),
            Some(r"C:\Program Files\WindowsApps\Microsoft.Foo_1.0\foo.exe".into()),
        );
        let registry = ScriptedRegistry {
            entries: vec![
                // SystemComponent=1: drivers, updates, Store residue.
                RegistryEntry {
                    display_name: "Driver Thing".into(),
                    publisher: None,
                    display_icon: Some(r"C:\Drivers\drv.exe".into()),
                    install_location: None,
                    uninstall_string: None,
                    system_component: true,
                    parent_key_name: None,
                },
                // Appx uninstall string.
                RegistryEntry {
                    display_name: "Store App".into(),
                    publisher: None,
                    display_icon: Some(r"C:\Apps\storeapp.exe".into()),
                    install_location: None,
                    uninstall_string: Some("Get-AppxPackage *StoreApp* | Remove-AppxPackage".into()),
                    system_component: false,
                    parent_key_name: None,
                },
                // DisplayIcon inside WindowsApps.
                RegistryEntry {
                    display_name: "WindowsApps Twin".into(),
                    publisher: None,
                    display_icon: Some(r"C:\Program Files\WindowsApps\Other\other.exe".into()),
                    install_location: None,
                    uninstall_string: None,
                    system_component: false,
                    parent_key_name: None,
                },
                // Child of an update entry.
                RegistryEntry {
                    display_name: "Update Child".into(),
                    publisher: None,
                    display_icon: Some(r"C:\Apps\child.exe".into()),
                    install_location: None,
                    uninstall_string: None,
                    system_component: false,
                    parent_key_name: Some("{GUID}".into()),
                },
            ],
        };

        let merged = walk(&[root], &registry, &scripted(resolve));
        assert!(merged.is_empty());
    }

    #[test]
    fn registry_exe_comes_from_icon_then_location() {
        let quoted = RegistryEntry {
            display_name: "Quoted".into(),
            publisher: None,
            display_icon: Some(r#""C:\Apps\Foo Bar\foo.exe",0"#.into()),
            install_location: None,
            uninstall_string: None,
            system_component: false,
            parent_key_name: None,
        };
        let icon_only = RegistryEntry {
            display_name: "Icon Only".into(),
            publisher: None,
            display_icon: Some(r"C:\Assets\icon.ico".into()),
            install_location: Some(r"C:\Apps\IconOnly".into()),
            uninstall_string: None,
            system_component: false,
            parent_key_name: None,
        };
        let location_exe = RegistryEntry {
            display_name: "Location Exe".into(),
            publisher: None,
            display_icon: None,
            install_location: Some(r"C:\Apps\Loc\loc.exe".into()),
            uninstall_string: None,
            system_component: false,
            parent_key_name: None,
        };
        let no_exe = RegistryEntry {
            display_name: "No Exe".into(),
            publisher: None,
            display_icon: None,
            install_location: Some(r"C:\Apps\NoExe".into()),
            uninstall_string: None,
            system_component: false,
            parent_key_name: None,
        };
        let blank = RegistryEntry {
            display_name: "   ".into(),
            publisher: None,
            display_icon: None,
            install_location: None,
            uninstall_string: None,
            system_component: false,
            parent_key_name: None,
        };
        let registry = ScriptedRegistry {
            entries: vec![quoted, icon_only, location_exe, no_exe, blank],
        };

        let merged = walk(&[], &registry, &scripted(HashMap::new()));
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].name, "Location Exe");
        assert_eq!(merged[0].exe_path.as_deref(), Some(r"C:\Apps\Loc\loc.exe"));
        assert_eq!(merged[0].target, merged[0].exe_path.clone().unwrap());
        assert_eq!(merged[1].name, "Quoted");
        assert_eq!(merged[1].exe_path.as_deref(), Some(r"C:\Apps\Foo Bar\foo.exe"));
    }

    #[test]
    fn icon_with_quotes_and_index_resolves() {
        assert_eq!(
            exe_from_icon(r#""C:\Apps\Foo Bar\foo.exe",0"#).as_deref(),
            Some(r"C:\Apps\Foo Bar\foo.exe")
        );
        assert_eq!(exe_from_icon(r"C:\Apps\foo.exe,0").as_deref(), Some(r"C:\Apps\foo.exe"));
        assert_eq!(exe_from_icon(r"C:\Apps\foo.exe").as_deref(), Some(r"C:\Apps\foo.exe"));
        assert_eq!(exe_from_icon(r"C:\Assets\icon.ico,0"), None);
        assert_eq!(exe_from_icon(r"C:\Windows\System32\shell32.dll,21"), None);
    }

    #[test]
    fn non_lnk_files_and_missing_roots_are_ignored() {
        let root = temp_root();
        std::fs::write(root.join("README.txt"), b"nope").unwrap();
        std::fs::write(root.join("install.cmd"), b"nope").unwrap();
        let gone = root.join("does-not-exist");
        let merged = walk(
            &[root, gone],
            &ScriptedRegistry { entries: vec![] },
            &scripted(HashMap::new()),
        );
        assert!(merged.is_empty());
    }

    #[test]
    fn results_are_sorted_by_name() {
        let root = temp_root();
        let b = make_lnk(&root, "Bravo.lnk");
        let a = make_lnk(&root, "alpha.lnk");
        let mut resolve = HashMap::new();
        resolve.insert(a.clone(), Some(r"C:\Apps\Alpha\a.exe".into()));
        resolve.insert(b.clone(), Some(r"C:\Apps\Bravo\b.exe".into()));
        let merged = walk(&[root], &ScriptedRegistry { entries: vec![] }, &scripted(resolve));
        assert_eq!(
            merged.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            vec!["alpha", "Bravo"]
        );
    }
}
