import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const MANAGER_SOURCE = readFileSync(
  new URL("../routes/companion/+page.svelte", import.meta.url),
  "utf8",
);

const NOTICE_SOURCE = readFileSync(
  new URL("./components/Notice.svelte", import.meta.url),
  "utf8",
);

describe("Companion site manager contract", () => {
  it("submits the add and edit form through Sprout's shared Button API", () => {
    expect(MANAGER_SOURCE).toContain('<Button kind="submit">');
    expect(MANAGER_SOURCE).not.toContain('<Button type="submit">');
  });

  it("keeps URL punctuation literal while the address is typed", () => {
    expect(MANAGER_SOURCE).toContain('type="text"');
    expect(MANAGER_SOURCE).toContain("font-variant-ligatures: none");
  });

  it("leaves active-site and height configuration in Settings", () => {
    expect(MANAGER_SOURCE).not.toContain("Active site");
    expect(MANAGER_SOURCE).not.toContain("Pane height");
  });

  describe("add-site Enable-now hint (ticket 163)", () => {
    it("offers Enable-now on the add notice through one Notice action slot", () => {
      expect(NOTICE_SOURCE).toContain("action");
      expect(MANAGER_SOURCE).toContain("Enable now");
      expect(MANAGER_SOURCE).toContain("{#snippet action()}");
    });

    it("activates the saved site on-surface with no Settings visit", () => {
      expect(MANAGER_SOURCE).toContain("enableNoticedSite");
      expect(MANAGER_SOURCE).toContain("await setCompanionUrl(url)");
      expect(MANAGER_SOURCE).toContain("activeUrl = url");
      expect(MANAGER_SOURCE).toContain("onclick={() => void enableNoticedSite()}");
    });

    it("auto-clears on today's flash timing and surfaces errors like today", () => {
      expect(MANAGER_SOURCE).toContain("noticeSiteUrl = null");
      expect(MANAGER_SOURCE).toContain("3200");
      expect(MANAGER_SOURCE).toContain("flash(`Enabled");
    });
  });
});
