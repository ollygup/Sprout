import { describe, expect, it } from "vitest";
import {
  ZOOM_MAX,
  ZOOM_MIN,
  canPan,
  clampPan,
  clampZoom,
  cssScale,
  displayedSize,
  fitState,
  fitZoom,
  isFit,
  panBy,
  panBounds,
  resetState,
  toggleActualSize,
  wheelPixels,
  wheelZoomFactor,
  zoomAt,
  zoomLabel,
  zoomPercent,
} from "./clipViewerZoom";

const VIEWPORT = { width: 960, height: 720, dpr: 1 };
const IMAGE = { width: 1000, height: 500 };

describe("zoom range", () => {
  it("clamps the interactive range to 25–800 %", () => {
    expect(clampZoom(0.1)).toBe(ZOOM_MIN);
    expect(clampZoom(0.25)).toBe(0.25);
    expect(clampZoom(2)).toBe(2);
    expect(clampZoom(8)).toBe(8);
    expect(clampZoom(20)).toBe(ZOOM_MAX);
  });

  it("reads a broken value as 100 %, never a frozen view", () => {
    expect(clampZoom(Number.NaN)).toBe(1);
    expect(clampZoom(Number.POSITIVE_INFINITY)).toBe(ZOOM_MAX);
  });

  it("reports a rounded percentage", () => {
    expect(zoomPercent(0.928)).toBe(93);
    expect(zoomLabel(1)).toBe("100%");
    expect(zoomLabel(8)).toBe("800%");
  });
});

describe("fit", () => {
  it("contains the image with the stage margin", () => {
    // 960 - 2×16 = 928 wide, 720 - 2×16 = 688 tall.
    expect(fitZoom(IMAGE, VIEWPORT)).toBeCloseTo(0.928, 6);
  });

  it("is device-pixel-ratio aware: the same CSS stage fits more physical pixels", () => {
    // The stage is CSS px; a 1.5 DPR display has 1392×1032 physical px inside
    // the margins, so the image can scale up further.
    expect(fitZoom(IMAGE, { ...VIEWPORT, dpr: 1.5 })).toBeCloseTo(1.392, 6);
    expect(fitZoom(IMAGE, { ...VIEWPORT, dpr: 2 })).toBeCloseTo(1.856, 6);
  });

  it("never exceeds 800 % for a small image", () => {
    expect(fitZoom({ width: 64, height: 48 }, VIEWPORT)).toBe(ZOOM_MAX);
  });

  it("keeps the range's 25 % floor even when containing needs less", () => {
    // 8000 px wide cannot fit a 928 px stage at 1 DPR; the supported range
    // wins and the image shows clipped.
    expect(fitZoom({ width: 8000, height: 6000 }, VIEWPORT)).toBe(ZOOM_MIN);
  });

  it("falls back to the floor for a stage that is not measured yet", () => {
    expect(fitZoom(IMAGE, { width: 0, height: 0, dpr: 1 })).toBe(ZOOM_MIN);
  });

  it("fit state centres the image", () => {
    expect(fitState(IMAGE, VIEWPORT)).toEqual({
      zoom: fitZoom(IMAGE, VIEWPORT),
      panX: 0,
      panY: 0,
    });
  });

  it("reset returns to fit and centre from any pan/zoom", () => {
    expect(resetState(IMAGE, VIEWPORT)).toEqual(fitState(IMAGE, VIEWPORT));
  });

  it("recognises the fit scale", () => {
    const fit = fitZoom(IMAGE, VIEWPORT);
    expect(isFit(fit, IMAGE, VIEWPORT)).toBe(true);
    expect(isFit(1, IMAGE, VIEWPORT)).toBe(false);
  });
});

describe("100 % and device pixel ratio", () => {
  it("renders one image pixel per physical display pixel", () => {
    for (const dpr of [1, 1.5, 2]) {
      const size = displayedSize(IMAGE, 1, dpr);
      expect(size.width * dpr).toBeCloseTo(IMAGE.width, 6);
      expect(size.height * dpr).toBeCloseTo(IMAGE.height, 6);
      expect(cssScale(1, dpr)).toBeCloseTo(1 / dpr, 6);
    }
  });

  it("treats a broken DPR as 1", () => {
    expect(cssScale(1, 0)).toBe(1);
    expect(cssScale(1, Number.NaN)).toBe(1);
  });

  it("scales CSS size by zoom over DPR", () => {
    expect(displayedSize(IMAGE, 2, 2)).toEqual({ width: 1000, height: 500 });
  });
});

describe("cursor-anchored zoom", () => {
  it("keeps the image point under the cursor fixed", () => {
    const anchor = { x: 100, y: 50 };
    const before = { zoom: 1, panX: 0, panY: 0 };
    const after = zoomAt(before, 2, anchor, { width: 1000, height: 800 }, {
      width: 800,
      height: 600,
      dpr: 1,
    });

    expect(after.zoom).toBe(2);
    expect(after.panX).toBe(-100);
    expect(after.panY).toBe(-50);
    // Same image-space point under the anchor before and after.
    expect((anchor.x - after.panX) / after.zoom).toBeCloseTo(anchor.x / 1, 6);
    expect((anchor.y - after.panY) / after.zoom).toBeCloseTo(anchor.y / 1, 6);
  });

  it("anchors at the viewport centre when the anchor is the origin", () => {
    const before = { zoom: 1, panX: -40, panY: -20 };
    const after = zoomAt(before, 2, { x: 0, y: 0 }, IMAGE, VIEWPORT);
    expect(after.panX).toBe(-80);
    expect(after.panY).toBe(-40);
  });

  it("clamps the result at the zoom limits and the pan edges", () => {
    const zoomedOut = zoomAt(
      { zoom: 0.5, panX: 0, panY: 0 },
      0.01,
      { x: 0, y: 0 },
      IMAGE,
      VIEWPORT
    );
    expect(zoomedOut.zoom).toBe(ZOOM_MIN);

    const zoomedIn = zoomAt(
      { zoom: 1, panX: 0, panY: 0 },
      20,
      { x: 400, y: 0 },
      IMAGE,
      VIEWPORT
    );
    expect(zoomedIn.zoom).toBe(ZOOM_MAX);
    const bounds = panBounds(IMAGE, ZOOM_MAX, VIEWPORT);
    expect(Math.abs(zoomedIn.panX)).toBeLessThanOrEqual(bounds.x);
  });
});

describe("pan", () => {
  it("bounds are half the overflow, zero when the image is visible", () => {
    expect(panBounds(IMAGE, 1, VIEWPORT)).toEqual({ x: 20, y: 0 });
    expect(panBounds(IMAGE, 0.5, VIEWPORT)).toEqual({ x: 0, y: 0 });
  });

  it("clamps pan so an edge never detaches", () => {
    const clamped = clampPan(
      { zoom: 1, panX: 500, panY: -100 },
      IMAGE,
      VIEWPORT
    );
    expect(clamped.panX).toBe(20);
    expect(clamped.panY).toBe(0);
  });

  it("pins a smaller axis to centre", () => {
    const panned = panBy({ zoom: 1, panX: 0, panY: 0 }, 0, 400, IMAGE, VIEWPORT);
    expect(panned.panY).toBe(0);
  });

  it("moves by the requested delta inside the bounds", () => {
    const panned = panBy({ zoom: 2, panX: 0, panY: 0 }, 40, -60, IMAGE, VIEWPORT);
    expect(panned.panX).toBe(40);
    expect(panned.panY).toBe(-60);
  });

  it("canPan is true only once an axis overflows", () => {
    expect(canPan(IMAGE, 0.5, VIEWPORT)).toBe(false);
    expect(canPan(IMAGE, 1, VIEWPORT)).toBe(true);
  });
});

describe("double-click toggle", () => {
  it("goes from fit to 100 % at the cursor", () => {
    const fit = fitState(IMAGE, VIEWPORT);
    const toggled = toggleActualSize(fit, { x: 0, y: 0 }, IMAGE, VIEWPORT);
    expect(toggled.zoom).toBe(1);
  });

  it("returns to fit and centre from actual size", () => {
    const toggled = toggleActualSize(
      { zoom: 1, panX: 20, panY: 40 },
      { x: 0, y: 0 },
      IMAGE,
      VIEWPORT
    );
    expect(toggled).toEqual(fitState(IMAGE, VIEWPORT));
  });

  it("treats any other zoom as the non-100 % side of the toggle", () => {
    const toggled = toggleActualSize(
      { zoom: 4, panX: 120, panY: 40 },
      { x: 0, y: 0 },
      IMAGE,
      VIEWPORT
    );
    expect(toggled.zoom).toBe(1);
  });
});

describe("wheel input", () => {
  it("zooms in on wheel-up and out on wheel-down", () => {
    expect(wheelZoomFactor(-120)).toBeGreaterThan(1);
    expect(wheelZoomFactor(120)).toBeLessThan(1);
    expect(wheelZoomFactor(0)).toBe(1);
  });

  it("is symmetric for equal travel", () => {
    expect(wheelZoomFactor(-120) * wheelZoomFactor(120)).toBeCloseTo(1, 6);
  });

  it("normalises line and page deltas to pixels", () => {
    expect(wheelPixels(3, 0, 600)).toBe(3);
    expect(wheelPixels(3, 1, 600)).toBe(48);
    expect(wheelPixels(2, 2, 600)).toBe(1200);
  });
});
