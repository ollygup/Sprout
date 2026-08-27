# Sprout

A Windows desktop app for composing, running, and sharing software-installation presets. Replaces the legacy PowerShell setup package, which passed the release parity gate and was removed at v1 (see `docs/release/parity-checklist.md`).

## Language

**Product**:
A thing installable on this machine — winget ID, display name, install-location hint, and default env suggestions.
_Avoid_: App, package, catalog entry

**Requirement**:
A declaration that the machine must have a specific Product in a specific state: a VersionPolicy, an optional Step, optional Env wiring, and optional verify commands. In presets created or forked in Sprout, the Requirement is a **live reference** to the Library Product by id — the name and winget step are resolved from the Library at plan and run time (ADR-0007). Imported presets keep their authored snapshot instead. A requirement whose product left the Library is "removed from library" (unresolved) and excluded from runs until the product is re-added. In the composer the Requirements of a preset are presented as **Applications** (a UI synonym, ticket 35) — the data shape stays `Requirement` end to end.
_Avoid_: Item, config entry, app entry

**VersionPolicy**:
How the machine must relate to a Product's version: `latest` (upgrade to newest), `pinned` (exact version), or `present` (installed, never upgraded).
_Avoid_: Version constraint, update behavior

**Step**:
The mechanism by which a Requirement is executed — `winget` or `command` — described as data with executor-specific parameters.
_Avoid_: Installer, script

**Preset**:
A named, versioned, exportable set of Requirements targeting a platform. The unit of sharing; immutable once imported, edited only by forking.
_Avoid_: Profile, configuration, setup

**Preset file**:
The `.sprout.json` file a Preset is exported to. Double-clicking one opens Sprout and imports it (registered by the installer via the Windows file association).
_Avoid_: Setup file, config file

**Plan**:
The computed expected actions for one or more Presets on this machine (will install / will upgrade / already OK / satisfies-by-newer / conflict), produced read-only before anything runs.
_Avoid_: Dry run, preview

**Run**:
One application of a Plan to this machine, stored with per-Requirement outcomes and log file paths.
_Avoid_: Session, install job

**Outcome**:
The overall verdict of a Run, derived from its per-Requirement results: *Applied* (everything applied or was already satisfied — nothing needed attention), *With notes* (the run completed but something needed attention, e.g. an unmanaged product was detected and skipped), *Cancelled* (the user aborted between Requirements), or *Failed* (a Requirement failed or timed out). A run is only ever "clean" when nothing needed attention.
_Avoid_: Status, result

**Env wiring**:
The `set` / `prepend` environment-variable operations a Requirement applies after a successful install. User scope only; never overwrites existing values; `<InstallLocation>` resolved from the uninstall registry at apply time.
_Avoid_: Environment variables (when referring to the whole mechanism)

**Library**:
The user's local collection of Presets and Products, stored in the local database. Products are the source of truth for requirement names and winget steps; deleting one removes it from the presets that reference it (local presets drop the requirement, imported presets keep their snapshot).
_Avoid_: Database, catalog

**Install location hint**:
A Product-level hint used to find where software landed after an install (e.g. a needle against the uninstall registry), also backing the `<InstallLocation>` env placeholder. It describes the product, not a policy — it never requests a directory.
_Avoid_: Install path, target directory

**Install directory**:
The machine-local Settings value (`settings.install_dir`) that names where installs and upgrades should land — empty means winget's own default. Runs pass it to winget as `--location`; the Plan shows "installs go to …"; a run result that lands elsewhere reports it ("installer ignored the requested directory"). Never part of a Preset, Plan payload, or export (ADR-0009). Per-product overrides are future work (ticket 36).
_Avoid_: Install location hint (a Product property, not a setting), install path

**Verify command**:
A command declared on a Requirement and run after install; a non-zero exit or non-matching output fails the Requirement.
_Avoid_: Post-install check

## Quick access (tray, window, dock)

**Launch entry**:
A single item in the Quick Launch list — either a picked app (`.lnk`/`.exe`, launched as-is) or a user-written command (PowerShell/cmd/no-shell, optional show-window). An entry may carry two independent, optional annotations, never to be confused: a **desktop assignment** (which Virtual desktop its window opens on) and a **Group** membership (which user-named bucket it sits in, ticket 89) — assigning a desktop never groups an entry, and grouping one never assigns a desktop. Desktop assignment has no master switch (ADR-0015): wherever virtual desktops are supported it is offered in the entry's own menu and takes effect as soon as an entry carries one; opting out is per-entry ("No assignment"). Group membership is opt-in via the collection's Groups toggle (default off); off means fully dormant — stored memberships are never shown or applied, but also never deleted. A desktop assignment never structures the list; a Group structures it only while its feature is on. Machine-local; never part of Presets or exports.
_Avoid_: App entry, shortcut

**Virtual desktop**:
One of Windows' own virtual desktops (Task View), identified by a stable GUID that survives reordering and named by the user's label or "Desktop N". Sprout assigns launched windows to them — including creating one on the user's behalf at their request — but owns none of the desktop management itself. Available only on Windows 11 24H2+; below that gate (or when the OS refuses) the whole assignment surface is hidden everywhere. Machine-local; never part of Presets or exports.
_Avoid_: Desktop grouping, workspace

**Group**:
A user-named bucket within exactly one collection (ticket 89): a Quick Action group holds only Quick Actions, a Clip group only Clips, a Launch-entry group only Launch entries — namespaces are isolated in storage, so cross-collection grouping is impossible, not just hidden. Each item belongs to at most one group; items start ungrouped. A group exists only while at least one member belongs to it: groups are created by assigning an item ("Move to group → New group…") and dissolve automatically when their last member leaves; explicitly deleting a populated group instead returns its members to ungrouped. A name is exclusive within its collection — creating or renaming onto a sibling's name, compared trimmed and case-insensitively, is refused, and uniqueness binds to live rows only, so deleting (or dissolving) a group frees its name for immediate reuse. Lists render ungrouped items first, then groups in the user's own order. Opt-in per collection via each page's Groups toggle (default off): off renders every list flat while stored groups and memberships survive untouched. A Group is structure for organizing lists — unrelated to virtual-desktop assignment, which decides where a launched window opens.
_Avoid_: Folder, category, tag, desktop group

**Quick Launch**:
The machine-local list of Launch entries that the tray one-click launcher and the Quick Launch window start. Never part of Presets, Plan, Run, or exports.
_Avoid_: Quick start, launcher

**Quick Launch window**:
The miniature window opened from the tray icon with two tabs — Quick Launch (a single Start button that starts the whole Quick Launch list) and Quick Actions — for read-only, fast access. A third tab, **Quick Clips**, joins them once any Clip exists; until then the window stays a two-tab palette. It floats as a persistent window (stays open until closed with × / Alt+F4 — blur never hides it — and is freely draggable) or docks as a Win32 AppBar (the Quick Launch dock) on the left/right screen edge. It has no configuration surface; all configuration happens in the main app.
_Avoid_: Miniature window, palette, tray menu

**Quick Launch dock**:
The Quick Launch window's docked form — a little window that can be pinned [Win32 AppBar] to the left or right side of a screen. **Fixed** keeps a thin strip always visible and squeezes other windows. **Auto-hide** hides completely [off-screen, no handle] and slides in [~0.18 s ease-out] only when you push the mouse into that screen's outer wall and hold a moment — otherwise other windows keep their full size, even if the system says that edge is busy. Whether it is pinned or floating is remembered for each screen [per-monitor] and restored when Sprout starts — even on auto-start [ADR-0013]. Never part of Presets or exports.
_Avoid_: Sidebar, tray bar, launcher bar

**Display arrangement**:
How your screens sit together — each screen's rectangle [rcMonitor] placed on a big invisible canvas [virtual-screen] whose zero point is the main screen [primary]. The user can move screens, but they must touch at least a little; this layout is the single place Sprout looks to tell screens apart and to know which edges are real walls.
_Avoid_: Screen layout, monitor topology

**Monitor seam**:
The line where two screens touch by more than a tiny corner [>1 px overlap]. Your mouse slips straight across a seam to the other screen instead of stopping [passes through]. A single-corner diagonal touch [≤1 px] is not a seam — that corner is still a wall.
_Avoid_: Shared edge, monitor border

**Eligible edge (cursor-stop)**:
An edge that is a real wall [not a seam for its full side] — the mouse can stop there and the hidden dock can be called. Sprout only offers left and right; a middle line [ineligible seam] has no handle and the opener does nothing — the saved choice quietly moves to the other wall of that same screen next time you dock.
_Avoid_: Available edge, valid edge

**Auto-start**:
Sprout's registration to start with Windows: at login only the tray-resident backend starts — the main window never appears — restoring the Quick Launch dock when it was docked, staying tray-only when it was floating (or not yet docked). On by default in installed builds; toggled in Settings. Machine-local; never part of Presets or exports.
_Avoid_: Run at login, startup app, launch on boot

**Clip**:
A machine-local piece of plain text stored for one-click re-copying — click it in the Quick Clips tab or page and its content is back on the clipboard. Authored by hand (pasted from the machine's own clipboard into the add dialog); never captured automatically in the background. Ordered by the user; never part of Presets, Plan, or Run.
_Avoid_: Snippet, clipboard entry, paste item

**Quick Clips**:
The list of Clips and its two surfaces: a main-app page (create, edit, reorder, delete) and a read-only tab in the Quick Launch window that appears only once at least one Clip exists. Included whole-app backups (Settings export); never part of Presets, Plan, Run, or exports of Presets.
_Avoid_: Quick copy, clipboard manager

**Quick Action**:
A machine-local, user-authored named command (PowerShell, optional working directory) run from the Quick Launch window's Quick Actions tab (e.g. "docker start" → `docker compose up -d`); runs hidden as the current user with no elevation and no status UI. Optionally **stoppable**: while its process runs, the Run button becomes Stop, which either runs the action's own stop command (e.g. `docker compose stop`) or kills the process tree; tracking covers foreground commands only — detached commands (e.g. `-d`) report as not running because the process exits while the service continues. Configured in the main app's Quick Actions page; never part of Presets, Plan, Run, or exports.
_Avoid_: Action (a Plan term), command entry, script

**Note**:
Optional free-form text a user attaches to exactly one Quick Action, for whatever they want to record about it — the content and its purpose are the user's alone, and Sprout gives it no behavior (it never affects how the action runs). Short formatted text (bullets, numbered steps), authored in the main app and rendered read-only wherever shown; rows carrying one are marked so readers know the text exists before opening it. Machine-local; never part of Presets, Plan, Run, or exports.
_Avoid_: Comment, description, remark
