/// The app-wide theme store (ticket 31).
///
/// One source of truth for the Settings screen's Theme knob and the `data-theme`
/// attribute every token reads. The mode ("system" follows the OS, "light" and
/// "dark" pin it) is persisted by the backend; a localStorage cache applies it
/// before first paint so a forced theme survives restarts without a flash. The
/// applied theme is always concrete — "system" resolves to light or dark.

import { getSettings, updateTheme } from "$lib/api";

export type ThemeMode = "system" | "light" | "dark";

const STORAGE_KEY = "sprout.theme";

export const theme = $state<{
  /// The mode the user picked (or the default "system").
  mode: ThemeMode;
  /// The concrete theme currently applied to the document.
  applied: "light" | "dark";
}>({
  mode: "system",
  applied: "light",
});

let media: MediaQueryList | null = null;
let started = false;

function systemPrefersDark(): boolean {
  return typeof window !== "undefined" && window.matchMedia("(prefers-color-scheme: dark)").matches;
}

function parseMode(value: string | null): ThemeMode {
  return value === "light" || value === "dark" ? value : "system";
}

function syncNativeWindowTheme(applied: "light" | "dark") {
  if (
    typeof window === "undefined" ||
    !("__TAURI_INTERNALS__" in window || "__TAURI_IPC__" in window)
  ) return;
  void import("@tauri-apps/api/window")
    .then(({ getCurrentWindow }) => getCurrentWindow().setTheme(applied))
    .catch((error) => console.error("native theme update failed", error));
}

/// Applies a mode to the document right now and caches it for the next
/// launch. The `color-scheme` property comes from the tokens' `data-theme`
/// blocks, so native controls match too.
function apply(mode: ThemeMode) {
  const applied = mode === "dark" || (mode === "system" && systemPrefersDark()) ? "dark" : "light";
  theme.mode = mode;
  theme.applied = applied;
  document.documentElement.dataset.theme = applied;
  syncNativeWindowTheme(applied);
  try {
    localStorage.setItem(STORAGE_KEY, mode);
  } catch {
    // Storage unavailable — the backend still holds the persisted mode.
  }
  document.querySelectorAll('meta[name="theme-color"]').forEach((meta) => {
    meta.setAttribute("media", meta.getAttribute("data-scheme") === applied ? "all" : "none");
  });
}

// Module scope runs before the layout renders, so the cached theme is on the
// document before the first paint.
let cached: ThemeMode = "system";
try {
  cached = parseMode(localStorage.getItem(STORAGE_KEY));
} catch {
  // Storage unavailable — fall back to following the OS.
}
apply(cached);

/// One-time wiring: the OS-preference listener for "system", plus a
/// reconciliation with the backend so a fresh install or a cleared cache
/// still lands on the persisted theme. The layout calls this once.
export function startTheme() {
  if (started) return;
  started = true;
  media = window.matchMedia("(prefers-color-scheme: dark)");
  media.addEventListener("change", () => {
    if (theme.mode === "system") apply("system");
  });
  getSettings()
    .then((settings) => {
      const mode = parseMode(settings.theme);
      if (mode !== theme.mode) apply(mode);
    })
    .catch(() => {
      // Offline or locked DB: the cache still holds the last look.
    });
}

/// Applies a mode read from the backend without writing it back — the value
/// already is the persisted one (used when a page loads settings fresh).
export function restoreTheme(mode: ThemeMode) {
  apply(mode);
}

/// The Settings screen's picker — applies instantly and persists on its own,
/// without saving the rest of the form. Rejects if the backend refused.
export async function selectTheme(mode: ThemeMode): Promise<void> {
  apply(mode);
  await updateTheme(mode);
}
