//! Platform engine seam (ADR-0001, spec decision 2).
//!
//! All machine-facing operations (detection, install, upgrade, verify, env
//! wiring) go through [`PlatformEngine`], held as `Arc<dyn PlatformEngine>`
//! in Tauri managed state. `WindowsWingetEngine` is the v1 implementation;
//! a future platform is a new implementation swapped in at startup, with no
//! changes elsewhere in the app.

pub mod windows;

use std::collections::HashMap;
use std::time::Duration;

use serde::Serialize;

use crate::domain::{EnvWiring, Product, Requirement, Step, VerifyCommand};
use crate::launch::LaunchEntryInput;
use std::fmt;

/// What the engine knows about a Product on this machine — read-only
/// (winget list + uninstall-registry heuristics, no elevation, nothing
/// written). The plan phase (ticket 04) turns one of these per Product into
/// an expected action.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Detection {
    /// Installed at all (winget list and/or uninstall registry).
    pub installed: bool,
    /// winget knows this install and can upgrade it.
    pub winget_managed: bool,
    /// Version winget reports for the installed package.
    pub installed_version: Option<String>,
    /// Newer version available in the winget source, when winget says so.
    pub available_version: Option<String>,
}

impl Detection {
    /// Nothing known about the Product — treated as "will install".
    pub fn absent() -> Self {
        Self::default()
    }
}

/// Outcome of one install/upgrade/verify action on a Requirement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepOutcome {
    pub ok: bool,
    pub reboot_required: bool,
    /// The step was killed when it outlived its per-Requirement timebox.
    pub timed_out: bool,
    pub detail: String,
    /// Raw merged stdout+stderr of the step, persisted to the per-run log.
    pub log: String,
}

/// Outcome of one verify command on a Requirement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyOutcome {
    /// Exit 0 and, when declared, the expected text found in the output.
    pub ok: bool,
    /// Human-readable verdict: what was checked and how it went.
    pub detail: String,
    /// Raw merged stdout+stderr of the command, persisted to the per-run log.
    pub log: String,
}

impl VerifyOutcome {
    pub fn passed(detail: impl Into<String>, log: impl Into<String>) -> Self {
        VerifyOutcome {
            ok: true,
            detail: detail.into(),
            log: log.into(),
        }
    }

    pub fn failed(detail: impl Into<String>, log: impl Into<String>) -> Self {
        VerifyOutcome {
            ok: false,
            detail: detail.into(),
            log: log.into(),
        }
    }
}

impl fmt::Display for VerifyOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ok {
            write!(f, "verify: {}", self.detail)
        } else {
            write!(f, "verify failed: {}", self.detail)
        }
    }
}

/// The platform strategy seam. Detection is implemented by ticket 04, the
/// timeboxed install/upgrade steps by ticket 05, verify commands and env
/// wiring by ticket 07, and command steps plus the winget bootstrap by
/// ticket 08.
pub trait PlatformEngine: Send + Sync {
    /// Called by the run loop before detection: a chance to make the machine
    /// ready (the Windows implementation bootstraps winget when it is missing
    /// and any Requirement needs it). An `Err` aborts the Run with its
    /// message — a missing winget on an unsupported OS build must never fail
    /// silently per-Requirement. The default does nothing.
    fn prepare(&self, requirements: &[&Requirement]) -> Result<(), String> {
        let _ = requirements;
        Ok(())
    }
    /// Is the product installed on this machine (winget list + uninstall
    /// registry heuristic)? Read-only, no elevation.
    fn detect(&self, _product: &Product, _step: &Step) -> Detection {
        Detection::absent()
    }

    /// Detection for many Requirements at once, keyed by Product id. The
    /// Windows implementation runs one `winget list` and one registry scan
    /// and answers every Product from those snapshots; the default falls
    /// back to one `detect` call per Requirement.
    fn detect_many(&self, requirements: &[&Requirement]) -> HashMap<String, Detection> {
        requirements
            .iter()
            .map(|r| {
                (
                    r.product.id.clone(),
                    self.detect(&r.product, &r.step),
                )
            })
            .collect()
    }

    /// Install a requirement's step under a per-requirement timebox.
    /// `install_dir` is the machine-local default install directory (ticket
    /// 34, ADR-0009): `None` means the platform's own default.
    fn install(
        &self,
        _step: &Step,
        _timeout_minutes: u32,
        _install_dir: Option<&str>,
    ) -> StepOutcome {
        unimplemented!("implemented in later tickets")
    }

    /// Upgrade an installed product's step under a per-requirement timebox.
    /// `install_dir` is the machine-local default install directory (ticket
    /// 34, ADR-0009): `None` means the platform's own default.
    fn upgrade(
        &self,
        _step: &Step,
        _timeout_minutes: u32,
        _install_dir: Option<&str>,
    ) -> StepOutcome {
        unimplemented!("implemented in later tickets")
    }

    /// Where the product actually landed, when a registry hint resolves it —
    /// the post-install honesty check (ticket 34): the run loop compares this
    /// against the requested directory and flags an installer that ignored
    /// it. The default reports nothing, so a platform without the heuristic
    /// never fabricates a note.
    fn actual_install_location(&self, _product: &Product) -> Option<String> {
        None
    }

    /// Run a verify command after install; non-zero exit or non-matching
    /// output fails the Requirement.
    fn verify(&self, _command: &VerifyCommand) -> VerifyOutcome {
        unimplemented!("implemented in later tickets")
    }

    /// Apply env wiring after a successful install (User scope only, never
    /// overwriting existing values, `<InstallLocation>` resolved from the
    /// uninstall registry). Returns the notes of everything that happened —
    /// applied values and skips with their reason — which the run surfaces in
    /// the Requirement's outcome.
    fn apply_env_wiring(&self, _product: &Product, _env: &[EnvWiring]) -> Vec<String> {
        unimplemented!("implemented in later tickets")
    }
}

/// One virtual desktop the assignment surface offers (ticket 44): its GUID
/// — stable across Task View reorder, which is why assignments reference it —
/// and its label: the Windows name when the desktop has one, "Desktop N"
/// otherwise. The label is resolved by the engine, so the page never formats
/// GUIDs. `current` marks the desktop the user is on right now, letting a
/// menu offer "pin to this one" as an explicit assignment (ADR-0015 round).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DesktopInfo {
    pub id: String,
    pub name: String,
    pub current: bool,
}

/// One spawn through the launcher seam (ticket 47): the process id when the
/// engine got one, plus the entry's target (the exe path or .lnk) — the key
/// the window resolution falls back to. A successful spawn can carry no pid:
/// launching a shortcut the shell hands to an already-running process (File
/// Explorer, wrapper apps that reuse an instance) succeeds with no process
/// handle of its own — that is a *started* launch, never a failure.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Spawned {
    /// The new process's id, when the engine got one. `None` means the
    /// launch was handed to an existing process — the window must be found
    /// by the image and child fallbacks.
    pub pid: Option<u32>,
    /// The entry's target (an exe path, or a .lnk the engine resolves to its
    /// target exe): the image the fallback window resolution matches against.
    pub target: String,
}

/// One visible window of an app the Quick Launch pipeline is deciding on
/// (ticket 48): the window's handle — the key for "appeared after the
/// launch" (snapshot membership) and the handle winvd moves — plus the
/// desktop answers the skip rule is decided from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppWindow {
    /// The window's handle. Opaque to the orchestrator: it only compares
    /// handles for snapshot membership and hands the handle back to the
    /// engine to move.
    pub hwnd: usize,
    /// The desktop GUID the window is on, when the engine could resolve it
    /// (winvd up, 24H2+); `None` when it could not — the assigned-desktop
    /// skip then never matches, so an entry is launched instead of being
    /// wrongly skipped over a window whose desktop cannot be verified.
    pub desktop: Option<String>,
    /// Whether the window is on the current desktop — the engine's answer,
    /// with the honest fallback when desktops cannot be queried (below the
    /// gate or winvd down): every visible window of the app's image counts,
    /// the closest available approximation of "open on this desktop".
    pub on_current_desktop: bool,
}

/// The launch strategy seam (ticket 42, sibling of [`PlatformEngine`]): all
/// machine-facing Quick Launch operations — spawn, window wait, the skip
/// decision, and the virtual desktop surface — go through [`LauncherEngine`].
/// `WindowsLauncherEngine` is the v1 implementation; the orchestrator
/// (`launch::run_launch_queue`) is pure logic driven by this seam and proven
/// against a fake in tests.
pub trait LauncherEngine: Send + Sync {
    /// Starts one Launch entry and returns what the engine spawned. App
    /// entries go through the shell as-is (the .lnk or exe path); command
    /// entries run under the engine's CREATE_NO_WINDOW convention, or visible
    /// when the entry's show-window toggle is on. A spawn failure is a failed
    /// entry — never an abort of the rest. A successful spawn whose pid is
    /// `None` (the shell handed the launch to an existing process) still
    /// counts as started — the window resolution finds its window.
    fn spawn(&self, entry: &LaunchEntryInput) -> Result<Spawned, String>;

    /// The app's visible windows, matched by image file name — the basename
    /// of the entry's target exe, so a versioned install directory (Edge's
    /// `...\Application\151.0.4129.86\msedge.exe`) matches the running
    /// instance's unversioned image (ticket 48). Each window carries which
    /// desktop it is on. This is the skip decision's only source — never the
    /// process table: an app running without a window, or with all its
    /// windows on other desktops, is not "open here".
    fn app_windows(&self, target: &str) -> Vec<AppWindow>;

    /// Whether the entry's target still exists on disk (ticket 48): a .lnk
    /// resolves to its target exe, and a bare executable name is
    /// PATH-resolvable and never a false failure. An app that updated its
    /// version folder fails the entry fast with "target no longer exists"
    /// instead of a silent 15 s window stall.
    fn target_exists(&self, target: &str) -> bool;

    /// Whether a window of the spawn's app that was NOT in the pre-launch
    /// snapshot `before` has appeared within `timeout` (ticket 48). The
    /// snapshot preference is what keeps a launch the shell handed to a
    /// running instance (Edge) from resolving an old window: the window that
    /// appeared after the launch — the running Edge's new window, the fresh
    /// Explorer window — is the one the orchestrator waits on, never one the
    /// user already has open. Returns the window's handle, which is exactly
    /// the window the orchestrator moves. The orchestrator polls in slices,
    /// so a window that appears frees its slot promptly; `None` (timeout or
    /// dead process) never means the launch failed — the 15 s window timeout
    /// counts as started, the queue never stalls.
    fn wait_for_new_window(
        &self,
        spawned: &Spawned,
        before: &[usize],
        timeout: Duration,
    ) -> Option<usize>;

    /// Moves the window `hwnd` — the NEW window [`Self::wait_for_new_window`]
    /// resolved, never one the user already had open — to the virtual desktop
    /// `guid` (ticket 44): called after the window appears, never on a
    /// desktop the orchestrator does not know about. The engine retries the
    /// move over ~1.5 s before giving up (the shell's view-registration race,
    /// ticket 47). A move failure never fails an entry — the launch already
    /// happened — but the orchestrator surfaces it as a note, never silently.
    fn move_window_to_desktop(&self, hwnd: usize, guid: &str) -> Result<(), String>;

    /// The machine's virtual desktops (ticket 44): each with its id and
    /// label, in Task View order. Empty below the Windows 11 24H2 gate —
    /// which hides the whole assignment surface — and on any winvd failure.
    fn desktops(&self) -> Vec<DesktopInfo> {
        Vec::new()
    }

    /// Creates a virtual desktop (ticket 44) and returns its GUID; `None`
    /// below the gate or when the OS refused.
    fn create_desktop(&self) -> Option<String> {
        None
    }
}