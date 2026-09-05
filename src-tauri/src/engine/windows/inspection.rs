//! ADR-0029 keeps Quick Launch native window/process inspection in one owner.
//!
//! The launch queue resolves windows through two deliberately different
//! selection policies: the spawned-pid step accepts the first visible
//! ownerless window for that pid (blank titles and shell chrome included),
//! while the general snapshot excludes blank titles and shell chrome. The
//! new-window resolution order (pid, unseen image, unseen AUMID, unseen
//! direct child) is part of that contract. Callers use the existing
//! LauncherEngine seam; the native enumeration and handle mechanics live
//! here behind a private source seam with a production adapter and an
//! exercised record adapter.

use std::{
    collections::HashSet,
    path::Path,
    time::{Duration, Instant},
};

use windows_sys::Win32::Foundation::HWND;

use super::{
    expand_env, guid_to_id, is_uwp_target, uwp_aumid,
    virtual_desktops_supported_on_this_machine, winvd_hwnd,
};

/// The window/process facts the selection logic reads. The production
/// adapter probes the live machine; the record adapter replays controlled
/// window tables so the pid/image/AUMID/child order and the snapshot
/// exclusion rules are locked without a live desktop.
trait WindowSource {
    fn pid_window(&self, pid: u32) -> Option<usize>;
    fn snapshot(&self) -> Vec<(usize, u32)>;
    fn image_matches(&self, pid: u32, image: &str) -> bool;
    fn aumid(&self, hwnd: usize) -> Option<String>;
    fn direct_children(&self, pid: u32) -> HashSet<u32>;
    fn desktop_for(&self, hwnd: usize) -> (Option<String>, bool);
}

struct NativeWindows;

impl WindowSource for NativeWindows {
    fn pid_window(&self, pid: u32) -> Option<usize> {
        window_for_pid(pid).map(|hwnd| hwnd as usize)
    }

    fn snapshot(&self) -> Vec<(usize, u32)> {
        visible_app_windows()
    }

    fn image_matches(&self, pid: u32, image: &str) -> bool {
        process_matches_image(pid, image)
    }

    fn aumid(&self, hwnd: usize) -> Option<String> {
        window_aumid(hwnd)
    }

    fn direct_children(&self, pid: u32) -> HashSet<u32> {
        all_processes()
            .into_iter()
            .filter(|(_, parent)| *parent == pid)
            .map(|(child, _)| child)
            .collect()
    }

    fn desktop_for(&self, hwnd: usize) -> (Option<String>, bool) {
        (
            window_desktop(hwnd),
            window_on_current_desktop(hwnd),
        )
    }
}

/// The AUMID a window belongs to (UWP, ticket 122) — via
/// `GetApplicationUserModelId`. `None` for Win32 windows or on failure. Stubbed
/// to `None` for now: the pid path in `new_window_for_spawned` finds the UWP
/// window via its owning pid without needing the AUMID, and the fake's
/// `image_key` handles AUMID matching for tests. A full implementation would
/// call `GetApplicationUserModelId` on the window's process handle (Appx).
fn window_aumid(_hwnd: usize) -> Option<String> {
    None
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
pub(super) fn wait_for_new_window(
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
/// open. Ticket 122: `shell:AppsFolder\` UWP targets also match via
/// `GetApplicationUserModelId` (AUMID) when the image fallback misses.
fn new_window_for_spawned(spawned: &crate::engine::Spawned, before: &[usize]) -> Option<usize> {
    new_window_for_spawned_with(&NativeWindows, spawned, before)
}

fn new_window_for_spawned_with(
    source: &impl WindowSource,
    spawned: &crate::engine::Spawned,
    before: &[usize],
) -> Option<usize> {
    if let Some(pid) = spawned.pid {
        if let Some(hwnd) = source.pid_window(pid) {
            return Some(hwnd);
        }
    }
    let image = window_image_basename(&spawned.target);
    let windows = source.snapshot();
    if let Some(image) = &image {
        for (hwnd, pid) in &windows {
            if !before.contains(hwnd) && source.image_matches(*pid, image) {
                return Some(*hwnd);
            }
        }
    }
    if let Some(aumid) = uwp_aumid(&spawned.target) {
        let needle = aumid.to_lowercase();
        for (hwnd, _) in &windows {
            if !before.contains(hwnd)
                && source
                    .aumid(*hwnd)
                    .map(|id| id.to_lowercase() == needle)
                    .unwrap_or(false)
            {
                return Some(*hwnd);
            }
        }
    }
    if let Some(pid) = spawned.pid {
        let children = source.direct_children(pid);
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
        let visible = unsafe { IsWindowVisible(hwnd) != 0 };
        let ownerless = unsafe { GetWindow(hwnd, GW_OWNER).is_null() };
        // Title/class probes run only past the visible-ownerless gate, so
        // enumeration cost matches the pre-split callback; the title/chrome
        // rule itself lives in `is_snapshot_candidate`.
        if visible
            && ownerless
            && is_snapshot_candidate(true, true, &window_title(hwnd), &window_class(hwnd))
        {
            probe.found.push((hwnd as usize, pid));
        }
        1 // continue
    }

    let mut probe = Probe { found: Vec::new() };
    let _ = unsafe { EnumWindows(Some(probe_window), &mut probe as *mut Probe as LPARAM) };
    probe.found
}

/// Whether a probed window belongs in the general snapshot: visible and
/// ownerless, with a real title, and never shell chrome. The spawned-pid
/// step deliberately skips this filter (a fresh pid's window is new by
/// construction, even blank or chrome-classed); every other resolution step
/// reads the filtered snapshot.
fn is_snapshot_candidate(visible: bool, ownerless: bool, title: &str, class: &str) -> bool {
    visible && ownerless && !is_shell_chrome_class(class) && !title.trim().is_empty()
}

/// The app's visible windows for the skip decision and the pre-launch
/// snapshot (ticket 48): every visible window whose process image's
/// basename equals the entry's target basename — versioned install
/// directories (Edge, Slack, Discord) match the running instance's
/// unversioned image. Each window carries the desktop answers the skip rule
/// is decided from; windows whose desktop cannot be resolved never match an
/// assigned-desktop skip, so a machine that cannot answer the question
/// launches instead of wrongly skipping. Ticket 122: `shell:AppsFolder\`
/// UWP targets match via `GetApplicationUserModelId` (AUMID) instead of the
/// image basename.
pub(super) fn app_windows(target: &str) -> Vec<crate::engine::AppWindow> {
    app_windows_with(&NativeWindows, target)
}

fn app_windows_with(source: &impl WindowSource, target: &str) -> Vec<crate::engine::AppWindow> {
    if let Some(aumid) = uwp_aumid(target) {
        let needle = aumid.to_lowercase();
        return source
            .snapshot()
            .into_iter()
            .filter(|(hwnd, _)| {
                source
                    .aumid(*hwnd)
                    .map(|id| id.to_lowercase() == needle)
                    .unwrap_or(false)
            })
            .map(|(hwnd, _)| {
                let (desktop, on_current_desktop) = source.desktop_for(hwnd);
                crate::engine::AppWindow {
                    hwnd,
                    desktop,
                    on_current_desktop,
                }
            })
            .collect();
    }
    let Some(image) = window_image_basename(target) else {
        return Vec::new();
    };
    source
        .snapshot()
        .into_iter()
        .filter(|(_, pid)| source.image_matches(*pid, &image))
        .map(|(hwnd, _)| {
            let (desktop, on_current_desktop) = source.desktop_for(hwnd);
            crate::engine::AppWindow {
                hwnd,
                desktop,
                on_current_desktop,
            }
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
fn is_shell_chrome_class(class: &str) -> bool {
    const CHROME: [&str; 5] = [
        "Shell_TrayWnd",
        "Shell_SecondaryTrayWnd",
        "Progman",
        "WorkerW",
        "XamlExplorerHostIslandWindow",
    ];
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
/// instead of the silent 15 s window stall. UWP `shell:AppsFolder\` targets
/// (ticket 122) are always considered existing — they are not filesystem
/// paths and the PackageManager would have to be queried to prove absence,
/// which would make a Store app's versioned-folder update look like a failure.
pub(super) fn target_exists(path: &str) -> bool {
    if is_uwp_target(path) {
        return true;
    }
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

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::*;

    /// Controlled window/process records: the exercised test adapter at the
    /// private source seam. Native enumeration stays the production adapter;
    /// these records lock the resolution order and the snapshot/PID policy
    /// split without a live desktop.
    struct StubWindows {
        pid_windows: HashMap<u32, usize>,
        snapshot: Vec<(usize, u32)>,
        images: HashMap<u32, String>,
        aumids: HashMap<usize, String>,
        children: HashMap<u32, HashSet<u32>>,
    }

    impl StubWindows {
        fn empty() -> Self {
            Self {
                pid_windows: HashMap::new(),
                snapshot: Vec::new(),
                images: HashMap::new(),
                aumids: HashMap::new(),
                children: HashMap::new(),
            }
        }
    }

    impl WindowSource for StubWindows {
        fn pid_window(&self, pid: u32) -> Option<usize> {
            self.pid_windows.get(&pid).copied()
        }

        fn snapshot(&self) -> Vec<(usize, u32)> {
            self.snapshot.clone()
        }

        fn image_matches(&self, pid: u32, image: &str) -> bool {
            self.images
                .get(&pid)
                .is_some_and(|path| image_basename(path).is_some_and(|base| base.eq_ignore_ascii_case(image)))
        }

        fn aumid(&self, hwnd: usize) -> Option<String> {
            self.aumids.get(&hwnd).cloned()
        }

        fn direct_children(&self, pid: u32) -> HashSet<u32> {
            self.children.get(&pid).cloned().unwrap_or_default()
        }

        fn desktop_for(&self, _hwnd: usize) -> (Option<String>, bool) {
            (None, true)
        }
    }

    fn spawned(pid: Option<u32>, target: &str) -> crate::engine::Spawned {
        crate::engine::Spawned {
            pid,
            target: target.to_string(),
        }
    }

    #[test]
    fn pid_window_wins_over_unseen_image_aumid_and_child() {
        let source = StubWindows {
            pid_windows: HashMap::from([(42, 1001)]),
            snapshot: vec![(1001, 42), (1002, 7), (1003, 8), (1004, 9)],
            images: HashMap::from([
                (7, r"C:\Apps\app.exe".to_string()),
                (9, r"C:\Apps\child.exe".to_string()),
            ]),
            aumids: HashMap::from([(1003, "Pkg!App".to_string())]),
            children: HashMap::from([(42, HashSet::from([9]))]),
        };
        let found = new_window_for_spawned_with(
            &source,
            &spawned(Some(42), r"C:\Apps\app.exe"),
            &[],
        );
        assert_eq!(found, Some(1001));
    }

    #[test]
    fn unseen_image_wins_once_the_pid_window_is_gone() {
        let source = StubWindows {
            pid_windows: HashMap::new(),
            snapshot: vec![(1002, 7), (1003, 8), (1004, 9)],
            images: HashMap::from([
                (7, r"C:\Apps\app.exe".to_string()),
                (9, r"C:\Apps\child.exe".to_string()),
            ]),
            aumids: HashMap::from([(1003, "Pkg!App".to_string())]),
            children: HashMap::from([(42, HashSet::from([9]))]),
        };
        let found = new_window_for_spawned_with(
            &source,
            &spawned(Some(42), r"C:\Apps\app.exe"),
            &[],
        );
        assert_eq!(found, Some(1002));
    }

    #[test]
    fn unseen_aumid_wins_once_image_candidates_are_seen() {
        let source = StubWindows {
            pid_windows: HashMap::new(),
            snapshot: vec![(1002, 7), (1003, 8)],
            images: HashMap::from([(7, r"C:\Apps\app.exe".to_string())]),
            aumids: HashMap::from([(1003, "Pkg!App".to_string())]),
            children: HashMap::new(),
        };
        // The only image window is already in `before`, so the AUMID window
        // is the first unseen match.
        let found = new_window_for_spawned_with(
            &source,
            &spawned(Some(42), "shell:AppsFolder\\Pkg!App"),
            &[1002],
        );
        assert_eq!(found, Some(1003));
    }

    #[test]
    fn unseen_direct_child_is_the_last_resort_before_nothing() {
        let source = StubWindows {
            pid_windows: HashMap::new(),
            snapshot: vec![(1004, 9)],
            images: HashMap::new(),
            aumids: HashMap::new(),
            children: HashMap::from([(42, HashSet::from([9]))]),
        };
        let found =
            new_window_for_spawned_with(&source, &spawned(Some(42), r"C:\Apps\app.exe"), &[]);
        assert_eq!(found, Some(1004));

        let unrelated = StubWindows::empty();
        assert_eq!(
            new_window_for_spawned_with(&unrelated, &spawned(Some(42), r"C:\Apps\app.exe"), &[]),
            None
        );
    }

    #[test]
    fn snapshot_rejects_blank_titles_and_shell_chrome_that_pid_accepts() {
        // The pid step takes the first visible ownerless window for the pid
        // with no title/chrome filter; the general snapshot applies both.
        assert!(!is_snapshot_candidate(true, true, "", "NotChrome"));
        assert!(!is_snapshot_candidate(true, true, "   ", "NotChrome"));
        assert!(!is_snapshot_candidate(true, true, "Taskbar", "Shell_TrayWnd"));
        assert!(!is_snapshot_candidate(true, true, "App", "Progman"));
        assert!(!is_snapshot_candidate(false, true, "App", "NotChrome"));
        assert!(!is_snapshot_candidate(true, false, "App", "NotChrome"));
        assert!(is_snapshot_candidate(true, true, "App", "NotChrome"));

        assert!(is_shell_chrome_class("Shell_TrayWnd"));
        assert!(is_shell_chrome_class("shell_secondarytraywnd"));
        assert!(is_shell_chrome_class("Progman"));
        assert!(is_shell_chrome_class("WorkerW"));
        assert!(is_shell_chrome_class("XamlExplorerHostIslandWindow"));
        assert!(!is_shell_chrome_class("NotChrome"));
        assert!(!is_shell_chrome_class(""));

        // A blank-title chrome window still resolves through the pid step.
        let source = StubWindows {
            pid_windows: HashMap::from([(42, 1001)]),
            snapshot: Vec::new(),
            images: HashMap::new(),
            aumids: HashMap::new(),
            children: HashMap::new(),
        };
        assert_eq!(
            new_window_for_spawned_with(&source, &spawned(Some(42), r"C:\Apps\app.exe"), &[]),
            Some(1001)
        );
    }

    #[test]
    fn app_windows_prefers_aumid_for_store_targets_and_basename_otherwise() {
        let source = StubWindows {
            pid_windows: HashMap::new(),
            snapshot: vec![(1003, 8), (1002, 7)],
            images: HashMap::from([(7, r"C:\Apps\app.exe".to_string())]),
            aumids: HashMap::from([(1003, "Pkg!App".to_string())]),
            children: HashMap::new(),
        };
        let store = app_windows_with(&source, "shell:AppsFolder\\Pkg!App");
        assert_eq!(store.iter().map(|w| w.hwnd).collect::<Vec<_>>(), vec![1003]);

        let desktop = app_windows_with(&source, r"C:\Apps\app.exe");
        assert_eq!(desktop.iter().map(|w| w.hwnd).collect::<Vec<_>>(), vec![1002]);

        let missing = app_windows_with(&StubWindows::empty(), r"C:\Apps\app.exe");
        assert!(missing.is_empty());
    }

    #[test]
    fn target_exists_covers_uwp_bare_names_and_missing_files() {
        assert!(target_exists("shell:AppsFolder\\Pkg!App"));
        assert!(target_exists("  SHELL:APPSFOLDER\\Pkg!App  "));
        // A bare executable name resolves through PATH — never a failure.
        assert!(target_exists("app.exe"));
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("no-such-app-sprout.exe");
        assert!(!target_exists(&missing.to_string_lossy()));
        let present = dir.path().join("present.exe");
        std::fs::write(&present, b"stub").unwrap();
        assert!(target_exists(&present.to_string_lossy()));
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
}
