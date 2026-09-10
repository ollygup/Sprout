/// Dock visibility filtering shared by the three main-app collections
/// (ADR-0028): one exclusive All / Shown / Hidden choice that intersects
/// with text search. The choice is page-local and never persisted — each
/// page owns its own state; this module owns only the pure predicates so
/// display, Start matching, and reorder gating cannot drift apart.

/// The exclusive visibility choice behind the Dock visibility trigger.
export type DockVisibility = "all" | "shown" | "hidden";

/// Anything carrying the per-item dock flag. The flag is optional here so
/// legacy records predating it keep reading as visible.
export interface DockVisible {
  show_in_dock?: boolean | null;
}

/// Whether the dock lists this item. Missing reads as visible so legacy
/// rows and old backups keep showing everywhere.
export function isDockVisible(item: DockVisible): boolean {
  return item.show_in_dock ?? true;
}

/// The visibility half of the combined predicate: All keeps everything,
/// Shown keeps dock-visible items, Hidden keeps dock-hidden ones.
export function matchesDockVisibility(
  item: DockVisible,
  visibility: DockVisibility
): boolean {
  if (visibility === "all") return true;
  const visible = isDockVisible(item);
  return visibility === "shown" ? visible : !visible;
}

/// The text half's normalization, shared so every collection agrees on
/// what counts as an effective query: trimmed, case-insensitive.
export function normalizeQuery(query: string): string {
  return query.trim().toLowerCase();
}

/// Whether either filter narrows the list: a non-blank query or a
/// non-All visibility. A whitespace-only query is no filter.
export function isFilterActive(
  query: string,
  visibility: DockVisibility
): boolean {
  return normalizeQuery(query) !== "" || visibility !== "all";
}

/// Reordering is unavailable under exactly the same condition: the
/// filtered neighbors are not the saved neighbors, so moving through
/// them would write a surprising order.
export function isReorderBlocked(
  query: string,
  visibility: DockVisibility
): boolean {
  return isFilterActive(query, visibility);
}

/// The trigger's content gate, read from the FULL collection before text
/// search: the filter stays discoverable while any item is hidden or a
/// non-All choice is active — even with zero matches or after the last
/// hidden item becomes shown. A query alone never hides the trigger.
export function shouldShowDockFilter<T extends DockVisible>(
  fullItems: T[],
  visibility: DockVisibility
): boolean {
  if (visibility !== "all") return true;
  return fullItems.some((item) => !isDockVisible(item));
}

/// Narrows a list to one visibility, preserving saved order. Pages AND
/// this with their own text predicate; the result is the one matching
/// collection used for display and for Start matching.
export function applyDockVisibility<T extends DockVisible>(
  items: T[],
  visibility: DockVisibility
): T[] {
  if (visibility === "all") return items;
  return items.filter((item) => matchesDockVisibility(item, visibility));
}
