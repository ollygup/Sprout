/// The single run-awareness store (ticket 18).
///
/// The layout banner polls the backend's run-active query and every page
/// reads this one source of truth, so a run stays visible across navigation
/// instead of being page-local state that dies with the Plan page. The same
/// poller owns completion: a Windows toast plus the banner's in-app notice,
/// each exactly once per run id.

import { cancelRun, getActiveRun, readRunProgress } from "$lib/api";
import { runOutcomeLabel } from "$lib/types";
import type { RunDoneInfo, RunOutcome, RunProgress } from "$lib/types";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";

export const runAwareness = $state<{
  /// The run in progress, or the run that just finished.
  activeRunId: string | null;
  /// The completion marker of a run that just finished; `null` while running.
  completion: RunDoneInfo | null;
  /// Whether the user asked this run to stop (the worker finishes the
  /// current step, then stops).
  cancelRequested: boolean;
  /// The progress events read so far, for the banner's one-line activity.
  events: RunProgress[];
}>({
  activeRunId: null,
  completion: null,
  cancelRequested: false,
  events: [],
});

/// The banner's one-line activity while a run is live. A function (not an
/// exported `$derived`, which module exports forbid); components wrap it in
/// their own `$derived(getActivity())` to keep reactivity.
export function getActivity(): string {
  const events = runAwareness.events;
  for (let i = events.length - 1; i >= 0; i--) {
    const event = events[i];
    if (event.type === "requirement_started") {
      return `${event.product_name} — step ${event.index + 1} of ${event.total} (${event.action})`;
    }
    if (event.type === "phase") return `${event.phase}…`;
  }
  return "starting…";
}

/// The user-facing outcome label ("Applied", "With notes", "Cancelled",
/// "Failed") for the banner and the toast; `null` while the run is live.
export function getCompletionLabel(): string | null {
  return runAwareness.completion
    ? runOutcomeLabel[runAwareness.completion.outcome]
    : null;
}

let timer: ReturnType<typeof setInterval> | undefined;
let offset = 0;
const toasted = new Set<string>();
let handledRunId: string | null = null;

/// Starts the poller. Idempotent — the layout calls it once, but a hot
/// reload may run it again.
export function startRunAwareness() {
  if (timer !== undefined) return;
  timer = setInterval(poll, 1000);
  poll();
}

function reset() {
  runAwareness.activeRunId = null;
  runAwareness.completion = null;
  runAwareness.cancelRequested = false;
  runAwareness.events = [];
  offset = 0;
  handledRunId = null;
}

async function poll() {
  try {
    const active = await getActiveRun();
    if (!active) {
      if (runAwareness.activeRunId !== null) reset();
      return;
    }
    if (active.run_id !== runAwareness.activeRunId) {
      runAwareness.activeRunId = active.run_id;
      runAwareness.events = [];
      offset = 0;
      runAwareness.cancelRequested = false;
      handledRunId = null;
    }
    if (active.done) {
      handleDone(active.run_id, active.done);
      return;
    }
    const chunk = await readRunProgress(active.run_id, offset);
    runAwareness.events.push(...chunk.events);
    offset = chunk.offset;
    if (chunk.done) handleDone(active.run_id, chunk.done);
  } catch {
    // A transient invoke failure is retried on the next tick — the banner
    // must never blink off because one poll hiccupped.
  }
}

function handleDone(runId: string, done: RunDoneInfo) {
  // The backend keeps surfacing a just-finished run for a short window —
  // only the first observation acts on it.
  if (handledRunId === runId) return;
  handledRunId = runId;
  runAwareness.completion = done;
  toast(runId, done.outcome);
}

/** Windows toast on completion — once per run, even across "Run again". */
async function toast(runId: string, outcome: RunOutcome) {
  if (toasted.has(runId)) return;
  toasted.add(runId);
  try {
    let granted = await isPermissionGranted();
    if (!granted) granted = (await requestPermission()) === "granted";
    if (granted) {
      sendNotification({
        title: "Sprout",
        body: `Run finished — ${runOutcomeLabel[outcome]}`,
      });
    }
  } catch {
    // Notification unavailable (e.g. dev mode without the installed
    // shortcut): the in-app banner notice still announces the outcome.
  }
}

/** The banner's cancel action — same worker marker the Plan page touches. */
export async function requestCancel() {
  if (!runAwareness.activeRunId) return;
  runAwareness.cancelRequested = true;
  try {
    await cancelRun(runAwareness.activeRunId);
  } catch {
    runAwareness.cancelRequested = false;
  }
}
