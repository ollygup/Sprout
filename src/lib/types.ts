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
   * "docked" — what the window reopens as, and what the in-window dock
   * toggle writes back. */
  dock_state: string;
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
}

/** A Launch entry as stored: the input plus its library id. */
export interface LaunchEntry extends LaunchEntryInput {
  id: number;
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
}

/** The editable shape of a Quick Action (ticket 50): a named PowerShell
 *  command with an optional working directory, run from the Quick Launch
 *  window's Quick Actions tab. Machine-local — never part of Presets, Plan,
 *  Run, or exports. */
export interface QuickActionInput {
  name: string;
  /** The PowerShell script, multi-line allowed. */
  command: string;
  /** Working directory the command starts in; null = the app's own. */
  cwd: string | null;
  /** Whether the window shows a Stop button while the action runs
   *  (ticket 62); false keeps the fire-and-forget behavior. */
  stoppable: boolean;
  /** Runs when Stop is clicked; null/empty = kills the process tree. */
  stop_command: string | null;
}

/** A Quick Action as stored: the input plus its library id. */
export interface QuickAction extends QuickActionInput {
  id: number;
}

/** One run-state change for a tracked Quick Action (ticket 62): emitted on
 *  start and again when the process exits, so the window flips Run ↔ Stop
 *  with no polling. */
export interface QuickActionRunState {
  id: number;
  running: boolean;
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
}
