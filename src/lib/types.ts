export type EnvAction = "set" | "prepend";

export interface EnvWiring {
  action: EnvAction;
  name: string;
  value: string;
}

export interface Product {
  id: string;
  name: string;
  winget_id: string | null;
  install_location_hint: string | null;
  /** Per-product override of the global default install directory (ticket
   * 36): empty means "use the Settings default". Machine-local — never
   * exported, never shared. */
  install_dir: string | null;
  default_env: EnvWiring[];
  /** Library-only metadata (ticket 13): present on products read from the
   * library, null on write payloads. */
  created_at: number | null;
  updated_at: number | null;
}

export const envActionLabel: Record<EnvAction, string> = {
  set: "SET",
  prepend: "PREPEND",
};

/** One row of the live winget registry search (ticket 13). */
export interface WingetMatch {
  id: string;
  name: string;
  publisher: string | null;
  version: string | null;
  source: string | null;
  match_kind: string | null;
}

/** One package's `winget show` details, enriching a picked match. */
export interface WingetShow {
  id: string;
  name: string | null;
  publisher: string | null;
  version: string | null;
  source: string | null;
  moniker: string | null;
}

export type Step =
  | { type: "winget"; id: string; scope: string }
  | { type: "command"; exe: string; args: string[]; success_codes: number[] };

export type VersionPolicy =
  | { kind: "latest" }
  | { kind: "pinned"; version: string }
  | { kind: "present" };

export const policyLabel: Record<VersionPolicy["kind"], string> = {
  latest: "latest",
  pinned: "pinned",
  present: "present",
};

export interface VerifyCommand {
  command: string;
  args: string[];
  match_text: string | null;
}

export interface Requirement {
  product: Product;
  step: Step;
  version_policy: VersionPolicy;
  depends_on: string[];
  timeout_minutes: number;
  env: EnvWiring[];
  verify: VerifyCommand[];
  /** The product left the library (ADR-0007): this requirement is a dangling
   * live reference. It is excluded from runs and shown as "product removed
   * from library". Never persisted; recomputed from the library at read. */
  unresolved?: boolean;
}

export interface Preset {
  schema_version: number;
  platform: string;
  name: string;
  description: string;
  author: string;
  version: string;
  requirements: Requirement[];
}

export interface PresetRecord extends Preset {
  id: string;
  imported: boolean;
}

export interface ImportResult {
  preset: PresetRecord;
  warning: string | null;
}

export type PlannedAction =
  | { kind: "install" }
  | { kind: "upgrade"; from: string; to: string }
  | { kind: "already_ok" }
  | { kind: "satisfied_by_newer"; installed: string; pinned: string }
  | { kind: "unmanaged_skip" };

export const actionLabel: Record<PlannedAction["kind"], string> = {
  install: "will install",
  upgrade: "will upgrade",
  already_ok: "already OK",
  satisfied_by_newer: "satisfied by newer",
  unmanaged_skip: "unmanaged — skip",
};

/** One way a selected Preset declares a Product, with the action that
 * declaration would produce on this machine. */
export interface Candidate {
  preset: string;
  requirement: Requirement;
  action: PlannedAction;
  detail: string;
}

/** One row of the Plan: a Product as declared by the selected Presets. */
export interface PlanEntry {
  product_id: string;
  product_name: string;
  conflict: boolean;
  candidates: Candidate[];
  sources: string[];
  merged: Requirement;
  /** All declarations of this product reference products that left the
   * library (ADR-0007): the row shows "removed from library" and is excluded
   * from the run. */
  unresolved?: boolean;
}

/** The delete prompt's impact: how many local Presets reference a Product
 * and will lose its Requirement. */
export interface ProductPresetImpact {
  preset_count: number;
}

/** The full read-only Plan for a selection of Presets. */
export interface Composition {
  preset_names: string[];
  entries: PlanEntry[];
}

export type RunStatus =
  | "installed"
  | "upgraded"
  | "already_ok"
  | "satisfied_by_newer"
  | "skipped_unmanaged"
  | "failed"
  | "timed_out";

export const runStatusLabel: Record<RunStatus, string> = {
  installed: "installed",
  upgraded: "upgraded",
  already_ok: "already OK",
  satisfied_by_newer: "satisfied by newer",
  skipped_unmanaged: "skipped — unmanaged",
  failed: "failed",
  timed_out: "timed out",
};

/** One Requirement's outcome inside a Run. */
export interface RequirementOutcome {
  product_id: string;
  product_name: string;
  status: RunStatus;
  detail: string;
  reboot_required: boolean;
  log_path: string;
}

export type RunOutcome = "ok" | "with_notes" | "failed" | "cancelled";

/** The four honest outcome labels (ticket 16) — text always carries the
 * meaning; the Notion status colors only reinforce it. */
export const runOutcomeLabel: Record<RunOutcome, string> = {
  ok: "Applied",
  with_notes: "With notes",
  cancelled: "Cancelled",
  failed: "Failed",
};

/** One application of a Plan, persisted with per-Requirement outcomes. */
export interface RunRecord {
  id: string;
  started_at: number;
  finished_at: number;
  preset_names: string[];
  outcome: RunOutcome;
  results: RequirementOutcome[];
}

/** One JSON-lines event the elevated worker appends to the per-run status
 * file; the UI tails it for live progress. */
export type RunProgress =
  | { type: "phase"; phase: string }
  | {
      type: "requirement_started";
      index: number;
      total: number;
      product_id: string;
      product_name: string;
      action: string;
    }
  | ({
      type: "requirement_finished";
    } & RequirementOutcome)
  | { type: "run_finished"; outcome: RunOutcome };

/** The worker's completion marker, once it has written it. */
export interface RunDoneInfo {
  outcome: RunOutcome;
  error: string | null;
}

/** The run-active query's answer (ticket 18): which run is live right now,
 * plus its completion marker when it just finished and no UI has surfaced
 * it yet. */
export interface ActiveRunInfo {
  run_id: string;
  done: RunDoneInfo | null;
}

/** One tail of the per-run status file. */
export interface RunProgressChunk {
  events: RunProgress[];
  offset: number;
  done: RunDoneInfo | null;
}

/** A Run handed off to the elevated worker. */
export interface StartRunResult {
  run_id: string;
}

/** One row of the Runs list: everything the History screen shows before a
 * run is reopened. */
export interface RunSummary {
  id: string;
  started_at: number;
  finished_at: number;
  preset_names: string[];
  outcome: RunOutcome;
}

/** The Settings screen's persisted knobs. */
export interface Settings {
  default_timeout_minutes: number;
  log_retention_days: number;
  /** "system" | "light" | "dark" — the app-wide theme (ticket 31). */
  theme: string;
  /** Machine-local default install directory (ticket 34, ADR-0009): "" means
   * winget's own default; otherwise an absolute Windows path like D:\Apps.
   * Never exported with presets. */
  install_dir: string;
  /** Quick Launch concurrency cap (ticket 38): how many Launch entries may
   * be in flight at once before the rest queue. */
  launch_concurrency: number;
  /** The Quick Launch dock's visibility mode (tickets 49/50): "auto-hide"
   * slides to a sliver when not hovered; "fixed" keeps the strip
   * permanently reserved. */
  dock_mode: string;
  /** The screen edge the Quick Launch dock attaches to by default (tickets
   * 49/50): "left" or "right". */
  dock_edge: string;
  /** The Quick Launch window's dock state (ticket 57): "floating" or
   *  "docked" — what the window reopens as, and what the in-window dock
   *  toggle writes back. */
  dock_state: string;
  /** The docked strip's width as % of its monitor (ticket 128): 10–30,
   *  default 18. Docked only — floating stays 340 — shared by fixed and
   *  auto-hide, with per-monitor memory falling back here. */
  dock_width_pct: number;
  /** The Quick Launch window's list density: "compact", "default", or
   *  "large" (default). Rescales the docked and floating lists only — the
   *  main app keeps its own sizing. */
  dock_density: string;
  /** Whether Sprout starts with Windows (ticket 75): "on" or "off" — the
   *  Run-key registration is reconciled beside every toggle. */
  autostart: string;
  /** Whether each list page offers its Groups feature (ticket 89): "on" or
   *  "off", default off per collection. Off is fully dormant — flat list, no
   *  group affordances — while stored groups and memberships survive for
   *  re-enabling. */
  launch_groups: string;
  action_groups: string;
  clip_groups: string;
  /** Reveal dwell (ticket 113): ms the cursor must hold in the sliver after
   *  accumulating threshold before the dock reveals. 0 is immediate. */
  reveal_dwell_ms: number;
  /** Reveal sensitivity threshold (ticket 113): px of toward-edge travel
   *  inside the sliver required before dwell starts. 0 needs no push. */
  reveal_sensitivity_px: number;
  /** Companion active URL (ticket 125): https URL or null (off) — machine-local.
   *  When null the dock shows no pane, no splitter, no header button. */
  companion_url: string | null;
  /** Companion height ratio (ticket 125): 0.25–0.60, default 0.40 — bottom fraction
   *  of the dock occupied by the web view. Per-monitor memory falls back here. */
  companion_height_ratio: number;
  /** Companion saved URL list: https URLs edited in the main app, each
   *  with the user's display name for it (blank = render the URL).
   *  Deduped on the URL trimmed case-insensitive. Machine-local. */
  companion_url_list: CompanionSite[];
  /** Companion mute (global, persisted): the dock toolbar's mute toggle writes
   *  it; the live WebView heals toward it on every read. Default unmuted. */
  companion_muted: boolean;
  /** AI assistance route (ADR-0031): "off" (default), "existing-local" (the
   *  user's own loopback service), "managed", or "cloud". Managed and cloud
   *  save as discoverable selections; generation through them fails closed
   *  until their own slices land. Machine-local, never backed up. */
  ai_provider: string;
  /** The existing-local service root, e.g. http://127.0.0.1:11434.
   *  Loopback HTTP only. */
  ai_base_url: string;
  /** The exact model name the local service exposes. Never substituted. */
  ai_model: string;
}

/** One Companion saved site: its https URL plus the user's display name for
 *  it. A blank name renders as the URL everywhere. `ua` is the site's browser
 *  identity — "mobile" (default) or "desktop" for desktop-only sites;
 *  missing/legacy reads as mobile. `zoom` is the user's explicit page zoom as
 *  a factor — absent/null means the width-derived auto zoom. */
export interface CompanionSite {
  url: string;
  name: string;
  ua?: "mobile" | "desktop";
  zoom?: number | null;
}

/** The dock Companion toolbar's audio picture: persisted mute plus live
 *  playback from the WebView. */
export interface CompanionAudioState {
  muted: boolean;
  playing: boolean;
}

/** The dock Companion toolbar's Back/Forward picture: what the live native
 *  child can step to through its in-page history. Missing child reads as
 *  disabled — never hidden. */
export interface CompanionHistoryState {
  can_go_back: boolean;
  can_go_forward: boolean;
}

/** Which collection a Group buckets (ticket 89) — the discriminator that
 *  keeps the three namespaces apart at the data layer. */
export type GroupsCollection = "launch" | "action" | "clip";

/** A user-named bucket within exactly one collection (ticket 89). Items hold
 *  at most one group; deleting a group returns members to ungrouped. */
export interface Group {
  id: number;
  collection: GroupsCollection;
  name: string;
}

/** What a Launch entry starts: a picked app (shortcut or exe) or a command
 * the user wrote (ticket 38). */
export type LaunchEntryKind = "app" | "command";

/** The shell a command entry runs under; null for app entries. */
export type LaunchShell = "powershell" | "cmd" | "none";

/** The editable shape of a Launch entry, as sent to the backend. */
export interface LaunchEntryInput {
  name: string;
  kind: LaunchEntryKind;
  /** App entries: the .lnk or exe path. Command entries: the command line. */
  target: string;
  /** Command entries only; null for app entries. */
  shell: LaunchShell | null;
  /** Command entries only: hidden by default, optional visible window. */
  show_window: boolean;
  /** Target virtual desktop GUID (ticket 44); null = current desktop. */
  desktop_id: string | null;
  /** Whether the dock lists this entry. Main-app lists and its Start-all see
   *  every entry; the dock filters on this. Missing (legacy) means visible. */
  show_in_dock: boolean;
}

/** A Launch entry as stored: the input plus its library id. `group_id` is
 *  its optional Group membership (ticket 89) — assignments go through the
 *  groups commands, never the edit payload. */
export interface LaunchEntry extends LaunchEntryInput {
  id: number;
  group_id: number | null;
}

/** The result of one Test click in the add-command dialog (ticket 41): exit
 * code + merged output of the timeboxed run. `timed_out` is honest — an
 * interactive command that outlives the box is not headless-verifiable,
 * never passed. */
export interface LaunchCommandTest {
  timed_out: boolean;
  exit_code: number | null;
  output: string;
}

/** How the shell choices read in the add-command dialog (ticket 41). */
export const launchShellLabel: Record<LaunchShell, string> = {
  powershell: "PowerShell",
  cmd: "cmd",
  none: "direct exe",
};

/** The outcome of one Quick Launch run (ticket 42): entry names grouped by
 * fate — started, skipped (with the reason, ticket 48: "Command Prompt —
 * already open on this desktop"), or failed (with the reason when the entry
 * failed before launch, ticket 48). */
export interface LaunchReport {
  started: string[];
  skipped: string[];
  failed: string[];
  /** Desktop-assignment notes (ticket 44): an entry whose desktop no longer
   * exists opened on the current desktop, and the note says so. */
  notes: string[];
}

/** One virtual desktop the assignment menu offers (ticket 44). */
export interface VirtualDesktop {
  /** The desktop's GUID — stable across Task View reorder, which is why
   * assignments reference it. */
  id: string;
  /** The Windows name when the desktop has one; "Desktop N" otherwise. */
  name: string;
  /** Whether this is the desktop the user is on right now — lets the
   * submenu offer "pin to here" as an explicit assignment (ADR-0015 round,
   * ticket 105). */
  current: boolean;
}

/** The shell a Quick Action runs under: explicit PowerShell or CMD. */
export type QuickActionShell = "powershell" | "cmd";

/** How the Quick Action shell choices read in the add/edit dialog. */
export const quickActionShellLabel: Record<QuickActionShell, string> = {
  powershell: "PowerShell",
  cmd: "cmd",
};

/** The editable shape of a Quick Action: a named shell command with an
 *  optional working directory, run from the Quick Launch window's Quick
 *  Actions tab. Machine-local — never part of Presets, Plan, Run, or
 *  exports. */
export interface QuickActionInput {
  name: string;
  /** The shell the command runs under — explicit since the PowerShell/CMD
   *  extension. Legacy records without one read back as PowerShell. */
  shell: QuickActionShell;
  /** The shell script, multi-line allowed. */
  command: string;
  /** Working directory the command starts in; null = the app's own. */
  cwd: string | null;
  /** Whether the window shows a Stop button while the action runs
   *  (ticket 62); false keeps the fire-and-forget behavior. */
  stoppable: boolean;
  /** Runs when Stop is clicked; null/empty = kills the process tree. */
  stop_command: string | null;
  /** Optional free-form note (ticket 117): formatted text for whatever the
   *  writer wants. Trimmed on save; empty/whitespace-only => null. */
  note?: string | null;
  /** Runs once per Sprout start, in list order, as if Run were clicked.
   *  Machine-local — carried by whole-app backup, never by Presets or
   *  exports. Default off. */
  auto_run: boolean;
  /** Whether the dock lists this action. Main-app lists see every action;
   *  the dock filters on this. Missing (legacy) means visible. */
  show_in_dock: boolean;
  /** The pre-action check: a shell command run first on every Run, under the
   *  action's own shell and working directory inside a short timebox — exit 0
   *  lets the main command run, anything else blocks it before anything
   *  spawns. Trimmed on save; empty becomes null so an unused section leaves
   *  no trace. Absent (legacy) means no check. */
  pre_check?: string | null;
  /** The pre-action fix: offered only when the check blocks a run, and run
   *  only through its explicit command — never as part of Run itself.
   *  Meaningless without a check, so saving one alone is refused. Same
   *  trim-empty-to-null rule as the check. */
  pre_fix?: string | null;
}

/** A Quick Action as stored: the input plus its library id. `group_id` is
 *  its optional Group membership (ticket 89). */
export interface QuickAction extends QuickActionInput {
  id: number;
  group_id: number | null;
}

/** One run-state change for a tracked Quick Action (ticket 62): emitted on
 *  start and again when the process exits, so the window flips Run ↔ Stop
 *  with no polling. */
export interface QuickActionRunState {
  id: number;
  running: boolean;
}

/** What one pre-action check decided: a pass lets the main command run, a
 *  fail blocks it before anything spawns. The blocking outcome is the warn
 *  dialog's payload — the trimmed check output plus whether a fix exists to
 *  offer — so the dialog never re-reads the database. */
export interface PreCheckReport {
  passed: boolean;
  output: string;
  timed_out: boolean;
  exit_code: number | null;
  duration_ms: number;
  has_fix: boolean;
}

/** What one explicit fix run decided. Reported back to its caller; the main
 *  command still needs a fresh Run — a fix never continues into main on its
 *  own. */
export interface PreFixResult {
  exit_code: number | null;
  timed_out: boolean;
  output: string;
  duration_ms: number;
}

/** What one Run click decided: the main command started and is tracked, or
 *  the pre-action check blocked it before anything spawned. The blocked
 *  variant carries the warn payload — the check report plus the run log that
 *  already holds the pre-check section (its path echoes back to the fix
 *  command so the fix appends to the same file). */
export type QuickActionRunOutcome =
  | { outcome: "started" }
  | ({ outcome: "check_blocked" } & PreCheckReport & {
      log_path: string | null;
    });

/** One file attached to a Quick Action: identity plus name and size as
 *  listed. The bytes travel only into the per-run staging directory and
 *  backups — the list, the editor autocomplete, and the export all share
 *  this shape and never the bytes. */
export interface QuickActionFileMeta {
  id: number;
  action_id: number;
  filename: string;
  size: number;
}

/** A machine-local plain-text Clip (ticket 78), hand-authored for one-click
 *  re-copying. Machine-local — never part of Presets, Plan, or Preset
 *  exports; included in whole-app backups. */
export interface ClipInput {
  /** Display name; "" when untitled — surfaces fall back to the content's
   *  first line so the list stays readable without invented names. */
  name: string;
  /** The text a copy puts back on the clipboard. Non-empty after trim for
   *  text Clips; blank by design for image Clips (ticket 178). */
  content: string;
  /** Whether the dock lists this clip. The main-app page sees every clip;
   *  the dock filters on this. Missing (legacy) means visible. */
  show_in_dock: boolean;
  /** The attached picture for an image-only Clip (ticket 178); absent for
   *  text Clips. Skipped in serialization when absent so text-clip backups
   *  keep their exact shape; defaulted on read so text-only backups parse. */
  image?: ClipImage | null;
}

/** One picture attached to an image-only Clip (ticket 178): normalized
 *  PNG/JPEG bytes (base64) plus the metadata lists and details render
 *  without re-decoding. `hash` is the bytes identity the backup merge keys
 *  on — the name is display-only. */
export interface ClipImage {
  /** Canonical mime: `image/png` or `image/jpeg` (backend-sniffed). */
  mime: string;
  /** The normalized raw bytes. */
  bytes_base64: string;
  /** Decoded pixel dimensions at ingest. */
  width: number;
  height: number;
  /** Lowercase hex identity hash over the raw bytes. */
  hash: string;
}

/** A Clip as stored: the input plus its library id. `group_id` is its
 *  optional Group membership (ticket 89). */
export interface Clip extends ClipInput {
  id: number;
  group_id: number | null;
}

/** The assignment surface's gate + list (ticket 44): `supported` is false
 * below Windows 11 24H2 (or when winvd failed), which hides the whole
 * grouping surface — the page's labels and assignments. */
export interface VirtualDesktops {
  supported: boolean;
  desktops: VirtualDesktop[];
}

/** One app the installed-app search found (ticket 39): display name, publisher
 * when known, the launchable target (shortcut or exe), and the resolved exe
 * path where determinable. */
export interface LaunchCandidate {
  name: string;
  publisher: string | null;
  target: string;
  exe_path: string | null;
}

/** One browsable log location: a run folder. */
export interface LogEntry {
  name: string;
  path: string;
  size_bytes: number;
  modified_at: number | null;
}

/** The Logs screen's picture of where logs live and how big they are. */
export interface LogLocations {
  data_dir: string;
  logs_dir: string;
  db_path: string;
  db_size_bytes: number;
  total_logs_bytes: number;
  runs: LogEntry[];
  /** One entry per Quick Action run folder, newest first (ticket 64). */
  quick_action_runs: LogEntry[];
  /** One entry per Quick Launch run folder, newest first (ticket 77). */
  quick_launch_runs: LogEntry[];
}

/** The Quick Launch dock's live state (tickets 53 & 59): the edge and
 * visibility mode the window is docked with — or, while it floats, the edge
 * and mode the toggle would dock to — plus whether the window is currently
 * docked. `blocked` (ticket 63) carries the shell's refusal reason when
 * auto-hide could not engage ("another auto-hide bar already owns this
 * edge"): transient, only ever set while docked, cleared as soon as the edge
 * frees up. The header renders it as the warning banner. */
export interface QuickLaunchDockState {
  edge: "left" | "right";
  mode: "auto-hide" | "fixed";
  docked: boolean;
  blocked: string | null;
  left_eligible: boolean;
  right_eligible: boolean;
  /** The device the live dock is attached to — null while floating, where
   *  per-monitor memory does not apply. */
  monitor: string | null;
  /** The dock monitor's hardware identity when one resolved at dock time. */
  monitor_identity: string | null;
}

/** One connected display (ticket 111): label, resolution, EDID identity
 * (when resolvable), and wall eligibility per edge via the single geometry
 * source. */
export interface DisplayInfo {
  device_name: string;
  identity: string | null;
  id: string;
  label: string;
  width: number;
  height: number;
  resolution: string;
  x: number;
  y: number;
  left_eligible: boolean;
  right_eligible: boolean;
}

/** A newer Sprout release (ADR-0012): the display version (tag stripped of
 * its `v`) and the setup-exe download URL. Also the payload of the one-shot
 * startup `update-available` event. */
export interface AvailableUpdate {
  version: string;
  url: string;
}

/** One update check's answer: the running build's version plus a newer
 * release when one exists. Per the silent-failure contract every check
 * failure — offline, private repo, malformed payload — resolves to
 * `update: null`, indistinguishable from up to date. */
export interface UpdateCheck {
  current_version: string;
  update: AvailableUpdate | null;
}

/** Per-collection item counts in a whole-app backup (ticket 80): what an
 * export wrote, what a file contains, and (as a pair) what a restore
 * inserted versus skipped. */
export interface BackupCounts {
  products: number;
  presets: number;
  launch_entries: number;
  quick_actions: number;
  clips: number;
}

/** A whole-app restore's outcome (ticket 80): how many items each collection
 * gained and how many were skipped because their identity already exists
 * locally. */
export interface BackupImportSummary {
  inserted: BackupCounts;
  skipped: BackupCounts;
}

/** Which collections a backup export includes (ticket 87): unchecked ones
 * are written as empty arrays in the same document, so the file restores
 * through the ordinary flow. */
export interface BackupSelection {
  products: boolean;
  presets: boolean;
  launch_entries: boolean;
  quick_actions: boolean;
  clips: boolean;
}

/** The AI assistance routes (ADR-0031): off until configured, the user's own
 *  loopback service, managed local, or the later cloud slice. */
export type AiProvider = "off" | "existing-local" | "managed" | "cloud";

/** How the AI provider choices read in Settings. */
export const aiProviderLabel: Record<AiProvider, string> = {
  off: "Off",
  "existing-local": "Existing local service",
  managed: "Managed local",
  cloud: "Cloud provider (later)",
};

/** One reviewable Script draft (ADR-0030): data, never an executed thing.
 *  `executed` is always false — the shape marks what review means. */
export interface AiDraft {
  shell: QuickActionShell;
  command: string;
  assumptions: string[];
  affected_targets: string[];
  explanation: string;
  executed: boolean;
}

export interface ManagedRuntimeStatus {
  name: string;
  version: string;
  artifact: string;
  source: string;
  license: string;
  status: string;
  blocker: string;
  qualified: boolean;
  download_size_bytes: number | null;
}

export interface ManagedModelStatus {
  id: string;
  intent: string;
  artifact: string;
  source: string;
  revision: string | null;
  quantization: string | null;
  sha256: string | null;
  download_size_bytes: number | null;
  license: string;
  license_source: string;
  status: string;
  blocker: string;
  context_limit_tokens: number | null;
  template_requirements: string | null;
  memory_needs_mb: number | null;
  minimum_runtime_version: string | null;
  installable: boolean;
  installed: boolean;
}

export interface ManagedCatalogStatus {
  schema_version: number;
  note: string;
  runtime: ManagedRuntimeStatus;
  models: ManagedModelStatus[];
}

export interface ManagedInstallResult {
  model_id: string;
  installed: boolean;
  message: string;
}

/** One generation request's outcome: a candidate, a refusal, a
 *  clarification, or an actionable failure. Refused, clarified, and failed
 *  outcomes carry no executable text. */
export type AiDraftOutcome =
  | { kind: "draft"; draft: AiDraft }
  | { kind: "refused"; message: string }
  | { kind: "clarify"; message: string }
  | { kind: "failed"; message: string };

/** Rechecking a candidate accepted through AI assistance: the same output
 *  checks, no provider, no persistence, no execution. */
export interface AiCheckVerdict {
  verdict: "allow" | "refuse" | "clarify";
  message: string | null;
}

/** Scoped local target discovery (ADR-0031): an explicit find request over
 *  installed apps and approved folders. Matches carry opaque request-scoped
 *  references — trusted local code binds them, the model never sees paths. */
export type AiDiscoveryScope = "apps" | "files" | "both";

/** One folder approved for local target discovery: names/paths only, never
 *  contents, never disclosure. */
export interface AiApprovedRoot {
  path: string;
  added_at: number;
}

/** What a local match is: an installed app, or a file/folder under an
 *  approved root. */
export type AiTargetKind = "app" | "file" | "folder";

/** One user-visible match: names/paths for review, plus the opaque
 *  reference binding uses. Never carries file contents. */
export interface AiFoundTarget {
  ref_id: string;
  kind: AiTargetKind;
  name: string;
  path: string;
  publisher: string | null;
}

/** One find request's answer: the session its references belong to, the
 *  bounded matches, whether more existed, and an honest notice when part of
 *  the answer needs explaining. */
export interface AiFindOutcome {
  session_id: number;
  matches: AiFoundTarget[];
  truncated: boolean;
  notice: string | null;
}

/** A separately requested file preview: bounded and labeled untrusted —
 *  previewing never authorizes disclosure or execution. */
export interface AiFileContent {
  ref_id: string;
  path: string;
  content: string;
  truncated: boolean;
  bytes: number;
  untrusted: boolean;
}

/** One locally bound target: the shell-quoted command plus the actual
 *  target for review. A non-null warning carries the output-check verdict
 *  when it is anything but allow. */
export interface AiBoundTarget {
  ref_id: string;
  shell: QuickActionShell;
  command: string;
  target: string;
  warning: string | null;
}

/** Which raw fields of one reference the user approved for disclosure to a
 *  provider. Discovery never implies this; the grant dies with its request. */
export interface AiDisclosureGrant {
  session_id: number;
  ref_id: string;
  fields: string[];
}
