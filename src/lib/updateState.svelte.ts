/// The single self-update store (ticket 74).
///
/// One source of truth for both update affordances — the NavRail footer's
/// pill and the Settings screen's check row — so the startup event or a
/// manual check moves them together no matter which page is open. All
/// networking stays in Rust (ADR-0012); this module only reflects what the
/// backend reported, and its silent-failure contract decides what a failed
/// manual check means: up to date.

import { getVersion } from "@tauri-apps/api/app";
import { listen } from "@tauri-apps/api/event";
import { checkForUpdate, installUpdate } from "$lib/api";
import type { AvailableUpdate } from "$lib/types";

/** The startup event the backend emits exactly once per launch when a newer
 * release exists; keep in step with `UPDATE_AVAILABLE_EVENT` in
 * `src-tauri/src/update.rs`. */
const UPDATE_AVAILABLE_EVENT = "update-available";

export const updateState = $state<{
  /// The running build's version, read once at startup.
  currentVersion: string;
  /// The newer release while one exists — set by the startup event or a
  /// manual check; `null` while up to date.
  available: AvailableUpdate | null;
  /// True from confirming until the installer takes the stage. The backend
  /// exits the app shortly after a successful spawn, so this state never
  /// outlives itself; every install button disables against it.
  installing: boolean;
}>({
  currentVersion: "",
  available: null,
  installing: false,
});

/** What a manual check told us, for the Settings row's result notice:
 * found-version renders the inline install action, "up-to-date" and
 * "failed" render quiet wording. */
export type UpdateCheckResult =
  | { status: "up-to-date" }
  | { status: "available" }
  | { status: "failed" };

let started = false;

/// Wires the startup `update-available` event into the store. Idempotent —
/// the layout calls it once for the main window.
export function watchUpdates() {
  if (started) return;
  started = true;
  getVersion()
    .then((v) => (updateState.currentVersion = v))
    .catch(() => {});
  listen<AvailableUpdate>(UPDATE_AVAILABLE_EVENT, (event) => {
    // An install already in flight owns the UI state until the app exits.
    if (!updateState.installing) updateState.available = event.payload;
  }).catch(() => {});
}

/// The Settings screen's manual re-check (ADR-0012). Updates the shared
/// store, so the rail pill follows live. A silent-failure miss reads as up
/// to date — it clears a stale pill unless an install is on its way, whose
/// progress wording depends on the pill staying put.
export async function checkForUpdates(): Promise<UpdateCheckResult> {
  try {
    const result = await checkForUpdate();
    updateState.currentVersion = result.current_version;
    if (result.update) {
      updateState.available = result.update;
      return { status: "available" };
    }
    if (!updateState.installing) updateState.available = null;
    return { status: "up-to-date" };
  } catch {
    // Only an IPC-level failure lands here; the wording stays quiet.
    return { status: "failed" };
  }
}

/// The confirmed apply step (ADR-0012): downloads and spawns the installer,
/// then the app exits so NSIS can replace it. Resolves when the handoff is
/// underway; throws with the backend's message when it refused, leaving the
/// previous state intact for a retry.
export async function installNow(): Promise<void> {
  const update = updateState.available;
  if (!update || updateState.installing) return;
  updateState.installing = true;
  try {
    await installUpdate(update.url);
  } catch (e) {
    updateState.installing = false;
    throw e;
  }
}
