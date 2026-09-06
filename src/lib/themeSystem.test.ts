import { beforeEach, describe, expect, it, vi } from "vitest";

/// System mode must never pin a concrete native theme.
///
/// The OS preference is read through `matchMedia("(prefers-color-scheme:
/// dark)")`, but WebView2 reports the *native window theme* there — the same
/// surface `Window.setTheme` writes to (research 0012: the runtime
/// initializes child WebViews with the parent window's theme and exposes it
/// as `prefers-color-scheme`). Pushing the resolved concrete value while in
/// system mode therefore poisons the next read: once a light native theme is
/// forced, `matchMedia` answers light forever and System can never resolve
/// dark again. The Tauri API anticipates exactly this — `setTheme(null)`
/// means "follow the system theme" — so system mode must pass null and leave
/// the OS read honest, while light/dark pins keep pushing concrete values.

const setThemeCalls: Array<string | null> = [];
let osDark = true;

function makeMedia() {
  const listeners = new Set<() => void>();
  return {
    get matches() {
      return osDark;
    },
    addEventListener: (_type: string, fn: () => void) => {
      listeners.add(fn);
    },
    removeEventListener: (_type: string, fn: () => void) => {
      listeners.delete(fn);
    },
    fire() {
      listeners.forEach((fn) => fn());
    },
  };
}

let media = makeMedia();

vi.mock("$lib/api", () => ({
  getSettings: vi.fn(async () => ({ theme: "system" })),
  updateTheme: vi.fn(async () => {}),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    setTheme: (t?: string | null) => {
      setThemeCalls.push(t ?? null);
      return Promise.resolve();
    },
  }),
}));

function stubPlatform() {
  vi.stubGlobal("localStorage", {
    getItem: () => null,
    setItem: () => {},
  });
  vi.stubGlobal("document", {
    documentElement: { dataset: {} as Record<string, string> },
    querySelectorAll: () => [],
  });
  vi.stubGlobal("window", {
    __TAURI_INTERNALS__: {},
    matchMedia: () => media,
  });
}

async function loadTheme() {
  vi.resetModules();
  setThemeCalls.length = 0;
  osDark = true;
  media = makeMedia();
  stubPlatform();
  return import("./theme.svelte");
}

async function flush() {
  await new Promise((r) => setTimeout(r, 0));
}

describe("Theme system mode follows the OS", () => {
  beforeEach(() => {
    vi.unstubAllGlobals();
  });

  it("passes null (follow system) to the native window instead of the resolved value", async () => {
    const { selectTheme } = await loadTheme();
    setThemeCalls.length = 0;
    await selectTheme("system");
    await flush();
    // The page still resolves its own tokens to the concrete OS value…
    expect(document.documentElement.dataset.theme).toBe("dark");
    // …but the native window must follow the system, not pin dark: pinning
    // the resolved value is what poisons the next matchMedia read.
    expect(setThemeCalls.at(-1)).toBeNull();
  });

  it("keeps pushing concrete native themes for the light/dark pins", async () => {
    const { selectTheme } = await loadTheme();
    setThemeCalls.length = 0;
    await selectTheme("light");
    await flush();
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(setThemeCalls.at(-1)).toBe("light");
    await selectTheme("dark");
    await flush();
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(setThemeCalls.at(-1)).toBe("dark");
  });

  it("re-resolves when the OS preference changes while in system mode", async () => {
    const { startTheme } = await loadTheme();
    setThemeCalls.length = 0;
    startTheme();
    await flush();
    expect(document.documentElement.dataset.theme).toBe("dark");
    osDark = false;
    media.fire();
    expect(document.documentElement.dataset.theme).toBe("light");
  });
});
