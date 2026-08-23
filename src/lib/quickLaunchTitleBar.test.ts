import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { titleBarDragRegion } from "./quickLaunchTitleBar";

// Contract tests for the Quick Launch window's title bar (ADR-0011): while
// the window is docked as a Win32 AppBar it must not offer window-drag
// motion — no OS mechanism ever moves an appbar, motion is always the app's
// own, and a Tauri drag region on the bar IS the app initiating motion. The
// drag affordance therefore has to be derived from the live dock state
// through `titleBarDragRegion`, never hardcoded onto the markup.
const ROUTE_SOURCE = readFileSync(
  new URL("../routes/quick-launch-window/+page.svelte", import.meta.url),
  "utf8",
);

describe("Quick Launch title-bar drag contract", () => {
  it("never hardcodes an always-on drag region", () => {
    expect(ROUTE_SOURCE).not.toMatch(/data-tauri-drag-region="(?:deep|true)?"/);
  });

  it("derives the drag region from dock state via titleBarDragRegion", () => {
    expect(ROUTE_SOURCE).toMatch(
      /data-tauri-drag-region=\{titleBarDragRegion\(/,
    );
  });
});

describe("titleBarDragRegion", () => {
  it("blocks dragging entirely while docked", () => {
    expect(titleBarDragRegion(true)).toBe("false");
  });

  it("keeps the bar freely draggable while floating", () => {
    expect(titleBarDragRegion(false)).toBe("deep");
  });
});
