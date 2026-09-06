import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const SETTINGS_SOURCE = readFileSync(
  new URL("../routes/settings/+page.svelte", import.meta.url),
  "utf8",
);

describe("Settings save never clobbers untouched Companion knobs (ticket 143)", () => {
  it("re-reads the live store at save time", () => {
    // This page loads once on mount while the splitter drag and the
    // companion manager write out-of-band — saving the stale baseline back
    // over them silently un-configures the pane. Save must consult fresh
    // state first.
    const saveAt = SETTINGS_SOURCE.indexOf("async function save()");
    expect(saveAt).toBeGreaterThan(-1);
    expect(SETTINGS_SOURCE.slice(saveAt)).toContain("await getSettings()");
  });

  it("tracks which companion knobs the user touched", () => {
    expect(SETTINGS_SOURCE).toContain("companionUrlTouched");
    expect(SETTINGS_SOURCE).toContain("companionRatioTouched");
  });

  it("refuses instead of silently overwriting on a real conflict", () => {
    // Touched here AND changed out-of-band since mount: neither value wins
    // by seniority — the save stops with an honest error naming the fix.
    expect(SETTINGS_SOURCE).toContain("Discard and re-apply");
  });

  it("always carries the manager-owned site list through fresh", () => {
    // The list is authored in /companion, never on this page — there is no
    // touched state for it, so it always rides the fresh read.
    expect(SETTINGS_SOURCE).toContain("companion manager owns the site list");
  });

  it("writes a single display's memory through on save", () => {
    // Otherwise the per-monitor entry shadows the just-saved global on a
    // one-screen machine and the knob appears dead: saved yet nothing moves.
    // Same single-vs-multi rule as the dock width memory — one screen means
    // no per-screen customization to protect.
    const saveAt = SETTINGS_SOURCE.indexOf("async function save()");
    expect(saveAt).toBeGreaterThan(-1);
    expect(SETTINGS_SOURCE.slice(saveAt)).toContain(
      "setCompanionHeightRatioForDisplay",
    );
  });

  it("refreshes untouched companion knobs when the dock reports changes", () => {
    // The divider drag persists out-of-band while this page loads once on
    // mount — without a live refresh the page displays (and can later save
    // back) stale values. Touched knobs keep the user's edits; the save-time
    // conflict check still guards those.
    expect(SETTINGS_SOURCE).toContain('listen("quick-launch-changed"');
    expect(SETTINGS_SOURCE).toContain("refreshCompanionKnobs");
  });
});
