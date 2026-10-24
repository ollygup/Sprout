import { afterEach, describe, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import SettingsPage from "../routes/settings/+page.svelte";
import {
  getDisplayBezelYRatio,
  getDisplayDockEdge,
  getDisplayDockMode,
  getDisplayDockWidthPct,
  listDisplays,
} from "./api";

vi.mock("$app/navigation", () => ({ beforeNavigate: vi.fn(), goto: vi.fn() }));
vi.mock("svelte/transition", () => ({ fade: () => ({ duration: 0 }) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn().mockResolvedValue(null) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));
vi.mock("./theme.svelte", () => ({ theme: { mode: "light" }, restoreTheme: vi.fn(), selectTheme: vi.fn() }));
vi.mock("./animation.svelte", () => ({ animation: { mode: "on" }, restoreAnimation: vi.fn(), selectAnimation: vi.fn() }));
vi.mock("./updateState.svelte", () => ({ updateState: {}, checkForUpdates: vi.fn(), installNow: vi.fn() }));
vi.mock("./api", () => ({
  getSettings: vi.fn().mockResolvedValue({
    default_timeout_minutes: 20, log_retention_days: 7, install_dir: "",
    launch_concurrency: 3, dock_mode: "auto-hide", dock_edge: "left",
    dock_state: "floating", dock_width_pct: 18, dock_density: "default",
    autostart: "off", theme: "light", animation: "on", ai_provider: "off", ai_model: "",
    ai_base_url: "http://127.0.0.1:11434", companion_url_list: [],
    launch_groups: "off", action_groups: "off", clip_groups: "off",
    reveal_dwell_ms: 200, reveal_sensitivity_px: 12, companion_url: null,
    companion_height_ratio: 0.4, companion_muted: false, native_frame: "off",
  }),
  aiManagedStatus: vi.fn().mockResolvedValue(null),
  aiManagedInstallActive: vi.fn().mockResolvedValue(false),
  aiManagedRuntimeStatus: vi.fn().mockResolvedValue({ running: false, active_model_id: null, uptime_secs: null }),
  listDisplays: vi.fn(),
  getDisplayDockEdge: vi.fn(),
  getDisplayDockMode: vi.fn(),
  getDisplayDockWidthPct: vi.fn(),
  getDisplayBezelYRatio: vi.fn(),
  getLanguage: vi.fn().mockResolvedValue("en"),
  updateSettings: vi.fn(),
  updateAutostart: vi.fn(),
  reconcileQuickLaunchSettings: vi.fn().mockResolvedValue(null),
}));

const mounted: ReturnType<typeof mount>[] = [];
afterEach(async () => {
  for (const component of mounted.splice(0)) await unmount(component);
  document.body.replaceChildren();
  localStorage.clear();
  vi.clearAllMocks();
});

const twoDisplays = [
  {
    device_name: "\\\\.\\DISPLAY1", identity: null, id: "\\\\.\\DISPLAY1",
    label: "Display 1", width: 1920, height: 1080, resolution: "1920 × 1080",
    x: 0, y: 0, left_eligible: true, right_eligible: false,
  },
  {
    device_name: "\\\\.\\DISPLAY2", identity: null, id: "\\\\.\\DISPLAY2",
    label: "Display 2", width: 2560, height: 1440, resolution: "2560 × 1440",
    x: 1920, y: 0, left_eligible: false, right_eligible: true,
  },
];

async function mountWithTwoDisplays(widthPct = 18) {
  vi.mocked(listDisplays).mockResolvedValue(twoDisplays);
  vi.mocked(getDisplayDockEdge).mockResolvedValue("left");
  vi.mocked(getDisplayDockMode).mockResolvedValue("auto-hide");
  vi.mocked(getDisplayDockWidthPct).mockResolvedValue(widthPct);
  vi.mocked(getDisplayBezelYRatio).mockResolvedValue(0.5);
  const host = document.createElement("div");
  document.body.append(host);
  mounted.push(mount(SettingsPage, { target: host }));
  await vi.waitFor(() =>
    expect(host.querySelector('[id^="per-monitor-width-"]')).not.toBeNull(),
  );
  await tick();
  return host;
}

describe("Per-monitor dock card stacks (ticket 224)", () => {
  it("uses the existing stack variant for each display card", async () => {
    const host = await mountWithTwoDisplays();
    const cards = [...host.querySelectorAll(".per-monitor article.knob")];
    expect(cards).toHaveLength(2);
    for (const card of cards) {
      expect(card.classList.contains("knob--stack")).toBe(true);
    }
  });

  it("flows label, edge/mode selects, width row and bezel row as full-width rows", async () => {
    const host = await mountWithTwoDisplays();
    for (const card of host.querySelectorAll(".per-monitor article.knob")) {
      // Four direct children, one per section — no control is squeezed into
      // the label row (the pre-fix overflow).
      const children = [...(card as HTMLElement).children] as HTMLElement[];
      expect(children).toHaveLength(4);
      expect(children[0].classList.contains("knob__body")).toBe(true);
      expect(children[1].classList.contains("knob__input")).toBe(true);
      expect(children[2].classList.contains("knob__input--wide")).toBe(true);
      expect(children[3].classList.contains("knob__input--wide")).toBe(true);
      expect(children[1].querySelectorAll("button")).toHaveLength(2);
      expect(children[2].querySelector('input[type="range"]')).not.toBeNull();
      expect(children[3].querySelector('input[type="range"]')).not.toBeNull();
    }
  });

  it("keeps the effective-width readout and per-mode caps", async () => {
    const host = await mountWithTwoDisplays();
    // 18% of 1920 → 345 px after the formula's floor/round rules; 18% of
    // 2560 → 460 px. Readout copy is unchanged by the layout fix.
    expect(host.querySelector('label[for^="per-monitor-width-"]')?.textContent).toContain("Width · 18% · ~345 px");
    expect(host.querySelectorAll('label[for^="per-monitor-width-"]')[1]?.textContent).toContain("Width · 18% · ~460 px");
    // Fixed keeps the 30% cap; auto-hide/bezel keep 60%.
    for (const slider of host.querySelectorAll('.per-monitor input[type="range"][id^="per-monitor-width-"]')) {
      expect(slider.getAttribute("max")).toBe("60");
    }
  });

  it("keeps the per-mode caps and re-clamps honestly when a row switches to Fixed", async () => {
    const host = await mountWithTwoDisplays(55);
    const readout = () =>
      host.querySelector('label[for^="per-monitor-width-"]')?.textContent ?? "";
    // 55% is legal in auto-hide (60 cap): 55% of 1920 → 1056 px.
    expect(readout()).toContain("Width · 55% · ~1056 px");

    const mode = host.querySelector('[id^="per-monitor-mode-"]') as HTMLButtonElement;
    mode.click();
    await tick();
    const fixed = [...document.querySelectorAll(".ctx-menu .ctx-item")].find(
      (item) => item.textContent?.trim() === "Fixed",
    ) as HTMLButtonElement | undefined;
    expect(fixed).toBeDefined();
    fixed!.click();
    await tick();

    // Fixed keeps the reserving-strip cap: the slider re-clamps 55 → 30.
    const width = host.querySelector(
      'input[type="range"][id^="per-monitor-width-"]',
    ) as HTMLInputElement;
    expect(width.max).toBe("30");
    expect(width.value).toBe("30");
    expect(readout()).toContain("Width · 30% · ~576 px");
  });
});
