import { afterEach, describe, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import { listen } from "@tauri-apps/api/event";
import SettingsPage from "../routes/settings/+page.svelte";
import { aiCancelManagedInstall, aiInstallManaged, aiManagedInstallActive, aiManagedResourceUsage, aiManagedRuntimeStatus, aiManagedStatus, aiRemoveManaged, aiStartManagedRuntime, aiStopManagedRuntime, getSettings, updateSettings } from "./api";
import { dismissManagedInstall } from "./managedInstall.svelte";
import { managedResourceDisclosure, setManagedResourceOpen } from "./managedResource.svelte";
import catalog from "../../src-tauri/resources/ai-model-recommendations.json";

vi.mock("$app/navigation", () => ({ beforeNavigate: vi.fn(), goto: vi.fn() }));
vi.mock("svelte/transition", () => ({ fade: () => ({ duration: 0 }) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn().mockResolvedValue(null) }));
vi.mock("./theme.svelte", () => ({ theme: { mode: "light" }, restoreTheme: vi.fn(), selectTheme: vi.fn() }));
vi.mock("./animation.svelte", () => ({ animation: { mode: "on" }, restoreAnimation: vi.fn(), selectAnimation: vi.fn() }));
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
  aiInstallManaged: vi.fn(), aiCancelManagedInstall: vi.fn().mockResolvedValue(false), aiRemoveManaged: vi.fn(), updateSettings: vi.fn(),
  aiManagedInstallActive: vi.fn().mockResolvedValue(false),
  aiManagedRuntimeStatus: vi.fn().mockResolvedValue({ running: false, active_model_id: null, uptime_secs: null }),
  aiManagedResourceUsage: vi.fn(),
  aiStartManagedRuntime: vi.fn(), aiStopManagedRuntime: vi.fn(),
}));

const mounted: ReturnType<typeof mount>[] = [];
afterEach(async () => {
  for (const component of mounted.splice(0)) await unmount(component);
  document.body.replaceChildren();
  localStorage.clear();
  dismissManagedInstall();
  setManagedResourceOpen(false);
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
  // The install store registers its progress listener once per module, while
  // `clearAllMocks` wipes the call record between tests — capture it on the
  // first mount that registers it so progress tests can fire events later.
  capturedProgress ??= (vi.mocked(listen).mock.calls.find((args) => args[0] === "managed-install-progress")?.[1] as
    | ((event: { payload: unknown }) => void)
    | undefined) ?? null;
  return host;
}

function button(host: HTMLElement, label: string) {
  return [...host.querySelectorAll("button")].find((item) => item.textContent?.trim() === label);
}

// Per-model verbs live in the row's own ⋯ menu — open it before picking.
async function openModelMenu(host: HTMLElement, modelId: string) {
  const trigger = host.querySelector(
    `[aria-label="Actions for ${modelId}"]`,
  ) as HTMLButtonElement | null;
  expect(trigger).not.toBeNull();
  trigger!.click();
  await tick();
}

// The AI provider Select renders a trigger button (ticket 204) — its
// current value reads from the data-value readout, not a native select.
function providerValue(host: HTMLElement) {
  return (host.querySelector("#ai-provider") as HTMLButtonElement | null)?.dataset.value;
}

let capturedProgress: ((event: { payload: unknown }) => void) | null = null;

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
    expect(providerValue(host)).toBe("existing-local");
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

  it("removes an installed model with a preview and falls back to Off when none remain", async () => {
    const status = bundledStatus();
    const installed = {
      ...status,
      runtime: { ...status.runtime, qualified: true, blocker: "", download_size_bytes: 104857600 },
      models: [{ ...status.models[0], installable: true, installed: true, status: "qualified", blocker: "", revision: "test-revision", download_size_bytes: 2147483648, memory_needs_mb: 4096 }],
    };
    const modelId = installed.models[0].id;
    vi.mocked(getSettings).mockResolvedValue({
      default_timeout_minutes: 20, log_retention_days: 7, install_dir: "",
      launch_concurrency: 3, dock_mode: "auto-hide", dock_edge: "left",
      dock_state: "floating", dock_width_pct: 18, dock_density: "default",
      autostart: "off", theme: "light", animation: "on", ai_provider: "managed", ai_model: modelId,
      ai_base_url: "http://127.0.0.1:11434", companion_url_list: [],
      launch_groups: "off", action_groups: "off", clip_groups: "off",
      reveal_dwell_ms: 200, reveal_sensitivity_px: 12, companion_url: null,
      companion_height_ratio: 0.4, companion_muted: false, native_frame: "off",
    });
    const emptied = { ...installed, models: [{ ...installed.models[0], installed: false }] };
    vi.mocked(aiManagedStatus).mockResolvedValueOnce(installed).mockResolvedValue(emptied);
    vi.mocked(aiRemoveManaged).mockResolvedValue({ model_id: modelId, removed: true, remaining_installed: 0, message: "Removed the managed model." });
    const host = await openSettings();
    await openModelMenu(host, modelId);
    button(host, "Remove…")!.click();
    await tick();
    const dialog = [...host.querySelectorAll("dialog")].find((item) => item.textContent?.includes("Managed model details"));
    expect(dialog?.textContent).toContain("Removes only Sprout's own files");
    expect(dialog?.textContent).toContain("Your saved Quick Actions stay");
    expect(aiRemoveManaged).not.toHaveBeenCalled();
    button(host, "Confirm remove")!.click();
    await vi.waitFor(() => expect(aiRemoveManaged).toHaveBeenCalledExactlyOnceWith(modelId));
    await vi.waitFor(() => expect(providerValue(host)).toBe("off"));
    expect(host.textContent).toContain("set to Off");
    expect(host.textContent).toContain("Unsaved changes");
    expect(updateSettings).not.toHaveBeenCalled();
  });

  it("keeps the managed route without retargeting when survivors remain — the next Active pick is explicit (ticket 191)", async () => {
    const status = bundledStatus();
    const base = {
      ...status,
      runtime: { ...status.runtime, qualified: true, blocker: "", download_size_bytes: 104857600 },
    };
    const first = { ...status.models[0], id: "alpha", installable: true, installed: true, status: "qualified", blocker: "", revision: "rev-a", download_size_bytes: 2147483648, memory_needs_mb: 4096 };
    const second = { ...status.models[0], id: "beta", installable: true, installed: true, status: "qualified", blocker: "", revision: "rev-b", download_size_bytes: 1073741824, memory_needs_mb: 2048 };
    const before = { ...base, models: [first, second] };
    const after = { ...base, models: [{ ...first, installed: false }, second] };
    vi.mocked(getSettings).mockResolvedValue({
      default_timeout_minutes: 20, log_retention_days: 7, install_dir: "",
      launch_concurrency: 3, dock_mode: "auto-hide", dock_edge: "left",
      dock_state: "floating", dock_width_pct: 18, dock_density: "default",
      autostart: "off", theme: "light", animation: "on", ai_provider: "managed", ai_model: "alpha",
      ai_base_url: "http://127.0.0.1:11434", companion_url_list: [],
      launch_groups: "off", action_groups: "off", clip_groups: "off",
      reveal_dwell_ms: 200, reveal_sensitivity_px: 12, companion_url: null,
      companion_height_ratio: 0.4, companion_muted: false, native_frame: "off",
    });
    vi.mocked(aiManagedStatus).mockResolvedValueOnce(before).mockResolvedValue(after);
    vi.mocked(aiRemoveManaged).mockResolvedValue({ model_id: "alpha", removed: true, remaining_installed: 1, message: "Removed alpha." });
    const host = await openSettings();
    await openModelMenu(host, "alpha");
    button(host, "Remove…")!.click();
    await tick();
    button(host, "Confirm remove")!.click();
    await vi.waitFor(() => expect(aiRemoveManaged).toHaveBeenCalledExactlyOnceWith("alpha"));
    await vi.waitFor(() => expect(host.textContent).toContain("Removed alpha."));
    expect(providerValue(host)).toBe("managed");
    // No silent switch: the draft still names the removed entry, so the
    // Active radio lists only the survivor with nothing checked — picking it
    // is an explicit, Save-deferred edit.
    const radios = [...host.querySelectorAll('input[name="active-managed-model"]')] as HTMLInputElement[];
    expect(radios.map((radio) => radio.value)).toEqual(["beta"]);
    expect(radios.some((radio) => radio.checked)).toBe(false);
    // No active means no status line until the user picks explicitly.
    expect(host.textContent).not.toContain("Running —");
    expect(updateSettings).not.toHaveBeenCalled();
  });

  it("requires an explicit confirm — Keep it dismisses without removing", async () => {
    const status = bundledStatus();
    const installed = {
      ...status,
      runtime: { ...status.runtime, qualified: true, blocker: "", download_size_bytes: 104857600 },
      models: [{ ...status.models[0], installable: true, installed: true, status: "qualified", blocker: "", revision: "test-revision", download_size_bytes: 2147483648, memory_needs_mb: 4096 }],
    };
    vi.mocked(aiManagedStatus).mockResolvedValue(installed);
    const host = await openSettings();
    await openModelMenu(host, installed.models[0].id);
    button(host, "Model details")!.click();
    await tick();
    button(host, "Remove model…")!.click();
    await tick();
    button(host, "Keep it")!.click();
    await tick();
    expect(aiRemoveManaged).not.toHaveBeenCalled();
    expect(aiInstallManaged).not.toHaveBeenCalled();
    expect(updateSettings).not.toHaveBeenCalled();
  });
});

describe("Managed Start/Stop + status (ticket 189)", () => {
  function installedCatalog() {
    const status = bundledStatus();
    return {
      ...status,
      runtime: { ...status.runtime, qualified: true, blocker: "", download_size_bytes: 104857600 },
      models: [{ ...status.models[0], installable: true, installed: true, status: "qualified", blocker: "", revision: "test-revision", download_size_bytes: 2147483648, memory_needs_mb: 4096 }],
    };
  }

  function managedSettings(modelId: string) {
    return {
      default_timeout_minutes: 20, log_retention_days: 7, install_dir: "",
      launch_concurrency: 3, dock_mode: "auto-hide", dock_edge: "left",
      dock_state: "floating", dock_width_pct: 18, dock_density: "default",
      autostart: "off", theme: "light", animation: "on", ai_provider: "managed", ai_model: modelId,
      ai_base_url: "http://127.0.0.1:11434", companion_url_list: [],
      launch_groups: "off", action_groups: "off", clip_groups: "off",
      reveal_dwell_ms: 200, reveal_sensitivity_px: 12, companion_url: null,
      companion_height_ratio: 0.4, companion_muted: false, native_frame: "off",
    };
  }

  it("shows Stopped + Start for an installed-but-idle model, and Start never touches the provider", async () => {
    const catalog = installedCatalog();
    const modelId = catalog.models[0].id;
    vi.mocked(getSettings).mockResolvedValue(managedSettings(modelId));
    vi.mocked(aiManagedStatus).mockResolvedValue(catalog);
    vi.mocked(aiManagedRuntimeStatus).mockResolvedValue({ running: false, active_model_id: null, uptime_secs: null });
    vi.mocked(aiStartManagedRuntime).mockResolvedValue(modelId);
    const host = await openSettings();
    await vi.waitFor(() => expect(host.textContent).toContain("Stopped"));
    button(host, "Start")!.click();
    await vi.waitFor(() => expect(aiStartManagedRuntime).toHaveBeenCalledExactlyOnceWith(modelId));
    expect(updateSettings).not.toHaveBeenCalled();
    expect(providerValue(host)).toBe("managed");
    expect(aiStopManagedRuntime).not.toHaveBeenCalled();
  });

  it("shows Running + Stop while the runtime is up, and Stop keeps provider and model", async () => {
    const catalog = installedCatalog();
    const modelId = catalog.models[0].id;
    vi.mocked(getSettings).mockResolvedValue(managedSettings(modelId));
    vi.mocked(aiManagedStatus).mockResolvedValue(catalog);
    vi.mocked(aiManagedRuntimeStatus).mockResolvedValue({ running: true, active_model_id: modelId, uptime_secs: 65 });
    vi.mocked(aiStopManagedRuntime).mockResolvedValue(undefined);
    const host = await openSettings();
    await vi.waitFor(() => expect(host.textContent).toContain("Running"));
    expect(host.textContent).toContain("up 1m");
    button(host, "Stop")!.click();
    await vi.waitFor(() => expect(aiStopManagedRuntime).toHaveBeenCalledTimes(1));
    expect(updateSettings).not.toHaveBeenCalled();
    expect(providerValue(host)).toBe("managed");
    expect(aiStartManagedRuntime).not.toHaveBeenCalled();
  });

  it("hides the status line when no managed model is installed", async () => {
    vi.mocked(aiManagedStatus).mockResolvedValue(bundledStatus());
    vi.mocked(aiManagedRuntimeStatus).mockResolvedValue({ running: false, active_model_id: null, uptime_secs: null });
    const host = await openSettings();
    await tick();
    expect(host.textContent).not.toContain("Running");
    expect(host.textContent).not.toContain("Stopped");
    expect(button(host, "Start")).toBeUndefined();
    expect(button(host, "Stop")).toBeUndefined();
  });

  it("offers Stop while a start is still polling health, and stopping swallows the cancelled-start error", { timeout: 20000 }, async () => {
    const catalog = installedCatalog();
    const modelId = catalog.models[0].id;
    vi.mocked(getSettings).mockResolvedValue(managedSettings(modelId));
    vi.mocked(aiManagedStatus).mockResolvedValue(catalog);
    vi.mocked(aiManagedRuntimeStatus).mockResolvedValue({ running: false, active_model_id: null, uptime_secs: null });
    let rejectStart!: (reason?: unknown) => void;
    vi.mocked(aiStartManagedRuntime).mockReturnValue(
      new Promise<string>((_, reject) => {
        rejectStart = reject;
      }),
    );
    vi.mocked(aiStopManagedRuntime).mockResolvedValue(undefined);
    const host = await openSettings();
    await vi.waitFor(() => expect(host.textContent).toContain("Stopped"));
    button(host, "Start")!.click();
    await tick();
    expect(button(host, "Starting…")).toBeDefined();
    // The process comes alive while health still polls: the control must
    // offer Stop (which cancels the startup), never a dead Starting… label.
    // The live ~5s status poll flips the switch; the start stays in flight.
    vi.mocked(aiManagedRuntimeStatus).mockResolvedValue({ running: true, active_model_id: modelId, uptime_secs: 3 });
    await vi.waitFor(() => expect(button(host, "Stop")).toBeDefined(), { timeout: 10000 });
    const stop = button(host, "Stop")!;
    expect(stop.hasAttribute("disabled")).toBe(false);
    stop.click();
    await tick();
    expect(aiStopManagedRuntime).toHaveBeenCalledTimes(1);
    // The orphaned start lands cancelled — Stop already announced, so the
    // cancellation must not surface as an error.
    rejectStart(new Error("Managed generation cancelled during runtime startup."));
    await new Promise((resolve) => setTimeout(resolve, 50));
    await tick();
    expect(host.textContent).not.toContain("cancelled during runtime startup");
    expect(button(host, "Stop")).toBeDefined();
  });
});

describe("Active managed model radio (ticket 191)", () => {
  function managedSettings(modelId: string) {
    return {
      default_timeout_minutes: 20, log_retention_days: 7, install_dir: "",
      launch_concurrency: 3, dock_mode: "auto-hide", dock_edge: "left",
      dock_state: "floating", dock_width_pct: 18, dock_density: "default",
      autostart: "off", theme: "light", animation: "on", ai_provider: "managed", ai_model: modelId,
      ai_base_url: "http://127.0.0.1:11434", companion_url_list: [],
      launch_groups: "off", action_groups: "off", clip_groups: "off",
      reveal_dwell_ms: 200, reveal_sensitivity_px: 12, companion_url: null,
      companion_height_ratio: 0.4, companion_muted: false, native_frame: "off",
    };
  }

  function qualifiedBase() {
    const status = bundledStatus();
    return {
      ...status,
      runtime: { ...status.runtime, qualified: true, blocker: "", download_size_bytes: 104857600 },
    };
  }

  function clickRadio(host: HTMLElement, value: string) {
    const radio = host.querySelector(
      `input[name="active-managed-model"][value="${value}"]`,
    ) as HTMLInputElement | null;
    expect(radio).not.toBeNull();
    radio!.click();
  }

  function checkedRadio(host: HTMLElement): string | null {
    const radios = [...host.querySelectorAll('input[name="active-managed-model"]')] as HTMLInputElement[];
    return radios.find((radio) => radio.checked)?.value ?? null;
  }

  it("lists installed models only with tier-first rows and keeps full identity in Details", async () => {
    const base = qualifiedBase();
    const status = bundledStatus();
    const lite = {
      ...status.models[0], installable: true, installed: true, status: "qualified",
      blocker: "", revision: "test-revision", download_size_bytes: 2147483648, memory_needs_mb: 4096,
    };
    const modelId = lite.id;
    vi.mocked(getSettings).mockResolvedValue(managedSettings(modelId));
    vi.mocked(aiManagedStatus).mockResolvedValue({ ...base, models: [lite] });
    vi.mocked(aiManagedRuntimeStatus).mockResolvedValue({ running: false, active_model_id: null, uptime_secs: null });
    const host = await openSettings();
    // Tier-first row chooses by cost…
    const radios = [...host.querySelectorAll('input[name="active-managed-model"]')] as HTMLInputElement[];
    expect(radios.map((radio) => radio.value)).toEqual([modelId]);
    expect(checkedRadio(host)).toBe(modelId);
    expect(host.textContent).toContain("Lightweight — 1.5B Q4");
    expect(host.textContent).toContain("4 GB RAM");
    // One flat option row, not two listings: the drafted active reads from
    // the radio itself, and the artifact identity is absent until Details.
    expect(host.querySelectorAll(".managed__option").length).toBe(1);
    expect(host.textContent).not.toContain(lite.artifact);
    // …while the full artifact identity stays one level down in Details.
    await openModelMenu(host, modelId);
    button(host, "Model details")!.click();
    await tick();
    const dialog = [...host.querySelectorAll("dialog")].find((item) => item.textContent?.includes("Managed model details"));
    expect(dialog?.textContent).toContain(lite.artifact);
    expect(dialog?.textContent).toContain(lite.source);
    expect(dialog?.textContent).toContain("test-revision");
    expect(dialog?.textContent).toContain(lite.sha256);
    expect(dialog?.textContent).toContain(lite.license);
  });

  it("switching the radio is Save-deferred and never touches installs or the runtime", async () => {
    const base = qualifiedBase();
    const status = bundledStatus();
    const alpha = { ...status.models[0], id: "alpha", installable: true, installed: true, status: "qualified", blocker: "", revision: "rev-a", download_size_bytes: 2147483648, memory_needs_mb: 4096 };
    const beta = { ...status.models[0], id: "beta", installable: true, installed: true, status: "qualified", blocker: "", revision: "rev-b", download_size_bytes: 1073741824, memory_needs_mb: 2048 };
    vi.mocked(getSettings).mockResolvedValue(managedSettings("alpha"));
    vi.mocked(aiManagedStatus).mockResolvedValue({ ...base, models: [alpha, beta] });
    vi.mocked(aiManagedRuntimeStatus).mockResolvedValue({ running: true, active_model_id: "alpha", uptime_secs: 65 });
    const host = await openSettings();
    await vi.waitFor(() => expect(host.textContent).toContain("Running — alpha"));
    clickRadio(host, "beta");
    await tick();
    expect(checkedRadio(host)).toBe("beta");
    expect(host.textContent).toContain("Unsaved changes");
    // No silent switch: nothing started, stopped, installed, or saved.
    expect(aiStartManagedRuntime).not.toHaveBeenCalled();
    expect(aiStopManagedRuntime).not.toHaveBeenCalled();
    expect(aiInstallManaged).not.toHaveBeenCalled();
    expect(updateSettings).not.toHaveBeenCalled();
    // The status still names the serving model, not the draft.
    expect(host.textContent).toContain("Running — alpha");
  });

  it("a later install never implicitly retargets the active model", async () => {
    const base = qualifiedBase();
    const status = bundledStatus();
    const alpha = { ...status.models[0], id: "alpha", installable: true, installed: true, status: "qualified", blocker: "", revision: "rev-a", download_size_bytes: 2147483648, memory_needs_mb: 4096 };
    const betaInstallable = { ...status.models[0], id: "beta", installable: true, installed: false, status: "qualified", blocker: "", revision: "rev-b", download_size_bytes: 1073741824, memory_needs_mb: 2048 };
    const before = { ...base, models: [alpha, betaInstallable] };
    const after = { ...base, models: [alpha, { ...betaInstallable, installed: true }] };
    vi.mocked(getSettings).mockResolvedValue(managedSettings("alpha"));
    vi.mocked(aiManagedStatus).mockResolvedValueOnce(before).mockResolvedValue(after);
    vi.mocked(aiManagedRuntimeStatus).mockResolvedValue({ running: false, active_model_id: null, uptime_secs: null });
    vi.mocked(aiInstallManaged).mockResolvedValue({ model_id: "beta", installed: true, message: "Installed beta." });
    const host = await openSettings();
    button(host, "Review & install…")!.click();
    await tick();
    button(host, "Install model and runtime")!.click();
    await vi.waitFor(() => expect(aiInstallManaged).toHaveBeenCalledExactlyOnceWith("beta"));
    await vi.waitFor(() => expect(host.textContent).toContain("Installed beta."));
    // The active draft survives the second install untouched.
    expect(checkedRadio(host)).toBe("alpha");
    expect(updateSettings).not.toHaveBeenCalled();
  });

  it("lists not-yet-installed models once under Available with tier-first titles", async () => {
    const base = qualifiedBase();
    const status = bundledStatus();
    const lite = {
      ...status.models[0], installable: true, installed: true, status: "qualified",
      blocker: "", revision: "test-revision", download_size_bytes: 2147483648, memory_needs_mb: 4096,
    };
    const stronger = {
      ...status.models[1], installable: true, installed: false, status: "qualified",
      blocker: "", download_size_bytes: 5033164800, memory_needs_mb: 8192,
    };
    vi.mocked(getSettings).mockResolvedValue(managedSettings(lite.id));
    vi.mocked(aiManagedStatus).mockResolvedValue({ ...base, models: [lite, stronger] });
    vi.mocked(aiManagedRuntimeStatus).mockResolvedValue({ running: false, active_model_id: null, uptime_secs: null });
    const host = await openSettings();
    // Installed lives in the Active options; Available holds the rest only.
    expect(host.querySelectorAll(".managed__option").length).toBe(1);
    expect(host.querySelectorAll(".managed__choice").length).toBe(1);
    expect(host.textContent).toContain("Available models");
    expect(host.textContent).toContain("Stronger — 7B Q4");
    expect(button(host, "Review & install…")).toBeDefined();
    // Neither artifact clutters the list — both prove identity in Details.
    expect(host.textContent).not.toContain(lite.artifact);
    expect(host.textContent).not.toContain(stronger.artifact);
  });

  it("surfaces a busy-switch refusal honestly without touching the selection", async () => {
    const base = qualifiedBase();
    const status = bundledStatus();
    const alpha = { ...status.models[0], id: "alpha", installable: true, installed: true, status: "qualified", blocker: "", revision: "rev-a", download_size_bytes: 2147483648, memory_needs_mb: 4096 };
    const beta = { ...status.models[0], id: "beta", installable: true, installed: true, status: "qualified", blocker: "", revision: "rev-b", download_size_bytes: 1073741824, memory_needs_mb: 2048 };
    vi.mocked(getSettings).mockResolvedValue(managedSettings("beta"));
    vi.mocked(aiManagedStatus).mockResolvedValue({ ...base, models: [alpha, beta] });
    vi.mocked(aiManagedRuntimeStatus).mockResolvedValue({ running: false, active_model_id: null, uptime_secs: null });
    vi.mocked(aiStartManagedRuntime).mockRejectedValue(
      new Error("Another managed model is serving an active request; try again when it finishes."),
    );
    const host = await openSettings();
    await vi.waitFor(() => expect(host.textContent).toContain("Stopped"));
    button(host, "Start")!.click();
    await vi.waitFor(() => expect(aiStartManagedRuntime).toHaveBeenCalledExactlyOnceWith("beta"));
    await vi.waitFor(() => expect(host.textContent).toContain("serving an active request"));
    // No silent switch: the draft and the provider stand exactly as authored.
    expect(checkedRadio(host)).toBe("beta");
    expect(providerValue(host)).toBe("managed");
    expect(updateSettings).not.toHaveBeenCalled();
  });
});

describe("Managed install progress + recovery (ticket 193)", () => {
  function installableCatalog() {
    const status = bundledStatus();
    return {
      ...status,
      runtime: { ...status.runtime, qualified: true, blocker: "", download_size_bytes: 104857600 },
      models: [{ ...status.models[0], installable: true, installed: false, status: "qualified", blocker: "", revision: "test-revision", download_size_bytes: 2147483648, memory_needs_mb: 4096 }],
    };
  }

  function unconfiguredSettings() {
    return {
      default_timeout_minutes: 20, log_retention_days: 7, install_dir: "",
      launch_concurrency: 3, dock_mode: "auto-hide", dock_edge: "left",
      dock_state: "floating", dock_width_pct: 18, dock_density: "default",
      autostart: "off", theme: "light", animation: "on", ai_provider: "managed", ai_model: "",
      ai_base_url: "http://127.0.0.1:11434", companion_url_list: [],
      launch_groups: "off", action_groups: "off", clip_groups: "off",
      reveal_dwell_ms: 200, reveal_sensitivity_px: 12, companion_url: null,
      companion_height_ratio: 0.4, companion_muted: false, native_frame: "off",
    };
  }

  function installedAfter(catalog: ReturnType<typeof installableCatalog>) {
    return { ...catalog, models: [{ ...catalog.models[0], installed: true }] };
  }

  function progressHandler() {
    if (!capturedProgress) throw new Error("progress listener was never registered");
    return capturedProgress;
  }

  it("renders bar + percent + bytes with Cancel in row and dialog, surviving as one flight", async () => {
    const catalog = installableCatalog();
    const modelId = catalog.models[0].id;
    vi.mocked(getSettings).mockResolvedValue(unconfiguredSettings());
    vi.mocked(aiManagedStatus).mockResolvedValue(catalog);
    let release!: (value: { model_id: string; installed: boolean; message: string }) => void;
    vi.mocked(aiInstallManaged).mockReturnValue(new Promise((resolve) => { release = resolve; }));
    const host = await openSettings();
    button(host, "Review & install…")!.click();
    await tick();
    button(host, "Install model and runtime")!.click();
    await vi.waitFor(() => expect(aiInstallManaged).toHaveBeenCalledExactlyOnceWith(modelId));
    // The flight is visible in both places while it runs.
    const cancels = [...host.querySelectorAll("button")].filter((item) => item.textContent?.trim() === "Cancel install");
    expect(cancels.length).toBeGreaterThanOrEqual(2);
    // A backend progress event feeds bar + percent + bytes everywhere.
    progressHandler()({ payload: { model_id: modelId, phase: "model", downloaded_bytes: 100 * 1024 * 1024, total_bytes: 200 * 1024 * 1024 } });
    await tick();
    expect(host.textContent).toContain("50%");
    expect(host.textContent).toContain("100 MB of 200 MB");
    // Cancel from the row cancels the one flight; nothing lands as installed.
    cancels[0].click();
    await tick();
    expect(aiCancelManagedInstall).toHaveBeenCalledTimes(1);
    // The landing catalog reports installed, so the managed route survives.
    vi.mocked(aiManagedStatus).mockResolvedValue(installedAfter(catalog));
    release({ model_id: modelId, installed: true, message: "Installed." });
    await vi.waitFor(() => expect(host.textContent).toContain("Installed."));
  });

  it("offers remove-and-retry plus keep-files on the occupying-revision guard, never auto-deleting", async () => {
    const catalog = installableCatalog();
    const modelId = catalog.models[0].id;
    vi.mocked(getSettings).mockResolvedValue(unconfiguredSettings());
    vi.mocked(aiManagedStatus).mockResolvedValue(catalog);
    vi.mocked(aiInstallManaged)
      .mockRejectedValueOnce(new Error("An incomplete installation already occupies this model revision. Nothing was replaced."))
      .mockResolvedValue({ model_id: modelId, installed: true, message: "Installed after retry." });
    vi.mocked(aiRemoveManaged).mockResolvedValue({ model_id: modelId, removed: true, remaining_installed: 0, message: "Removed." });
    const host = await openSettings();
    button(host, "Review & install…")!.click();
    await tick();
    button(host, "Install model and runtime")!.click();
    await vi.waitFor(() => expect(button(host, "Remove that revision and retry")).toBeDefined());
    expect(aiRemoveManaged).not.toHaveBeenCalled();
    expect(button(host, "Keep files")).toBeDefined();
    button(host, "Remove that revision and retry")!.click();
    await vi.waitFor(() => expect(aiRemoveManaged).toHaveBeenCalledExactlyOnceWith(modelId));
    // The retry's landing catalog reports installed, so the route survives.
    vi.mocked(aiManagedStatus).mockResolvedValue(installedAfter(catalog));
    await vi.waitFor(() => expect(aiInstallManaged).toHaveBeenCalledTimes(2));
    expect(host.textContent).toContain("Installed after retry.");
  });
});

describe("Managed resource Details (ticket 190)", () => {
  function runningCatalog() {
    const status = bundledStatus();
    return {
      ...status,
      runtime: { ...status.runtime, qualified: true, blocker: "", download_size_bytes: 104857600 },
      models: [{ ...status.models[0], installable: true, installed: true, status: "qualified", blocker: "", revision: "test-revision", download_size_bytes: 2147483648, memory_needs_mb: 4096 }],
    };
  }

  function managedSettings(modelId: string) {
    return {
      default_timeout_minutes: 20, log_retention_days: 7, install_dir: "",
      launch_concurrency: 3, dock_mode: "auto-hide", dock_edge: "left",
      dock_state: "floating", dock_width_pct: 18, dock_density: "default",
      autostart: "off", theme: "light", animation: "on", ai_provider: "managed", ai_model: modelId,
      ai_base_url: "http://127.0.0.1:11434", companion_url_list: [],
      launch_groups: "off", action_groups: "off", clip_groups: "off",
      reveal_dwell_ms: 200, reveal_sensitivity_px: 12, companion_url: null,
      companion_height_ratio: 0.4, companion_muted: false, native_frame: "off",
    };
  }

  it("costs nothing while closed — no usage command until disclosed", async () => {
    const catalog = runningCatalog();
    const modelId = catalog.models[0].id;
    vi.mocked(getSettings).mockResolvedValue(managedSettings(modelId));
    vi.mocked(aiManagedStatus).mockResolvedValue(catalog);
    vi.mocked(aiManagedRuntimeStatus).mockResolvedValue({ running: true, active_model_id: modelId, uptime_secs: 40 });
    const host = await openSettings();
    await vi.waitFor(() => expect(host.textContent).toContain("Running"));
    expect(button(host, "Resource usage")).toBeDefined();
    expect(aiManagedResourceUsage).not.toHaveBeenCalled();
  });

  it("shows static copy while stopped without querying the process", async () => {
    const catalog = runningCatalog();
    const modelId = catalog.models[0].id;
    vi.mocked(getSettings).mockResolvedValue(managedSettings(modelId));
    vi.mocked(aiManagedStatus).mockResolvedValue(catalog);
    vi.mocked(aiManagedRuntimeStatus).mockResolvedValue({ running: false, active_model_id: null, uptime_secs: null });
    const host = await openSettings();
    button(host, "Resource usage")!.click();
    await tick();
    expect(host.textContent).toContain("Stopped — will start on next Generate.");
    expect(aiManagedResourceUsage).not.toHaveBeenCalled();
  });

  it("polls live numbers while open + running, and close cancels without persisting metrics", async () => {
    const catalog = runningCatalog();
    const modelId = catalog.models[0].id;
    vi.mocked(getSettings).mockResolvedValue(managedSettings(modelId));
    vi.mocked(aiManagedStatus).mockResolvedValue(catalog);
    vi.mocked(aiManagedRuntimeStatus).mockResolvedValue({ running: true, active_model_id: modelId, uptime_secs: 40 });
    vi.mocked(aiManagedResourceUsage).mockResolvedValue({
      running: true, active_model_id: modelId, uptime_secs: 40,
      working_set_bytes: 256 * 1024 * 1024, cpu_percent: 12.5,
    });
    const host = await openSettings();
    button(host, "Resource usage")!.click();
    await vi.waitFor(() => expect(aiManagedResourceUsage).toHaveBeenCalled());
    await vi.waitFor(() => expect(host.textContent).toContain("256 MB"));
    expect(host.textContent).toContain("Model");
    expect(host.textContent).toContain("Memory");
    expect(host.textContent).toContain("CPU");
    // Close drops the numbers and stops polling — metrics never persist.
    const calls = vi.mocked(aiManagedResourceUsage).mock.calls.length;
    button(host, "Resource usage")!.click();
    await tick();
    expect(host.textContent).not.toContain("256 MB");
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(vi.mocked(aiManagedResourceUsage).mock.calls.length).toBe(calls);
    expect(managedResourceDisclosure.open).toBe(false);
  });
});

describe("Managed install stages + retry (ticket 193 follow-up)", () => {
  function installableCatalog() {
    const status = bundledStatus();
    return {
      ...status,
      runtime: { ...status.runtime, qualified: true, blocker: "", download_size_bytes: 104857600 },
      models: [{ ...status.models[0], installable: true, installed: false, status: "qualified", blocker: "", revision: "test-revision", download_size_bytes: 2147483648, memory_needs_mb: 4096 }],
    };
  }

  function unconfiguredSettings() {
    return {
      default_timeout_minutes: 20, log_retention_days: 7, install_dir: "",
      launch_concurrency: 3, dock_mode: "auto-hide", dock_edge: "left",
      dock_state: "floating", dock_width_pct: 18, dock_density: "default",
      autostart: "off", theme: "light", animation: "on", ai_provider: "managed", ai_model: "",
      ai_base_url: "http://127.0.0.1:11434", companion_url_list: [],
      launch_groups: "off", action_groups: "off", clip_groups: "off",
      reveal_dwell_ms: 200, reveal_sensitivity_px: 12, companion_url: null,
      companion_height_ratio: 0.4, companion_muted: false, native_frame: "off",
    };
  }

  function installedAfter(catalog: ReturnType<typeof installableCatalog>) {
    return { ...catalog, models: [{ ...catalog.models[0], installed: true }] };
  }

  function progressHandler() {
    if (!capturedProgress) throw new Error("progress listener was never registered");
    return capturedProgress;
  }

  it("names each silent post-download step instead of sitting on 100%", async () => {
    const catalog = installableCatalog();
    const modelId = catalog.models[0].id;
    const total = catalog.models[0].download_size_bytes as number;
    vi.mocked(getSettings).mockResolvedValue(unconfiguredSettings());
    vi.mocked(aiManagedStatus).mockResolvedValue(catalog);
    let release!: (value: { model_id: string; installed: boolean; message: string }) => void;
    vi.mocked(aiInstallManaged).mockReturnValue(new Promise((resolve) => { release = resolve; }));
    const host = await openSettings();
    button(host, "Review & install…")!.click();
    await tick();
    button(host, "Install model and runtime")!.click();
    await vi.waitFor(() => expect(aiInstallManaged).toHaveBeenCalledExactlyOnceWith(modelId));
    for (const [stage, copy] of [
      ["verifying-model", "Verifying model"],
      ["extracting", "Extracting runtime"],
      ["activating", "Activating"],
    ] as const) {
      progressHandler()({ payload: { model_id: modelId, phase: "model", stage, downloaded_bytes: total, total_bytes: total } });
      await tick();
      expect(host.textContent).toContain(copy);
    }
    vi.mocked(aiManagedStatus).mockResolvedValue(installedAfter(catalog));
    release({ model_id: modelId, installed: true, message: "Installed." });
    await vi.waitFor(() => expect(host.textContent).toContain("Installed."));
  });

  it("retries a failed install in one click without re-reviewing", async () => {
    const catalog = installableCatalog();
    const modelId = catalog.models[0].id;
    vi.mocked(getSettings).mockResolvedValue(unconfiguredSettings());
    vi.mocked(aiManagedStatus).mockResolvedValue(catalog);
    vi.mocked(aiInstallManaged)
      .mockRejectedValueOnce(new Error("Verifying model failed: checksum mismatch."))
      .mockResolvedValue({ model_id: modelId, installed: true, message: "Installed after retry." });
    const host = await openSettings();
    button(host, "Review & install…")!.click();
    await tick();
    button(host, "Install model and runtime")!.click();
    await vi.waitFor(() => expect(aiInstallManaged).toHaveBeenCalledTimes(1));
    await vi.waitFor(() => expect(button(host, "Retry install")).toBeDefined());
    expect(host.textContent).toContain("Verifying model failed");
    button(host, "Retry install")!.click();
    await vi.waitFor(() => expect(aiInstallManaged).toHaveBeenCalledTimes(2));
    vi.mocked(aiManagedStatus).mockResolvedValue(installedAfter(catalog));
    await vi.waitFor(() => expect(host.textContent).toContain("Installed after retry."));
  });
});

describe("Quick Action setup pointer landing (ticket 192)", () => {
  it("expands the AI group and focuses the provider control, then consumes the flag", async () => {
    sessionStorage.setItem("sprout.settings.focus", "ai-provider");
    try {
      const host = await openSettings();
      expect(host.querySelector("#ai-provider")).not.toBeNull();
      await vi.waitFor(() => expect(document.activeElement?.id).toBe("ai-provider"));
      expect(sessionStorage.getItem("sprout.settings.focus")).toBeNull();
    } finally {
      sessionStorage.removeItem("sprout.settings.focus");
    }
  });
});

describe("Managed install folder (manual deletion)", () => {
  it("shows the on-disk folder with a copy action so the user can delete it by hand", async () => {
    const status = bundledStatus();
    const folder = "C:\\Users\\tester\\AppData\\Local\\Sprout\\ai-managed\\installations\\lite\\rev1";
    const installed = {
      ...status,
      runtime: { ...status.runtime, qualified: true, blocker: "", download_size_bytes: 104857600 },
      models: [{ ...status.models[0], installable: true, installed: true, status: "qualified", blocker: "", revision: "rev1", download_size_bytes: 2147483648, memory_needs_mb: 4096, installed_dir: folder }],
    };
    vi.mocked(aiManagedStatus).mockResolvedValue(installed);
    const writeText = vi.fn().mockResolvedValue(undefined);
    const prototype = Object.getPrototypeOf(navigator);
    const original =
      Object.getOwnPropertyDescriptor(navigator, "clipboard") ??
      Object.getOwnPropertyDescriptor(prototype, "clipboard");
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    try {
      const host = await openSettings();
      await openModelMenu(host, installed.models[0].id);
      button(host, "Model details")!.click();
      await tick();
      expect(host.textContent).toContain("Installed at:");
      expect(host.textContent).toContain("ai-managed");
      button(host, "Copy path")!.click();
      await vi.waitFor(() => expect(writeText).toHaveBeenCalledWith(folder));
      await vi.waitFor(() => expect(host.textContent).toContain("Copied."));
    } finally {
      if (original) {
        Object.defineProperty(navigator, "clipboard", original);
      } else {
        delete (navigator as unknown as Record<string, unknown>).clipboard;
      }
    }
  });
});
