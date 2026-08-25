/// App-icon loading for entry rows (ticket 40, extended by ticket 97): the
/// IntersectionObserver-backed Svelte action behind "fetch this row's icon
/// only once it scrolls into view", plus the per-webview memory cache the
/// rows read. The backend extracts icons from `.lnk`/exe targets via the
/// shell; a target that is gone or has no icon simply never lands in the
/// cache and its row keeps its kind glyph. Icons are held in memory only —
/// never cached to disk.

import { candidateIcon } from "$lib/api";

/** Icon data URLs keyed by the entry's target path. */
export const appIcons = $state<Record<string, string>>({});

/** Fetches deduplicated only while in flight — the cache itself prevents
 *  refetching successes, so a failed or empty attempt (shell had no icon,
 *  one rejected IPC call during the boot burst) retries naturally when the
 *  row next mounts and becomes visible instead of being poisoned for the
 *  whole webview session. */
const inFlight = new Set<string>();

async function fetchIcon(target: string) {
  if (appIcons[target] !== undefined || inFlight.has(target)) return;
  inFlight.add(target);
  try {
    const url = await candidateIcon(target);
    if (url) appIcons[target] = url;
    else console.warn(`[lazyIcon] no icon extracted for ${target}`);
  } catch (e) {
    console.warn(`[lazyIcon] icon fetch failed for ${target}:`, e);
  } finally {
    inFlight.delete(target);
  }
}

/** Attach to a stable ancestor of the icon slot (the row), passing the
 *  entry's target — empty string skips the fetch entirely (command entries,
 *  whose target is shell text rather than a file). */
export function lazyIcon(node: HTMLElement, target: string) {
  if (!target) return {};
  const io = new IntersectionObserver((entries) => {
    if (!entries.some((e) => e.isIntersecting)) return;
    io.disconnect();
    fetchIcon(target);
  });
  io.observe(node);
  return {
    update(next: string) {
      target = next;
    },
    destroy() {
      io.disconnect();
    },
  };
}
