/// Drop-target resolution for the shared drag controller (tickets 232–233).
/// Pointer events only supply *what is under the cursor*; this module turns
/// that into the one semantic drop the backend understands — a target group
/// (or top-level parent) plus a global/sibling position — and names every
/// refusal. Keeping it pure means the whole rulebook is tested without a DOM.
///
/// Items always resolve to a **global list position** (`OrderedList::place_at`
/// semantics): the item's order inside its group and Start-all's order stay
/// one contract. Groups resolve to a sibling position under a parent.

import type { Group } from "./types";

export type DropReason =
  | "filtering"
  | "no-target"
  | "no-op"
  | "depth"
  | "cycle"
  | "has-children"
  | "cross-collection"
  | "collision";

export interface DropRefusal {
  ok: false;
  reason: DropReason;
}

export interface ItemDropSuccess {
  ok: true;
  groupId: number | null;
  position: number;
}

export interface GroupDropSuccess {
  ok: true;
  parentId: number | null;
  position: number;
}

export type ItemDrop = ItemDropSuccess | DropRefusal;
export type GroupDrop = GroupDropSuccess | DropRefusal;

export interface ItemDropInput {
  dragId: number;
  /** Every item id in the collection's global order. */
  order: number[];
  filtering: boolean;
  /** The row under the pointer, and which side of its midpoint. */
  overRow?: { id: number; groupId: number | null; after: boolean } | null;
  /** A group header, an empty-group hint, or the top-level area was hit:
   *  append to that group (`null` = ungrouped) at the global end. */
  hasAppendTarget?: boolean;
  appendGroupId?: number | null;
}

export function resolveItemDrop(input: ItemDropInput): ItemDrop {
  if (input.filtering) return { ok: false, reason: "filtering" };

  if (input.hasAppendTarget) {
    return {
      ok: true,
      groupId: input.appendGroupId ?? null,
      position: input.order.length - (input.order.includes(input.dragId) ? 1 : 0),
    };
  }

  if (!input.overRow) return { ok: false, reason: "no-target" };
  if (input.overRow.id === input.dragId) return { ok: false, reason: "no-op" };

  const without = input.order.filter((id) => id !== input.dragId);
  const index = without.indexOf(input.overRow.id);
  if (index < 0) return { ok: false, reason: "no-target" };
  return {
    ok: true,
    groupId: input.overRow.groupId,
    position: input.overRow.after ? index + 1 : index,
  };
}

export interface GroupDropInput {
  dragId: number;
  groups: Group[];
  filtering: boolean;
  /** A group header under the pointer — nest into it when eligible. */
  overHeaderId?: number | null;
  /** An insertion slot: the destination level and its raw index among the
   *  rendered siblings (including the dragged group when it is there). */
  overSlot?: { parentId: number | null; index: number } | null;
  /** The top-level empty area — unnest. */
  overTopLevelArea?: boolean;
}

function siblingSlot(
  groups: Group[],
  drag: Group,
  parentId: number | null,
  rawIndex: number,
): number {
  const siblings = groups.filter((group) => group.parent_id === parentId);
  const dragIndex = siblings.findIndex((group) => group.id === drag.id);
  let index = rawIndex;
  if (dragIndex >= 0 && dragIndex < index) index -= 1;
  const max = siblings.length - (dragIndex >= 0 ? 1 : 0);
  return Math.min(Math.max(index, 0), max);
}

export function resolveGroupDrop(input: GroupDropInput): GroupDrop {
  const drag = input.groups.find((group) => group.id === input.dragId);
  if (!drag) return { ok: false, reason: "no-target" };
  if (input.filtering) return { ok: false, reason: "filtering" };

  let parentId: number | null;
  if (input.overHeaderId != null) {
    if (input.overHeaderId === drag.id) return { ok: false, reason: "no-op" };
    // Dropping on the parent a child already sits under is a no-op.
    if (input.overHeaderId === drag.parent_id) {
      return { ok: false, reason: "no-op" };
    }
    parentId = input.overHeaderId;
  } else if (input.overSlot) {
    parentId = input.overSlot.parentId;
  } else if (input.overTopLevelArea) {
    parentId = null;
  } else {
    return { ok: false, reason: "no-target" };
  }

  if (parentId !== null) {
    const parent = input.groups.find((group) => group.id === parentId);
    if (!parent) return { ok: false, reason: "no-target" };
    if (parent.id === drag.id) return { ok: false, reason: "cycle" };
    // Only a top-level group can hold children — no third level.
    if (parent.parent_id !== null) return { ok: false, reason: "depth" };
    if (parent.collection !== drag.collection) {
      return { ok: false, reason: "cross-collection" };
    }
    // A group with children is a valid parent but never a valid child.
    if (input.groups.some((group) => group.parent_id === drag.id)) {
      return { ok: false, reason: "has-children" };
    }
  }

  // Sibling names are exclusive, so a reparent into a destination that
  // already uses this name must be refused (reorder within the same parent
  // can never collide).
  if (parentId !== drag.parent_id) {
    const name = drag.name.trim().toLowerCase();
    const collides = input.groups.some(
      (group) =>
        group.id !== drag.id &&
        group.parent_id === parentId &&
        group.name.trim().toLowerCase() === name,
    );
    if (collides) return { ok: false, reason: "collision" };
  }

  const position = input.overSlot
    ? siblingSlot(input.groups, drag, parentId, input.overSlot.index)
    : input.groups.filter(
        (group) => group.parent_id === parentId && group.id !== drag.id,
      ).length;

  return { ok: true, parentId, position };
}
