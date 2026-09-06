import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import {
  buildSettingsSearchIndex,
  matchSettingsSearch,
  resolveSettingsFilter,
  type SettingsSearchSnapshot,
} from "./settingsSearch";

const SETTINGS_SOURCE = readFileSync(
  new URL("../routes/settings/+page.svelte", import.meta.url),
  "utf8",
);

const SNAPSHOT: SettingsSearchSnapshot = {
  themeMode: "system",
  themeLabel: "System",
  installDir: "",
  autostart: "on",
  timeoutMinutes: 10,
  retentionDays: 30,
  launchConcurrency: 8,
  dockMode: "auto-hide",
  dockEdge: "left",
  dockState: "floating",
  dockWidthPct: 18,
  dockDensity: "default",
  revealDwellMs: 200,
  revealSensitivityPx: 12,
  companionActiveName: null,
  companionRatioPct: 40,
  companionSiteCount: 2,
  companionSiteNames: ["Music", "https://open.spotify.com"],
  companionMuted: false,
  updateSummary: "Up to date",
};

function matchedIds(query: string): string[] {
  return [...matchSettingsSearch(buildSettingsSearchIndex(SNAPSHOT), query)].sort();
}

describe("settings search index", () => {
  it("surfaces the theme knob for theme, light, and dark alike", () => {
    for (const query of ["theme", "light", "dark"]) {
      expect(matchedIds(query)).toContain("theme");
    }
  });

  it("surfaces dock width for width without matching unrelated knobs", () => {
    const ids = matchedIds("width");
    expect(ids).toContain("dock-width");
    expect(ids).not.toContain("theme");
    expect(ids).not.toContain("backup");
  });

  it("lands mute on the Companion group, whose toolbar owns the toggle", () => {
    expect(matchedIds("mute")).toContain("group:companion");
  });

  it("lands backup on the backup knobs", () => {
    const ids = matchedIds("backup");
    expect(ids).toContain("backup");
    expect(ids).toContain("group:backup");
  });

  it("matches current values, not just labels", () => {
    expect(matchedIds("18%")).toContain("dock-width");
    expect(matchedIds("Music")).toContain("companion-sites");
  });

  it("requires every keyword to match (AND, case-insensitive)", () => {
    expect(matchedIds("DOCK LEFT")).toContain("dock-edge");
    expect(matchedIds("dock banana")).not.toContain("dock-edge");
  });

  it("returns nothing for a nonsense query so the page shows its empty state", () => {
    expect(matchedIds("zxqv banana")).toEqual([]);
    expect(matchedIds("   ")).toEqual([]);
  });

  it("keeps every entry shaped for future knobs: label, synonyms, values, description", () => {
    for (const entry of buildSettingsSearchIndex(SNAPSHOT)) {
      expect(entry.id.length).toBeGreaterThan(0);
      expect(entry.label.length).toBeGreaterThan(0);
      expect(entry.description.length).toBeGreaterThan(0);
      expect(Array.isArray(entry.synonyms)).toBe(true);
      expect(Array.isArray(entry.values)).toBe(true);
    }
  });
});

describe("settings filter resolution", () => {
  function resolved(query: string) {
    return resolveSettingsFilter(buildSettingsSearchIndex(SNAPSHOT), query);
  }

  it("shows only the theme knob for theme and for light alike", () => {
    for (const query of ["theme", "light", "dark"]) {
      const { visibleKnobIds, wholeGroups } = resolved(query);
      expect([...visibleKnobIds].sort()).toEqual(["theme"]);
      expect(wholeGroups.size).toBe(0);
    }
  });

  it("shows dock width (and per-monitor widths) for width", () => {
    const { visibleKnobIds } = resolved("width");
    expect([...visibleKnobIds].sort()).toEqual(["dock-width", "per-monitor"]);
  });

  it("falls back to the whole Companion group for mute, which no knob owns", () => {
    const { visibleKnobIds, wholeGroups } = resolved("mute");
    expect(wholeGroups).toEqual(new Set(["companion"]));
    expect([...visibleKnobIds].sort()).toEqual([
      "companion-active",
      "companion-height",
      "companion-sites",
    ]);
  });

  it("opens the whole group for a bare group name", () => {
    const { visibleKnobIds, wholeGroups } = resolved("companion");
    expect(wholeGroups).toEqual(new Set(["companion"]));
    expect(visibleKnobIds.size).toBe(3);
  });

  it("resolves nothing for a nonsense query so the page shows its empty state", () => {
    const { visibleKnobIds, wholeGroups } = resolved("zxqv banana");
    expect(visibleKnobIds.size).toBe(0);
    expect(wholeGroups.size).toBe(0);
  });
});

describe("settings groups + filter contract", () => {
  it("renders the four groups through the shared accordion, headers bare", () => {
    for (const label of ["General", "Dock", "Companion", "Backup & housekeeping"]) {
      expect(SETTINGS_SOURCE).toContain(`name="${label}"`);
    }
    expect(SETTINGS_SOURCE).toContain("GroupAccordion");
    // Collapsed summaries were removed on review: search covers findability,
    // so headers stay caret + name + count like every other grouped surface.
    expect(SETTINGS_SOURCE).not.toContain("summary=");
    expect(SETTINGS_SOURCE).not.toContain('<section class="group"');
  });

  it("filters from the header toolbar slot through the shared search input", () => {
    expect(SETTINGS_SOURCE).toContain("{#snippet toolbar()}");
    expect(SETTINGS_SOURCE).toContain("SearchInput");
    expect(SETTINGS_SOURCE).toContain("Nothing matches");
    expect(SETTINGS_SOURCE).toContain("resolveSettingsFilter");
    expect(SETTINGS_SOURCE).not.toContain("filter-row");
  });

  it("expands the owning group and focuses on a failing save", () => {
    expect(SETTINGS_SOURCE).toContain("settings-error");
    expect(SETTINGS_SOURCE).toContain(".focus()");
  });

  it("lets the dirty bar own saving — no static Save button", () => {
    expect(SETTINGS_SOURCE).not.toContain("Save settings");
    expect(SETTINGS_SOURCE).not.toContain("form__actions");
    expect(SETTINGS_SOURCE).toContain('aria-label="Unsaved changes"');
    expect(SETTINGS_SOURCE).toContain("Discard");
  });

  it("adds no second rail, tabs, or routes", () => {
    expect(SETTINGS_SOURCE).not.toMatch(/NavRail|side-rail|sub-nav/);
    expect(SETTINGS_SOURCE).not.toContain('goto("/settings');
  });
});
