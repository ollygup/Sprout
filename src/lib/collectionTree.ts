/// The two-level Groups tree the three main-app collections render (ticket
/// 230). `list_groups` hands back one flat list in tree order — a parent
/// immediately followed by its children — and every surface rebuilds the same
/// shape here: ungrouped rows first, then each top-level group with its own
/// direct rows and its child groups. Keeping the derivation pure means the
/// filter rule, the subtree count, and the "Move into…" destinations are
/// tested without mounting a page.

import type { Group } from "./types";

/** One group at either level: its direct members (in global item order) and,
 *  at the top level only, its child groups in sibling order. */
export interface TreeSection<T> {
  group: Group;
  rows: T[];
  children: TreeSection<T>[];
}

export interface CollectionTree<T> {
  ungrouped: T[];
  sections: TreeSection<T>[];
}

function rowsOf<T extends { group_id: number | null }>(
  items: T[],
  groupId: number,
  matches: (item: T) => boolean,
): T[] {
  return items.filter((item) => item.group_id === groupId && matches(item));
}

/**
 * Builds the render tree. `matches` is the page's one visibility predicate
 * (text search ∩ dock visibility); `filtering` says whether it is narrowing
 * at all.
 *
 * While filtering, a child drops when it has no matching row, and a parent
 * stays only when it or a descendant has a match — so a parent whose only
 * matches sit in a child shows just that child. With no filter, every stored
 * group renders, including an empty one (the caller shows its hint), matching
 * the flat list's absent-until-content rule.
 *
 * A `parent_id` pointing at a missing group is treated as top level: a stale
 * reference must never hide a group or its items.
 */
export function collectionTree<T extends { group_id: number | null }>(
  groups: Group[],
  items: T[],
  matches: (item: T) => boolean = () => true,
  filtering = false,
): CollectionTree<T> {
  const known = new Set(groups.map((group) => group.id));
  const childrenByParent = new Map<number, Group[]>();
  const roots: Group[] = [];
  for (const group of groups) {
    if (group.parent_id !== null && known.has(group.parent_id)) {
      const siblings = childrenByParent.get(group.parent_id) ?? [];
      siblings.push(group);
      childrenByParent.set(group.parent_id, siblings);
    } else {
      roots.push(group);
    }
  }

  const sections: TreeSection<T>[] = [];
  for (const group of roots) {
    const rows = rowsOf(items, group.id, matches);
    const children = (childrenByParent.get(group.id) ?? [])
      .map((child) => ({
        group: child,
        rows: rowsOf(items, child.id, matches),
        children: [],
      }))
      .filter((child) => !filtering || child.rows.length > 0);
    const visible = !filtering || rows.length > 0 || children.length > 0;
    if (visible) sections.push({ group, rows, children });
  }

  return {
    ungrouped: items.filter(
      (item) => item.group_id === null && matches(item),
    ),
    sections,
  };
}

/** One group's own membership — the child badge's number. */
export function countMembers<T extends { group_id: number | null }>(
  items: T[],
  groupId: number,
): number {
  return items.filter((item) => item.group_id === groupId).length;
}

/** The top-level badge's number: direct members plus every child's members.
 *  Counts are unfiltered on purpose — a filter hides rows, not the section's
 *  total (spec 221 C.8). */
export function subtreeCount<T extends { group_id: number | null }>(
  items: T[],
  groups: Group[],
  groupId: number,
): number {
  return groups
    .filter((group) => group.id === groupId || group.parent_id === groupId)
    .reduce((sum, group) => sum + countMembers(items, group.id), 0);
}

/** A group's direct child groups, in sibling order. */
export function childGroups(groups: Group[], groupId: number): Group[] {
  return groups.filter((group) => group.parent_id === groupId);
}

export function groupHasChildren(groups: Group[], groupId: number): boolean {
  return groups.some((group) => group.parent_id === groupId);
}

/** The sibling set a group reorders within — top level or one parent's
 *  children. */
export function siblingGroups(groups: Group[], group: Group): Group[] {
  return groups.filter((other) => other.parent_id === group.parent_id);
}

export function siblingIndex(groups: Group[], group: Group): number {
  return siblingGroups(groups, group).findIndex((other) => other.id === group.id);
}

/**
 * The eligible "Move into…" destinations for one group, or `null` when the
 * menu must not offer the submenu at all.
 *
 * - A group with children can only reorder at the top level — no destination.
 * - An item-only top-level group may join any *other* top-level group.
 * - A child may return to the top level or join another top-level group;
 *   its current parent is omitted because that drop is a no-op.
 *
 * `null` inside the array is the top-level destination.
 */
export function moveIntoDestinations(
  groups: Group[],
  group: Group,
): (Group | null)[] | null {
  if (groupHasChildren(groups, group.id)) return null;
  const roots = groups.filter(
    (other) => other.parent_id === null && other.id !== group.id,
  );
  if (group.parent_id === null) return roots;
  return [null, ...roots.filter((other) => other.id !== group.parent_id)];
}
