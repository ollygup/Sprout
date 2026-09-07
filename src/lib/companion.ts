import type { CompanionSite } from "./types";

/** The name every surface shows for a site — the user's label, or the URL
 *  when the label is blank, so nothing ever renders blank. */
export function companionDisplayName(site: CompanionSite): string {
  const name = site.name?.trim();
  return name ? name : site.url;
}

/** Identity key for one URL: trimmed, lowercased, trailing slashes ignored —
 *  the same key dedup and duplicate checks share. */
export function companionUrlKey(url: string): string {
  return url.trim().toLowerCase().replace(/\/+$/, "");
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
