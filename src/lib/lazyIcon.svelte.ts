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

const requested = new Set<string>();

async function fetchIcon(target: string) {
  if (requested.has(target)) return;
  requested.add(target);
  try {
    const url = await candidateIcon(target);
    if (url) appIcons[target] = url;
  } catch (e) {
    console.error(e);
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
