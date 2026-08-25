import { invoke } from "@tauri-apps/api/core";
import type {
  ActiveRunInfo,
  BackupCounts,
  BackupImportSummary,
  BackupSelection,
  Clip,
  ClipInput,
  Composition,
  ImportResult,
  LaunchCandidate,
  LaunchCommandTest,
  LaunchEntry,
  LaunchEntryInput,
  LaunchShell,
  LogLocations,
  PresetRecord,
  Product,
  ProductPresetImpact,
  QuickAction,
  QuickActionInput,
  QuickLaunchDockState,
  Requirement,
  RunProgressChunk,
  RunRecord,
  RunSummary,
  Settings,
  StartRunResult,
  UpdateCheck,
  VirtualDesktops,
  WingetMatch,
  WingetShow,
} from "./types";

export function listProducts(query: string | null): Promise<Product[]> {
  return invoke<Product[]>("list_products", { query });
}

export function createProduct(product: Product): Promise<void> {
  return invoke<void>("create_product", { product });
}

export function updateProduct(product: Product): Promise<void> {
  return invoke<void>("update_product", { product });
}

export function deleteProduct(id: string): Promise<void> {
  return invoke<void>("delete_product", { id });
}

/** The count behind the delete prompt: local Presets that reference the
 * Product and will lose its Requirement. */
export function productPresetImpact(id: string): Promise<ProductPresetImpact> {
  return invoke<ProductPresetImpact>("product_presets_impact", { id });
}

/** Live winget registry search for the product dialog's picker. */
export function searchWinget(query: string): Promise<WingetMatch[]> {
  return invoke<WingetMatch[]>("search_winget", { query });
}

/** One package's `winget show` details, enriching a picked match. */
export function showWinget(id: string): Promise<WingetShow> {
  return invoke<WingetShow>("show_winget", { id });
}

export function listPresets(): Promise<PresetRecord[]> {
  return invoke<PresetRecord[]>("list_presets");
}

export function createPreset(preset: PresetRecord): Promise<void> {
  return invoke<void>("create_preset", { preset });
}

export function updatePreset(preset: PresetRecord): Promise<void> {
  return invoke<void>("update_preset", { preset });
}

export function deletePreset(id: string): Promise<void> {
  return invoke<void>("delete_preset", { id });
}

export function exportPreset(path: string, presetId: string): Promise<void> {
  return invoke<void>("export_preset", { path, presetId });
}

export function importPreset(path: string): Promise<ImportResult> {
  return invoke<ImportResult>("import_preset", { path });
}

/** Writes one backup (ticket 80) to `path`, including only the selected
 *  collections (ticket 87) — unchecked ones are empty arrays in the same
 *  JSON file — and returns the per-collection counts for the notice. */
export function exportBackup(path: string, selection: BackupSelection): Promise<BackupCounts> {
  return invoke<BackupCounts>("export_backup", { path, selection });
}

/** Parses a whole-app backup without writing anything (ticket 80): the
 *  parsed counts shown in the restore confirmation before the user commits. */
export function inspectBackup(path: string): Promise<BackupCounts> {
  return invoke<BackupCounts>("inspect_backup", { path });
}

/** Restores a whole-app backup (ticket 80): a transactional merge that skips
 *  identities which already exist. Returns inserted/skipped per collection. */
export function importBackup(path: string): Promise<BackupImportSummary> {
  return invoke<BackupImportSummary>("import_backup", { path });
}

export function takePendingImport(): Promise<string | null> {
  return invoke<string | null>("take_pending_import");
}

export function computePlan(presetIds: string[]): Promise<Composition> {
  return invoke<Composition>("compute_plan", { presetIds });
}

/** Quick install's Plan half (ticket 21): the single synthetic Requirement
 * synthesized from a Library Product, planned against this machine and
 * labeled "Quick install — {product}" so the run History carries it. */
export function quickInstallPlan(productId: string): Promise<Composition> {
  return invoke<Composition>("quick_install_plan", { productId });
}

/** Hands the Plan to the elevated worker: writes the run request, relaunches
 * this exe as `--worker` under one UAC prompt, and returns the run id to
 * tail. */
export function startRun(
  presetNames: string[],
  requirements: Requirement[]
): Promise<StartRunResult> {
  return invoke<StartRunResult>("start_run", { presetNames, requirements });
}

/** Tails the worker's JSON-lines status file from `offset`. */
export function readRunProgress(
  runId: string,
  offset: number
): Promise<RunProgressChunk> {
  return invoke<RunProgressChunk>("read_run_progress", { runId, offset });
}

/** Asks the worker to stop after the current step. */
export function cancelRun(runId: string): Promise<void> {
  return invoke<void>("cancel_run", { runId });
}

/** The run-active query (ticket 18): whether a run is in progress right now
 * and which one — the layout banner's source of truth, from any page. */
export function getActiveRun(): Promise<ActiveRunInfo | null> {
  return invoke<ActiveRunInfo | null>("get_active_run");
}

/** Loads the persisted Run the worker wrote, for the summary screen. */
export function getRun(runId: string): Promise<RunRecord | null> {
  return invoke<RunRecord | null>("get_run", { runId });
}

/** Lists every Run's summary row, newest first — the History screen. */
export function listRuns(): Promise<RunSummary[]> {
  return invoke<RunSummary[]>("list_runs");
}

/** Loads the persisted knobs (default timeout, log retention). */
export function getSettings(): Promise<Settings> {
  return invoke<Settings>("get_settings");
}

/** Persists the Settings screen's knobs. */
export function updateSettings(settings: Settings): Promise<void> {
  return invoke<void>("update_settings", { settings });
}

/** Persists the theme on its own — it applies the moment it is picked. */
export function updateTheme(theme: string): Promise<void> {
  return invoke<void>("update_theme", { theme });
}

/** Persists the auto-start preference and reconciles the Windows Run-key
 *  registration beside the save (ticket 75) — effective immediately. */
export function updateAutostart(enabled: boolean): Promise<void> {
  return invoke<void>("update_autostart", { enabled });
}

/** Persists the desktop-assignments toggle (ticket 88) on its own — the next
 *  Quick Launch run obeys it, and a live window is told via
 *  `quick-launch-changed`. */
export function updateDesktopAssignments(enabled: boolean): Promise<void> {
  return invoke<void>("update_desktop_assignments", { enabled });
}

/** The Logs screen's picture of where logs live and how big they are. */
export function listLogs(): Promise<LogLocations> {
  return invoke<LogLocations>("list_logs");
}

/** The Logs screen's open-folder action: reveals a path in Explorer. */
export function openFolder(path: string): Promise<void> {
  return invoke<void>("open_folder", { path });
}

export function listLaunchEntries(): Promise<LaunchEntry[]> {
  return invoke<LaunchEntry[]>("list_launch_entries");
}

export function createLaunchEntry(entry: LaunchEntryInput): Promise<LaunchEntry> {
  return invoke<LaunchEntry>("create_launch_entry", { entry });
}

export function updateLaunchEntry(entry: LaunchEntry): Promise<void> {
  return invoke<void>("update_launch_entry", { entry });
}

export function deleteLaunchEntry(id: number): Promise<void> {
  return invoke<void>("delete_launch_entry", { id });
}

export function moveLaunchEntry(id: number, toPosition: number): Promise<void> {
  return invoke<void>("move_launch_entry", { id, toPosition });
}

/** One Test click in the add-command dialog (ticket 41): runs the command
 * entry under its shell, timeboxed, and returns the exit code + captured
 * output. A timed-out result means the command is interactive — not
 * headless-verifiable. */
export function testLaunchCommand(
  shell: LaunchShell,
  target: string
): Promise<LaunchCommandTest> {
  return invoke<LaunchCommandTest>("test_launch_command", { shell, target });
}

/** Starts the whole Quick Launch list (ticket 42): capped, queued, on a
 * background thread — the shared trigger for the Quick Launch window's and
 * the page's Start buttons (ticket 54). The page listens for the
 * `launch-run-done` event and the summary arrives as a system notification.
 * Rejected while a run is already in flight. */
export function startQuickLaunch(): Promise<void> {
  return invoke<void>("start_quick_launch");
}

/** The fresh installed-app snapshot behind the Quick Launch search (ticket
 * 39): Start Menu shortcuts + uninstall-registry entries, re-walked on every
 * call — never cached. The frontend filters the returned list locally. */
export function listLaunchCandidates(): Promise<LaunchCandidate[]> {
  return invoke<LaunchCandidate[]>("list_launch_candidates");
}

/** The icon for one search candidate, as a PNG data URL (ticket 40): fetched
 * lazily for visible rows only, held in memory — never cached to disk.
 * `null` when the target has no icon. */
export function candidateIcon(target: string): Promise<string | null> {
  return invoke<string | null>("candidate_icon", { target });
}

/** The virtual-desktop assignment surface (ticket 44): every desktop with
 * its label, plus the gate. `supported` false below Windows 11 24H2 hides
 * the whole grouping UI. */
export function listVirtualDesktops(): Promise<VirtualDesktops> {
  return invoke<VirtualDesktops>("list_virtual_desktops");
}

/** Creates a virtual desktop on the user's behalf (ticket 44); `null` when
 * unsupported or the OS refused. */
export function createVirtualDesktop(): Promise<string | null> {
  return invoke<string | null>("create_virtual_desktop");
}

export function listQuickActions(): Promise<QuickAction[]> {
  return invoke<QuickAction[]>("list_quick_actions");
}

export function createQuickAction(action: QuickActionInput): Promise<QuickAction> {
  return invoke<QuickAction>("create_quick_action", { action });
}

export function updateQuickAction(action: QuickAction): Promise<void> {
  return invoke<void>("update_quick_action", { action });
}

export function deleteQuickAction(id: number): Promise<void> {
  return invoke<void>("delete_quick_action", { id });
}

export function moveQuickAction(id: number, toPosition: number): Promise<void> {
  return invoke<void>("move_quick_action", { id, toPosition });
}

/** Runs one stored Quick Action (tickets 50 & 62): hidden PowerShell,
 *  working directory honored, current user, no elevation, no status UI. The
 *  run is tracked for its lifetime — the window learns Run ↔ Stop through
 *  `quick-action-run-state-changed` events. */
export function runQuickAction(id: number): Promise<void> {
  return invoke<void>("run_quick_action", { id });
}

/** Stops a running Quick Action (ticket 62): runs its stop command when it
 *  has one, otherwise kills the process tree. */
export function stopQuickAction(id: number): Promise<void> {
  return invoke<void>("stop_quick_action", { id });
}

/** The ids of every Quick Action whose tracked process is still alive
 *  (ticket 62) — the window's starting picture; events keep it current. */
export function listRunningQuickActions(): Promise<number[]> {
  return invoke<number[]>("list_running_quick_actions");
}

/** One Test click in the Quick Actions editor (ticket 50): runs the command
 * under PowerShell, timeboxed, and returns exit code + captured output. A
 * timed-out result means the command is interactive — not
 * headless-verifiable. */
export function testQuickAction(
  command: string,
  cwd: string | null
): Promise<LaunchCommandTest> {
  return invoke<LaunchCommandTest>("test_quick_action", { command, cwd });
}

/** Lists every Clip in order (ticket 78). */
export function listClips(): Promise<Clip[]> {
  return invoke<Clip[]>("list_clips");
}

export function createClip(clip: ClipInput): Promise<Clip> {
  return invoke<Clip>("create_clip", { clip });
}

export function updateClip(clip: Clip): Promise<void> {
  return invoke<void>("update_clip", { clip });
}

export function deleteClip(id: number): Promise<void> {
  return invoke<void>("delete_clip", { id });
}

export function moveClip(id: number, toPosition: number): Promise<void> {
  return invoke<void>("move_clip", { id, toPosition });
}

/** Puts one stored Clip's content back on the clipboard (ticket 78). Resolves
 *  only after the write landed, so a "Copied" flash never lies. */
export function copyClip(id: number): Promise<void> {
  return invoke<void>("copy_clip", { id });
}

/** The Quick Launch dock's toggle (ticket 53): docks the window to its
 * current monitor's remembered (or Settings-default) edge, or undocks back
 * to the floating window when already docked. */
export function toggleQuickLaunchDock(): Promise<void> {
  return invoke<void>("toggle_quick_launch_dock");
}

/** The left↔right edge-switch arrows (ticket 53): moves the docked window to
 * the given edge without unregistering the AppBar. */
export function switchQuickLaunchDockEdge(edge: string): Promise<void> {
  return invoke<void>("switch_quick_launch_dock_edge", { edge });
}

/** The dock chrome's state query (tickets 53 & 59): the current edge and
 * mode when docked, or — while the window floats — the target edge/mode the
 * toggle would dock to; `docked` tells the two apart. */
export function getQuickLaunchDockState(): Promise<QuickLaunchDockState> {
  return invoke<QuickLaunchDockState>("get_quick_launch_dock_state");
}

/** Checks GitHub Releases for a newer Sprout (ADR-0012). Runs on the
 * backend's blocking pool; per the silent-failure contract every failure
 * resolves to `update: null` rather than an error. */
export function checkForUpdate(): Promise<UpdateCheck> {
  return invoke<UpdateCheck>("check_for_update");
}

/** The user-confirmed apply step (ADR-0012): streams the setup exe to
 * %TEMP%, spawns it passively, and exits the app shortly after so NSIS can
 * replace it and relaunch. Failures here are reported — the user asked. */
export function installUpdate(url: string): Promise<void> {
  return invoke<void>("install_update", { url });
}
