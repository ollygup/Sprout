import { describe, expect, it } from "vitest";
import {
  GROUP_COLORS,
  GROUP_COLOR_TINT,
  groupColorSwatch,
  normalizeGroupColor,
} from "./groupColor";

describe("group color normalization", () => {
  it("accepts exactly #RRGGBB and lowercases it", () => {
    expect(normalizeGroupColor("#AABBCC")).toBe("#aabbcc");
    expect(normalizeGroupColor("#1a2b3c")).toBe("#1a2b3c");
    expect(normalizeGroupColor("  #0F7B6C  ")).toBe("#0f7b6c");
  });

  it("refuses alpha, shorthand, named, rgb(), hsl(), and missing hashes", () => {
    for (const invalid of [
      "",
      "   ",
      "#abc",
      "#aabbccdd",
      "#aabbcc00",
      "aabbcc",
      "red",
      "rgb(1, 2, 3)",
      "hsl(120, 50%, 40%)",
      "#gggggg",
      "#12 345",
    ]) {
      expect(normalizeGroupColor(invalid), invalid).toBeNull();
    }
  });
});

describe("curated group palette", () => {
  it("carries ten unique valid hues with their own copy keys", () => {
    expect(GROUP_COLORS).toHaveLength(10);
    expect(new Set(GROUP_COLORS.map((swatch) => swatch.hex)).size).toBe(10);
    expect(new Set(GROUP_COLORS.map((swatch) => swatch.labelKey)).size).toBe(10);
    for (const swatch of GROUP_COLORS) {
      expect(normalizeGroupColor(swatch.hex), swatch.labelKey).toBe(swatch.hex);
    }
  });

  it("keeps the body tint at or below the 12 % ceiling", () => {
    expect(GROUP_COLOR_TINT).toBeGreaterThan(0);
    expect(GROUP_COLOR_TINT).toBeLessThanOrEqual(0.12);
  });

  it("selects the matching swatch and treats custom or neutral values as unset", () => {
    expect(groupColorSwatch("#EA3B2B")?.labelKey).toBe("groups.color.red");
    expect(groupColorSwatch(" #1c9139 ")?.labelKey).toBe("groups.color.green");
    expect(groupColorSwatch("#123456")).toBeNull();
    expect(groupColorSwatch(null)).toBeNull();
    expect(groupColorSwatch(undefined)).toBeNull();
    expect(groupColorSwatch("not-a-color")).toBeNull();
  });
});
