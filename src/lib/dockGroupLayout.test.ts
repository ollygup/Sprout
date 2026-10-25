import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import {
  DOCK_GROUP_NEST_WIDTH,
  dockGroupColor,
  dockGroupMode,
  dockGroupSections,
  dockSectionCount,
  flattenDockRows,
} from "./dockGroupLayout";
import type { Group } from "./types";

function group(
  id: number,
  name: string,
  parent_id: number | null = null,
): Group {
  return { id, collection: "launch", name, color: null, parent_id };
}

interface Row {
  id: number;
  group_id: number | null;
}

const row = (id: number, group_id: number | null = null): Row => ({
  id,
  group_id,
});

const GROUPS = [
  group(1, "Work"),
  group(2, "Builds", 1),
  group(3, "Reviews", 1),
  group(4, "Play"),
  group(5, "Empty"),
];

const ITEMS = [
  row(10),
  row(12, 2),
  row(13, 3),
  row(14, 4),
];

describe("dock group mode", () => {
  it("flattens below 420 CSS px and nests at and above it", () => {
    expect(dockGroupMode(419)).toBe("flat");
    expect(dockGroupMode(420)).toBe("nested");
    expect(dockGroupMode(421)).toBe("nested");
    expect(DOCK_GROUP_NEST_WIDTH).toBe(420);
  });

  it("pins the page's container query to the same breakpoint", () => {
    const source = readFileSync(
      new URL("../routes/quick-launch-window/+page.svelte", import.meta.url),
      "utf8",
    );
    expect(source).toContain("@container (min-width: 420px)");
    expect(source).toContain("--dock-group-mode");
    expect(source).toContain("container-type: inline-size");
  });
});

describe("dock nested strip (ticket 231 amendment)", () => {
  const DOCK_PAGE = readFileSync(
    new URL("../routes/quick-launch-window/+page.svelte", import.meta.url),
    "utf8",
  );

  it("lets every child sub-header collapse on its own", () => {
    expect(DOCK_PAGE).not.toContain("collapsible={false}");
    for (const store of ["launchCollapse", "actionCollapse", "clipCollapse"]) {
      expect(DOCK_PAGE).toContain(`${store}.isOpen(child.group.id)`);
      expect(DOCK_PAGE).toContain(`${store}.toggle(child.group.id)`);
    }
  });

  it("gives the nested strip room for its own beam", () => {
    const accordion = readFileSync(
      new URL("./components/GroupAccordion.svelte", import.meta.url),
      "utf8",
    );
    expect(accordion).toContain(".group--nested.group--flush");
  });

  it("keeps the group tint formula in one place", () => {
    const accordion = readFileSync(
      new URL("./components/GroupAccordion.svelte", import.meta.url),
      "utf8",
    );
    const tokens = readFileSync(
      new URL("./styles/tokens.css", import.meta.url),
      "utf8",
    );
    expect(accordion).toContain("class:group-tint=");
    expect(accordion).not.toContain("--group-row-bg: color-mix");
    expect(tokens).toContain("--group-row-bg: color-mix");
  });
});

describe("group geometry is colour-independent (ticket 234)", () => {
  const accordion = readFileSync(
    new URL("./components/GroupAccordion.svelte", import.meta.url),
    "utf8",
  );

  it("reserves the leading gutter on every group, not only coloured ones", () => {
    const base = accordion.slice(
      accordion.indexOf("\n  .group {"),
      accordion.indexOf("\n  .group:first-child"),
    );
    expect(base).toContain("--group-inset: var(--space-3)");
    expect(base).toContain("padding-left: var(--group-inset)");
  });

  it("adds no colour-only inset or body padding", () => {
    expect(accordion).not.toContain(".group--colored .group__rows");
    expect(accordion).not.toContain(".group--colored.group--nested");
    expect(accordion).not.toMatch(/\.group--colored\s*\{[^}]*padding-left/);
    expect(accordion).not.toMatch(/\.group--colored\s*\{[^}]*--group-inset/);
  });

  it("gives the dock's loose rows the same step as its groups", () => {
    const page = readFileSync(
      new URL("../routes/quick-launch-window/+page.svelte", import.meta.url),
      "utf8",
    );
    expect(page).toContain(".qlw__groups > .qlw__entries");
    expect(page).toContain(".qlw__groups > .qlw__actions");
    expect(page).toContain(".qlw__groups > .qlw__clips");
  });
});

describe("dock group sections", () => {
  it("keeps a parent whose only rows are in a child", () => {
    const sections = dockGroupSections(GROUPS, ITEMS);
    expect(sections.map((section) => section.group.id)).toEqual([1, 4]);
    expect(sections[0].rows).toEqual([]);
    expect(sections[0].children.map((child) => child.group.id)).toEqual([
      2, 3,
    ]);
    expect(sections[0].children[0].rows.map((item) => item.id)).toEqual([12]);
    expect(sections[1].rows.map((item) => item.id)).toEqual([14]);
  });

  it("drops empty children and a parent whose whole subtree is hidden", () => {
    const sections = dockGroupSections(GROUPS, [row(14, 4)]);
    expect(sections.map((section) => section.group.id)).toEqual([4]);
  });

  it("counts the whole visible subtree on the parent badge", () => {
    const sections = dockGroupSections(GROUPS, ITEMS);
    expect(dockSectionCount(sections[0])).toBe(2);
    expect(dockSectionCount(sections[1])).toBe(1);
  });
});

describe("flat-mode card colour", () => {
  it("gives each flattened row its own group's hue, not the parent's", () => {
    const groups: Group[] = [
      { ...group(1, "Work"), color: "#2f6f4f" },
      { ...group(2, "Builds", 1), color: "#a33c2f" },
      { ...group(3, "Reviews", 1), color: null },
    ];
    expect(dockGroupColor(groups, 2)).toBe("#a33c2f");
    // A neutral child clears the parent's inherited tint.
    expect(dockGroupColor(groups, 3)).toBeNull();
    // No group: keep whatever the container publishes.
    expect(dockGroupColor(groups, null)).toBeUndefined();
    expect(dockGroupColor(groups, 99)).toBeUndefined();
  });

  it("paints the card from the row's own colour while the beam stays the parent's", () => {
    const source = readFileSync(
      new URL("./components/QuickLaunchRow.svelte", import.meta.url),
      "utf8",
    );
    expect(source).toContain("group-tint");
    expect(source).toContain("dock-row--neutral");
  });
});

describe("flat ordering", () => {
  it("lists parent rows first, then each child's rows in child order", () => {
    const sections = dockGroupSections(GROUPS, ITEMS);
    expect(flattenDockRows(sections[0]).map((item) => item.id)).toEqual([
      12, 13,
    ]);

    const mixed = dockGroupSections(
      [group(1, "Work"), group(2, "Builds", 1), group(3, "Reviews", 1)],
      [row(11, 1), row(12, 2), row(13, 3)],
    );
    expect(flattenDockRows(mixed[0]).map((item) => item.id)).toEqual([
      11, 12, 13,
    ]);
  });
});
