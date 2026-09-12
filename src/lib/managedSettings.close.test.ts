import { afterEach, describe, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import SettingsPage from "../routes/settings/+page.svelte";
import { aiInstallManaged, aiManagedStatus, updateSettings } from "./api";
import catalog from "../../src-tauri/resources/ai-model-recommendations.json";

vi.mock("$app/navigation", () => ({ beforeNavigate: vi.fn(), goto: vi.fn() }));
vi.mock("svelte/transition", () => ({ fade: () => ({ duration: 0 }) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn().mockResolvedValue(null) }));
vi.mock("./theme.svelte", () => ({ theme: { mode: "light" }, restoreTheme: vi.fn(), selectTheme: vi.fn() }));
vi.mock("./updateState.svelte", () => ({ updateState: {}, checkForUpdates: vi.fn(), installNow: vi.fn() }));
vi.mock("./api", () => ({
  getSettings: vi.fn().mockResolvedValue({
    default_timeout_minutes: 20, log_retention_days: 7, install_dir: "",
    launch_concurrency: 3, dock_mode: "auto-hide", dock_edge: "left",
    dock_state: "floating", dock_width_pct: 18, dock_density: "default",
    autostart: "off", theme: "light", ai_provider: "managed", ai_model: "",
    ai_base_url: "http://127.0.0.1:11434", companion_url_list: [],
  }),
  aiManagedStatus: vi.fn(), listDisplays: vi.fn().mockResolvedValue([]),
  aiInstallManaged: vi.fn(), aiCancelManagedInstall: vi.fn(), updateSettings: vi.fn(),
}));

const mounted: ReturnType<typeof mount>[] = [];
afterEach(async () => {
  for (const component of mounted.splice(0)) await unmount(component);
  document.body.replaceChildren();
  localStorage.clear();
  vi.clearAllMocks();
});

function bundledStatus() {
  return {
    schema_version: catalog.schema_version, note: catalog.note,
    runtime: {
      ...catalog.managed_runtime, version: catalog.managed_runtime.candidate_version,
      artifact: catalog.managed_runtime.windows_cpu_x64_artifact,
      qualified: false, download_size_bytes: null,
    },
    models: catalog.tiers.map((model) => ({
      ...model, artifact: model.candidate_artifact, source: model.artifact_source,
      sha256: model.download_hash, installable: false, installed: false,
    })),
  };
}

async function openSettings() {
  const host = document.createElement("div");
  document.body.append(host);
  mounted.push(mount(SettingsPage, { target: host }));
  await vi.waitFor(() => expect(host.querySelector("#ai-provider")).not.toBeNull());
  await tick();
  return host;
}

function button(host: HTMLElement, label: string) {
  return [...host.querySelectorAll("button")].find((item) => item.textContent?.trim() === label);
}

describe("Managed setup disclosure", () => {
  it("explains build availability without presenting the unqualified candidates as recommendations", async () => {
    vi.mocked(aiManagedStatus).mockResolvedValue(bundledStatus());
    const host = await openSettings();
    const form = host.querySelector("form")!;
    expect(form.textContent).toContain("Managed setup is unavailable in this build");
    // The shipped catalog's qualified entries carry an empty blocker, and
    // every string contains "" — only assert the blocker stays out of the
    // form when there is one to leak.
    if (catalog.managed_runtime.blocker) {
      expect(form.textContent).not.toContain(catalog.managed_runtime.blocker);
    }
    expect(form.textContent).not.toContain("Not qualified");
    expect(form.textContent).not.toContain(catalog.tiers[0].candidate_artifact);
    const details = button(host, "Why unavailable?");
    expect(details).toBeDefined();
    details!.click();
    await tick();
    const dialog = [...host.querySelectorAll("dialog")].find((item) => item.textContent?.includes("Managed model details"));
    expect(dialog?.textContent).toContain(catalog.tiers[0].candidate_artifact);
    expect(dialog?.textContent).toContain(catalog.managed_runtime.blocker);
    expect(dialog?.textContent).toContain(catalog.tiers[0].license);
    expect(aiInstallManaged).not.toHaveBeenCalled();
    expect(updateSettings).not.toHaveBeenCalled();
  });

  it("offers existing-local setup without silently saving or installing", async () => {
    vi.mocked(aiManagedStatus).mockResolvedValue(bundledStatus());
    const host = await openSettings();
    button(host, "Use existing local service")!.click();
    await tick();
    expect((host.querySelector("#ai-provider") as HTMLSelectElement).value).toBe("existing-local");
    expect(document.activeElement?.id).toBe("ai-base-url");
    expect(host.textContent).toContain("Unsaved changes");
    expect(aiInstallManaged).not.toHaveBeenCalled();
    expect(updateSettings).not.toHaveBeenCalled();
  });

  it("reviews a qualified download before the explicit install action", async () => {
    const status = bundledStatus();
    const ready = {
      ...status,
      runtime: { ...status.runtime, qualified: true, blocker: "", download_size_bytes: 104857600 },
      models: [{ ...status.models[0], installable: true, status: "qualified", blocker: "", revision: "test-revision", download_size_bytes: 2147483648, memory_needs_mb: 4096 }],
    };
    vi.mocked(aiManagedStatus).mockResolvedValue(ready);
    vi.mocked(aiInstallManaged).mockResolvedValue({ model_id: ready.models[0].id, installed: true, message: "Model installed." });
    const host = await openSettings();
    expect(host.textContent).not.toContain("Why unavailable?");
    button(host, "Review & install…")!.click();
    await tick();
    expect(aiInstallManaged).not.toHaveBeenCalled();
    const dialog = [...host.querySelectorAll("dialog")].find((item) => item.textContent?.includes("Install local model"));
    expect(dialog?.textContent).toContain("2 GiB");
    expect(dialog?.textContent).toContain("4,096 MB RAM/VRAM");
    expect(dialog?.textContent).toContain("Runtime download:");
    expect(dialog?.textContent).toContain(ready.models[0].license);
    button(host, "Install model and runtime")!.click();
    await vi.waitFor(() => expect(aiInstallManaged).toHaveBeenCalledExactlyOnceWith(ready.models[0].id));
    expect(updateSettings).not.toHaveBeenCalled();
  });
});
