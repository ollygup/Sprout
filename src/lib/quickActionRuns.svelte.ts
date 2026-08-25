/// The one home for Quick Action run state (ticket 98).
///
/// The Quick Actions page and the Quick Launch window's Actions tab render
/// the identical Run → Stop → Stopping control (tickets 62 & 92's
/// vocabulary), so its state lives here instead of in twin per-surface
/// copies that can drift: one event handler, one registry seed, one Stop
/// lifecycle. Each webview (main app, Quick Launch window) runs its own
/// copy of this module and seeds and listens for itself; because both read
/// the same backend events through this same code, the surfaces cannot
/// disagree.
///
/// Stopping semantics (research 0004 rule 5 — silence reads as breakage):
/// set the moment Stop is clicked, cleared only by the process's exit event,
/// or on a Stop refusal, which propagates to the caller's error surface. A
/// hung stop cannot wedge it — the backend force-kills at its ten-second
/// watchdog and the same exit event follows (ticket 92's lifecycle).

import { listen } from "@tauri-apps/api/event";
import { listRunningQuickActions, stopQuickAction } from "$lib/api";
import type { QuickActionRunState } from "$lib/types";

/** The ids whose tracked process is alive and the ids with a Stop in
 *  flight — read directly by row templates. Sets are always replaced, not
 *  mutated, keeping reactivity (the groupCollapse pattern). */
export const quickActionRuns = $state({
  running: new Set<number>(),
  stopping: new Set<number>(),
});

let listening = false;

/** Seeds the running picture from the backend registry and installs the
 *  run-state listener once per webview (idempotent like startRunAwareness —
 *  a hot reload may re-run callers). Call after every load; the events keep
 *  it current with no polling. */
export async function syncQuickActionRuns(): Promise<void> {
  if (!listening) {
    listening = true;
    // The backend emits one event per tracked action on start and again
    // when its process exits — these drive the whole Run → Running →
    // Stopping → Run machine; nothing else mutates the state. An exit event
    // ends Stopping even when the watchdog had to force-kill, since both
    // paths end in this same event.
    listen<QuickActionRunState>("quick-action-run-state-changed", (e) => {
      const next = new Set(quickActionRuns.running);
      const nextStopping = new Set(quickActionRuns.stopping);
      if (e.payload.running) {
        next.add(e.payload.id);
      } else {
        next.delete(e.payload.id);
      }
      nextStopping.delete(e.payload.id);
      quickActionRuns.running = next;
      quickActionRuns.stopping = nextStopping;
    });
  }
  quickActionRuns.running = new Set(await listRunningQuickActions());
}

/** Stop (tickets 62 & 92): flips the control to Stopping immediately and
 *  recovers only through the exit event — or clears when the backend
 *  refuses (the registry already dropped the action, say), which surfaces
 *  through the rejection. */
export async function stopActionRun(id: number): Promise<void> {
  const next = new Set(quickActionRuns.stopping);
  next.add(id);
  quickActionRuns.stopping = next;
  try {
    await stopQuickAction(id);
  } catch (e) {
    const recovered = new Set(quickActionRuns.stopping);
    recovered.delete(id);
    quickActionRuns.stopping = recovered;
    throw e;
  }
}
