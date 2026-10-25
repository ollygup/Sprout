import { describe, expect, it } from "vitest";
import {
  childGroups,
  collectionTree,
  countMembers,
  groupHasChildren,
  moveIntoDestinations,
  siblingIndex,
  subtreeCount,
} from "./collectionTree";
import type { Group } from "./types";

function group(
  id: number,
  name: string,
  parent_id: number | null = null,
): Group {
  return { id, collection: "action", name, color: null, parent_id };
}

interface Item {
  id: number;
  group_id: number | null;
}

function item(id: number, group_id: number | null = null): Item {
  return { id, group_id };
}

// Parent (1) with direct items, children (2, 3); another parent (4); plus a
// top-level group (5) with no items at all.
const GROUPS: Group[] = [
  group(1, "Work"),
  group(2, "Builds", 1),
  group(3, "Reviews", 1),
  group(4, "Play"),
  group(5, "Empty"),
];

const ITEMS: Item[] = [
  item(10, null),
  item(11, 1),
  item(12, 2),
  item(13, 2),
  item(14, 3),
  item(15, 4),
];

const ids = (rows: Item[]) => rows.map((row) => row.id);

describe("collection tree derivation", () => {
  it("puts ungrouped rows first, then each parent with its direct rows and children", () => {
    const tree = collectionTree(GROUPS, ITEMS);
    expect(ids(tree.ungrouped)).toEqual([10]);
    expect(tree.sections.map((section) => section.group.id)).toEqual([
      1, 4, 5,
    ]);
    expect(ids(tree.sections[0].rows)).toEqual([11]);
    expect(tree.sections[0].children.map((child) => child.group.id)).toEqual([
      2, 3,
    ]);
    expect(ids(tree.sections[0].children[0].rows)).toEqual([12, 13]);
    expect(ids(tree.sections[0].children[1].rows)).toEqual([14]);
    expect(ids(tree.sections[1].rows)).toEqual([15]);
    expect(tree.sections[2].rows).toEqual([]);
  });

  it("treats a stale parent reference as a top-level group", () => {
    const stale = [group(1, "Orphan", 99)];
    const tree = collectionTree(stale, [item(10, 1)]);
    expect(tree.sections.map((section) => section.group.id)).toEqual([1]);
    expect(ids(tree.sections[0].rows)).toEqual([10]);
  });

  it("keeps every stored group when no filter is active, empty ones included", () => {
    const tree = collectionTree(GROUPS, ITEMS, () => false, false);
    expect(tree.sections.map((section) => section.group.id)).toEqual([
      1, 4, 5,
    ]);
    expect(tree.sections[0].children).toHaveLength(2);
    expect(tree.ungrouped).toEqual([]);
  });
});

describe("subtree counts", () => {
  it("counts direct members for a child and the whole subtree for a parent", () => {
    expect(countMembers(ITEMS, 2)).toBe(2);
    expect(subtreeCount(ITEMS, GROUPS, 1)).toBe(4); // 11 + 12 + 13 + 14
    expect(subtreeCount(ITEMS, GROUPS, 4)).toBe(1);
    expect(subtreeCount(ITEMS, GROUPS, 5)).toBe(0);
  });

  it("stays a total while a filter hides rows", () => {
    const onlyBuilds = collectionTree(
      GROUPS,
      ITEMS,
      (row) => row.id === 12,
      true,
    );
    expect(subtreeCount(ITEMS, GROUPS, 1)).toBe(4);
    expect(onlyBuilds.sections[0].rows).toEqual([]);
    expect(ids(onlyBuilds.sections[0].children[0].rows)).toEqual([12]);
  });
});

describe("filter visibility", () => {
  const matchIds =
    (...wanted: number[]) =>
    (row: Item) =>
      wanted.includes(row.id);

  it("keeps a parent whose only match is in a child, showing only that child", () => {
    const tree = collectionTree(GROUPS, ITEMS, matchIds(14), true);
    expect(tree.sections.map((section) => section.group.id)).toEqual([1]);
    expect(tree.sections[0].rows).toEqual([]);
    expect(tree.sections[0].children.map((child) => child.group.id)).toEqual([
      3,
    ]);
    expect(ids(tree.sections[0].children[0].rows)).toEqual([14]);
  });

  it("drops empty children and parents with no match anywhere", () => {
    const tree = collectionTree(GROUPS, ITEMS, matchIds(11), true);
    expect(tree.sections.map((section) => section.group.id)).toEqual([1]);
    expect(tree.sections[0].children).toEqual([]);
    expect(tree.ungrouped).toEqual([]);

    const noMatches = collectionTree(GROUPS, ITEMS, matchIds(999), true);
    expect(noMatches.sections).toEqual([]);
    expect(noMatches.ungrouped).toEqual([]);
  });

  it("keeps ungrouped matches and a parent with its own match", () => {
    const tree = collectionTree(GROUPS, ITEMS, matchIds(10, 15), true);
    expect(ids(tree.ungrouped)).toEqual([10]);
    expect(tree.sections.map((section) => section.group.id)).toEqual([4]);
    expect(ids(tree.sections[0].rows)).toEqual([15]);
  });
});

describe("Move into destinations", () => {
  it("offers the other top-level groups to an item-only top-level group", () => {
    const destinations = moveIntoDestinations(GROUPS, GROUPS[3]); // Play
    expect(destinations?.map((destination) => destination?.id)).toEqual([
      1, 5,
    ]);
  });

  it("offers Top level plus the other parents to a child", () => {
    const destinations = moveIntoDestinations(GROUPS, GROUPS[1]); // Builds
    expect(destinations?.[0]).toBeNull();
    expect(destinations?.slice(1).map((destination) => destination?.id)).toEqual([
      4, 5,
    ]);
  });

  it("omits the submenu entirely for a group that has children", () => {
    expect(moveIntoDestinations(GROUPS, GROUPS[0])).toBeNull();
  });

  it("has no destinations when no other top-level group exists", () => {
    const solo = [group(1, "Solo"), group(2, "Child", 1)];
    expect(moveIntoDestinations(solo, solo[1])).toEqual([null]);
    expect(moveIntoDestinations([group(1, "Solo")], group(1, "Solo"))).toEqual(
      [],
    );
  });
});

describe("sibling helpers", () => {
  it("scopes order to the group's own level", () => {
    expect(childGroups(GROUPS, 1).map((child) => child.id)).toEqual([2, 3]);
    expect(siblingIndex(GROUPS, GROUPS[2])).toBe(1); // Reviews is second child
    expect(siblingIndex(GROUPS, GROUPS[4])).toBe(2); // Empty is third root
    expect(groupHasChildren(GROUPS, 1)).toBe(true);
    expect(groupHasChildren(GROUPS, 2)).toBe(false);
  });
});
