import type { CompanionSite } from "./types";

/** The name every surface shows for a site — the user's label, or the URL
 *  when the label is blank, so nothing ever renders blank. */
export function companionDisplayName(site: CompanionSite): string {
  const name = site.name?.trim();
  return name ? name : site.url;
}

/** A picker row keeps a user-authored name readable without hiding the
 *  address that distinguishes similarly named sites. */
export function companionPickerLabel(site: CompanionSite): string {
  const name = site.name?.trim();
  return name ? `${name} — ${site.url}` : site.url;
}

/** Identity key for one URL: trimmed, lowercased, trailing slashes ignored —
 *  the same key dedup and duplicate checks share. */
export function companionUrlKey(url: string): string {
  return url.trim().toLowerCase().replace(/\/+$/, "");
}

/** The session trail behind the dock Companion's Back/Forward (ADR-0022
 *  Companion is a single isolated site: Back/Forward are its sole history
 *  chrome, and the pinned JavaScript child-WebView surface exposes no
 *  traversal, so steps re-drive the saved-address switch they came from).
 *  A history step only moves the marker to that visit; every fresh
 *  saved-address arrival drops the forward trail and records itself, so
 *  returning restores nothing and Reload always has the saved address to
 *  return to. Key-compared, so case or trailing-slash drift never forks the
 *  trail. A step whose target left the trail falls back to a fresh visit
 *  rather than stranding the marker. Pure — the dock page owns the reactive
 *  state and decides which arrivals count as steps. */
export function trailApplyVisit(
  history: string[],
  index: number,
  url: string,
  isHistoryStep: boolean,
): { history: string[]; index: number } {
  if (isHistoryStep) {
    const key = companionUrlKey(url);
    for (let i = history.length - 1; i >= 0; i -= 1) {
      if (companionUrlKey(history[i]) === key) return { history, index: i };
    }
  }
  return { history: [...history.slice(0, index + 1), url], index: index + 1 };
}

export interface CompanionSiteSwitchCallbacks {
  persist: (url: string) => Promise<void>;
  onPending: (site: CompanionSite | null) => void;
  onApplied: (site: CompanionSite) => void;
  onFailure: (site: CompanionSite, error: unknown) => void;
  onIdle?: () => void;
}

/** Serial persistence makes the newest request land last even when users
 *  choose again before a save returns. Intermediate successes never replace
 *  the live child or selected marker, and only the final failure is surfaced. */
export function createCompanionSiteSwitchQueue(
  callbacks: CompanionSiteSwitchCallbacks,
): (site: CompanionSite) => void {
  let queued: CompanionSite | null = null;
  let inFlight: CompanionSite | null = null;
  let running = false;

  async function drain() {
    if (running) return;
    running = true;
    try {
      while (queued) {
        const target = queued;
        queued = null;
        inFlight = target;
        try {
          await callbacks.persist(target.url);
          if (queued === null) callbacks.onApplied(target);
        } catch (error) {
          if (queued === null) callbacks.onFailure(target, error);
        } finally {
          inFlight = null;
        }
      }
    } finally {
      running = false;
      callbacks.onPending(null);
      callbacks.onIdle?.();
    }
  }

  return (site) => {
    if (queued && companionUrlKey(queued.url) === companionUrlKey(site.url)) return;
    if (
      queued === null &&
      inFlight &&
      companionUrlKey(inFlight.url) === companionUrlKey(site.url)
    ) return;
    queued = site;
    callbacks.onPending(site);
    void drain();
  };
}

/** Tolerant list cleanup mirroring the backend: drops empty and non-https
 *  entries, dedups on the URL key keeping the first occurrence, trims values.
 *  Duplicate names are refused at authoring, never cleaned here. Identity
 *  reads tolerant (missing/unknown = mobile); explicit zoom clamps into
 *  50–200%, anything else reads as auto. Neither ever affects URL identity. */
export function normalizeCompanionSites(list: CompanionSite[]): CompanionSite[] {
  const seen = new Set<string>();
  const out: CompanionSite[] = [];
  for (const site of list) {
    const url = site.url?.trim() ?? "";
    if (!url) continue;
    if (!url.toLowerCase().startsWith("https://")) continue;
    const key = companionUrlKey(url);
    if (seen.has(key)) continue;
    seen.add(key);
    out.push({
      url,
      name: site.name?.trim() ?? "",
      ua: normalizeCompanionSiteUa(site.ua),
      ...(normalizeCompanionSiteZoom(site.zoom) === null
        ? {}
        : { zoom: normalizeCompanionSiteZoom(site.zoom)! }),
    });
  }
  return out;
}

/** Tolerant identity read mirroring the backend: desktop stays desktop,
 *  everything else (missing, legacy, hand-edited) reads as mobile. */
export function normalizeCompanionSiteUa(ua: unknown): "mobile" | "desktop" {
  return typeof ua === "string" && ua.trim().toLowerCase() === "desktop"
    ? "desktop"
    : "mobile";
}

/** Whether the site loads with the desktop identity. */
export function companionSiteIsDesktop(site: Pick<CompanionSite, "ua"> | undefined): boolean {
  return site?.ua === "desktop";
}

/** Tolerant zoom read mirroring the backend: finite values clamp into
 *  50–200%, anything else (missing, null, NaN) reads as auto (null). */
export function normalizeCompanionSiteZoom(zoom: unknown): number | null {
  const v = typeof zoom === "number" ? zoom : Number.NaN;
  if (!Number.isFinite(v)) return null;
  return Math.min(2, Math.max(0.5, v));
}
