import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import type { Clip } from "./types";

/// The viewer page's Tauri seams: the clip read, the close command, the copy
/// path, and the retarget event. The real modules are mocked so the component
/// contract (labels, keyboard handlers, empty state) runs without a window.
const api = vi.hoisted(() => ({
  getClip: vi.fn(),
  takeClipViewerRequest: vi.fn(),
  closeClipViewer: vi.fn(),
  copyClipImage: vi.fn(),
  decodeImageToRgba: vi.fn(),
  clipImageUrl: vi.fn(),
  clipImageMeta: vi.fn(),
}));

const listeners = new Map<string, (event: { payload: unknown }) => void>();

vi.mock("$lib/api", () => api);

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(
    async (name: string, handler: (event: { payload: unknown }) => void) => {
      listeners.set(name, handler);
      return () => listeners.delete(name);
    }
  ),
}));

import Viewer from "../routes/clip-viewer/+page.svelte";

const cleanups: Array<() => void> = [];

afterEach(() => {
  for (const fn of cleanups.splice(0)) fn();
  listeners.clear();
});

beforeEach(() => {
  api.getClip.mockReset();
  api.takeClipViewerRequest.mockReset();
  api.closeClipViewer.mockReset().mockResolvedValue(undefined);
  api.copyClipImage.mockReset().mockResolvedValue(undefined);
  api.decodeImageToRgba.mockReset().mockResolvedValue({
    rgbaBase64: "AAAA",
    width: 1920,
    height: 1080,
  });
  api.clipImageUrl.mockReset().mockImplementation(
    (image: { mime: string; bytes_base64: string }) =>
      `data:${image.mime};base64,${image.bytes_base64}`
  );
  api.clipImageMeta.mockReset().mockImplementation(
    (image: { mime: string; width: number; height: number }) =>
      `PNG · ${image.width} × ${image.height}`
  );
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
      width: 1920,
      height: 1080,
      hash: "hash-7",
    },
    ...overrides,
  };
}

async function flush() {
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
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

describe("clip viewer controls", () => {
  it("renders the clip name, picture summary, and labeled Copy/close controls", async () => {
    api.takeClipViewerRequest.mockResolvedValue(7);
    api.getClip.mockResolvedValue(fixture());

    const host = await mountViewer();

    expect(host.querySelector("h1")?.textContent?.trim()).toBe("Sunset");
    expect(host.textContent).toContain("1920 × 1080");
    const copy = buttonByLabel(host, "Copy Sunset to the clipboard");
    expect(copy.textContent).toContain("Copy");
    buttonByLabel(host, "Close viewer");
  });

  it("closes on Esc and on the close control", async () => {
    api.takeClipViewerRequest.mockResolvedValue(7);
    api.getClip.mockResolvedValue(fixture());

    const host = await mountViewer();

    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    await tick();
    expect(api.closeClipViewer).toHaveBeenCalledTimes(1);

    buttonByLabel(host, "Close viewer").click();
    await tick();
    expect(api.closeClipViewer).toHaveBeenCalledTimes(2);
  });

  it("copies through the image command and flashes the same Copied feedback", async () => {
    api.takeClipViewerRequest.mockResolvedValue(7);
    api.getClip.mockResolvedValue(fixture());

    const host = await mountViewer();

    buttonByLabel(host, "Copy Sunset to the clipboard").click();
    await flush();

    expect(api.copyClipImage).toHaveBeenCalledWith(7, "AAAA", 1920, 1080);
    const copy = buttonByLabel(host, "Copy Sunset to the clipboard");
    expect(copy.textContent).toContain("Copied");
    expect(host.querySelector('[role="status"]')?.textContent).toContain(
      "Sunset copied."
    );
  });

  it("shows the friendly empty state for a missing Clip and still closes on Esc", async () => {
    api.takeClipViewerRequest.mockResolvedValue(7);
    api.getClip.mockResolvedValue(null);

    const host = await mountViewer();

    expect(host.textContent).toContain("This clip is gone");
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    await tick();
    expect(api.closeClipViewer).toHaveBeenCalledTimes(1);
  });

  it("retargets to the new Clip when the open event arrives", async () => {
    api.takeClipViewerRequest.mockResolvedValue(7);
    api.getClip.mockImplementation(async (id: number) =>
      id === 8 ? fixture({ id: 8, name: "Later" }) : fixture()
    );

    const host = await mountViewer();
    expect(host.querySelector("h1")?.textContent?.trim()).toBe("Sunset");

    listeners.get("clip-viewer-open")?.({ payload: 8 });
    await flush();

    expect(host.querySelector("h1")?.textContent?.trim()).toBe("Later");
  });
});
