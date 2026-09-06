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
 *  Duplicate names are refused at authoring, never cleaned here. */
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
    out.push({ url, name: site.name?.trim() ?? "" });
  }
  return out;
}
