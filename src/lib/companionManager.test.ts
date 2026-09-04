import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const MANAGER_SOURCE = readFileSync(
  new URL("../routes/companion/+page.svelte", import.meta.url),
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
});
