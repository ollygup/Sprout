import { invoke } from "@tauri-apps/api/core";
import type {
  ActiveRunInfo,
  AiCheckVerdict,
  AiDraftOutcome,
  BackupCounts,
  BackupImportSummary,
  BackupSelection,
  Clip,
  ClipImage,
  ClipInput,
  Composition,
  ImportResult,
  Group,
  GroupsCollection,
  LaunchCandidate,
  LaunchCommandTest,
  LaunchEntry,
  LaunchEntryInput,
  LaunchShell,
  LogLocations,
  PresetRecord,
  PreFixResult,
  Product,
  ProductPresetImpact,
  QuickAction,
  QuickActionFileMeta,
  QuickActionInput,
  QuickActionRunOutcome,
  QuickActionShell,
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

/** Writes one Quick Action to `path` as the unchanged backup document — a
 *  one-element `quick_actions` array with four empty siblings — so the file
 *  restores through the ordinary flow with honest counts. */
export function exportQuickAction(path: string, id: number): Promise<BackupCounts> {
  return invoke<BackupCounts>("export_quick_action", { path, id });
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

/** Starts Quick Launch entries through the capped, queued pipeline (the
 *  shared trigger for the Quick Launch window's and the page's Start
 *  buttons). Omitted ids keeps the full-list behavior; an explicit id
 *  subset starts exactly those saved entries in saved order (the main
 *  page's Start matching); an explicit empty list is rejected without
 *  launching anything. The page listens for the `launch-run-done` event
 *  and the summary arrives as a system notification. Rejected while a run
 *  is already in flight. */
export function startQuickLaunch(ids?: number[]): Promise<void> {
  return invoke<void>(
    "start_quick_launch",
    ids === undefined ? {} : { ids }
  );
}

/** Starts what the dock shows: the dock-visible subset only. The main-app
 *  Start-all keeps `startQuickLaunch` (every entry); each surface starts
 *  exactly what it lists. */
export function startDockQuickLaunch(): Promise<void> {
  return invoke<void>("start_dock_quick_launch");
}

/** Starts one Launch entry through the same pipeline as Start all (ticket
 * 93) — the Quick Launch window's clickable entry rows. Same single-flight
 * guard, event, and summary notification as the whole-list run. */
export function startLaunchEntry(id: number): Promise<void> {
  return invoke<void>("start_launch_entry", { id });
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
 *  `quick-action-run-state-changed` events. A configured pre-action check
 *  runs first: a pass continues to the tracked main command, a fail stops
 *  before anything spawns and resolves to the warn payload instead. */
export function runQuickAction(id: number): Promise<QuickActionRunOutcome> {
  return invoke<QuickActionRunOutcome>("run_quick_action", { id });
}

/** Runs one action's pre-action fix exactly once — only this explicit
 *  command runs it, never Run itself. The fix appends to the blocked run's
 *  log when that run's path echoes back intact, else to a fresh run folder.
 *  A fix never continues into the main command; that still needs a fresh
 *  Run. Refused when the action has no fix configured. */
export function runQuickActionFix(
  id: number,
  logPath: string | null
): Promise<PreFixResult> {
  return invoke<PreFixResult>("run_quick_action_fix", { id, logPath });
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

/** One Test click in the Quick Actions editor: runs the command under its
 * selected shell, timeboxed, and returns exit code + captured output. A
 * timed-out result means the command is interactive — not
 * headless-verifiable. */
export function testQuickAction(
  shell: QuickActionShell,
  command: string,
  cwd: string | null
): Promise<LaunchCommandTest> {
  return invoke<LaunchCommandTest>("test_quick_action", { shell, command, cwd });
}

// ------------------- Action files ------------------------------------------

/** The largest one attached file may hold: 5 MB of raw bytes. The dialog
 *  pre-checks it for instant feedback; the backend enforces it before
 *  anything is written. */
export const QUICK_ACTION_FILE_MAX_BYTES = 5 * 1024 * 1024;

/** The largest one action's files may total: the 20 MB v1 cap, pre-checked
 *  and enforced likewise. */
export const QUICK_ACTION_FILES_MAX_BYTES = 20 * 1024 * 1024;

/** Lists one action's attached files in name order: id, name, and size —
 *  never the bytes. Runs and backups read the bytes backend-side. */
export function listQuickActionFiles(id: number): Promise<QuickActionFileMeta[]> {
  return invoke<QuickActionFileMeta[]>("list_quick_action_files", { id });
}

/** Attaches one file to an action: plain file names only, unique per action,
 *  5 MB per file and 20 MB per action. The content arrives base64-encoded and
 *  is validated before anything reaches the disk. */
export function attachQuickActionFile(
  id: number,
  filename: string,
  bytesBase64: string
): Promise<QuickActionFileMeta> {
  return invoke<QuickActionFileMeta>("attach_quick_action_file", {
    id,
    filename,
    bytesBase64,
  });
}

/** Deletes one attached file row. An unknown id is a plain error, never a
 *  silent success. */
export function removeQuickActionFile(fileId: number): Promise<void> {
  return invoke<void>("remove_quick_action_file", { fileId });
}

/** One-line byte size for file rows — the row's mono metadata voice, the same
 *  shape the image-clip rows use. */
export function formatActionFileBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(bytes >= 10240 ? 0 : 1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** Requests one AI Script draft through the single configured route: a
 *  candidate, a refusal, a clarification, or an actionable failure.
 *  Generation never executes — running stays with the manual controls. */
export function aiGenerateDraft(
  request: string,
  shell: QuickActionShell,
  context: string | null,
  requestId: string,
): Promise<AiDraftOutcome> {
  return invoke<AiDraftOutcome>("ai_generate_draft", { request, shell, context, requestId });
}

export function aiCancelDraft(requestId: string): Promise<boolean> {
  return invoke<boolean>("ai_cancel_draft", { requestId });
}

export function aiManagedStatus(): Promise<import("./types").ManagedCatalogStatus> {
  return invoke<import("./types").ManagedCatalogStatus>("ai_managed_status");
}

export function aiInstallManaged(modelId: string): Promise<import("./types").ManagedInstallResult> {
  return invoke<import("./types").ManagedInstallResult>("ai_install_managed", { modelId });
}

export function aiCancelManagedInstall(): Promise<boolean> {
  return invoke<boolean>("ai_cancel_managed_install");
}

/** Tests an existing-local service without saving anything: classifies the
 *  endpoint and checks the named model against what the service exposes.
 *  Resolves to the exposed model names. */
export function aiCheckExistingLocal(baseUrl: string, model: string): Promise<string[]> {
  return invoke<string[]>("ai_check_existing_local", { baseUrl, model });
}

/** Rechecks a candidate accepted through AI assistance: the same output
 *  checks, no provider, no persistence, no execution. */
export function aiCheckCandidate(
  shell: QuickActionShell,
  command: string
): Promise<AiCheckVerdict> {
  return invoke<AiCheckVerdict>("ai_check_candidate", { shell, command });
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

// ------------------- Image Clips (ticket 178) ------------------------------

/** The largest image an image Clip accepts: 5 MB of raw bytes. The frontend
 *  pre-checks it for instant feedback; the backend enforces it. */
export const CLIP_IMAGE_MAX_BYTES = 5 * 1024 * 1024;

/** Appends an image-only Clip from pasted/picked bytes (raw base64, no
 *  data-URL prefix): name plus one PNG/JPEG picture. Over-cap or
 *  non-PNG-JPEG bytes are refused plainly before anything stores. */
export function createClipImage(
  name: string,
  dataBase64: string
): Promise<Clip> {
  return invoke<Clip>("create_clip_image", { name, dataBase64 });
}

/** Renames an image Clip / flips its dock flag in place. Text ids are
 *  refused plainly — text edits stay on `updateClip` untouched. */
export function updateClipImage(
  id: number,
  name: string,
  showInDock: boolean
): Promise<void> {
  return invoke<void>("update_clip_image", { id, name, showInDock });
}

/** Puts one image Clip's picture back on the clipboard through the same
 *  Rust-command-driven clipboard path text copies use (no new JS plugin
 *  surface beyond `invoke`). The pixels come from `decodeImageToRgba`;
 *  resolves only after the write landed, so the "Copied" flash never lies. */
export function copyClipImage(
  id: number,
  rgbaBase64: string,
  width: number,
  height: number
): Promise<void> {
  return invoke<void>("copy_clip_image", { id, rgbaBase64, width, height });
}

/** Renders one stored image as a same-document data URL for `<img>` —
 *  thumbnails and details decode nothing themselves. */
export function clipImageUrl(image: ClipImage): string {
  return `data:${image.mime};base64,${image.bytes_base64}`;
}

/** One-line picture summary for rows and tooltips — kind plus decoded
 *  dimensions plus byte size, in the row's mono metadata voice. */
export function clipImageMeta(image: ClipImage): string {
  const kind =
    image.mime === "image/png"
      ? "PNG"
      : image.mime === "image/jpeg"
        ? "JPEG"
        : image.mime;
  const dims =
    image.width > 0 && image.height > 0
      ? ` · ${image.width} × ${image.height}`
      : "";
  const bytes = Math.floor((image.bytes_base64.length * 3) / 4);
  return `${kind}${dims} · ${formatClipImageBytes(bytes)}`;
}

function formatClipImageBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(bytes >= 10240 ? 0 : 1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** Decodes a data URL to RGBA pixels via canvas — the one image decoder
 *  this tree already ships, so no new dependency decodes JPEG. Drawn from
 *  a data URL, the canvas is never tainted. Failures throw plainly for the
 *  caller's error line — never silent. */
export async function decodeImageToRgba(dataUrl: string): Promise<{
  rgbaBase64: string;
  width: number;
  height: number;
}> {
  try {
    const img = new Image();
    img.src = dataUrl;
    await img.decode();
    if (!img.naturalWidth || !img.naturalHeight) {
      throw new Error("empty image");
    }
    const canvas = document.createElement("canvas");
    canvas.width = img.naturalWidth;
    canvas.height = img.naturalHeight;
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    if (!ctx) throw new Error("no 2d context");
    ctx.drawImage(img, 0, 0);
    const data = ctx.getImageData(0, 0, canvas.width, canvas.height);
    const pixels = data.data;
    let binary = "";
    for (let i = 0; i < pixels.length; i += 0x8000) {
      binary += String.fromCharCode(...pixels.subarray(i, i + 0x8000));
    }
    return {
      rgbaBase64: btoa(binary),
      width: canvas.width,
      height: canvas.height,
    };
  } catch (e) {
    throw new Error(
      `That image couldn't be decoded for copying — ${e instanceof Error ? e.message : String(e)}`
    );
  }
}

// ------------------- Groups (ticket 89) ------------------------------------

/** Lists one collection's Groups in user order (ticket 89). */
export function listGroups(collection: GroupsCollection): Promise<Group[]> {
  return invoke<Group[]>("list_groups", { collection });
}

export function createGroup(
  collection: GroupsCollection,
  name: string
): Promise<Group> {
  return invoke<Group>("create_group", { collection, name });
}

export function renameGroup(id: number, name: string): Promise<void> {
  return invoke<void>("rename_group", { id, name });
}

/** Deleting a group returns its members to ungrouped — never deletes them. */
export function deleteGroup(id: number): Promise<void> {
  return invoke<void>("delete_group", { id });
}

export function moveGroup(id: number, toPosition: number): Promise<void> {
  return invoke<void>("move_group", { id, toPosition });
}

/** An item joins a group of its own collection only; cross-collection
 *  assignments are refused at the data layer. */
export function assignToGroup(
  collection: GroupsCollection,
  itemId: number,
  groupId: number
): Promise<void> {
  return invoke<void>("assign_to_group", { collection, itemId, groupId });
}

export function unassignFromGroup(
  collection: GroupsCollection,
  itemId: number
): Promise<void> {
  return invoke<void>("unassign_from_group", { collection, itemId });
}

/** One collection's Groups toggle (ticket 89); persisted per collection. */
export function updateGroupsEnabled(
  collection: GroupsCollection,
  enabled: boolean
): Promise<void> {
  return invoke<void>("update_groups_enabled", { collection, enabled });
}

/** Companion: set the active https URL (ticket 125) — null = off. */
export function setCompanionUrl(url: string | null): Promise<void> {
  return invoke<void>("set_companion_url", { url });
}

export function openCompanionExternal(url: string): Promise<void> {
  return invoke<void>("open_companion_external", { url });
}

/** Companion audio: the toolbar's volume-mixer shortcut — opens the OS
 *  per-app Volume mixer page at `ms-settings:apps-volume`. Fixed target. */
export function openVolumeMixer(): Promise<void> {
  return invoke<void>("open_volume_mixer");
}

/** Companion: set the height ratio (ticket 125) — 0.25–0.60. */
export function setCompanionHeightRatio(ratio: number): Promise<void> {
  return invoke<void>("set_companion_height_ratio", { ratio });
}

/** Companion: set the saved sites — each URL plus its display name.
 *  Duplicates are refused with a message naming what collided. */
export function setCompanionUrlList(sites: import("./types").CompanionSite[]): Promise<void> {
  return invoke<void>("set_companion_url_list", { sites });
}

/** Companion audio: persisted mute plus live playback for the dock toolbar.
 *  Healing a drifted WebView toward the persisted mute happens inside. */
export function getCompanionAudioState(): Promise<import("./types").CompanionAudioState> {
  return invoke<import("./types").CompanionAudioState>("get_companion_audio_state");
}

/** Companion audio: the dock toolbar's mute toggle — persists, pushes into
 *  the live WebView, and resolves with the fanned-out state. */
export function setCompanionMuted(muted: boolean): Promise<import("./types").CompanionAudioState> {
  return invoke<import("./types").CompanionAudioState>("set_companion_muted", { muted });
}

/** Companion history: the Back/Forward enable-state of the live native child
 *  (in-page website routing). Missing child reads as disabled. */
export function getCompanionHistoryState(): Promise<import("./types").CompanionHistoryState> {
  return invoke<import("./types").CompanionHistoryState>("get_companion_history_state");
}

/** Steps the live native child back through its in-page history; resolves
 *  with the fresh enable-state. */
export function companionGoBack(): Promise<import("./types").CompanionHistoryState> {
  return invoke<import("./types").CompanionHistoryState>("companion_go_back");
}

/** Steps the live native child forward through its in-page history; resolves
 *  with the fresh enable-state. */
export function companionGoForward(): Promise<import("./types").CompanionHistoryState> {
  return invoke<import("./types").CompanionHistoryState>("companion_go_forward");
}

/** Attaches the native history observers to the live child (idempotent) and
 *  resolves with the current enable-state. Observers forward every in-page
 *  navigation as `COMPANION_HISTORY_CHANGED_EVENT`. */
export function ensureCompanionHistoryHook(): Promise<import("./types").CompanionHistoryState> {
  return invoke<import("./types").CompanionHistoryState>("ensure_companion_history_hook");
}

/** The frontend event carrying fresh Back/Forward enable-state after any
 *  native in-page navigation. */
export const COMPANION_HISTORY_CHANGED_EVENT = "companion-history-changed";

/** Companion per-monitor height ratio (ticket 125) — falls back to global. */
export function getCompanionHeightRatio(display: string): Promise<number | null> {
  return invoke<number | null>("get_companion_height_ratio", { display });
}

export function setCompanionHeightRatioForDisplay(display: string, ratio: number): Promise<void> {
  return invoke<void>("set_companion_height_ratio_for_display", { display, ratio });
}

/** Companion mobile identity: Chromium on Android, so responsive sites serve
 *  their mobile layout to the narrow dock pane. */
export const COMPANION_MOBILE_UA =
  "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Mobile Safari/537.36";

/** Companion desktop identity (Edge variant): `Windows NT 10.0` covers
 *  current Windows releases (Win11 differs only via Client Hints); the
 *  Edge token first-classes sites that block generic mobile identities. */
export const COMPANION_DESKTOP_UA =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36 Edg/150.0.0.0";

/** One connected display (ticket 111): label, resolution, identity, and
 * wall eligibility from the live arrangement. */
export function listDisplays(): Promise<import("./types").DisplayInfo[]> {
  return invoke<import("./types").DisplayInfo[]>("list_displays");
}

export function getDisplayDockEdge(display: string): Promise<string | null> {
  return invoke<string | null>("get_display_dock_edge", { display });
}

export function setDisplayDockEdge(display: string, edge: string): Promise<void> {
  return invoke<void>("set_display_dock_edge", { display, edge });
}

export function getDisplayDockMode(display: string): Promise<string | null> {
  return invoke<string | null>("get_display_dock_mode", { display });
}

export function setDisplayDockMode(display: string, mode: string): Promise<void> {
  return invoke<void>("set_display_dock_mode", { display, mode });
}

/** The remembered dock width % for one display (ticket 128): null means no
 * override — the global default applies. */
export function getDisplayDockWidthPct(display: string): Promise<number | null> {
  return invoke<number | null>("get_display_dock_width_pct", { display });
}

/** Persists one display's dock width % (ticket 128): 10–30, validated. */
export function setDisplayDockWidthPct(display: string, pct: number): Promise<void> {
  return invoke<void>("set_display_dock_width_pct", { display, pct });
}

/** Applies the complete saved dock/Companion state after Settings batch writes. */
export function reconcileQuickLaunchSettings(): Promise<void> {
  return invoke<void>("reconcile_quick_launch_settings");
}

/** Opens (or focuses) the main window — the dock header's mark click
 * (ticket 123) and any future non-tray entry point. */
export function openMainWindow(): Promise<void> {
  return invoke<void>("open_main_window_cmd");
}

export function mainWindowReady(): Promise<void> {
  return invoke<void>("main_window_ready");
}

export function openSprout(): Promise<void> {
  return invoke<void>("open_sprout_cmd");
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

/** Scoped local target discovery (ADR-0031): approved folders hold
 *  names/paths permission only — never contents, never disclosure. */

/** Lists the folders approved for local target discovery. */
export function aiListApprovedRoots(): Promise<import("./types").AiApprovedRoot[]> {
  return invoke<import("./types").AiApprovedRoot[]>("ai_list_approved_roots");
}

/** Approves one folder for discovery: it must exist and is stored
 *  canonicalized. Names/paths only. */
export function aiApproveRoot(path: string): Promise<import("./types").AiApprovedRoot> {
  return invoke<import("./types").AiApprovedRoot>("ai_approve_root", { path });
}

/** Forgets one approved folder; later binds of its targets fail honestly. */
export function aiRevokeRoot(path: string): Promise<boolean> {
  return invoke<boolean>("ai_revoke_root", { path });
}

/** Runs one explicit find request over installed apps and approved folders:
 *  bounded, read-only, no execution. Returns request-scoped references. */
export function aiFindTargets(
  query: string,
  scope: import("./types").AiDiscoveryScope
): Promise<import("./types").AiFindOutcome> {
  return invoke<import("./types").AiFindOutcome>("ai_find_targets", { query, scope });
}

/** Reads one file match's bounded preview: a separate explicit request that
 *  stays untrusted input. */
export function aiReadTargetFile(refId: string): Promise<import("./types").AiFileContent> {
  return invoke<import("./types").AiFileContent>("ai_read_target_file", { refId });
}

/** Binds one validated reference to a shell-quoted command: reviewable
 *  text with its actual target, never an execution. */
export function aiBindTarget(
  refId: string,
  shell: QuickActionShell
): Promise<import("./types").AiBoundTarget> {
  return invoke<import("./types").AiBoundTarget>("ai_bind_target", { refId, shell });
}

/** Records approval to disclose raw fields of one reference to a provider.
 *  Discovery never implies this. */
export function aiApproveDisclosure(
  refId: string,
  fields: string[]
): Promise<import("./types").AiDisclosureGrant> {
  return invoke<import("./types").AiDisclosureGrant>("ai_approve_disclosure", { refId, fields });
}
