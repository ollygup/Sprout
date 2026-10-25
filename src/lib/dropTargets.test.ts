import { describe, expect, it } from "vitest";
import { resolveGroupDrop, resolveItemDrop } from "./dropTargets";
import type { Group } from "./types";

function group(
  id: number,
  name: string,
  parent_id: number | null = null,
  collection: Group["collection"] = "action",
): Group {
  return { id, collection, name, color: null, parent_id };
}

// Global item order: ungrouped 10, group 1's 11, group 2's 12/13, group 3's 14.
const ORDER = [10, 11, 12, 13, 14];

describe("item drop resolution", () => {
  it("reorders before a row inside the same group", () => {
    // Drag 13 above 12: after removing 13, 12 sits at index 2 of [10,11,12,14].
    expect(
      resolveItemDrop({
        dragId: 13,
        order: ORDER,
        filtering: false,
        overRow: { id: 12, groupId: 2, after: false },
      }),
    ).toEqual({ ok: true, groupId: 2, position: 2 });
  });

  it("reorders after a row inside the same group", () => {
    // Drag 12 below 13: without 12 the order is [10,11,13,14]; 13 is index 2.
    expect(
      resolveItemDrop({
        dragId: 12,
        order: ORDER,
        filtering: false,
        overRow: { id: 13, groupId: 2, after: true },
      }),
    ).toEqual({ ok: true, groupId: 2, position: 3 });
  });

  it("moves between groups, taking the target row's group", () => {
    // Drag 10 (ungrouped) below 12 (group 2).
    expect(
      resolveItemDrop({
        dragId: 10,
        order: ORDER,
        filtering: false,
        overRow: { id: 12, groupId: 2, after: true },
      }),
    ).toEqual({ ok: true, groupId: 2, position: 2 });
  });

  it("moves into the ungrouped section when the target row is ungrouped", () => {
    expect(
      resolveItemDrop({
        dragId: 14,
        order: ORDER,
        filtering: false,
        overRow: { id: 10, groupId: null, after: false },
      }),
    ).toEqual({ ok: true, groupId: null, position: 0 });
  });

  it("appends to a group header or empty-group hint at the global end", () => {
    // Drag 10 into group 3: after removal there are four items, so the end is 4.
    expect(
      resolveItemDrop({
        dragId: 10,
        order: ORDER,
        filtering: false,
        hasAppendTarget: true,
        appendGroupId: 3,
      }),
    ).toEqual({ ok: true, groupId: 3, position: 4 });

    expect(
      resolveItemDrop({
        dragId: 11,
        order: ORDER,
        filtering: false,
        hasAppendTarget: true,
        appendGroupId: null,
      }),
    ).toEqual({ ok: true, groupId: null, position: 4 });
  });

  it("treats dropping on the dragged row itself as a no-op", () => {
    expect(
      resolveItemDrop({
        dragId: 12,
        order: ORDER,
        filtering: false,
        overRow: { id: 12, groupId: 2, after: true },
      }),
    ).toEqual({ ok: false, reason: "no-op" });
  });

  it("refuses drops while a filter is active", () => {
    expect(
      resolveItemDrop({
        dragId: 10,
        order: ORDER,
        filtering: true,
        hasAppendTarget: true,
      }),
    ).toEqual({ ok: false, reason: "filtering" });
  });

  it("refuses when nothing is under the pointer", () => {
    expect(
      resolveItemDrop({ dragId: 10, order: ORDER, filtering: false }),
    ).toEqual({ ok: false, reason: "no-target" });
    expect(
      resolveItemDrop({
        dragId: 10,
        order: ORDER,
        filtering: false,
        overRow: { id: 999, groupId: null, after: false },
      }),
    ).toEqual({ ok: false, reason: "no-target" });
  });
});

describe("group drop resolution", () => {
  // 1 Work (has children), 2 Builds (child of 1), 3 Reviews (child of 1),
  // 4 Play (top level), 5 Empty (top level).
  const GROUPS: Group[] = [
    group(1, "Work"),
    group(2, "Builds", 1),
    group(3, "Reviews", 1),
    group(4, "Play"),
    group(5, "Empty"),
  ];

  it("reorders top-level groups through an insertion slot", () => {
    // Drag 4 before 1: raw index 0 among roots, dragged group is after it.
    expect(
      resolveGroupDrop({
        dragId: 4,
        groups: GROUPS,
        filtering: false,
        overSlot: { parentId: null, index: 0 },
      }),
    ).toEqual({ ok: true, parentId: null, position: 0 });
  });

  it("adjusts the slot when the dragged group already sits before it", () => {
    // Roots are [1,4,5]. Drag 1 to raw index 3 (after 5) → position 2.
    expect(
      resolveGroupDrop({
        dragId: 1,
        groups: GROUPS,
        filtering: false,
        overSlot: { parentId: null, index: 3 },
      }),
    ).toEqual({ ok: true, parentId: null, position: 2 });
  });

  it("nests an item-only top-level group into another top-level group", () => {
    expect(
      resolveGroupDrop({
        dragId: 4,
        groups: GROUPS,
        filtering: false,
        overHeaderId: 1,
      }),
    ).toEqual({ ok: true, parentId: 1, position: 2 });
  });

  it("reparents a child into another parent", () => {
    expect(
      resolveGroupDrop({
        dragId: 2,
        groups: GROUPS,
        filtering: false,
        overHeaderId: 5,
      }),
    ).toEqual({ ok: true, parentId: 5, position: 0 });
  });

  it("unnests a child onto the top-level empty area", () => {
    expect(
      resolveGroupDrop({
        dragId: 2,
        groups: GROUPS,
        filtering: false,
        overTopLevelArea: true,
      }),
    ).toEqual({ ok: true, parentId: null, position: 3 });
  });

  it("reorders children inside one parent through a slot", () => {
    // Children of 1 are [2,3]. Drag 2 after 3 → position 1.
    expect(
      resolveGroupDrop({
        dragId: 2,
        groups: GROUPS,
        filtering: false,
        overSlot: { parentId: 1, index: 2 },
      }),
    ).toEqual({ ok: true, parentId: 1, position: 1 });
  });

  it("treats dropping on the current parent as a no-op", () => {
    expect(
      resolveGroupDrop({
        dragId: 2,
        groups: GROUPS,
        filtering: false,
        overHeaderId: 1,
      }),
    ).toEqual({ ok: false, reason: "no-op" });
  });

  it("refuses dropping a group on itself", () => {
    expect(
      resolveGroupDrop({
        dragId: 4,
        groups: GROUPS,
        filtering: false,
        overHeaderId: 4,
      }),
    ).toEqual({ ok: false, reason: "no-op" });
  });

  it("refuses a third level", () => {
    // Builds (2) is already a child, so it cannot host Reviews (3).
    expect(
      resolveGroupDrop({
        dragId: 3,
        groups: GROUPS,
        filtering: false,
        overHeaderId: 2,
      }),
    ).toEqual({ ok: false, reason: "depth" });
    expect(
      resolveGroupDrop({
        dragId: 4,
        groups: GROUPS,
        filtering: false,
        overSlot: { parentId: 2, index: 0 },
      }),
    ).toEqual({ ok: false, reason: "depth" });
  });

  it("refuses a cycle through a slot pointing back at the dragged group", () => {
    expect(
      resolveGroupDrop({
        dragId: 1,
        groups: GROUPS,
        filtering: false,
        overSlot: { parentId: 1, index: 0 },
      }),
    ).toEqual({ ok: false, reason: "cycle" });
  });

  it("refuses a group with children as a child", () => {
    expect(
      resolveGroupDrop({
        dragId: 1,
        groups: GROUPS,
        filtering: false,
        overHeaderId: 4,
      }),
    ).toEqual({ ok: false, reason: "has-children" });
  });

  it("refuses cross-collection nesting", () => {
    const withClip = [...GROUPS, group(9, "Shots", null, "clip")];
    expect(
      resolveGroupDrop({
        dragId: 9,
        groups: withClip,
        filtering: false,
        overHeaderId: 1,
      }),
    ).toEqual({ ok: false, reason: "cross-collection" });
  });

  it("refuses a sibling name collision in the destination", () => {
    const colliding = [
      group(1, "Work"),
      group(2, "Builds", 1),
      group(4, "builds "),
    ];
    expect(
      resolveGroupDrop({
        dragId: 4,
        groups: colliding,
        filtering: false,
        overHeaderId: 1,
      }),
    ).toEqual({ ok: false, reason: "collision" });
  });

  it("refuses drops while filtering and when nothing is targeted", () => {
    expect(
      resolveGroupDrop({
        dragId: 4,
        groups: GROUPS,
        filtering: true,
        overHeaderId: 1,
      }),
    ).toEqual({ ok: false, reason: "filtering" });
    expect(
      resolveGroupDrop({ dragId: 4, groups: GROUPS, filtering: false }),
    ).toEqual({ ok: false, reason: "no-target" });
    expect(
      resolveGroupDrop({
        dragId: 999,
        groups: GROUPS,
        filtering: false,
        overHeaderId: 1,
      }),
    ).toEqual({ ok: false, reason: "no-target" });
    expect(
      resolveGroupDrop({
        dragId: 4,
        groups: GROUPS,
        filtering: false,
        overHeaderId: 999,
      }),
    ).toEqual({ ok: false, reason: "no-target" });
  });
});
