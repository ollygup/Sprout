import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import type { Clip } from "./types";

/// The viewer page's Tauri seams. The zoom/pan math itself is covered by
/// `clipViewerZoom.test.ts`; these tests hold the component contract: labeled
/// controls, keyboard shortcuts, wheel gestures, DPR rendering, re-fit, and
/// the released zoom range.
const api = vi.hoisted(() => ({
  getClip: vi.fn(),
  takeClipViewerRequest: vi.fn(),
  closeClipViewer: vi.fn(),
  copyClipImage: vi.fn(),
  decodeImageToRgba: vi.fn(),
  clipImageUrl: vi.fn(),
  clipImageMeta: vi.fn(),
}));

vi.mock("$lib/api", () => api);

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => undefined),
}));

import Viewer from "../routes/clip-viewer/+page.svelte";

const cleanups: Array<() => void> = [];

let stage = { left: 0, top: 120, width: 800, height: 600 };

function rect(): DOMRect {
  return {
    left: stage.left,
    top: stage.top,
    width: stage.width,
    height: stage.height,
    right: stage.left + stage.width,
    bottom: stage.top + stage.height,
    x: stage.left,
    y: stage.top,
    toJSON: () => stage,
  } as DOMRect;
}

beforeEach(() => {
  stage = { left: 0, top: 120, width: 800, height: 600 };
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(rect);
  api.getClip.mockReset();
  api.takeClipViewerRequest.mockReset().mockResolvedValue(7);
  api.closeClipViewer.mockReset().mockResolvedValue(undefined);
  api.copyClipImage.mockReset().mockResolvedValue(undefined);
  api.decodeImageToRgba.mockReset().mockResolvedValue({
    rgbaBase64: "AAAA",
    width: 1000,
    height: 900,
  });
  api.clipImageUrl.mockReset().mockReturnValue("data:image/png;base64,AAAA");
  api.clipImageMeta.mockReset().mockReturnValue("PNG · 1000 × 900");
  api.getClip.mockResolvedValue(fixture());
});

afterEach(() => {
  for (const fn of cleanups.splice(0)) fn();
  vi.restoreAllMocks();
  delete (window as { devicePixelRatio?: number }).devicePixelRatio;
});

function fixture(overrides: Partial<Clip> = {}): Clip {
  return {
    id: 7,
    name: "Sunset",
    content: "",
    show_in_dock: true,
    group_id: null,
    image: {
      mime: "image/png",
      bytes_base64: "AAAA",
      width: 1000,
      height: 900,
      hash: "hash-7",
    },
    ...overrides,
  };
}

async function flush() {
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
  await tick();
  await tick();
  await tick();
}

async function mountViewer(): Promise<HTMLElement> {
  const host = document.createElement("div");
  document.body.appendChild(host);
  const instance = mount(Viewer, { target: host });
  cleanups.push(() => {
    unmount(instance);
    host.remove();
  });
  await flush();
  return host;
}

function buttonByLabel(host: HTMLElement, label: string): HTMLButtonElement {
  const button = host.querySelector<HTMLButtonElement>(
    `button[aria-label="${label}"]`
  );
  expect(button, `button labelled "${label}"`).toBeTruthy();
  return button as HTMLButtonElement;
}

function readout(host: HTMLElement): string {
  return host.querySelector("output")?.textContent?.trim() ?? "";
}

function percentButton(host: HTMLElement): HTMLButtonElement {
  return host.querySelector<HTMLButtonElement>(
    'button[aria-label^="Zoom level"]'
  )!;
}

function imageStyle(host: HTMLElement): string {
  return host.querySelector(".viewer__image")?.getAttribute("style") ?? "";
}

function scaleOf(host: HTMLElement): number {
  const match = /scale\(([-\d.eE]+)\)/.exec(imageStyle(host));
  return match ? Number(match[1]) : Number.NaN;
}

function translateOf(host: HTMLElement): { x: number; y: number } {
  const match = /translate\(([-\d.eE]+)px, ([-\d.eE]+)px\)/.exec(imageStyle(host));
  return match ? { x: Number(match[1]), y: Number(match[2]) } : { x: NaN, y: NaN };
}

function stageEl(host: HTMLElement): HTMLElement {
  const el = host.querySelector<HTMLElement>(".viewer__stage");
  expect(el).toBeTruthy();
  return el as HTMLElement;
}

function press(key: string, init: KeyboardEventInit = {}) {
  window.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, ...init }));
}

/// happy-dom's WheelEvent drops the MouseEvent fields the viewer reads
/// (ctrlKey, shiftKey, clientX/Y), so the gesture is dispatched as a
/// MouseEvent named "wheel" with the wheel deltas attached.
function wheel(
  host: HTMLElement,
  init: {
    deltaY?: number;
    deltaX?: number;
    deltaMode?: number;
    ctrlKey?: boolean;
    shiftKey?: boolean;
    clientX?: number;
    clientY?: number;
  }
) {
  const event = new MouseEvent("wheel", {
    bubbles: true,
    cancelable: true,
    ctrlKey: init.ctrlKey ?? false,
    shiftKey: init.shiftKey ?? false,
    clientX: init.clientX ?? 0,
    clientY: init.clientY ?? 0,
  });
  Object.defineProperties(event, {
    deltaY: { value: init.deltaY ?? 0 },
    deltaX: { value: init.deltaX ?? 0 },
    deltaMode: { value: init.deltaMode ?? 0 },
  });
  stageEl(host).dispatchEvent(event);
}

describe("viewer zoom controls", () => {
  it("starts fitted and exposes labeled, focusable controls with a live readout", async () => {
    const host = await mountViewer();

    // Stage 800×600, 16 px margin, 1000×900 image at DPR 1 → 568/900 = 63 %.
    expect(readout(host)).toBe("63%");
    expect(scaleOf(host)).toBeCloseTo(0.6311, 3);

    const zoomOut = buttonByLabel(host, "Zoom out");
    const zoomIn = buttonByLabel(host, "Zoom in");
    expect(zoomOut.tagName).toBe("BUTTON");
    expect(zoomIn.tagName).toBe("BUTTON");
    expect(host.textContent).toContain("Fit");
    expect(buttonByLabel(host, "Actual size (100%)").textContent).toContain("100%");
    expect(buttonByLabel(host, "Reset zoom").textContent).toContain("Reset");
    expect(
      host.querySelector('[role="group"][aria-label="Zoom"]')
    ).toBeTruthy();
    const percent = percentButton(host);
    expect(percent.tagName).toBe("BUTTON");
    expect(percent.getAttribute("aria-label")).toContain("63%");
    expect(host.querySelector("output")?.getAttribute("aria-live")).toBe("polite");
  });

  it("steps zoom in and out, fits, goes to 100 %, and resets", async () => {
    const host = await mountViewer();
    const fitPercent = readout(host);

    buttonByLabel(host, "Zoom in").click();
    await flush();
    expect(readout(host)).toBe("79%"); // 0.6311 × 1.25 = 0.7889

    buttonByLabel(host, "Zoom out").click();
    await flush();
    expect(readout(host)).toBe(fitPercent);

    buttonByLabel(host, "Actual size (100%)").click();
    await flush();
    expect(readout(host)).toBe("100%");
    expect(scaleOf(host)).toBeCloseTo(1, 6);

    buttonByLabel(host, "Fit").click();
    await flush();
    expect(readout(host)).toBe(fitPercent);

    buttonByLabel(host, "Actual size (100%)").click();
    await flush();
    buttonByLabel(host, "Reset zoom").click();
    await flush();
    expect(readout(host)).toBe(fitPercent);
    expect(translateOf(host).x).toBeCloseTo((800 - 1000 * (568 / 900)) / 2, 6);
  });

  it("clamps the released range at 25 % and 800 %", async () => {
    const host = await mountViewer();

    for (let i = 0; i < 30; i++) buttonByLabel(host, "Zoom out").click();
    await flush();
    expect(readout(host)).toBe("25%");
    expect(buttonByLabel(host, "Zoom out").disabled).toBe(true);

    for (let i = 0; i < 60; i++) buttonByLabel(host, "Zoom in").click();
    await flush();
    expect(readout(host)).toBe("800%");
    expect(buttonByLabel(host, "Zoom in").disabled).toBe(true);
  });

  it("keeps the controls working while the percentage is announced", async () => {
    const host = await mountViewer();
    const output = host.querySelector("output");

    buttonByLabel(host, "Zoom in").click();
    await flush();

    expect(output?.textContent?.trim()).toBe("79%");
    expect(output?.getAttribute("aria-atomic")).toBe("true");
  });

  it("uses the percentage button as the keyboard toggle between fit and 100 %", async () => {
    const host = await mountViewer();
    const fitPercent = readout(host);
    const percent = () => percentButton(host);

    expect(percent().getAttribute("aria-label")).toBe(
      `Zoom level ${fitPercent}. Toggle fit and 100 %.`
    );

    percent().click();
    await flush();
    expect(readout(host)).toBe("100%");

    percent().click();
    await flush();
    expect(readout(host)).toBe(fitPercent);
  });
});

describe("viewer zoom keyboard", () => {
  it("handles plus, minus, 0 (fit), 1 (100 %), and Ctrl+0 (reset)", async () => {
    const host = await mountViewer();
    const fitPercent = readout(host);

    press("+");
    await tick();
    expect(readout(host)).toBe("79%");

    press("-");
    await tick();
    expect(readout(host)).toBe(fitPercent);

    press("1");
    await tick();
    expect(readout(host)).toBe("100%");

    press("0");
    await tick();
    expect(readout(host)).toBe(fitPercent);

    press("1");
    await tick();
    press("0", { ctrlKey: true });
    await tick();
    expect(readout(host)).toBe(fitPercent);
  });

  it("pans with the arrow keys only once the image overflows", async () => {
    const host = await mountViewer();

    const before = translateOf(host);
    press("ArrowLeft");
    await tick();
    expect(translateOf(host).x).toBeCloseTo(before.x, 6);

    press("1");
    await tick();
    const atActual = translateOf(host);
    press("ArrowLeft");
    await tick();
    expect(translateOf(host).x).toBeCloseTo(atActual.x - 48, 6);

    press("ArrowDown");
    await tick();
    expect(translateOf(host).y).toBeCloseTo(atActual.y + 48, 6);
  });
});

describe("viewer zoom pointer input", () => {
  it("zooms at the cursor with Ctrl+wheel and keeps the anchored point fixed", async () => {
    const host = await mountViewer();
    press("1");
    await tick();

    const before = translateOf(host);
    const widthBefore = 1000 * scaleOf(host);
    const fractionBefore = (300 - before.x) / widthBefore;

    wheel(host, { ctrlKey: true, deltaY: -120, clientX: 300, clientY: 300 });
    await tick();

    const zoomed = Number(readout(host).replace("%", ""));
    expect(zoomed).toBeGreaterThan(100);
    expect(zoomed).toBeLessThan(200);

    const after = translateOf(host);
    const widthAfter = 1000 * scaleOf(host);
    expect((300 - after.x) / widthAfter).toBeCloseTo(fractionBefore, 2);
  });

  it("pans vertically with a plain wheel and horizontally with Shift+wheel", async () => {
    const host = await mountViewer();
    press("1");
    await tick();
    const atActual = translateOf(host);

    wheel(host, { deltaY: 100, clientX: 400, clientY: 420 });
    await tick();
    expect(translateOf(host).y).toBeCloseTo(atActual.y - 100, 6);

    wheel(host, { deltaY: 100, shiftKey: true, clientX: 400, clientY: 420 });
    await tick();
    expect(translateOf(host).x).toBeCloseTo(atActual.x - 100, 6);
  });

  it("toggles fit and 100 % on double-click", async () => {
    const host = await mountViewer();
    const fitPercent = readout(host);

    stageEl(host).dispatchEvent(
      new MouseEvent("dblclick", { bubbles: true, clientX: 400, clientY: 420 })
    );
    await tick();
    expect(readout(host)).toBe("100%");

    stageEl(host).dispatchEvent(
      new MouseEvent("dblclick", { bubbles: true, clientX: 400, clientY: 420 })
    );
    await tick();
    expect(readout(host)).toBe(fitPercent);
  });
});

describe("viewer DPR and resize", () => {
  it("renders 100 % as one image pixel per physical display pixel", async () => {
    Object.defineProperty(window, "devicePixelRatio", {
      value: 1.5,
      configurable: true,
    });
    const host = await mountViewer();

    buttonByLabel(host, "Actual size (100%)").click();
    await flush();

    expect(readout(host)).toBe("100%");
    expect(scaleOf(host)).toBeCloseTo(1 / 1.5, 6);
    expect(imageStyle(host)).toContain("width: 1000px");
  });

  it("re-fits a fitted view on resize and keeps a manual zoom", async () => {
    const host = await mountViewer();
    expect(readout(host)).toBe("63%");

    stage = { left: 0, top: 120, width: 400, height: 300 };
    window.dispatchEvent(new Event("resize"));
    await tick();
    expect(readout(host)).toBe("30%"); // 268/900

    buttonByLabel(host, "Actual size (100%)").click();
    await flush();
    window.dispatchEvent(new Event("resize"));
    await tick();
    expect(readout(host)).toBe("100%");
  });
});
