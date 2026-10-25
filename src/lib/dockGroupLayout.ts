/// The dock's two-level group layout (ticket 231). The threshold is a content
/// breakpoint owned here — never a window-size constant (ADR-0021 amendment):
/// the tab panel's content width decides whether child sub-headers earn their
/// indent or the subtree flattens into one list.
///
/// The runtime reads the mode from the dock's container query (the CSS owns
/// the actual 420 px line, since a container query cannot take a custom
/// property); [`dockGroupMode`] is the same decision as a pure function, used
/// as the fallback where computed styles are unavailable and pinned by tests
/// at 419/420/421.

import type { Group } from "./types";

/** The dock's nested-vs-flat content breakpoint, in CSS px. Kept beside the
 *  page's `@container (min-width: 420px)` rule — the layout test guards the
 *  pair. */
export const DOCK_GROUP_NEST_WIDTH = 420;

export type DockGroupMode = "flat" | "nested";

export function dockGroupMode(contentWidth: number): DockGroupMode {
  return contentWidth >= DOCK_GROUP_NEST_WIDTH ? "nested" : "flat";
}

export interface DockChildSection<T> {
  group: Group;
  rows: T[];
}

export interface DockGroupSection<T> {
  group: Group;
  /** The parent's own visible rows, in global item order. */
  rows: T[];
  /** Child groups that still have a visible row, in sibling order. */
  children: DockChildSection<T>[];
}

/**
 * Builds the dock's read-only tree from the groups and the already
 * dock-visibility-filtered items. A parent renders while it has a direct row
 * or a child with rows; an empty child drops (there is no menu here to fill
 * it), and a parent whose whole subtree is hidden drops too.
 */
export function dockGroupSections<T extends { group_id: number | null }>(
  groups: Group[],
  items: T[],
): DockGroupSection<T>[] {
  const known = new Set(groups.map((group) => group.id));
  return groups
    .filter(
      (group) => group.parent_id === null || !known.has(group.parent_id),
    )
    .map((group) => {
      const children = groups
        .filter((child) => child.parent_id === group.id)
        .map((child) => ({
          group: child,
          rows: items.filter((item) => item.group_id === child.id),
        }))
        .filter((child) => child.rows.length > 0);
      return {
        group,
        rows: items.filter((item) => item.group_id === group.id),
        children,
      };
    })
    .filter((section) => section.rows.length > 0 || section.children.length > 0);
}

/** The parent badge in the dock: every visible row in its subtree. */
export function dockSectionCount<T>(section: DockGroupSection<T>): number {
  return (
    section.rows.length +
    section.children.reduce((sum, child) => sum + child.rows.length, 0)
  );
}

/** The hue a row wears when the dock renders it outside its own group's
 *  section (flat mode): the parent's beam keeps the section identity, while
 *  each card keeps the colour of the group it actually belongs to (ticket 231
 *  amendment). `undefined` means "no group" — leave the row on whatever its
 *  container publishes; `null` means a neutral group, which must clear a tint
 *  inherited from the section it sits in. */
export function dockGroupColor(
  groups: Group[],
  groupId: number | null,
): string | null | undefined {
  if (groupId === null) return undefined;
  return groups.find((group) => group.id === groupId)?.color;
}

/** Below the breakpoint: the parent's direct rows first, then each child's
 *  rows in child order, with no sub-headers between them. */
export function flattenDockRows<T>(section: DockGroupSection<T>): T[] {
  return [
    ...section.rows,
    ...section.children.flatMap((child) => child.rows),
  ];
}
