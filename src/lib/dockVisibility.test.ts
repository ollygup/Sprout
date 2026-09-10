import { describe, expect, it } from "vitest";
import {
  applyDockVisibility,
  isDockVisible,
  isFilterActive,
  isReorderBlocked,
  matchesDockVisibility,
  normalizeQuery,
  shouldShowDockFilter,
} from "./dockVisibility";

const shown = { id: 1, show_in_dock: true };
const hidden = { id: 2, show_in_dock: false };
const legacy = { id: 3 } as { id: number; show_in_dock?: boolean };

describe("isDockVisible", () => {
  it("reads the flag, defaulting legacy items to visible", () => {
    expect(isDockVisible(shown)).toBe(true);
    expect(isDockVisible(hidden)).toBe(false);
    expect(isDockVisible(legacy)).toBe(true);
    expect(isDockVisible({ show_in_dock: null })).toBe(true);
    expect(isDockVisible({ show_in_dock: undefined })).toBe(true);
  });
});

describe("matchesDockVisibility", () => {
  it("keeps everything under All", () => {
    expect(matchesDockVisibility(shown, "all")).toBe(true);
    expect(matchesDockVisibility(hidden, "all")).toBe(true);
    expect(matchesDockVisibility(legacy, "all")).toBe(true);
  });

  it("splits shown from hidden, reading legacy as shown", () => {
    expect(matchesDockVisibility(shown, "shown")).toBe(true);
    expect(matchesDockVisibility(legacy, "shown")).toBe(true);
    expect(matchesDockVisibility(hidden, "shown")).toBe(false);
    expect(matchesDockVisibility(hidden, "hidden")).toBe(true);
    expect(matchesDockVisibility(shown, "hidden")).toBe(false);
    expect(matchesDockVisibility(legacy, "hidden")).toBe(false);
  });
});

describe("normalizeQuery", () => {
  it("trims and lowercases", () => {
    expect(normalizeQuery("  Code  ")).toBe("code");
    expect(normalizeQuery("POSTGRES")).toBe("postgres");
    expect(normalizeQuery("   ")).toBe("");
  });
});

describe("isFilterActive / isReorderBlocked", () => {
  it("is inactive only with a blank query and All", () => {
    expect(isFilterActive("", "all")).toBe(false);
    expect(isFilterActive("   ", "all")).toBe(false);
  });

  it("activates on text alone, visibility alone, or both", () => {
    expect(isFilterActive("code", "all")).toBe(true);
    expect(isFilterActive("", "shown")).toBe(true);
    expect(isFilterActive("", "hidden")).toBe(true);
    expect(isFilterActive("code", "hidden")).toBe(true);
  });

  it("gates reordering under exactly the same condition", () => {
    expect(isReorderBlocked("", "all")).toBe(false);
    expect(isReorderBlocked("   ", "all")).toBe(false);
    expect(isReorderBlocked("code", "all")).toBe(true);
    expect(isReorderBlocked("", "shown")).toBe(true);
    expect(isReorderBlocked("", "hidden")).toBe(true);
  });
});

describe("shouldShowDockFilter", () => {
  it("omits the trigger with no hidden items under All", () => {
    expect(shouldShowDockFilter([shown, legacy], "all")).toBe(false);
    expect(shouldShowDockFilter([], "all")).toBe(false);
  });

  it("shows the trigger once any full-collection item is hidden", () => {
    expect(shouldShowDockFilter([shown, hidden], "all")).toBe(true);
    expect(shouldShowDockFilter([hidden], "all")).toBe(true);
  });

  it("keeps the trigger with a non-All choice even when nothing is hidden", () => {
    // The last-hidden-becomes-shown and zero-result cases keep a reset path.
    expect(shouldShowDockFilter([shown, legacy], "hidden")).toBe(true);
    expect(shouldShowDockFilter([shown], "shown")).toBe(true);
    expect(shouldShowDockFilter([], "hidden")).toBe(true);
  });
});

describe("applyDockVisibility", () => {
  const items = [shown, hidden, legacy];

  it("returns the same list under All", () => {
    expect(applyDockVisibility(items, "all")).toEqual(items);
  });

  it("keeps saved order while splitting shown from hidden", () => {
    expect(applyDockVisibility(items, "shown")).toEqual([shown, legacy]);
    expect(applyDockVisibility(items, "hidden")).toEqual([hidden]);
  });
});
