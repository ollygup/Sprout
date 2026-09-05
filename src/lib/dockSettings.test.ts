import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const SETTINGS_SOURCE = readFileSync(
  new URL("../routes/settings/+page.svelte", import.meta.url),
  "utf8",
);

describe("single-display dock settings contract", () => {
  it("keeps a real single monitor aligned with the global dock knobs", () => {
    expect(SETTINGS_SOURCE).toContain(
      "const displayTargets = physicalDisplays.length > 1 ? displays : physicalDisplays;",
    );
    expect(SETTINGS_SOURCE).toContain(
      "physicalDisplays.length > 1 ? displayModes[d.device_name] : dockMode",
    );
    // Ticket 128: the single display's width memory tracks the global slider.
    expect(SETTINGS_SOURCE).toContain("setDisplayDockWidthPct");
    expect(SETTINGS_SOURCE).toContain("displayWidths[d.device_name]");
  });

  it("does not persist DEV-only preview displays", () => {
    expect(SETTINGS_SOURCE).toContain("physicalDisplays = list;");
    expect(SETTINGS_SOURCE).not.toContain("const displayTargets = displays;");
  });
});
