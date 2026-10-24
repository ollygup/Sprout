<script lang="ts">
  import { onMount, tick } from "svelte";
  import { beforeNavigate, goto } from "$app/navigation";
  import type { BackupCounts, CompanionSite, DisplayInfo, ManagedCatalogStatus, ManagedResourceUsage, Settings } from "$lib/types";
  import { aiProviderLabelText } from "$lib/types";
  import type { AiProvider } from "$lib/types";
  import { companionDisplayName, normalizeCompanionSites } from "$lib/companion";
  import {
    aiCheckExistingLocal,
    aiManagedInstallActive,
    aiManagedResourceUsage,
    aiManagedRuntimeStatus,
    aiManagedStatus,
    aiRemoveManaged,
    aiStartManagedRuntime,
    aiStopManagedRuntime,
    exportBackup,
    getDisplayBezelYRatio,
    getDisplayDockEdge,
    getDisplayDockMode,
    getDisplayDockWidthPct,
    getLanguage,
    getSettings,
    importBackup,
    inspectBackup,
    listDisplays,
    setCompanionHeightRatioForDisplay,
    setDisplayBezelYRatio,
    setDisplayDockEdge,
    setDisplayDockMode,
    setDisplayDockWidthPct,
    reconcileQuickLaunchSettings,
    updateAutostart,
    updateSettings,
  } from "$lib/api";
  import { listen } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import Dialog from "$lib/components/Dialog.svelte";
  import Button from "$lib/components/Button.svelte";
  import Checkbox from "$lib/components/Checkbox.svelte";
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import ContextMenu, {
    type ContextMenuItem,
    type ContextMenuState,
  } from "$lib/components/ContextMenu.svelte";
  import Disclosure from "$lib/components/Disclosure.svelte";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import GroupAccordion from "$lib/components/GroupAccordion.svelte";
  import IconButton from "$lib/components/IconButton.svelte";
  import Notice from "$lib/components/Notice.svelte";
  import PageHeader from "$lib/components/PageHeader.svelte";
  import SearchInput from "$lib/components/SearchInput.svelte";
  import Select from "$lib/components/Select.svelte";
  import { open, save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { theme, restoreTheme, selectTheme } from "$lib/theme.svelte";
  import type { ThemeMode } from "$lib/theme.svelte";
  import { localeState, parseLocale, restoreLocale, selectLocale } from "$lib/copy/locale.svelte";
  import { animation, restoreAnimation, selectAnimation } from "$lib/animation.svelte";
  import type { AnimationMode } from "$lib/animation.svelte";
  import {
    nativeFrame,
    restoreNativeFrame,
    selectNativeFrame,
  } from "$lib/nativeFrame.svelte";
  import type { NativeFrameMode } from "$lib/windowChrome";
  import { parseNativeFrame } from "$lib/windowChrome";
  import {
    checkForUpdates,
    installNow,
    updateState,
  } from "$lib/updateState.svelte";
  import { EXPORT_ORDER } from "$lib/collections";
  import type { CollectionKey } from "$lib/collections";  import {
    buildSettingsSearchIndex,
    resolveSettingsFilter,
    SETTINGS_GROUP_KNOBS,
    type SettingsGroupKey,
  } from "$lib/settingsSearch";
  import {
    cancelManagedInstall as cancelStoreInstall,
    dismissManagedInstall,
    formatManagedBytes,
    formatManagedRam,
    formatManagedUptime,
    isOccupyingRevisionError,
    managedInstall,
    managedInstallPercent,
    managedModelParams,
    managedTierLabel,
    reconcileManagedInstall,
    removeAndRetryManagedInstall,
    startManagedInstall,
    watchManagedInstall,
  } from "$lib/managedInstall.svelte";
  import { honorAiSetupFocus } from "$lib/aiSetupPointer";
  import { t, tCount } from "$lib/copy";
  import {
    managedResourceDisclosure,
    setManagedResourceOpen,
  } from "$lib/managedResource.svelte";

  // Option rows are derived (not const) so every dropdown follows a
  // language switch live; the Select rebuilds its popout on every open.
  const themeOptions = $derived<{ mode: ThemeMode; label: string }[]>([
    { mode: "system", label: t("settings.opt.theme.system") },
    { mode: "light", label: t("settings.opt.theme.light") },
    { mode: "dark", label: t("settings.opt.theme.dark") },
  ]);

  const dockModeOptions = $derived<{ value: string; label: string }[]>([
    { value: "auto-hide", label: t("settings.opt.mode.auto") },
    { value: "fixed", label: t("settings.opt.mode.fixed") },
    { value: "bezel", label: t("settings.opt.mode.bezel") },
  ]);

  const dockEdgeOptions = $derived<{ value: string; label: string }[]>([
    { value: "left", label: t("settings.opt.edge.left") },
    { value: "right", label: t("settings.opt.edge.right") },
  ]);

  const dockStateOptions = $derived<{ value: string; label: string }[]>([
    { value: "floating", label: t("settings.opt.state.floating") },
    { value: "docked", label: t("settings.opt.state.docked") },
  ]);

  const dockDensityOptions = $derived<{ value: string; label: string }[]>([
    { value: "compact", label: t("settings.opt.density.compact") },
    { value: "default", label: t("settings.opt.density.default") },
    { value: "large", label: t("settings.opt.density.large") },
  ]);

  const autostartOptions = $derived<{ value: string; label: string }[]>([
    { value: "on", label: t("common.on") },
    { value: "off", label: t("common.off") },
  ]);

  const languageOptions: { value: string; label: string }[] = [
    { value: "en", label: "English" },
    { value: "zh-CN", label: "中文（简体）" },
  ];

  const animationOptions = $derived<{ value: AnimationMode; label: string }[]>([
    { value: "on", label: t("common.on") },
    { value: "off", label: t("common.off") },
  ]);

  const nativeFrameOptions = $derived<{ value: NativeFrameMode; label: string }[]>([
    { value: "off", label: t("settings.opt.frame.modern") },
    { value: "on", label: t("settings.opt.frame.native") },
  ]);

  const aiProviderOptions = $derived<{ value: AiProvider; label: string }[]>([
    { value: "off", label: aiProviderLabelText("off") },
    { value: "existing-local", label: aiProviderLabelText("existing-local") },
    { value: "managed", label: aiProviderLabelText("managed") },
    { value: "cloud", label: aiProviderLabelText("cloud") },
  ]);

  const AI_BASE_URL_DEFAULT = "http://127.0.0.1:11434";

  let settings: Settings | null = $state(null);
  let timeout = $state(10);
  let retention = $state(30);
  let installDir = $state("");
  let launchConcurrency = $state(8);
  let dockMode = $state("auto-hide");
  let dockEdge = $state("left");
  let dockState = $state("floating");
  // Dock width: % of the docked monitor (10–60 stored, default 18 ≈ 346px
  // on 1920). Single size source is constants/window.rs — this mirrors
  // DOCK_WIDTH_{MIN,MAX,DEFAULT}_PCT; fixed applies at most 30 (it reserves
  // workspace, ADR-0011) while auto-hide and bezel may apply to 60 (they overlay).
  // The backend validates the stored range and floors at 340.
  const DOCK_WIDTH_MIN_PCT = 10;
  const DOCK_WIDTH_MAX_PCT_FIXED = 30;
  const DOCK_WIDTH_MAX_PCT_AUTOHIDE = 60;
  const DOCK_WIDTH_MAX_PCT = DOCK_WIDTH_MAX_PCT_AUTOHIDE;
  const DOCK_WIDTH_DEFAULT_PCT = 18;
  const DOCK_WIDTH_FLOOR_PX = 340;
  let dockWidthPct = $state(DOCK_WIDTH_DEFAULT_PCT);
  // List density: the Quick Launch window's list text size across all three
  // tabs — Compact steps rows down one type token, Large steps them up one,
  // Default is today's sizing. Deferred to Save beside the other dock knobs.
  let dockDensity = $state("default");
  let revealDwellMs = $state(200);
  let revealSensitivityPx = $state(12);
  // Four accordion groups on the one route (research 0014: disclosure
  // sections over page splits; search before hierarchy), rendered through
  // the shared GroupAccordion. All open on first visit; the remembered
  // choice survives across visits.
  const GROUP_STORAGE_KEY = "sprout.settings.groups.v1";
  function loadGroupOpen(): Record<SettingsGroupKey, boolean> {
    const allOpen = { general: true, dock: true, companion: true, backup: true, ai: true };
    try {
      const raw = localStorage.getItem(GROUP_STORAGE_KEY);
      if (!raw) return allOpen;
      const parsed = JSON.parse(raw) as Partial<Record<SettingsGroupKey, boolean>>;
      return {
        general: parsed.general ?? true,
        dock: parsed.dock ?? true,
        companion: parsed.companion ?? true,
        backup: parsed.backup ?? true,
        ai: parsed.ai ?? true,
      };
    } catch {
      return allOpen;
    }
  }
  let groupOpen = $state<Record<SettingsGroupKey, boolean>>(loadGroupOpen());
  function toggleGroup(key: SettingsGroupKey) {
    groupOpen = { ...groupOpen, [key]: !groupOpen[key] };
    try {
      localStorage.setItem(GROUP_STORAGE_KEY, JSON.stringify(groupOpen));
    } catch {
      // Storage unavailable — the groups still toggle for this visit.
    }
  }
  function expandGroups(keys: SettingsGroupKey[]) {
    groupOpen = { ...groupOpen, ...Object.fromEntries(keys.map((k) => [k, true])) };
    try {
      localStorage.setItem(GROUP_STORAGE_KEY, JSON.stringify(groupOpen));
    } catch {
      // Storage unavailable — the groups still expand for this visit.
    }
  }
  // The local filter (research 0014 rule 6): matches knob labels, synonyms,
  // current values, and one-line descriptions from the data-driven index.
  let filter = $state("");
  let autostart = $state("on");
  // Companion: active URL (null=off), height ratio 0.25–0.60, saved sites with names
  let companionUrl: string | null = $state(null);
  let companionHeightRatio = $state(0.40);
  let companionUrlList = $state<CompanionSite[]>([]);
  // AI assistance (ADR-0031): off until configured. The route is Save-deferred
  // like every other knob; Test connection below is the explicit command.
  let aiProvider = $state<AiProvider>("off");
  let aiBaseUrl = $state(AI_BASE_URL_DEFAULT);
  let aiModel = $state("");
  let aiTestBusy = $state(false);
  let aiTestStatus = $state("");
  let aiTestError = $state("");
  let managedCatalog = $state<ManagedCatalogStatus | null>(null);
  // Remove-only busy (ticket 186 path); installs ride the app-level
  // `managedInstall` store (ticket 193) so progress survives tab switches.
  let managedBusy = $state(false);
  let managedError = $state("");
  let managedNotice = $state("");
  let managedDetailsOpen = $state(false);
  let managedReviewId = $state<string | null>(null);
  // Ticket 189: the cheap running indicator. Polled ~5s while Settings is
  // open; the interval dies on unmount so closed Settings costs zero.
  let runtimeRunning = $state(false);
  let runtimeActiveId = $state<string | null>(null);
  let runtimeUptimeSecs = $state<number | null>(null);
  // Separate flags per action: one shared busy flag plus a status-derived
  // label wedged the control on "Stopping…" (disabled) whenever a start was
  // still polling health while the process already showed as running — only
  // a remount cleared it. Stop applies to a live process even mid-startup
  // (it cancels the wait); only a stop in flight or a not-yet-visible
  // process disables the control.
  let startBusy = $state(false);
  let stopBusy = $state(false);
  const stopApplies = $derived(runtimeRunning && !stopBusy);
  const startApplies = $derived(!runtimeRunning && !startBusy && !stopBusy);
  let runtimeError = $state("");
  let statusTimer: ReturnType<typeof setInterval> | null = null;
  // Ticket 190: lazy resource Details (research 0004 rule 2 rare-behind-obvious,
  // 0006 pattern 7 collapsible over page splits). Collapsed by default; only
  // the open flag survives tab switches (app-level store — the page unmounts
  // on navigation); metrics never persist (local, refetched). Closed costs
  // nothing (no command, no interval); open + running polls ~2s; open +
  // stopped shows static copy. Numbers update as text with no animated
  // gauges, so the Animation switch and OS reduced-motion have nothing to gate.
  const resourceOpen = $derived(managedResourceDisclosure.open);
  let resourceUsage = $state<ManagedResourceUsage | null>(null);
  let resourceError = $state("");
  let resourceTimer: ReturnType<typeof setInterval> | null = null;
  // Ticket 193: the store reset (reload) while the worker kept the flight —
  // honest background notice with Cancel instead of a second flight.
  let backgroundInstall = $state(false);
  const installInFlight = $derived(managedInstall.status === "installing");
  const installPercent = $derived(managedInstallPercent());
  // Silent post-download steps breathe: dots cycle under the stage copy while
  // a long hash runs, so the line visibly lives. Dots are decorative motion —
  // the Animation switch or OS reduced-motion freezes them to a static mark
  // while the stage copy keeps naming the work.
  let installTick = $state(0);
  let installDotsTimer: ReturnType<typeof setInterval> | null = null;
  const installDots = $derived.by(() => {
    if (!installInFlight || managedInstall.stage === "downloading") return "";
    if (!installMotionOn()) return "…";
    return [".", "..", "..."][installTick % 3];
  });
  const managedInstalled = $derived(managedCatalog?.models.filter((model) => model.installed) ?? []);
  // Ticket 191: the explicit Active model. Save-deferred like the provider —
  // the radio drafts `aiModel`, Save persists it. No fallback to the first
  // installed entry: a dangling selection (removed model, fresh install)
  // reads as no active until the user picks one explicitly — no silent
  // switch. Start/Stop and the status line act on this draft only.
  const activeManagedId = $derived(
    aiModel.trim() && managedInstalled.some((model) => model.id === aiModel.trim())
      ? aiModel.trim()
      : null,
  );
  // Ticket 193: the occupying-revision guard carries explicit recovery.
  const installGuardHit = $derived(
    managedInstall.status === "error" &&
      managedInstall.error !== null &&
      isOccupyingRevisionError(managedInstall.error),
  );
  const managedChoices = $derived(managedCatalog?.models.filter((model) => model.installable || model.installed) ?? []);
  // The install list shows only what is NOT on disk yet (0006 pattern 2:
  // what you have lives in the Active cards above; what you can get lists
  // here). Installed entries never render twice.
  const managedAvailable = $derived(managedChoices.filter((model) => !model.installed));
  const managedReviewModel = $derived(managedCatalog?.models.find((model) => model.id === managedReviewId));
  // Whether the companion knobs were authored on this page since mount —
  // the page loads once while the dock divider and the companion manager
  // write out-of-band, so save must tell "left alone" from "edited here".
  let companionUrlTouched = $state(false);
  let companionRatioTouched = $state(false);
  let loading = $state(true);
  let loadFailed = $state(false);
  let saving = $state(false);
  let saved = $state("");
  let error = $state("");

  // Per-monitor dock (ticket 111): only when >1 display; global above stays fallback.
  let displays = $state<DisplayInfo[]>([]);
  let physicalDisplays = $state<DisplayInfo[]>([]);
  let displayEdges = $state<Record<string, string>>({});
  let displayModes = $state<Record<string, string>>({});
  // Ticket 128: each display remembers its own width % too.
  let displayWidths = $state<Record<string, number>>({});
  // Spec 214 (ticket 218): each display remembers its bezel tab Y ratio too
  // (0..1 of the tab's travel) — the Settings mirror of the dock drag.
  let displayBezelY = $state<Record<string, number>>({});
  let displayErrors = $state<Record<string, string>>({});
  // The single-display tab-position knob (spec 214): a view onto the one
  // display's memory — several displays use their own per-row sliders below.
  let bezelYRatio = $state(0.5);

  // Ticket 115: dirty snapshot — post-clamp comparison against the loaded values.
  // Theme and autostart are immediate (tickets 31/75) and never dirty; per-monitor
  // choices are deferred until Save and count as dirty.
  let baseline = $state<{
    timeout: number;
    retention: number;
    installDir: string;
    launchConcurrency: number;
    dockMode: string;
    dockEdge: string;
    dockState: string;
    dockWidthPct: number;
    /** The single-display bezel tab position (spec 214): 0..1 of the travel,
     *  centered until moved — post-clamp, like the width % above. */
    bezelYRatio: number;
    dockDensity: string;
    revealDwellMs: number;
    revealSensitivityPx: number;
    companionUrl: string | null;
    companionHeightRatio: number;
    companionUrlList: CompanionSite[];
    aiProvider: AiProvider;
    aiBaseUrl: string;
    aiModel: string;
  } | null>(null);
  let baselineDisplayEdges = $state<Record<string, string>>({});
  let baselineDisplayModes = $state<Record<string, string>>({});
  let baselineDisplayWidths = $state<Record<string, number>>({});
  let baselineDisplayBezelY = $state<Record<string, number>>({});

  function clampTimeout(v: number): number {
    return Math.max(1, Math.floor(v) || 1);
  }
  function clampRetention(v: number): number {
    return Math.max(1, Math.floor(v) || 1);
  }
  function clampConcurrency(v: number): number {
    return Math.min(50, Math.max(1, Math.floor(v) || 1));
  }
  function clampDwell(v: number): number {
    return Math.min(1000, Math.max(0, Math.floor(v) || 0));
  }
  function clampSens(v: number): number {
    return Math.min(50, Math.max(0, Math.floor(v) || 0));
  }
  function clampWidthPct(v: number): number {
    const n = Math.floor(Number(v));
    if (!Number.isFinite(n)) return DOCK_WIDTH_DEFAULT_PCT;
    return Math.min(DOCK_WIDTH_MAX_PCT, Math.max(DOCK_WIDTH_MIN_PCT, n));
  }
  /** The slider cap for a dock mode: fixed stays a strip (30), auto-hide
   * and bezel may overlay wide (60 — bezel reserves nothing extra, spec 214).
   * Unknown modes take the fixed cap — the conservative reservation assumption. */
  function dockWidthMaxForMode(mode: string): number {
    return mode === "auto-hide" || mode === "bezel" ? DOCK_WIDTH_MAX_PCT_AUTOHIDE : DOCK_WIDTH_MAX_PCT_FIXED;
  }
  /** Clamps a width % into a mode's applied range, so a mode switch re-clamps
   * honestly: 55 in auto-hide becomes 30 in fixed. */
  function clampWidthPctForMode(v: number, mode: string): number {
    const n = Math.floor(Number(v));
    if (!Number.isFinite(n)) return DOCK_WIDTH_DEFAULT_PCT;
    return Math.min(dockWidthMaxForMode(mode), Math.max(DOCK_WIDTH_MIN_PCT, n));
  }
  /** Clamps a bezel tab Y ratio into 0..1 (spec 214): a broken value
   *  centers — the same fallback the backend applies, so the knob never
   *  holds an unrepresentable position. Mirrors the dock window's clamp. */
  function clampBezelYRatio(v: number): number {
    const f = Number(v);
    if (!Number.isFinite(f)) return 0.5;
    return Math.min(1, Math.max(0, f));
  }
  /** A stored density the menu does not offer reads back as today's sizing —
   *  the same fallback the backend applies, so a broken value never leaves
   *  the knob in an unrepresentable state. */
  function validDensity(value: unknown): string {
    return value === "compact" || value === "large" ? (value as string) : "default";
  }
  /** Effective strip px for `monitorWidth` at `pct` % in `mode` (ticket 128;
   * per-mode caps in ADR-0021): mirrors the backend `dock_width_px_for_mode`
   * — % of monitor, floored at 340, capped at the mode's cap — so the
   * slider's readout tells the truth per display. */
  function effectiveWidthPx(monitorWidth: number, pct: number, mode?: string): number {
    if (!Number.isFinite(monitorWidth) || monitorWidth <= 0) return DOCK_WIDTH_FLOOR_PX;
    const max = mode ? dockWidthMaxForMode(mode) : DOCK_WIDTH_MAX_PCT;
    const n = Math.floor(Number(pct));
    const p = Number.isFinite(n)
      ? Math.min(max, Math.max(DOCK_WIDTH_MIN_PCT, n))
      : DOCK_WIDTH_DEFAULT_PCT;
    const cap = Math.floor((monitorWidth * max) / 100);
    const want = Math.floor((monitorWidth * p) / 100);
    return Math.min(Math.max(want, DOCK_WIDTH_FLOOR_PX), Math.max(cap, DOCK_WIDTH_FLOOR_PX));
  }
  function clampCompanionRatio(v: number): number {
    const f = Number(v);
    if (!Number.isFinite(f)) return 0.40;
    return Math.min(0.60, Math.max(0.25, Math.round(f * 100) / 100));
  }
  function normalizeCompanionList(list: CompanionSite[]): CompanionSite[] {
    return normalizeCompanionSites(list);
  }

  /** A stored switch position the menu does not offer reads back as on —
   *  the same fallback the backend applies, so a broken value never freezes
   *  the UI. */
  function validAnimation(value: unknown): AnimationMode {
    return value === "off" ? "off" : "on";
  }

  /** A stored route the menu does not offer reads back as off — the same
   *  fallback the backend applies, so a broken value never wakes inference. */
  function validAiProvider(value: unknown): AiProvider {
    return value === "existing-local" || value === "managed" || value === "cloud"
      ? (value as AiProvider)
      : "off";
  }

  /** Tests the existing-local service without saving anything: classifies the
   *  endpoint and checks the named model against what the service exposes.
   *  An explicit command, distinct from the Save-deferred fields above. */
  async function testAiConnection() {
    aiTestBusy = true;
    aiTestStatus = "";
    aiTestError = "";
    try {
      const exposed = await aiCheckExistingLocal(aiBaseUrl.trim(), aiModel.trim());
      aiTestStatus =
        exposed.length === 1
          ? t("settings.testOkOne").replace("{name}", exposed[0])
          : t("settings.testOkMany")
              .replace("{count}", String(exposed.length))
              .replace("{list}", exposed.join(", "));
    } catch (e) {
      aiTestError = String(e);
    } finally {
      aiTestBusy = false;
    }
  }

  async function loadManagedCatalog() {
    managedError = "";
    try {
      managedCatalog = await aiManagedStatus();
      // A saved managed model that is no longer installed (removed outside
      // the app, or a partial install) must not leave generation stuck on a
      // route with nothing to serve — fall back to Off so the next Generate
      // says plainly that assistance is off instead of failing on the
      // missing files. Save persists the fallback like any other knob.
      if (
        aiProvider === "managed" &&
        aiModel.trim() !== "" &&
        (managedCatalog?.models.some((model) => model.installed) !== true)
      ) {
        aiProvider = "off";
        aiModel = "";
        managedNotice = t("settings.modelGone");
      }
    } catch (cause) {
      managedCatalog = null;
      managedError = String(cause);
    }
  }

  // Ticket 193: installs ride the app-level store (bar + % + bytes fed by
  // backend progress events), so every surface reads the same flight and a
  // second start while one runs is a no-op instead of a per-tab busy string.
  // Success selects the model only when nothing is active yet (first install
  // or dangling selection) — ticket 191: later installs never implicitly
  // retarget the Active radio; failure/cancel never presents a partial
  // install as installed (the catalog reload is the proof).
  async function installManaged(modelId: string) {
    if (managedInstall.status === "installing" || managedBusy) return;
    managedError = "";
    managedNotice = "";
    backgroundInstall = false;
    await startManagedInstall(modelId);
    if (managedInstall.status === "done") {
      // Ticket 191: establish an active only when the draft names no
      // installed model (first install or dangling selection). A healthy
      // active survives later installs — no implicit retarget.
      const installedIds = new Set(
        (managedCatalog?.models ?? []).filter((model) => model.installed).map((model) => model.id),
      );
      if (!installedIds.has(aiModel.trim()) || aiModel.trim() === "") {
        aiModel = modelId;
      }
      managedNotice = managedInstall.notice ?? "";
      await loadManagedCatalog();
      await refreshRuntimeStatus();
    } else if (managedInstall.status === "error" && !installGuardHit) {
      managedError = managedInstall.error ?? t("managed.installFailed");
    }
  }

  function cancelManagedInstall() {
    cancelStoreInstall();
    backgroundInstall = false;
    if (managedInstall.status === "installing") {
      managedNotice = managedInstall.notice ?? "";
    }
  }

  // Ticket 193 guard-hit recovery: explicitly removes the occupying
  // app-owned revision, then retries the same install. Never auto-deletes —
  // this runs only from the "Remove that revision and retry" action.
  async function retryAfterRemove(modelId: string) {
    if (managedInstall.status === "installing" || managedBusy) return;
    managedError = "";
    managedNotice = "";
    await removeAndRetryManagedInstall(modelId);
    if (managedInstall.status === "done") {
      aiModel = modelId;
      managedNotice = managedInstall.notice ?? "";
      await loadManagedCatalog();
      await refreshRuntimeStatus();
    } else if (managedInstall.status === "error" && !installGuardHit) {
      managedError = managedInstall.error ?? t("managed.installFailed");
    }
  }

  function keepInstallFiles() {
    dismissManagedInstall();
    managedError = "";
  }

  function retryManagedInstall() {
    const modelId = managedInstall.modelId;
    dismissManagedInstall();
    if (modelId) void installManaged(modelId);
  }

  // Ticket 189: Start pre-warms via the health path so metrics read live
  // immediately; Stop calls the existing stop path and keeps
  // `provider=managed` + `ai_model` — neither ever mutates the selection,
  // and Generate after Stop lazily restarts.
  async function refreshRuntimeStatus() {
    if (aiProvider !== "managed" || !activeManagedId) {
      runtimeRunning = false;
      runtimeActiveId = null;
      runtimeUptimeSecs = null;
      return;
    }
    try {
      const status = await aiManagedRuntimeStatus();
      runtimeRunning = status.running;
      runtimeActiveId = status.active_model_id;
      runtimeUptimeSecs = status.uptime_secs;
    } catch (cause) {
      console.error("managed runtime status failed", cause);
    }
  }

  function startStatusPoll() {
    stopStatusPoll();
    void refreshRuntimeStatus();
    statusTimer = setInterval(() => {
      void refreshRuntimeStatus();
    }, 5000);
  }

  function stopStatusPoll() {
    if (statusTimer !== null) {
      clearInterval(statusTimer);
      statusTimer = null;
    }
  }

  // Ticket 190: lazy resource fetch. Closed costs nothing; open + running
  // polls ~2s; open + stopped never queries (static copy instead). A stopped
  // answer flips the status line promptly so the panel never shows stale live
  // numbers past a crash. One query at a time (a slow answer never stacks),
  // and repeated failures pause the poll instead of hammering the machine.
  let resourceFetchInFlight = false;
  let resourceFailures = 0;
  async function refreshResourceUsage() {
    if (!resourceOpen || aiProvider !== "managed" || !activeManagedId || !runtimeRunning) return;
    if (resourceFetchInFlight) return;
    resourceFetchInFlight = true;
    try {
      const usage = await aiManagedResourceUsage();
      if (!resourceOpen) return;
      if (!usage.running) {
        runtimeRunning = false;
        runtimeActiveId = null;
        runtimeUptimeSecs = null;
        resourceUsage = null;
        resourceError = "";
        resourceFailures = 0;
        stopResourcePoll();
        return;
      }
      resourceUsage = usage;
      resourceError = "";
      resourceFailures = 0;
    } catch (cause) {
      console.error("managed resource usage failed", cause);
      resourceError = String(cause);
      resourceFailures += 1;
      if (resourceFailures >= 3) stopResourcePoll();
    } finally {
      resourceFetchInFlight = false;
    }
  }

  function startResourcePoll() {
    stopResourcePoll();
    if (!resourceOpen || aiProvider !== "managed" || !activeManagedId || !runtimeRunning) return;
    void refreshResourceUsage();
    resourceTimer = setInterval(() => {
      void refreshResourceUsage();
    }, 2000);
  }

  function stopResourcePoll() {
    if (resourceTimer !== null) {
      clearInterval(resourceTimer);
      resourceTimer = null;
    }
  }

  function syncResourcePoll() {
    if (!resourceOpen || aiProvider !== "managed" || !activeManagedId || !runtimeRunning) {
      stopResourcePoll();
      if (!resourceOpen) {
        resourceUsage = null;
        resourceError = "";
      }
      return;
    }
    startResourcePoll();
  }

  function toggleResourceDetails() {
    const next = !resourceOpen;
    setManagedResourceOpen(next);
    if (!next) {
      stopResourcePoll();
      resourceUsage = null;
      resourceError = "";
      return;
    }
    resourceError = "";
    resourceFailures = 0;
    if (runtimeRunning) startResourcePoll();
  }

  function formatResourceCpu(cpu: number | null): string {
    if (cpu === null || !Number.isFinite(cpu)) return t("managed.measuring");
    return `${new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 }).format(cpu)}%`;
  }

  function installMotionOn(): boolean {
    if (animation.mode !== "on") return false;
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") return true;
    return !window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  }

  function stopInstallDots() {
    if (installDotsTimer !== null) {
      clearInterval(installDotsTimer);
      installDotsTimer = null;
    }
    installTick = 0;
  }

  async function startManaged() {
    if (!activeManagedId || startBusy || stopBusy) return;
    startBusy = true;
    runtimeError = "";
    try {
      await aiStartManagedRuntime(activeManagedId);
      managedNotice = t("managed.startedFlash").replace("{tier}", managedTierLabel(activeManagedId));
      await refreshRuntimeStatus();
    } catch (cause) {
      const message = String(cause);
      // A prewarm cancelled mid-flight was cancelled by Stop or by leaving
      // the managed route — those paths already announce, so reporting it as
      // an error would blame the user's own Stop.
      if (
        !message.includes("cancelled during runtime startup") &&
        !message.includes("cancelled before startup")
      ) {
        runtimeError = message;
      }
    } finally {
      startBusy = false;
    }
  }

  async function stopManaged() {
    if (stopBusy) return;
    stopBusy = true;
    runtimeError = "";
    try {
      await aiStopManagedRuntime();
      managedNotice = t("managed.stoppedDownloads");
      await refreshRuntimeStatus();
    } catch (cause) {
      runtimeError = String(cause);
    } finally {
      stopBusy = false;
    }
  }

  let managedConfirmRemoveId = $state<string | null>(null);
  let copyPathFeedback = $state("");
  let copyPathTimer: ReturnType<typeof setTimeout> | null = null;

  function askRemoveManaged(modelId: string) {
    managedConfirmRemoveId = null;
    reviewManaged(modelId);
    managedConfirmRemoveId = modelId;
  }

  /** One ⋯ menu per installed model, shared by the Active list and the
   *  downloads list (0005 rule 5 — same rows, same treatment). Details
   *  first, Remove danger-last behind a separator (0006 pattern 10);
   *  controls sit at their object (0006 pattern 4). */
  let modelMenu: (ContextMenuState & { modelId?: string }) | null =
    $state(null);

  function openModelMenu(
    modelId: string,
    anchor: HTMLButtonElement,
    viaKeyboard: boolean
  ) {
    if (modelMenu?.modelId === modelId) {
      modelMenu = null;
      return;
    }
    const items: ContextMenuItem[] = [
      {
        label: t("managed.modelDetails"),
        icon: "info",
        onselect: () => reviewManaged(modelId),
      },
      { label: "", separator: true, onselect: () => {} },
      {
        label: t("managed.removeEllipsis"),
        icon: "trash",
        danger: true,
        onselect: () => askRemoveManaged(modelId),
      },
    ];
    modelMenu = {
      modelId,
      open: true,
      label: t("packet.actionsFor").replace("{name}", modelId),
      anchor,
      focusFirst: viaKeyboard,
      returnTo: anchor,
      items,
    };
  }

  /** Moment-of-use preview for removal: the owned targets plus the space
   *  released where known. Never a permanent artifact listing — this text
   *  exists only on the removal path. */
  function managedRemovePreview(modelId: string): string {
    const model = managedCatalog?.models.find((entry) => entry.id === modelId);
    if (!model) return t("managed.removePreviewBare");
    const runtimeSize = managedCatalog?.runtime.download_size_bytes ?? null;
    return t("managed.removePreviewFor")
      .replace("{artifact}", model.artifact)
      .replace("{modelSize}", managedSize(model.download_size_bytes))
      .replace("{runtimeSize}", managedSize(runtimeSize));
  }

  async function removeManaged(modelId: string) {
    if (managedBusy) return;
    managedBusy = true;
    managedError = "";
    managedNotice = "";
    try {
      const result = await aiRemoveManaged(modelId);
      managedConfirmRemoveId = null;
      await loadManagedCatalog();
      await refreshRuntimeStatus();
      const installed =
        managedCatalog?.models.filter((model) => model.installed) ?? [];
      if (result.removed && installed.length === 0) {
        // Nothing left to serve — fall back to Off rather than sticking on
        // an empty managed route. Save persists it like any other knob.
        aiProvider = "off";
        aiModel = "";
        managedNotice = t("managed.removedToOff").replace("{message}", result.message);
      } else {
        // Ticket 191: survivors keep the draft exactly as authored — even
        // when the removed entry was the active one. The dangling draft
        // reads as not-ready until the user picks a new Active model
        // explicitly; no silent switch to a survivor.
        managedNotice = result.message;
      }
    } catch (cause) {
      managedError = String(cause);
    } finally {
      managedBusy = false;
    }
  }

  function managedSize(bytes: number | null): string {
    if (bytes === null) return t("managed.sizeUnverified");
    const gib = bytes / 1024 / 1024 / 1024;
    return `${new Intl.NumberFormat(undefined, { maximumFractionDigits: 2 }).format(gib)} GiB`;
  }

  // Ticket 193: one progress line for the row compact bar and the dialog
  // full bar (`Step 1 of 2 · Runtime · 38% · 412 MB of 1.04 GB`) so the two
  // cannot disagree. The step prefix is load-bearing: runtime 100% then model
  // 0% is the plan (two artifacts), not a restart — without it the phase
  // switch reads as "completed, then redownloading". Past 100% the backend's
  // stage names the silent work (verifying → extracting → activating) while
  // dots breathe beneath it, so a gigabyte hash never reads as a stuck bar.
  function installStageCopy(): string {
    switch (managedInstall.stage) {
      case "verifying-runtime":
        return t("managed.stageVerifyingRuntime");
      case "verifying-model":
        return t("managed.stageVerifyingModel");
      case "extracting":
        return t("managed.stageExtracting");
      case "activating":
        return t("managed.stageActivating");
      default:
        return "";
    }
  }

  function installProgressLine(): string {
    const step = managedInstall.phase === "runtime" ? t("managed.stepRuntime") : t("managed.stepModel");
    const pct = installPercent;
    if (pct === null || managedInstall.totalBytes <= 0)
      return t("managed.progressStarting").replace("{step}", step);
    const bytes = t("managed.bytesOf")
      .replace("{a}", formatManagedBytes(managedInstall.downloadedBytes))
      .replace("{b}", formatManagedBytes(managedInstall.totalBytes));
    const stageCopy = installStageCopy();
    if (stageCopy)
      return t("managed.progressStage")
        .replace("{step}", step)
        .replace("{bytes}", bytes)
        .replace("{stage}", stageCopy)
        .replace("{dots}", installDots);
    // WHY the generic tail: events from an older backend carry no stage, so
    // 100% with the flight still open still names the likeliest work.
    if (pct >= 100)
      return t("managed.progressVerifying")
        .replace("{step}", step)
        .replace("{bytes}", bytes);
    return `${step} · ${pct}% · ${bytes}`;
  }

  function reviewManaged(modelId: string | null = null) {
    managedReviewId = modelId;
    managedDetailsOpen = true;
    copyPathFeedback = "";
  }

  async function copyInstallPath() {
    const path = managedReviewModel?.installed_dir;
    if (!path) return;
    try {
      if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(path);
      } else if (typeof document !== "undefined") {
        const area = document.createElement("textarea");
        area.value = path;
        area.setAttribute("readonly", "");
        area.style.position = "absolute";
        area.style.left = "-9999px";
        document.body.appendChild(area);
        area.select();
        document.execCommand("copy");
        document.body.removeChild(area);
      } else {
        return;
      }
      copyPathFeedback = t("common.copiedDone");
      if (copyPathTimer !== null) clearTimeout(copyPathTimer);
      copyPathTimer = setTimeout(() => (copyPathFeedback = ""), 1500);
    } catch (e) {
      console.error(e);
    }
  }

  function useExistingLocal() {
    aiProvider = "existing-local";
    aiModel = "";
    filter = "";
    expandGroups(["ai"]);
    void tick().then(() => document.getElementById("ai-base-url")?.focus());
  }

  const isDirty = $derived.by(() => {
    if (!settings || !baseline) return false;
    if (clampTimeout(timeout) !== baseline.timeout) return true;
    if (clampRetention(retention) !== baseline.retention) return true;
    if (installDir.trim() !== baseline.installDir) return true;
    if (clampConcurrency(launchConcurrency) !== baseline.launchConcurrency) return true;
    if (dockMode !== baseline.dockMode) return true;
    if (dockEdge !== baseline.dockEdge) return true;
    if (dockState !== baseline.dockState) return true;
    if (clampWidthPctForMode(dockWidthPct, dockMode) !== baseline.dockWidthPct) return true;
    if (clampBezelYRatio(bezelYRatio) !== baseline.bezelYRatio) return true;
    if (dockDensity !== baseline.dockDensity) return true;
    if (clampDwell(revealDwellMs) !== baseline.revealDwellMs) return true;
    if (clampSens(revealSensitivityPx) !== baseline.revealSensitivityPx) return true;
    if ((companionUrl ?? null) !== (baseline.companionUrl ?? null)) return true;
    if (clampCompanionRatio(companionHeightRatio) !== baseline.companionHeightRatio) return true;
    if (JSON.stringify(normalizeCompanionList(companionUrlList)) !== JSON.stringify(baseline.companionUrlList)) return true;
    if (aiProvider !== baseline.aiProvider) return true;
    if (aiBaseUrl.trim() !== baseline.aiBaseUrl) return true;
    if (aiModel.trim() !== baseline.aiModel) return true;
    if (displays.length > 1) {
      for (const d of displays) {
        const cur = displayEdges[d.device_name];
        const base = baselineDisplayEdges[d.device_name];
        if (cur !== base) return true;
      }
      for (const d of displays) {
        const cur = displayModes[d.device_name];
        const base = baselineDisplayModes[d.device_name];
        if (cur !== base) return true;
      }
      for (const d of displays) {
        const rowMode = displayModes[d.device_name] ?? dockMode;
        const cur = clampWidthPctForMode(displayWidths[d.device_name] ?? DOCK_WIDTH_DEFAULT_PCT, rowMode);
        const base = baselineDisplayWidths[d.device_name];
        if (cur !== base) return true;
      }
      for (const d of displays) {
        const curY = clampBezelYRatio(displayBezelY[d.device_name] ?? 0.5);
        const baseY = baselineDisplayBezelY[d.device_name];
        if (curY !== baseY) return true;
      }
    }
    return false;
  });

  const filtering = $derived(filter.trim().length > 0);
  // Update state in user terms, feeding the search index so update queries
  // land on the Sprout updates knob.
  const backupSummary = $derived.by(() => {
    if (updateState.available)
      return t("update.readyInstall").replace("{version}", updateState.available.version);
    if (checkResult === "current") return t("update.upToDate");
    return t("settings.backupTitle");
  });
  const searchIndex = $derived.by(() =>
    buildSettingsSearchIndex({
      themeMode: theme.mode,
      themeLabel: themeOptions.find((o) => o.mode === theme.mode)?.label ?? t("settings.opt.theme.system"),
      language: localeState.current,
      languageLabel: languageOptions.find((o) => o.value === localeState.current)?.label ?? "English",
      animation: animation.mode,
      animationLabel: animationOptions.find((o) => o.value === animation.mode)?.label ?? t("common.on"),
      nativeFrame: nativeFrame.mode,
      nativeFrameLabel: nativeFrameOptions.find((o) => o.value === nativeFrame.mode)?.label ?? t("settings.opt.frame.modern"),
      installDir,
      autostart,
      timeoutMinutes: clampTimeout(timeout),
      retentionDays: clampRetention(retention),
      launchConcurrency: clampConcurrency(launchConcurrency),
      dockMode,
      dockEdge,
      dockState,
      dockWidthPct: clampWidthPctForMode(dockWidthPct, dockMode),
      bezelYRatioPct: Math.round(clampBezelYRatio(bezelYRatio) * 100),
      dockDensity,
      revealDwellMs: clampDwell(revealDwellMs),
      revealSensitivityPx: clampSens(revealSensitivityPx),
      companionActiveName: companionUrl
        ? (companionUrlList.find((s) => s.url === companionUrl)?.name.trim() ||
          companionDisplayName({ url: companionUrl, name: "" }))
        : null,
      companionRatioPct: Math.round(clampCompanionRatio(companionHeightRatio) * 100),
      companionSiteCount: companionUrlList.length,
      companionSiteNames: companionUrlList.map((s) => companionDisplayName(s)),
      companionMuted: settings?.companion_muted ?? false,
      updateSummary: backupSummary,
      aiProvider,
      aiProviderLabel: aiProviderOptions.find((o) => o.value === aiProvider)?.label ?? t("common.off"),
      aiModel: aiModel.trim(),
    }),
  );
  const resolution = $derived(resolveSettingsFilter(searchIndex, filter));
  // A knob shows while idle, or when the resolver kept it; knob matches win
  // over group matches, so a precise query never surfaces a whole group.
  function knobVisible(id: string): boolean {
    if (!filtering) return true;
    return resolution.visibleKnobIds.has(id);
  }
  function groupVisible(group: SettingsGroupKey): boolean {
    if (!filtering) return true;
    return SETTINGS_GROUP_KNOBS[group].some((id) => resolution.visibleKnobIds.has(id));
  }
  // The badge counts what the section holds — the visible knobs while
  // filtering, so the number can never disagree with the page.
  function groupKnobCount(group: SettingsGroupKey): number {
    const ids = SETTINGS_GROUP_KNOBS[group];
    if (!filtering) return ids.length;
    return ids.filter((id) => resolution.visibleKnobIds.has(id)).length;
  }
  // While filtering, matching groups open so no match hides behind a collapse
  // — the same filter pattern every other list page follows.
  function groupEffectiveOpen(key: SettingsGroupKey): boolean {
    return filtering ? groupVisible(key) : groupOpen[key];
  }
  const matchCount = $derived(resolution.visibleKnobIds.size);
  const noGroupVisible = $derived(
    filtering &&
      !groupVisible("general") &&
      !groupVisible("dock") &&
      !groupVisible("companion") &&
      !groupVisible("backup") &&
      !groupVisible("ai"),
  );
  // Ticket 115: polite live region that announces appearance and disappearance
  // without moving focus or scrolling — text + color, never color alone.
  let dirtyLiveMessage = $state("");
  $effect(() => {
    if (isDirty) {
      dirtyLiveMessage = t("settings.dirtyMsg");
    } else if (dirtyLiveMessage.startsWith(t("settings.dirtyPrefix"))) {
      dirtyLiveMessage = t("settings.dirtyClear");
      const clearTimer = setTimeout(() => {
        dirtyLiveMessage = "";
      }, 1500);
      return () => clearTimeout(clearTimer);
    }
  });

  // Ticket 116: guard leaving dirty Settings — rail navigation + window close
  // share one three-way dialog. State mirrors research 0008: consequence-named
  // actions, never Yes/No.
  let guardOpen = $state(false);
  let pendingNav: string | null = $state(null);
  let pendingClose = $state(false);
  let guardSaving = $state(false);

  // Sync dirty flag to the backend so the Rust close handler can gate on it
  // (dirty while the Settings route is alive; the flag is cleared on save,
  // discard, or unmount). Fire-and-forget.
  $effect(() => {
    void invoke("set_settings_dirty", { dirty: isDirty }).catch(() => {});
  });

  // Ticket 190: keeps the lazy Details poll in step with its gate — open +
  // running + managed + active installed polls, anything else costs nothing.
  // Reads only the gate; writes only the interval (plus a metrics clear on
  // close), so it never loops on the numbers it fetches.
  $effect(() => {
    resourceOpen;
    runtimeRunning;
    aiProvider;
    activeManagedId;
    syncResourcePoll();
  });

  // Breathes dots under a silent install stage — runs only while a flight is
  // past downloading with motion on; anything else (or unmount via the
  // returned cleanup) stops the interval and parks the dots.
  $effect(() => {
    const silent = installInFlight && managedInstall.stage !== "downloading";
    const motion = installMotionOn();
    if (!silent || !motion) return;
    installDotsTimer = setInterval(() => {
      installTick += 1;
    }, 700);
    return () => stopInstallDots();
  });

  beforeNavigate((nav) => {
    if (!isDirty || guardOpen) return;
    // willUnload is the window-close path — handled by the Rust emit
    // (`settings-dirty-close-requested`) so the same dialog is shared.
    if (nav.willUnload) return;
    const dest = nav.to?.url.pathname;
    if (!dest || dest === "/settings") return;
    nav.cancel();
    pendingNav = nav.to!.url.pathname + nav.to!.url.search + nav.to!.url.hash;
    pendingClose = false;
    guardOpen = true;
  });

  // The manual update check's outcome (ticket 74): the found-version state
  // itself lives in the shared store, so the rail pill follows along.
  let checking = $state(false);
  let checkResult = $state<"idle" | "current" | "failed">("idle");
  let installConfirmOpen = $state(false);
  let installError = $state("");

  // The whole-app backup (ticket 80): its own notices, separate from the
  // form's save feedback.
  let backupBusy = $state(false);
  let backupStatus = $state("");
  let backupError = $state("");
  let restoreFile = $state("");
  let restoreCounts: BackupCounts | null = $state(null);

  // Selective export (ticket 87): the collection checklist lives inside the
  // export dialog, not on the knob row (research 0007). Everything starts
  // included, so the plain flow still writes the whole-app backup.
  let exportOpen = $state(false);
  let include = $state<Record<CollectionKey, boolean>>({
    launch_entries: true,
    quick_actions: true,
    clips: true,
    products: true,
    presets: true,
  });
  const anyIncluded = $derived(Object.values(include).some(Boolean));

  onMount(() => {
    void (async () => {
      await load();
      await loadManagedCatalog();
      await loadDisplays();
      // Ticket 192: the Quick Action empty-state pointer lands here with a
      // session flag — expand the AI group and focus the provider control.
      // Plain focus only (the focus ring is the highlight); no smooth
      // scroll, no pulse, no per-group route.
      await honorAiSetupFocus({
        session: sessionStorage,
        expandAi: () => {
          expandGroups(["ai"]);
          filter = "";
        },
        focusProvider: () => document.getElementById("ai-provider")?.focus(),
        nextTick: tick,
      });
      // Ticket 193: one event handler for the whole webview — progress
      // survives tab switches because the store outlives every surface.
      watchManagedInstall();
      try {
        reconcileManagedInstall(await aiManagedInstallActive());
      } catch (cause) {
        console.error("managed install reconcile failed", cause);
      }
      // Reload honesty: the store reset but the worker kept the flight.
      // Never invent a second flight — offer Cancel and let the flight land.
      try {
        if (managedInstall.status === "idle" && (await aiManagedInstallActive())) {
          backgroundInstall = true;
        }
      } catch (cause) {
        console.error("managed install background check failed", cause);
      }
      // Ticket 189: cheap status (~5s) while Settings is open; the interval
      // dies on unmount so closed Settings costs zero.
      startStatusPoll();
    })();
    const off = listen("displays-changed", () => {
      void loadDisplays();
    }).catch(() => () => {});
    const offCompanion = listen("quick-launch-changed", () => {
      void refreshCompanionKnobs();
    }).catch(() => () => {});
    const offDirtyClose = listen("settings-dirty-close-requested", () => {
      if (!isDirty || guardOpen) return;
      pendingNav = null;
      pendingClose = true;
      guardOpen = true;
    }).catch(() => () => {});
    // also re-load when app regains focus (display change may have happened while hidden)
    const onFocus = () => void loadDisplays();
    window.addEventListener("focus", onFocus);
    return () => {
      void off.then((fn) => fn());
      void offCompanion.then((fn) => fn());
      void offDirtyClose.then((fn) => fn());
      window.removeEventListener("focus", onFocus);
      stopStatusPoll();
      stopResourcePoll();
      if (copyPathTimer !== null) {
        clearTimeout(copyPathTimer);
        copyPathTimer = null;
      }
      // Clear the backend flag when leaving the Settings route while
      // unmounting — a clean navigation or a destroy after Save/Discard
      // also clears via the sync effect, but unmount is the final backstop.
      void invoke("set_settings_dirty", { dirty: false }).catch(() => {});
    };
  });

  async function load() {
    loading = true;
    loadFailed = false;
    error = "";
    try {
      const loaded = await getSettings();
      settings = loaded;
      timeout = loaded.default_timeout_minutes;
      retention = loaded.log_retention_days;
      installDir = loaded.install_dir;
      launchConcurrency = loaded.launch_concurrency;
      dockMode = loaded.dock_mode;
      dockEdge = loaded.dock_edge;
      dockState = loaded.dock_state;
      // Width % falls back to the shipped default when the stored value is
      // broken (Settings::load), then re-clamps into the loaded mode so the
      // fixed slider never holds above 30%.
      dockWidthPct = clampWidthPctForMode(
        loaded.dock_width_pct ?? DOCK_WIDTH_DEFAULT_PCT,
        loaded.dock_mode,
      );
      // The tab-position knob starts centered — loadDisplays mirrors the one
      // display's memory over it once the arrangement resolves.
      bezelYRatio = 0.5;
      dockDensity = validDensity(loaded.dock_density);
      // Ticket 113: reveal tuning knobs default to shipped gate constants;
      // fall back to defaults when the stored value is broken (Settings::load).
      revealDwellMs = loaded.reveal_dwell_ms ?? 200;
      revealSensitivityPx = loaded.reveal_sensitivity_px ?? 12;
      autostart = loaded.autostart;
      companionUrl = loaded.companion_url ?? null;
      companionHeightRatio = clampCompanionRatio(loaded.companion_height_ratio ?? 0.40);
      companionUrlList = normalizeCompanionList(loaded.companion_url_list ?? []);
      aiProvider = validAiProvider(loaded.ai_provider);
      aiBaseUrl = (loaded.ai_base_url ?? "").trim() || AI_BASE_URL_DEFAULT;
      aiModel = (loaded.ai_model ?? "").trim();
      aiTestStatus = "";
      aiTestError = "";
      const persisted = loaded.theme as ThemeMode;
      if (persisted === "system" || persisted === "light" || persisted === "dark") {
        if (persisted !== theme.mode) restoreTheme(persisted);
      }
      restoreAnimation(validAnimation(loaded.animation));
      restoreNativeFrame(parseNativeFrame(loaded.native_frame));
      try {
        restoreLocale(parseLocale(await getLanguage()));
      } catch {
        // Offline or locked DB — the cached language still stands.
      }
      baseline = {
        timeout: loaded.default_timeout_minutes,
        retention: loaded.log_retention_days,
        installDir: loaded.install_dir.trim(),
        launchConcurrency: loaded.launch_concurrency,
        dockMode: loaded.dock_mode,
        dockEdge: loaded.dock_edge,
        dockState: loaded.dock_state,
        dockWidthPct: clampWidthPctForMode(
          loaded.dock_width_pct ?? DOCK_WIDTH_DEFAULT_PCT,
          loaded.dock_mode,
        ),
        bezelYRatio: 0.5,
        dockDensity: validDensity(loaded.dock_density),
        revealDwellMs: loaded.reveal_dwell_ms ?? 200,
        revealSensitivityPx: loaded.reveal_sensitivity_px ?? 12,
        companionUrl: loaded.companion_url ?? null,
        companionHeightRatio: clampCompanionRatio(loaded.companion_height_ratio ?? 0.40),
        companionUrlList: normalizeCompanionList(loaded.companion_url_list ?? []),
        aiProvider: validAiProvider(loaded.ai_provider),
        aiBaseUrl: (loaded.ai_base_url ?? "").trim() || AI_BASE_URL_DEFAULT,
        aiModel: (loaded.ai_model ?? "").trim(),
      };
      loadFailed = false;
    } catch {
      loadFailed = true;
    } finally {
      loading = false;
    }
  }

  // The dock divider and the companion manager write out-of-band while this
  // page loads once on mount — refresh the untouched companion knobs live so
  // the page never displays (or later saves back) stale values. Touched knobs
  // keep the user's edits; the save-time conflict check still guards those.
  // No loading chrome: this is a background truth-sync, and a failed one
  // degrades to today's behavior (save-time merge) rather than an error.
  async function refreshCompanionKnobs() {
    try {
      const fresh = await getSettings();
      if (!companionUrlTouched) {
        companionUrl = fresh.companion_url ?? null;
        if (baseline) baseline.companionUrl = companionUrl;
      }
      if (!companionRatioTouched) {
        companionHeightRatio = clampCompanionRatio(fresh.companion_height_ratio ?? 0.40);
        if (baseline) baseline.companionHeightRatio = companionHeightRatio;
      }
      const freshList = normalizeCompanionList(fresh.companion_url_list ?? []);
      companionUrlList = [...freshList];
      if (baseline) baseline.companionUrlList = [...freshList];
    } catch (e) {
      console.error("settings companion refresh failed", e);
    }
  }

  async function loadDisplays() {
    try {
      const list = await listDisplays();
      physicalDisplays = list;
      // DEV-only preview for flatten without 2 physical displays (A).
      // `?preview-per-monitor=1` forces 2 mocked displays so you can see the
      // flattened per-monitor knobs on a single monitor. Tree-shaken out of
      // production build via `import.meta.env.DEV`.
      let effective: DisplayInfo[] = list;
      if (import.meta.env.DEV && typeof window !== "undefined") {
        const preview = new URLSearchParams(window.location.search).get("preview-per-monitor");
        if (preview && effective.length <= 1) {
          effective = [
            {
              device_name: "\\\\.\\DISPLAY1",
              identity: null,
              id: "\\\\.\\DISPLAY1",
              label: "Display 1",
              width: 1920,
              height: 1080,
              resolution: "1920 \u00D7 1080",
              x: 0,
              y: 0,
              left_eligible: true,
              right_eligible: false,
            },
            {
              device_name: "\\\\.\\DISPLAY2",
              identity: null,
              id: "\\\\.\\DISPLAY2",
              label: "Display 2",
              width: 2560,
              height: 1440,
              resolution: "2560 \u00D7 1440",
              x: 1920,
              y: 0,
              left_eligible: false,
              right_eligible: true,
            },
          ];
        }
      }
      displays = effective;
      const nextEdges: Record<string, string> = {};
      const nextModes: Record<string, string> = {};
      const nextWidths: Record<string, number> = {};
      const nextBezelY: Record<string, number> = {};
      for (const d of effective) {
        try {
          const e = await getDisplayDockEdge(d.device_name);
          nextEdges[d.device_name] = e ?? dockEdge;
        } catch {
          nextEdges[d.device_name] = dockEdge;
        }
        try {
          const m = await getDisplayDockMode(d.device_name);
          nextModes[d.device_name] = m ?? dockMode;
        } catch {
          nextModes[d.device_name] = dockMode;
        }
        try {
          const w = await getDisplayDockWidthPct(d.device_name);
          const mode = nextModes[d.device_name] ?? dockMode;
          nextWidths[d.device_name] = clampWidthPctForMode(w ?? dockWidthPct, mode);
        } catch {
          const mode = nextModes[d.device_name] ?? dockMode;
          nextWidths[d.device_name] = clampWidthPctForMode(dockWidthPct, mode);
        }
        // The tab-position mirror of the width memory above: a broken or
        // missing read centers, so a drag in the dock window shows up here
        // on focus without ever parking the knob off-screen.
        try {
          const y = await getDisplayBezelYRatio(d.device_name);
          nextBezelY[d.device_name] = clampBezelYRatio(y ?? 0.5);
        } catch {
          nextBezelY[d.device_name] = 0.5;
        }
      }
      displayEdges = nextEdges;
      displayModes = nextModes;
      displayWidths = nextWidths;
      displayBezelY = nextBezelY;
      baselineDisplayEdges = { ...nextEdges };
      baselineDisplayModes = { ...nextModes };
      baselineDisplayWidths = { ...nextWidths };
      baselineDisplayBezelY = { ...nextBezelY };
      // A single display has no per-monitor rows, so the global tab-position
      // knob mirrors its memory — the same rule as the width memory above.
      if (effective.length === 1) {
        bezelYRatio = nextBezelY[effective[0].device_name] ?? 0.5;
        if (baseline) baseline.bezelYRatio = bezelYRatio;
      }
    } catch {
      displays = [];
      physicalDisplays = [];
    }
  }

  // Per-monitor follows the global Save pattern (0009 explicit-save). Only
  // Theme is immediate (ticket 31 / 0008 immediate toggle rule). Per-row
  // changes are local until Save, then batch-written — matches dock_mode/edge
  // deferred semantics and avoids mid-form WS writes.
  function changeDisplayEdge(device: string, edge: string) {
    displayErrors = { ...displayErrors, [device]: "" };
    displayEdges = { ...displayEdges, [device]: edge };
  }

  function changeDisplayMode(device: string, mode: string) {
    displayErrors = { ...displayErrors, [device]: "" };
    displayModes = { ...displayModes, [device]: mode };
    // Switching modes re-clamps honestly: a 55% auto-hide width becomes 30%
    // in fixed instead of exploding the reserving strip.
    const cur = displayWidths[device];
    if (cur !== undefined) {
      displayWidths = { ...displayWidths, [device]: clampWidthPctForMode(cur, mode) };
    }
  }

  /** The global dock-mode switch re-clamps the global width the same way, so
   * the fixed slider can never hold above 30%. Per-monitor rows still
   * following the old global follow the switch too — otherwise a global-only
   * change is written back stale per-row on Save and the reconcile restores
   * the old mode (the flip visibly dies). Rows customized away from the old
   * global keep their mode (and their width clamp with it). */
  function changeDockMode(mode: string) {
    const prev = dockMode;
    dockMode = mode;
    dockWidthPct = clampWidthPctForMode(dockWidthPct, mode);
    for (const d of displays) {
      if ((displayModes[d.device_name] ?? prev) !== prev) continue;
      displayModes = { ...displayModes, [d.device_name]: mode };
      const cur = displayWidths[d.device_name];
      if (cur !== undefined) {
        displayWidths = { ...displayWidths, [d.device_name]: clampWidthPctForMode(cur, mode) };
      }
    }
  }

  function changeDisplayWidth(device: string, pct: number) {
    displayErrors = { ...displayErrors, [device]: "" };
    const mode = displayModes[device] ?? dockMode;
    displayWidths = { ...displayWidths, [device]: clampWidthPctForMode(pct, mode) };
  }

  /** The bezel tab's parking height (spec 214): the global knob carries a
   *  ratio 0..1, the per-row sliders carry whole percent — both clamp into
   *  the travel, so a broken value centers instead of losing the tab. */
  function changeBezelY(device: string | null, ratio: number) {
    const clamped = clampBezelYRatio(ratio);
    if (device === null) {
      bezelYRatio = clamped;
    } else {
      displayErrors = { ...displayErrors, [device]: "" };
      displayBezelY = { ...displayBezelY, [device]: clamped };
    }
  }

  // Ticket 125 companion list helpers — dedup trimmed case-insensitive on host+path, machine-local
  async function pick(mode: ThemeMode) {
    saved = "";
    error = "";
    try {
      await selectTheme(mode);
    } catch {
      error = t("settings.saveThemeFail");
    }
  }

  async function pickAnimation(value: AnimationMode) {
    const previous = animation.mode;
    saved = "";
    error = "";
    try {
      await selectAnimation(value);
    } catch {
      // The switch never landed — put it back so it tells the truth.
      restoreAnimation(previous);
      error = t("settings.saveAnimationFail");
    }
  }

  async function pickLanguage(value: string) {
    const previous = localeState.current;
    saved = "";
    error = "";
    try {
      await selectLocale(parseLocale(value));
    } catch {
      // The switch never landed — put it back so it tells the truth.
      restoreLocale(previous);
      error = t("settings.saveLanguageFail");
    }
  }

  async function pickNativeFrame(value: NativeFrameMode) {
    const previous = nativeFrame.mode;
    saved = "";
    error = "";
    try {
      await selectNativeFrame(value);
    } catch {
      // The switch never landed — put it back so it tells the truth.
      restoreNativeFrame(previous);
      error = t("settings.saveFrameFail");
    }
  }

  async function pickAutostart(value: string) {
    const previous = autostart;
    autostart = value;
    saved = "";
    error = "";
    try {
      await updateAutostart(value === "on");
    } catch {
      // Neither the setting nor the registration changed — put the toggle
      // back so it tells the truth.
      autostart = previous;
      error = t("settings.autostartFail");
    }
  }

  async function browseInstallDir() {
    error = "";
    try {
      const picked = await open({
        title: t("settings.installDirTitle"),
        multiple: false,
        directory: true,
      });
      if (typeof picked === "string" && picked) installDir = picked;
    } catch {
      error = t("settings.pickerFail");
    }
  }

  // Ticket 115: discard restores the loaded snapshot; the bar disappears
  // because isDirty compares post-clamp — no scroll, no focus move.
  function discard() {
    if (!baseline) return;
    timeout = baseline.timeout;
    retention = baseline.retention;
    installDir = baseline.installDir;
    launchConcurrency = baseline.launchConcurrency;
    dockMode = baseline.dockMode;
    dockEdge = baseline.dockEdge;
    dockState = baseline.dockState;
    dockWidthPct = baseline.dockWidthPct;
    bezelYRatio = baseline.bezelYRatio;
    dockDensity = baseline.dockDensity;
    revealDwellMs = baseline.revealDwellMs;
    revealSensitivityPx = baseline.revealSensitivityPx;
      companionUrl = baseline.companionUrl;
      companionHeightRatio = baseline.companionHeightRatio;
      companionUrlList = [...baseline.companionUrlList];
      companionUrlTouched = false;
      companionRatioTouched = false;
    aiProvider = baseline.aiProvider;
    aiBaseUrl = baseline.aiBaseUrl || AI_BASE_URL_DEFAULT;
    aiModel = baseline.aiModel;
    aiTestStatus = "";
    aiTestError = "";
    if (displays.length > 1) {
      displayEdges = { ...baselineDisplayEdges };
      displayModes = { ...baselineDisplayModes };
      displayWidths = { ...baselineDisplayWidths };
      displayBezelY = { ...baselineDisplayBezelY };
    }
    displayErrors = {};
    saved = "";
    error = "";
  }

  async function save() {
    if (!settings) return;
    saving = true;
    saved = "";
    error = "";
    // Clear per-monitor row errors before batch save.
    displayErrors = {};
    // The AI route guards its own save like the per-monitor rows do: a
    // missing model under existing-local refuses with focus on the field
    // instead of a backend round-trip.
    if (aiProvider === "existing-local" && !aiModel.trim()) {
      error = t("settings.modelNameFirst");
      expandGroups(["ai"]);
      await tick();
      document.getElementById("ai-model")?.focus();
      saving = false;
      return;
    }
    try {
      // Companion knobs are written out-of-band (dock divider, companion
      // manager) while this page loads once on mount — saving the stale
      // baseline back over them silently un-configures the pane. Knobs left
      // alone ride the fresh read; a knob edited here AND changed out there
      // refuses with an honest error instead of picking a silent winner. The
      // companion manager owns the site list, so it always rides fresh.
      const clampedPageRatio = clampCompanionRatio(companionHeightRatio);
      let saveCompanionUrl = companionUrl;
      let saveCompanionRatio = clampedPageRatio;
      let saveCompanionList = normalizeCompanionList(companionUrlList);
      try {
        const fresh = await getSettings();
        if (!companionUrlTouched) {
          saveCompanionUrl = fresh.companion_url ?? null;
        } else if (
          baseline &&
          (fresh.companion_url ?? null) !== (baseline.companionUrl ?? null) &&
          (fresh.companion_url ?? null) !== (companionUrl ?? null)
        ) {
          throw new Error(t("settings.conflictSite"));
        }
        if (!companionRatioTouched) {
          saveCompanionRatio = clampCompanionRatio(fresh.companion_height_ratio ?? 0.40);
        } else if (
          baseline &&
          clampCompanionRatio(fresh.companion_height_ratio ?? 0.40) !== baseline.companionHeightRatio &&
          clampCompanionRatio(fresh.companion_height_ratio ?? 0.40) !== clampedPageRatio
        ) {
          throw new Error(t("settings.conflictHeight"));
        }
        saveCompanionList = normalizeCompanionList(fresh.companion_url_list ?? []);
      } catch (e) {
        if (e instanceof Error && e.message.includes(t("settings.discardReapply"))) throw e;
        console.error("settings companion refresh failed", e);
      }
      companionUrl = saveCompanionUrl;
      companionHeightRatio = saveCompanionRatio;
      companionUrlList = [...saveCompanionList];
      // Ticket 113: clamp reveal knobs to sane ranges before persisting
      // (same 0–1000 ms / 0–50 px the backend validates; broken stored
      // values already fell back to defaults on load).
      const clampedDwell = Math.min(1000, Math.max(0, Math.floor(revealDwellMs) || 0));
      const clampedSens = Math.min(50, Math.max(0, Math.floor(revealSensitivityPx) || 0));
      const clampedCompanionRatio = clampCompanionRatio(companionHeightRatio);
      const normalizedList = normalizeCompanionList(companionUrlList);
      // Clamp the width % into the current mode before persisting (fixed
      // 10–30, auto-hide 10–60 — the backend validates the stored 10–60;
      // broken stored values already fell back to the default on load).
      const clampedWidthPct = clampWidthPctForMode(dockWidthPct, dockMode);
      await updateSettings({
        default_timeout_minutes: Math.max(1, Math.floor(timeout) || 1),
        log_retention_days: Math.max(1, Math.floor(retention) || 1),
        theme: theme.mode,
        // The motion switch saves on its own like the theme — the bulk save
        // only carries it through so a save never resets it.
        animation: animation.mode,
        // The frame switch saves on its own too — carried through for the
        // same reason.
        native_frame: nativeFrame.mode,
        install_dir: installDir.trim(),
        launch_concurrency: Math.min(50, Math.max(1, Math.floor(launchConcurrency) || 1)),
        dock_mode: dockMode,
        dock_edge: dockEdge,
        dock_state: dockState,
        dock_width_pct: clampedWidthPct,
        // The window's list density lives here with the other dock knobs —
        // one knob reshaping all three window tabs is a window-global
        // concern (research 0008 rule 1), deferred to Save like its
        // neighbors; broken stored values already fell back on load.
        dock_density: validDensity(dockDensity),
        autostart,
        // Not this screen's knobs either — each list page owns its
        // collection's Groups toggle (ticket 89); loaded values pass through.
        launch_groups: settings.launch_groups,
        action_groups: settings.action_groups,
        clip_groups: settings.clip_groups,
        reveal_dwell_ms: clampedDwell,
        reveal_sensitivity_px: clampedSens,
        companion_url: companionUrl,
        companion_height_ratio: clampedCompanionRatio,
        companion_url_list: normalizedList,
        // The dock toolbar owns the mute toggle — Settings only carries the
        // stored value through so a save never resets it.
        companion_muted: settings.companion_muted ?? false,
        ai_provider: aiProvider,
        ai_base_url: aiBaseUrl.trim() || AI_BASE_URL_DEFAULT,
        ai_model: aiModel.trim(),
      });
      // Per-monitor follows Save (only Theme is immediate per 0009). Batch
      // the deferred writes so global + per-monitor share one success notice.
      // Disabled seam options prevent picking an ineligible edge, but a race
      // (screens moved after load) still surfaces as a row error.
      let perMonitorError = false;
      let companionHeightError = "";
      // A single physical display has no visible per-monitor controls, so its
      // remembered values must follow the global knobs. DEV preview displays
      // are visual fixtures only and must never write fake monitor records.
      const displayTargets = physicalDisplays.length > 1 ? displays : physicalDisplays;
      for (const d of displayTargets) {
        const edge = physicalDisplays.length > 1 ? displayEdges[d.device_name] : dockEdge;
        if (edge !== undefined) {
          try {
            await setDisplayDockEdge(d.device_name, edge);
          } catch (e) {
            displayErrors = { ...displayErrors, [d.device_name]: String(e) };
            perMonitorError = true;
          }
        }
        const mode = physicalDisplays.length > 1 ? displayModes[d.device_name] : dockMode;
        if (mode !== undefined) {
          try {
            await setDisplayDockMode(d.device_name, mode);
          } catch (e) {
            displayErrors = { ...displayErrors, [d.device_name]: String(e) };
            perMonitorError = true;
          }
        }
        // Width follows the same single-vs-multi rule — the single display's
        // memory tracks the global slider; each display's own slider writes
        // its own row when several are connected. Each row clamps into its
        // own mode so a fixed display never stores above 30%.
        const width =
          physicalDisplays.length > 1
            ? (displayWidths[d.device_name] ?? clampedWidthPct)
            : clampedWidthPct;
        if (width !== undefined) {
          try {
            const rowMode = physicalDisplays.length > 1 ? (mode ?? dockMode) : dockMode;
            await setDisplayDockWidthPct(d.device_name, clampWidthPctForMode(width, rowMode));
          } catch (e) {
            displayErrors = { ...displayErrors, [d.device_name]: String(e) };
            perMonitorError = true;
          }
        }
        // Tab position follows the same single-vs-multi rule — the single
        // display's memory tracks the global knob; each display's own slider
        // writes its own row when several are connected.
        const yRatio =
          physicalDisplays.length > 1
            ? clampBezelYRatio(displayBezelY[d.device_name] ?? 0.5)
            : clampBezelYRatio(bezelYRatio);
        try {
          await setDisplayBezelYRatio(d.device_name, yRatio);
        } catch (e) {
          displayErrors = { ...displayErrors, [d.device_name]: String(e) };
          perMonitorError = true;
        }
      }
      await reconcileQuickLaunchSettings();
      // Single display only: its height memory tracks the global knob, the
      // same rule as the width memory above — otherwise the surviving
      // per-monitor entry shadows the just-saved global and the knob reads
      // dead. Several displays keep their own memories (per-screen heights
      // stay distinct); the global remains their fallback.
      if (physicalDisplays.length === 1 && displayTargets.length === 1) {
        try {
          await setCompanionHeightRatioForDisplay(
            displayTargets[0].device_name,
            clampedCompanionRatio
          );
        } catch (e) {
          companionHeightError = t("settings.companionHeightFail").replace("{detail}", String(e));
        }
      }
      if (perMonitorError) {
        error = t("settings.perMonitorFail");
        // A failing save expands its owning group and lands focus on the
        // first refusing row, so the failure is found, not hunted.
        expandGroups(["dock"]);
        await tick();
        const failed = displays.find((d) => displayErrors[d.device_name]);
        if (failed) {
          const rowId = `per-monitor-edge-${failed.device_name.replace(/[^a-zA-Z0-9]/g, "-")}`;
          document.getElementById(rowId)?.focus();
        }
      } else if (companionHeightError) {
        error = companionHeightError;
      } else {
        saved = t("settings.savedRun");
      }
      // Reflect any clamping back into the fields.
      timeout = Math.max(1, Math.floor(timeout) || 1);
      retention = Math.max(1, Math.floor(retention) || 1);
      launchConcurrency = Math.min(50, Math.max(1, Math.floor(launchConcurrency) || 1));
      dockWidthPct = clampWidthPctForMode(dockWidthPct, dockMode);
      dockDensity = validDensity(dockDensity);
      bezelYRatio = clampBezelYRatio(bezelYRatio);
      for (const d of displays) {
        if (displayWidths[d.device_name] !== undefined) {
          const rowMode = displayModes[d.device_name] ?? dockMode;
          displayWidths[d.device_name] = clampWidthPctForMode(displayWidths[d.device_name], rowMode);
        }
        if (displayBezelY[d.device_name] !== undefined) {
          displayBezelY[d.device_name] = clampBezelYRatio(displayBezelY[d.device_name]);
        }
      }
      revealDwellMs = Math.min(1000, Math.max(0, Math.floor(revealDwellMs) || 0));
      revealSensitivityPx = Math.min(50, Math.max(0, Math.floor(revealSensitivityPx) || 0));
      companionHeightRatio = clampCompanionRatio(companionHeightRatio);
      companionUrlList = normalizeCompanionList(companionUrlList);
      // Ticket 115: after a successful persist, the snapshot becomes the saved
      // values — post-clamp, trimmed — so the bar clears without a reload.
      baseline = {
        timeout,
        retention,
        installDir: installDir.trim(),
        launchConcurrency,
        dockMode,
        dockEdge,
        dockState,
        dockWidthPct,
        bezelYRatio,
        dockDensity,
        revealDwellMs,
        revealSensitivityPx,
        companionUrl,
        companionHeightRatio,
        companionUrlList: [...companionUrlList],
        aiProvider,
        aiBaseUrl: aiBaseUrl.trim() || AI_BASE_URL_DEFAULT,
        aiModel: aiModel.trim(),
      };
      if (!perMonitorError) {
        baselineDisplayEdges = { ...displayEdges };
        baselineDisplayModes = { ...displayModes };
        baselineDisplayWidths = { ...displayWidths };
        baselineDisplayBezelY = { ...displayBezelY };
      }
      companionUrlTouched = false;
      companionRatioTouched = false;
      // Keep the Settings object in sync so the next save's passthrough
      // (launch_groups etc) stays truthful.
      settings = {
        ...settings,
        default_timeout_minutes: timeout,
        log_retention_days: retention,
        install_dir: installDir.trim(),
        launch_concurrency: launchConcurrency,
        dock_mode: dockMode,
        dock_edge: dockEdge,
        dock_state: dockState,
        dock_width_pct: dockWidthPct,
        dock_density: dockDensity,
        theme: theme.mode,
        autostart,
        reveal_dwell_ms: revealDwellMs,
        reveal_sensitivity_px: revealSensitivityPx,
        companion_url: companionUrl,
        companion_height_ratio: companionHeightRatio,
        companion_url_list: [...companionUrlList],
        companion_muted: settings.companion_muted ?? false,
        ai_provider: aiProvider,
        ai_base_url: aiBaseUrl.trim() || AI_BASE_URL_DEFAULT,
        ai_model: aiModel.trim(),
      };
    } catch (cause) {
      console.error("settings save failed", cause);
      const detail = String(cause).trim();
      error = detail
        ? t("settings.saveFailDetail").replace("{detail}", detail)
        : t("settings.saveFail");
      // A backend refusal names no field, so every group opens and focus
      // lands on the error itself — the same expand-and-focus promise.
      expandGroups(["general", "dock", "companion", "backup", "ai"]);
      await tick();
      document.getElementById("settings-error")?.focus();
    } finally {
      saving = false;
    }
  }

  // Ticket 116: the three-way guard dialog — Save changes / Discard changes / Keep editing.
  // Initial focus sits on Keep editing, Escape means Keep editing, focus
  // returns to the triggering control on close (Dialog's lastFocus). Save
  // completes the save then continues the navigation/close the user started.
  function handleKeepEditing() {
    if (guardSaving || saving) return;
    guardOpen = false;
    pendingNav = null;
    pendingClose = false;
    guardSaving = false;
  }

  function handleDiscardGuard() {
    if (guardSaving || saving) return;
    discard();
    const nav = pendingNav;
    const doClose = pendingClose;
    guardOpen = false;
    pendingNav = null;
    pendingClose = false;
    guardSaving = false;
    if (doClose) {
      void invoke("destroy_main_window").catch(() => {});
    } else if (nav) {
      void goto(nav);
    }
  }

  async function handleSaveGuard() {
    if (guardSaving) return;
    guardSaving = true;
    try {
      await save();
      if (isDirty || error) {
        guardSaving = false;
        return;
      }
      const nav = pendingNav;
      const doClose = pendingClose;
      guardOpen = false;
      pendingNav = null;
      pendingClose = false;
      guardSaving = false;
      if (doClose) {
        void invoke("destroy_main_window").catch(() => {});
      } else if (nav) {
        await goto(nav);
      }
    } catch {
      guardSaving = false;
    }
  }

  async function runUpdateCheck() {
    checking = true;
    try {
      const result = await checkForUpdates();
      checkResult =
        result.status === "available"
          ? "idle"
          : result.status === "up-to-date"
            ? "current"
            : "failed";
    } finally {
      checking = false;
    }
  }

  async function applyInstall() {
    try {
      await installNow();
      // A successful spawn exits the app within the second.
      installConfirmOpen = false;
    } catch (e) {
      // Failure reopens the dialog with the error; the row's install
      // button stays available for a retry.
      installError = String(e);
      installConfirmOpen = true;
    }
  }

  /** "3 products, 1 preset, 5 launch entries" — the count list behind both
   *  backup notices and the restore confirmation. Nouns come from the
   *  shared collection names so notices never drift from the tabs. */
  function describeCounts(counts: BackupCounts): string {
    const phrase = (n: number, nouns: { one: string; many: string }) =>
      n && `${n} ${n === 1 ? nouns.one : nouns.many}`;
    return [
      phrase(counts.products, { one: t("collection.products.one"), many: t("collection.products.many") }),
      phrase(counts.presets, { one: t("collection.presets.one"), many: t("collection.presets.many") }),
      phrase(counts.launch_entries, { one: t("collection.launch_entries.one"), many: t("collection.launch_entries.many") }),
      phrase(counts.quick_actions, { one: t("collection.quick_actions.one"), many: t("collection.quick_actions.many") }),
      phrase(counts.clips, { one: t("collection.clips.one"), many: t("collection.clips.many") }),
    ]
      .filter(Boolean)
      .join(", ");
  }

  function openExportDialog() {
    backupStatus = "";
    backupError = "";
    exportOpen = true;
  }

  async function exportSelected() {
    backupStatus = "";
    backupError = "";
    try {
      const path = await saveDialog({
        title: t("settings.backupTitleDialog"),
        defaultPath: "sprout-backup.json",
        filters: [{ name: t("files.actionBackup"), extensions: ["json"] }],
      });
      if (!path) return;
      backupBusy = true;
      const counts = await exportBackup(path, include);
      backupStatus = t("settings.backedUp")
        .replace("{counts}", describeCounts(counts))
        .replace("{path}", path);
    } catch (e) {
      console.error(e);
      // Rejections are authored backend copy; infrastructure failures are rare.
      backupError = String(e);
    } finally {
      backupBusy = false;
    }
  }

  async function restoreViaDialog() {
    backupStatus = "";
    backupError = "";
    try {
      const picked = await open({
        title: t("settings.openBackupTitle"),
        multiple: false,
        directory: false,
        filters: [{ name: t("files.actionBackup"), extensions: ["json", "zip"] }],
      });
      if (typeof picked !== "string") return;
      backupBusy = true;
      restoreFile = picked;
      restoreCounts = await inspectBackup(picked);
    } catch (e) {
      console.error(e);
      backupError = String(e);
      restoreFile = "";
      restoreCounts = null;
    } finally {
      backupBusy = false;
    }
  }

  async function importBackupFile(file: string) {
    backupBusy = true;
    try {
      const summary = await importBackup(file);
      const restored = describeCounts(summary.inserted);
      const skipped =
        summary.skipped.products +
        summary.skipped.presets +
        summary.skipped.launch_entries +
        summary.skipped.quick_actions +
        summary.skipped.clips;
      if (!restored) {
        backupStatus = t("settings.nothingToRestore");
      } else if (skipped > 0) {
        backupStatus = (skipped === 1 ? t("settings.restoredSkippedOne") : t("settings.restoredSkippedMany"))
          .replace("{restored}", restored)
          .replace("{count}", String(skipped));
      } else {
        backupStatus = t("settings.restored").replace("{restored}", restored);
      }
    } catch (e) {
      console.error(e);
      // Rejections are authored backend copy; infrastructure failures are rare.
      backupError = String(e);
    } finally {
      backupBusy = false;
    }
  }
</script>

<section class="settings" class:settings--dirty={isDirty} aria-labelledby="settings-title">
  <PageHeader titleId="settings-title" title={t("nav.settings")}>
    {#snippet subtitle()}
      {t("settings.subtitle")}
    {/snippet}
    {#snippet toolbar()}
      <SearchInput
        value={filter}
        placeholder={t("settings.filterPh")}
        ariaLabel={t("settings.filterLabel")}
        onchange={(v) => (filter = v)}
      />
      {#if filtering}
        <p class="filter-count" role="status">
          {matchCount === 1 ? t("settings.matchOne") : t("settings.matchMany").replace("{count}", String(matchCount))}
        </p>
      {/if}
    {/snippet}
  </PageHeader>

  <div id="settings-error" tabindex="-1" class="settings-error-anchor">
    {#if error}
      <Notice tone="error">{error}</Notice>
    {/if}
  </div>
  {#if saved}
    <Notice tone="ok">{saved}</Notice>
  {/if}

  {#if loading}
    <p class="sifting" aria-live="polite">Loading…</p>
  {:else if loadFailed || !settings}
    <EmptyState icon="x" title={t("settings.dbTitle")}>
      <p>
        {t("settings.dbBody")}
        <span class="mono">%LOCALAPPDATA%\Sprout\sprout.db</span>{t("settings.dbTail")}
      </p>
      <p>{t("logs.retryLater")}</p>
      <div class="empty-cta">
        <Button variant="secondary" onclick={load}>{t("common.retry")}</Button>
      </div>
    </EmptyState>
  {:else}
    {#if noGroupVisible}
      <EmptyState icon="search" title={t("common.noMatchFor").replace("{query}", filter.trim())}>
        <p>{t("settings.noMatchHint")}</p>
        <div class="empty-cta">
          <Button variant="secondary" onclick={() => (filter = "")}>{t("common.clearFilter")}</Button>
        </div>
      </EmptyState>
    {/if}
    {#if !noGroupVisible}
    <form
      class="form"
      onsubmit={(e) => {
        e.preventDefault();
        save();
      }}
    >
      {#if groupVisible("general")}
        <!-- General: theme, animation, window frame, install directory, auto-start, and run defaults. -->
        <GroupAccordion
          open={groupEffectiveOpen("general")}
          controls="group-general-body"
          name={t("settings.group.general")}
          count={groupKnobCount("general")}
          onToggle={() => toggleGroup("general")}
        >
      <article class="knob" hidden={!knobVisible("theme")}>
        <div class="knob__body">
          <span class="knob__label">{t("settings.theme.label")}</span>
          <p class="knob__hint">
            {t("settings.theme.hint")}
          </p>
        </div>
        <div class="theme-picker" role="radiogroup" aria-label={t("settings.theme.label")}>
          {#each themeOptions as option (option.mode)}
            <button
              type="button"
              role="radio"
              class="theme-picker__option"
              class:theme-picker__option--active={theme.mode === option.mode}
              aria-checked={theme.mode === option.mode}
              onclick={() => pick(option.mode)}
            >
              {option.label}
            </button>
          {/each}
        </div>
      </article>

      <article class="knob" hidden={!knobVisible("language")}>
        <div class="knob__body">
          <span class="knob__label">{t("settings.language.label")}</span>
          <p class="knob__hint">
            {t("settings.language.hint")}
          </p>
        </div>
        <div class="knob__input">
          <Select
            id="language"
            variant="small"
            value={localeState.current}
            onchange={(v) => pickLanguage(v)}
            options={languageOptions}
          />
        </div>
      </article>

      <article class="knob" hidden={!knobVisible("animation")}>
        <div class="knob__body">
          <span class="knob__label">{t("settings.animation.label")}</span>
          <p class="knob__hint">
            {t("settings.animation.hint")}
          </p>
        </div>
        <div class="knob__input">
          <Select
            id="animation"
            variant="small"
            value={animation.mode}
            onchange={(v) => pickAnimation(validAnimation(v))}
            options={animationOptions}
          />
        </div>
      </article>

      <article class="knob" hidden={!knobVisible("native-frame")}>
        <div class="knob__body">
          <span class="knob__label">{t("settings.native-frame.label")}</span>
          <p class="knob__hint">
            {t("settings.native-frame.hint")}
          </p>
        </div>
        <div class="knob__input">
          <Select
            id="native-frame"
            variant="small"
            value={nativeFrame.mode}
            onchange={(v) => pickNativeFrame(parseNativeFrame(v))}
            options={nativeFrameOptions}
          />
        </div>
      </article>

      <article class="knob" hidden={!knobVisible("install-dir")}>
        <div class="knob__body">
          <label class="knob__label" for="install-dir">{t("settings.install-dir.label")}</label>
          <p class="knob__hint">
            {t("settings.install-dir.hint")}
          </p>
        </div>
        <div class="knob__input knob__input--wide">
          <input
            id="install-dir"
            name="install-dir"
            class="field__input field__input--dir"
            type="text"
            autocomplete="off"
            spellcheck="false"
            placeholder={t("settings.wingetDefault")}
            value={installDir}
            oninput={(e) => (installDir = (e.target as HTMLInputElement).value)}
          />
          <Button type="button" variant="secondary" onclick={browseInstallDir}>{t("settings.browse")}</Button>
          {#if installDir}
            <Button type="button" variant="ghost" onclick={() => (installDir = "")}>{t("settings.clearDir")}</Button>
          {/if}
        </div>
      </article>

      <article class="knob" hidden={!knobVisible("autostart")}>
        <div class="knob__body">
          <span class="knob__label">{t("settings.autostart.label")}</span>
          <p class="knob__hint">
            {t("settings.autostart.hint")}
          </p>
        </div>
        <div class="knob__input">
          <Select
            id="autostart"
            variant="small"
            value={autostart}
            onchange={(v) => pickAutostart(v)}
            options={autostartOptions}
          />
        </div>
      </article>

      <article class="knob" hidden={!knobVisible("default-timeout")}>
        <div class="knob__body">
          <label class="knob__label" for="default-timeout">{t("settings.default-timeout.label")}</label>
          <p class="knob__hint">
            {t("settings.default-timeout.hint")}
          </p>
        </div>
        <div class="knob__input">
          <input
            id="default-timeout"
            name="default-timeout"
            class="field__input"
            type="number"
            min="1"
            max="1440"
            autocomplete="off"
            value={timeout}
            oninput={(e) => (timeout = Number((e.target as HTMLInputElement).value))}
          />
          <span class="knob__unit">min</span>
        </div>
      </article>

      <article class="knob" hidden={!knobVisible("log-retention")}>
        <div class="knob__body">
          <label class="knob__label" for="log-retention">{t("settings.log-retention.label")}</label>
          <p class="knob__hint">
            {t("settings.log-retention.hint")}
          </p>
        </div>
        <div class="knob__input">
          <input
            id="log-retention"
            name="log-retention"
            class="field__input"
            type="number"
            min="1"
            max="3650"
            autocomplete="off"
            value={retention}
            oninput={(e) => (retention = Number((e.target as HTMLInputElement).value))}
          />
          <span class="knob__unit">days</span>
        </div>
      </article>

      <article class="knob" hidden={!knobVisible("launch-concurrency")}>
        <div class="knob__body">
          <label class="knob__label" for="launch-concurrency">{t("settings.launch-concurrency.label")}</label>
          <p class="knob__hint">
            {t("settings.launch-concurrency.hint")}
          </p>
        </div>
        <div class="knob__input">
          <input
            id="launch-concurrency"
            name="launch-concurrency"
            class="field__input"
            type="number"
            min="1"
            max="50"
            autocomplete="off"
            value={launchConcurrency}
            oninput={(e) => (launchConcurrency = Number((e.target as HTMLInputElement).value))}
          />
          <span class="knob__unit">apps</span>
        </div>
      </article>
        </GroupAccordion>
      {/if}

      {#if groupVisible("dock")}
        <!-- Dock: window state, mode, edge, width, density, per-monitor, reveal. -->
        <GroupAccordion
          open={groupEffectiveOpen("dock")}
          controls="group-dock-body"
          name={t("settings.group.dock")}
          count={groupKnobCount("dock")}
          onToggle={() => toggleGroup("dock")}
        >
      <article class="knob" hidden={!knobVisible("dock-state")}>
        <div class="knob__body">
          <label class="knob__label" for="dock-state">{t("settings.dock-state.label")}</label>
          <p class="knob__hint">
            {t("settings.dock-state.hint")}
          </p>
        </div>
        <div class="knob__input">
          <Select
            id="dock-state"
            variant="small"
            value={dockState}
            onchange={(v) => (dockState = v)}
            options={dockStateOptions}
          />
        </div>
      </article>

      <article class="knob" hidden={!knobVisible("dock-mode")}>
        <div class="knob__body">
          <label class="knob__label" for="dock-mode">{t("settings.dock-mode.label")}</label>
          <p class="knob__hint">
            {t("settings.dock-mode.hint")}
          </p>
        </div>
        <div class="knob__input">
          <Select id="dock-mode" variant="small" value={dockMode} onchange={(v) => changeDockMode(v)} options={dockModeOptions} />
        </div>
      </article>

      <article class="knob" hidden={!knobVisible("dock-edge")}>
        <div class="knob__body">
          <label class="knob__label" for="dock-edge">{t("settings.dock-edge.label")}</label>
          <p class="knob__hint">
            {t("settings.dock-edge.hint")}
          </p>
        </div>
        <div class="knob__input">
          <Select id="dock-edge" variant="small" value={dockEdge} onchange={(v) => (dockEdge = v)} options={dockEdgeOptions} />
        </div>
      </article>

      {#if displays.length <= 1}
        <!-- Bezel tab position (spec 214): the non-drag path for the tab's
             parking height — several displays get one slider per row below
             (research 0006 pattern 4: the control sits with its display). -->
        <article class="knob" hidden={!knobVisible("bezel-y")}>
          <div class="knob__body">
            <label class="knob__label" for="bezel-y">{t("quickwindow.bezelPosition")}</label>
            <p class="knob__hint">
              {t("settings.bezel-y.hint")}
            </p>
          </div>
          <div class="knob__input knob__input--wide">
            <input
              id="bezel-y"
              name="bezel-y"
              class="knob__range"
              type="range"
              min="0"
              max="100"
              step="1"
              value={Math.round(clampBezelYRatio(bezelYRatio) * 100)}
              oninput={(e) => changeBezelY(null, Number((e.target as HTMLInputElement).value) / 100)}
              aria-describedby="bezel-y-value"
            />
            <span class="knob__unit knob__unit--auto" id="bezel-y-value" role="status">
              {Math.round(clampBezelYRatio(bezelYRatio) * 100)}%
            </span>
          </div>
        </article>
      {/if}

      <article class="knob" hidden={!knobVisible("dock-width")}>
        <div class="knob__body">
          <label class="knob__label" for="dock-width">{t("settings.dock-width.label")}</label>
          <p class="knob__hint">
            {t("settings.dock-width.hint")}
            {#if displays.length > 1}
              {" "}<a href="#per-monitor-title">{t("settings.dock-width.multiHint")}</a>
            {/if}
          </p>
        </div>
        <div class="knob__input knob__input--wide">
          <input
            id="dock-width"
            name="dock-width"
            class="knob__range"
            type="range"
            min={DOCK_WIDTH_MIN_PCT}
            max={dockWidthMaxForMode(dockMode)}
            step="1"
            disabled={displays.length > 1}
            value={clampWidthPctForMode(dockWidthPct, dockMode)}
            oninput={(e) => (dockWidthPct = clampWidthPctForMode(Number((e.target as HTMLInputElement).value), dockMode))}
            aria-describedby="dock-width-value"
          />
          <span class="knob__unit knob__unit--auto" id="dock-width-value" role="status">
            {clampWidthPctForMode(dockWidthPct, dockMode)}%{
              (physicalDisplays.length === 1 && physicalDisplays[0])
                ? ` · ~${effectiveWidthPx(physicalDisplays[0].width, dockWidthPct, dockMode)} px`
                : (displays.length > 0 && displays[0]
                  ? ` · ~${effectiveWidthPx(displays[0].width, dockWidthPct, dockMode)} px on ${displays[0].label}`
                  : "")
            }
          </span>
        </div>
      </article>

      <article class="knob" hidden={!knobVisible("dock-density")}>
        <div class="knob__body">
          <label class="knob__label" for="dock-density">{t("settings.dock-density.label")}</label>
          <p class="knob__hint">
            {t("settings.dock-density.hint")}
          </p>
        </div>
        <div class="knob__input">
          <Select id="dock-density" variant="small" value={dockDensity} onchange={(v) => (dockDensity = v)} options={dockDensityOptions} />
        </div>
      </article>

      {#if displays.length > 1 && knobVisible("per-monitor")}
        <!-- Per-monitor: flat knobs per display (flattened to reuse .knob, 0005 rule 5). No nested card. Global defaults above stay fallback. Each display row stacks vertically — the row layout squeezes the label, both selects, and two sliders past the column edge at narrow widths, so it reuses the same stack variant as the managed-list knobs instead of a second card pattern. -->
        <div class="per-monitor" aria-labelledby="per-monitor-title">
          <div class="per-monitor__header">
            <span class="knob__label" id="per-monitor-title">{t("settings.per-monitor.label")}</span>
            <p class="knob__hint">
              {t("settings.per-monitor.hint")}
            </p>
          </div>
          {#each displays as d (d.device_name)}
            {@const edgeId = `per-monitor-edge-${d.device_name.replace(/[^a-zA-Z0-9]/g, "-")}`}
            {@const modeId = `per-monitor-mode-${d.device_name.replace(/[^a-zA-Z0-9]/g, "-")}`}
            {@const widthId = `per-monitor-width-${d.device_name.replace(/[^a-zA-Z0-9]/g, "-")}`}
            {@const bezelYId = `per-monitor-bezely-${d.device_name.replace(/[^a-zA-Z0-9]/g, "-")}`}
            {@const reasonId = `per-monitor-reason-${d.device_name.replace(/[^a-zA-Z0-9]/g, "-")}`}
            {@const hasSeam = !d.left_eligible || !d.right_eligible}
            {@const perMode = displayModes[d.device_name] ?? dockMode}
            {@const widthPct = clampWidthPctForMode(displayWidths[d.device_name] ?? dockWidthPct, perMode)}
            {@const bezelYPct = Math.round(clampBezelYRatio(displayBezelY[d.device_name] ?? 0.5) * 100)}
            <article class="knob knob--stack" hidden={!knobVisible("per-monitor")}>
              <div class="knob__body">
                <span class="knob__label">{d.label} · {d.resolution}</span>
                {#if hasSeam}
                  <p class="knob__hint" id={reasonId} style="color: var(--warn-text)">{t("dock.seam.tooltip")}</p>
                {/if}
                {#if displayErrors[d.device_name]}
                  <p class="knob__hint" style="color: var(--danger-text)" role="alert">{displayErrors[d.device_name]}</p>
                {/if}
              </div>
              <div class="knob__input">
                <Select
                  id={edgeId}
                  variant="small"
                  value={displayEdges[d.device_name] ?? dockEdge}
                  onchange={(v) => changeDisplayEdge(d.device_name, v)}
                  aria-label={t("settings.edgeOnLabel").replace("{label}", d.label)}
                  aria-describedby={hasSeam ? reasonId : undefined}
                  options={[
                    { value: "left", label: t("settings.opt.edge.left"), disabled: !d.left_eligible },
                    { value: "right", label: t("settings.opt.edge.right"), disabled: !d.right_eligible },
                  ]}
                />
                <Select
                  id={modeId}
                  variant="small"
                  value={displayModes[d.device_name] ?? dockMode}
                  onchange={(v) => changeDisplayMode(d.device_name, v)}
                  aria-label={t("settings.modeOnLabel").replace("{label}", d.label)}
                  options={[
                    { value: "auto-hide", label: t("settings.opt.mode.auto") },
                    { value: "fixed", label: t("settings.opt.mode.fixed") },
                    { value: "bezel", label: t("settings.opt.mode.bezel") },
                  ]}
                />
              </div>
              <div class="knob__input knob__input--wide">
                <label class="knob__unit knob__unit--auto" for={widthId}>
                  Width · {widthPct}% · ~{effectiveWidthPx(d.width, widthPct, perMode)} px
                </label>
                <input
                  id={widthId}
                  name={widthId}
                  class="knob__range"
                  type="range"
                  min={DOCK_WIDTH_MIN_PCT}
                  max={dockWidthMaxForMode(perMode)}
                  step="1"
                  value={widthPct}
                  oninput={(e) =>
                    changeDisplayWidth(d.device_name, Number((e.target as HTMLInputElement).value))}
                />
              </div>
              <div class="knob__input knob__input--wide">
                <label class="knob__unit knob__unit--auto" for={bezelYId}>
                  {t("quickwindow.bezelPosition")} · {bezelYPct}%
                </label>
                <input
                  id={bezelYId}
                  name={bezelYId}
                  class="knob__range"
                  type="range"
                  min="0"
                  max="100"
                  step="1"
                  value={bezelYPct}
                  oninput={(e) =>
                    changeBezelY(d.device_name, Number((e.target as HTMLInputElement).value) / 100)}
                />
              </div>
            </article>
          {/each}
        </div>
      {/if}
      {#if dockMode === "auto-hide"}
        <!-- Reveal tuning lives at the dock group's footer as flat reused rows
             (research 0006 pattern 7 without a nested disclosure — at most two
             disclosure levels, so the group header is the only collapse).
             Hidden entirely when auto-hide is not the active dock mode. -->
        <article class="knob" hidden={!knobVisible("reveal-dwell")}>
          <div class="knob__body">
            <label class="knob__label" for="reveal-dwell">{t("settings.reveal-dwell.label")}</label>
            <p class="knob__hint">
              {t("settings.reveal-dwell.hint")}
            </p>
          </div>
          <div class="knob__input">
            <input
              id="reveal-dwell"
              name="reveal-dwell"
              class="field__input"
              type="number"
              min="0"
              max="1000"
              autocomplete="off"
              value={revealDwellMs}
              oninput={(e) => (revealDwellMs = Number((e.target as HTMLInputElement).value))}
            />
            <span class="knob__unit">ms</span>
          </div>
        </article>

        <article class="knob" hidden={!knobVisible("reveal-sensitivity")}>
          <div class="knob__body">
            <label class="knob__label" for="reveal-sensitivity">{t("settings.reveal-sensitivity.label")}</label>
            <p class="knob__hint">
              {t("settings.reveal-sensitivity.hint")}
            </p>
          </div>
          <div class="knob__input">
            <input
              id="reveal-sensitivity"
              name="reveal-sensitivity"
              class="field__input"
              type="number"
              min="0"
              max="50"
              autocomplete="off"
              value={revealSensitivityPx}
              oninput={(e) => (revealSensitivityPx = Number((e.target as HTMLInputElement).value))}
            />
            <span class="knob__unit">px</span>
          </div>
        </article>
      {/if}
        </GroupAccordion>
      {/if}

      {#if groupVisible("companion")}
        <!-- Companion: active site, pane height, and saved sites. URL authoring
             stays on its dedicated configuration surface (research 0006 pattern 1). -->
        <GroupAccordion
          open={groupEffectiveOpen("companion")}
          controls="group-companion-body"
          name={t("nav.companion")}
          count={groupKnobCount("companion")}
          onToggle={() => toggleGroup("companion")}
        >
          <article class="knob" hidden={!knobVisible("companion-active")}>
            <div class="knob__body">
              <label class="knob__label" for="companion-url">{t("settings.companion-active.label")}</label>
              <p class="knob__hint">
                {t("settings.companion-active.hint")}
              </p>
            </div>
            <div class="knob__input">
              <Select
                id="companion-url"
                variant="small"
                value={companionUrl ?? ""}
                onchange={(v) => {
                  companionUrlTouched = true;
                  companionUrl = v ? v : null;
                }}
                options={[
                  { value: "", label: t("common.off") },
                  ...companionUrlList.map((site) => ({
                    value: site.url,
                    label: companionDisplayName(site),
                  })),
                ]}
              />
            </div>
          </article>

          <article class="knob" hidden={!knobVisible("companion-height")}>
            <div class="knob__body">
              <label class="knob__label" for="companion-ratio">{t("settings.companion-height.label")}</label>
              <p class="knob__hint">
                {t("settings.companion-height.hint")}
              </p>
            </div>
            <div class="knob__input">
              <input
                id="companion-ratio"
                name="companion-ratio"
                class="field__input"
                type="number"
                min="0.25"
                max="0.60"
                step="0.05"
                autocomplete="off"
                value={companionHeightRatio}
                oninput={(e) => {
                  companionRatioTouched = true;
                  companionHeightRatio = Number((e.target as HTMLInputElement).value);
                }}
              />
              <span class="knob__unit">× dock</span>
            </div>
          </article>

          <div class="companion-manager" hidden={!knobVisible("companion-sites")}>
            <div class="knob__body">
              <span class="knob__label">{t("settings.companion-sites.label")}</span>
              <p class="knob__hint">
                {companionUrlList.length === 1 ? t("settings.companion-sites.hintOne") : tCount("settings.companion-sites.hintMany", companionUrlList.length)}
              </p>
            </div>
            <Button variant="secondary" onclick={() => goto("/companion")}>{t("settings.manageSites")}</Button>
          </div>
        </GroupAccordion>
      {/if}

      {#if groupVisible("backup")}
        <!-- Backup & housekeeping: whole-app backup and Sprout updates. -->
        <GroupAccordion
          open={groupEffectiveOpen("backup")}
          controls="group-backup-body"
          name={t("settings.group.backup")}
          count={groupKnobCount("backup")}
          onToggle={() => toggleGroup("backup")}
        >
      <article class="knob" hidden={!knobVisible("backup")}>
        <div class="knob__body">
          <span class="knob__label">{t("settings.backup.label")}</span>
          <p class="knob__hint">
            {t("settings.backup.hint")}
          </p>
          {#if backupStatus}
            <Notice tone="ok">{backupStatus}</Notice>
          {/if}
          {#if backupError}
            <Notice tone="error">{backupError}</Notice>
          {/if}
        </div>
        <div class="knob__input">
          <Button variant="secondary" onclick={openExportDialog} disabled={backupBusy}>
            {t("settings.exportBtn")}
          </Button>
          <Button variant="secondary" onclick={restoreViaDialog} disabled={backupBusy}>
            {t("settings.restoreBtn")}
          </Button>
        </div>
      </article>

      <article class="knob" hidden={!knobVisible("updates")}>
        <div class="knob__body">
          <span class="knob__label">{t("settings.updates.label")}</span>
          <p class="knob__hint">
            {t("settings.updates.hint")}
          </p>
          {#if updateState.installing}
            <p class="knob__status" role="status">
              {t("update.installingNote").replace("{version}", updateState.available?.version ?? "")}
            </p>
          {:else if updateState.available}
            <Notice tone="ok">{t("update.availableNote").replace("{version}", updateState.available.version)}</Notice>
          {:else if checking}
            <p class="knob__status" role="status">{t("common.busy.checking")}</p>
          {:else if checkResult === "current"}
            <Notice tone="ok">{t("update.currentNote")}</Notice>
          {:else if checkResult === "failed"}
            <Notice tone="warn">
              {t("update.feedFail")}
            </Notice>
          {/if}
        </div>
        <div class="knob__input">
          {#if updateState.installing}
            <Button disabled>{t("common.busy.installing")}</Button>
          {:else if updateState.available}
            <Button
              onclick={() => {
                installError = "";
                installConfirmOpen = true;
              }}
            >
              {t("update.installBtn").replace("{version}", updateState.available.version)}
            </Button>
          {:else}
            <Button variant="secondary" onclick={runUpdateCheck} disabled={checking}>
              {checking ? t("common.busy.checking") : t("update.checkBtn")}
            </Button>
          {/if}
        </div>
      </article>
        </GroupAccordion>
      {/if}

      {#if groupVisible("ai")}
        <!-- AI assistance: optional drafting help, off until configured. The
             route is Save-deferred like every other knob; Test connection is
             the explicit command and saves nothing. -->
        <GroupAccordion
          open={groupEffectiveOpen("ai")}
          controls="group-ai-body"
          name={t("settings.group.ai")}
          count={groupKnobCount("ai")}
          onToggle={() => toggleGroup("ai")}
        >
      <article class="knob" hidden={!knobVisible("ai-provider")}>
        <div class="knob__body">
          <label class="knob__label" for="ai-provider">{t("settings.ai-provider.label")}</label>
          <p class="knob__hint">
            {t("settings.ai-provider.hint")}
          </p>
          {#if aiProvider === "managed"}
            <p class="knob__hint">
              {t("settings.ai-provider.hintManaged")}
            </p>
          {/if}
          {#if aiProvider === "cloud"}
            <p class="knob__hint">
              {t("settings.ai-provider.hintCloud")}
            </p>
          {/if}
        </div>
        <div class="knob__input">
          <Select
            id="ai-provider"
            variant="small"
            value={aiProvider}
            onchange={(v) => (aiProvider = v as AiProvider)}
            options={aiProviderOptions}
          />
        </div>
      </article>

      {#if aiProvider === "managed"}
        <!-- Managed local model: separate standard knobs, not one mega-card
             (0005 rules 5/6 — rhythm owned by the shared knob, never
             re-declared; 0014 flat rows, never nested cards). Status follows
             the row grammar (state left, the one verb right — 0005 rule 2,
             0006 pattern 6); the pick and install lists are full-width list
             knobs gated on content (0004 rule 2, 0006 patterns 2/11). -->
        {#if managedCatalog && activeManagedId}
          <!-- Ticket 189: live Running/Stopped indicator — process liveness,
               never config readiness. The same control morphs Start↔Stop
               (primary↔danger); full model identity stays in the Model
               details dialog. Static dot, no pulse. -->
          <article class="knob" hidden={!knobVisible("ai-model")}>
            <div class="knob__body">
              <span class="knob__label">{t("settings.ai-managed.label")}</span>
              <p class="knob__status managed__status" role="status">
                <span
                  class="managed__dot"
                  class:managed__dot--running={runtimeRunning}
                  aria-hidden="true"
                ></span>
                {#if runtimeRunning}
                  <span class="managed__running">
                    {t("managed.runningNow")
                      .replace("{tier}", managedTierLabel(runtimeActiveId ?? activeManagedId))
                      .replace("{extra}", runtimeUptimeSecs !== null ? t("managed.runningExtra").replace("{uptime}", formatManagedUptime(runtimeUptimeSecs)) : "")}
                  </span>
                {:else}
                  <span class="managed__stopped">{t("managed.stopped")}</span>
                {/if}
              </p>
              <p class="knob__hint">{t("settings.ai-managed.hint")}</p>
              {#if runtimeError}
                <Notice tone="error">{runtimeError}</Notice>
              {/if}
              <!-- Ticket 190: lazy resource usage (0004 rule 2 rare-behind-obvious,
                   0006 pattern 7 collapsible over page splits). Shared Disclosure
                   only, collapsed by default, never persisted. Closed costs
                   nothing; open + running polls ~2s; open + stopped shows static
                   copy. Named for what it reveals — the model-details dialog
                   owns the bare "Details" word (0005 rule 4 scent logic).
                   Tooltip-grade numbers as text only — no animated gauges,
                   so Animation/reduced-motion have nothing to gate. -->
              <div class="managed__details">
                <Disclosure
                  open={resourceOpen}
                  controls="managed-resource-details"
                  label={t("managed.resourceUsage")}
                  onclick={toggleResourceDetails}
                />
                {#if resourceOpen}
                  <div id="managed-resource-details" class="managed__resource">
                    {#if !runtimeRunning}
                      <p class="knob__hint">{t("managed.stoppedHint")}</p>
                    {:else if resourceError}
                      <p class="knob__hint" role="status">{resourceError}</p>
                    {:else if resourceUsage}
                      <p class="knob__hint" role="status">
                        {t("managed.resourceLine")
                          .replace("{tier}", managedTierLabel(resourceUsage.active_model_id ?? activeManagedId))
                          .replace("{mem}", resourceUsage.working_set_bytes !== null ? formatManagedBytes(resourceUsage.working_set_bytes) : t("managed.measuring"))
                          .replace("{cpu}", formatResourceCpu(resourceUsage.cpu_percent))
                          .replace("{uptime}", formatManagedUptime(resourceUsage.uptime_secs))}
                      </p>
                    {:else}
                      <p class="knob__hint" role="status">{t("managed.readingUsage")}</p>
                    {/if}
                  </div>
                {/if}
              </div>
            </div>
            <div class="knob__input">
              <Button
                type="button"
                variant={stopApplies || stopBusy ? "danger" : "primary"}
                disabled={stopBusy || (startBusy && !runtimeRunning)}
                onclick={() => void (stopApplies ? stopManaged() : startApplies ? startManaged() : undefined)}
              >
                {stopBusy
                  ? t("common.busy.stopping")
                  : startBusy && !runtimeRunning
                    ? t("common.busy.starting")
                    : stopApplies
                      ? t("action.stop")
                      : t("launch.start")}
              </Button>
            </div>
          </article>
        {/if}
            {#if managedInstalled.length > 0}
              <!-- Ticket 191: the explicit Active model — installed entries
                   only, as flat option rows (0014 flat rows, never nested
                   cards). Radios stay visible: the pick is mutually exclusive
                   (NN/g + 0022 verdict 1) and choosing is frequent (0004
                   rule 2); per-model verbs move into the row's own ⋯ menu
                   (0006 patterns 4/10 — near their object, destruction last).
                   The wrapping label is the single hit target
                   (web-design-guidelines); the menu trigger stays a sibling
                   so opening it never toggles the pick. Save-deferred like
                   the provider (picking drafts `aiModel`). Tier-first names
                   choose by cost; the full artifact identity stays one level
                   down in the Details dialog. -->
              <article class="knob knob--stack" hidden={!knobVisible("ai-model")}>
                <div class="knob__body">
                  <span class="knob__label">{t("settings.ai-active.label")}</span>
                  <p class="knob__hint">
                    {t("settings.ai-active.hint")}
                  </p>
                </div>
                <fieldset class="managed__active managed__list">
                  <legend class="sr-only">{t("managed.installedLegend")}</legend>
                  {#each managedInstalled as model (model.id)}
                    <div class="managed__option">
                      <label class="managed__pick">
                        <input
                          id={"active-managed-" + model.id}
                          type="radio"
                          name="active-managed-model"
                          value={model.id}
                          checked={aiModel.trim() === model.id}
                          onchange={() => (aiModel = model.id)}
                        />
                        <span class="managed__texts">
                          <span class="managed__name"
                            >{managedTierLabel(model.id)} — {managedModelParams(model.id)}</span
                          >
                          <span class="managed__meta"
                            >{model.download_size_bytes !== null
                              ? formatManagedBytes(model.download_size_bytes)
                              : t("managed.sizePending")} · {formatManagedRam(model.memory_needs_mb)}</span
                          >
                        </span>
                      </label>
                      <IconButton
                        icon="dots"
                        label={t("packet.actionsFor").replace("{name}", model.id)}
                        quiet
                        data-ctx-trigger
                        disabled={managedBusy || installInFlight}
                        onclick={(e) =>
                          openModelMenu(
                            model.id,
                            e.currentTarget as HTMLButtonElement,
                            e.detail === 0
                          )}
                      />
                    </div>
                  {/each}
                </fieldset>
                {#if managedBusy}
                  <p class="knob__status" role="status">{t("managed.removing")}</p>
                {/if}
                {#if managedNotice}
                  <p class="knob__status" role="status">{managedNotice}</p>
                {/if}
              </article>
            {/if}
            {#if managedCatalog}
              {#if managedChoices.length === 0}
                <article class="knob" hidden={!knobVisible("ai-model")}>
                  <div class="knob__body">
                    <span class="knob__label">{t("settings.ai-unavailable.label")}</span>
                    <p class="knob__hint">{t("settings.ai-unavailable.hint")}</p>
                    <div class="managed__actions">
                      <Button type="button" variant="secondary" onclick={useExistingLocal}>{t("managed.useExisting")}</Button>
                      <Button type="button" variant="ghost" onclick={() => reviewManaged()}>{t("managed.whyUnavailable")}</Button>
                    </div>
                  </div>
                </article>
              {:else if managedAvailable.length > 0}
                <!-- Available installs: content-gated (0004 rule 2, 0006
                     patterns 2/11) — zero installable models render no
                     section at all, not even the header. One explicit action
                     per row keeps scent for the rare install (0008 rule 3);
                     progress stays inline at its row (ticket 193). -->
                <article class="knob knob--stack" hidden={!knobVisible("ai-model")}>
                  <div class="knob__body">
                    <span class="knob__label">{t("settings.ai-available.label")}</span>
                    <p class="knob__hint">{t("settings.ai-available.hint")}</p>
                  </div>
                  <div class="managed__list">
                    {#each managedAvailable as model (model.id)}
                      <div class="managed__choice">
                      <p class="managed__name">{managedTierLabel(model.id)} — {managedModelParams(model.id)}</p>
                      <p class="managed__meta">{t("managed.downloadMeta").replace("{size}", managedSize(model.download_size_bytes))}</p>
                      {#if installInFlight && managedInstall.modelId === model.id}
                        <!-- Ticket 193: row-level compact bar + Cancel. The same
                             flight renders in the dialog — one store, both
                             places, surviving tab switches. -->
                        <div class="managed__progress" role="status" aria-label={t("managed.installProgress")}>
                          <div class="managed__bar">
                            <div class="managed__fill" style="width: {installPercent ?? 0}%"></div>
                          </div>
                          <p class="knob__hint">{installProgressLine()}</p>
                          <div class="managed__actions">
                            <Button type="button" variant="secondary" onclick={cancelManagedInstall}>
                              {t("managed.cancelInstall")}
                            </Button>
                          </div>
                        </div>
                      {/if}
                      <div class="managed__actions">
                        <Button type="button" variant="secondary" disabled={managedBusy || installInFlight} onclick={() => reviewManaged(model.id)}>
                          {t("managed.reviewInstall")}
                        </Button>
                      </div>
                    </div>
                  {/each}
                  </div>
                  {#if backgroundInstall}
                    <Notice tone="warn">
                      {t("managed.backgroundInstall")}
                    </Notice>
                    <div class="managed__actions">
                      <Button type="button" variant="secondary" onclick={cancelManagedInstall}>
                        {t("managed.cancelInstall")}
                      </Button>
                    </div>
                  {/if}
                  {#if installGuardHit}
                    <!-- Ticket 193 guard-hit recovery: explicit remove-and-retry
                         for that revision, or keep the files. Never auto-deletes. -->
                    <Notice tone="error">{managedInstall.error}</Notice>
                    <div class="managed__actions">
                      <Button
                        type="button"
                        variant="secondary"
                        disabled={managedBusy}
                        onclick={() => managedInstall.modelId && void retryAfterRemove(managedInstall.modelId)}
                      >
                        {t("managed.removeRetry")}
                      </Button>
                      <Button type="button" variant="ghost" onclick={keepInstallFiles}>
                        {t("managed.keepFiles")}
                      </Button>
                    </div>
                  {:else if managedInstall.status === "interrupted"}
                    <Notice tone="warn">{managedInstall.error}</Notice>
                    <div class="managed__actions">
                      <Button type="button" variant="secondary" onclick={retryManagedInstall}>
                        {t("managed.retryInstall")}
                      </Button>
                      <Button type="button" variant="ghost" onclick={keepInstallFiles}>
                        {t("common.dismiss")}
                      </Button>
                    </div>
                  {:else if managedInstall.status === "error" && managedInstall.error}
                    <!-- Any failed install retries in one click — the guard-hit
                         branch above keeps its own remove-and-retry, so this never
                         loops a revision the backend refused to overwrite. -->
                    <div class="managed__actions">
                      <Button type="button" variant="secondary" onclick={retryManagedInstall}>
                        {t("managed.retryInstall")}
                      </Button>
                      <Button type="button" variant="ghost" onclick={keepInstallFiles}>
                        {t("common.dismiss")}
                      </Button>
                    </div>
                  {/if}
                  {#if managedInstall.status === "done" && managedInstall.notice}
                    <p class="knob__status" role="status">{managedInstall.notice}</p>
                  {/if}
                </article>
              {/if}
            {:else if !managedError}
              <article class="knob" hidden={!knobVisible("ai-model")}>
                <div class="knob__body">
                  <p class="knob__status" role="status">{t("managed.readingRec")}</p>
                </div>
              </article>
            {/if}
            {#if managedError}
              <article class="knob" hidden={!knobVisible("ai-model")}>
                <div class="knob__body">
                  <span class="knob__label">{t("settings.ai-unavailable.label")}</span>
                  <Notice tone="error">{managedError}</Notice>
                  {#if !managedCatalog}
                    <div class="managed__actions">
                      <Button type="button" variant="secondary" onclick={() => void loadManagedCatalog()}>{t("managed.retryBtn")}</Button>
                    </div>
                  {/if}
                </div>
              </article>
            {/if}
      {/if}

      {#if aiProvider === "existing-local"}
      <article class="knob" hidden={!knobVisible("ai-endpoint")}>
        <div class="knob__body">
          <label class="knob__label" for="ai-base-url">{t("settings.ai-endpoint.label")}</label>
          <p class="knob__hint">
            {t("settings.ai-endpoint.hint")}
          </p>
        </div>
        <div class="knob__input knob__input--wide">
          <input
            id="ai-base-url"
            name="ai-base-url"
            class="field__input field__input--dir"
            type="text"
            inputmode="url"
            autocomplete="off"
            spellcheck="false"
            placeholder="http://127.0.0.1:11434"
            value={aiBaseUrl}
            oninput={(e) => (aiBaseUrl = (e.target as HTMLInputElement).value)}
          />
        </div>
      </article>

      <article class="knob" hidden={!knobVisible("ai-model")}>
        <div class="knob__body">
          <label class="knob__label" for="ai-model">{t("settings.ai-model.label")}</label>
          <p class="knob__hint">
            {t("settings.ai-model.hint")}
          </p>
          {#if aiTestStatus}
            <Notice tone="ok">{aiTestStatus}</Notice>
          {/if}
          {#if aiTestError}
            <Notice tone="error">{aiTestError}</Notice>
          {/if}
        </div>
        <div class="knob__input knob__input--wide">
          <input
            id="ai-model"
            name="ai-model"
            class="field__input"
            type="text"
            autocomplete="off"
            spellcheck="false"
            placeholder={t("settings.modelPh")}
            value={aiModel}
            oninput={(e) => (aiModel = (e.target as HTMLInputElement).value)}
          />
          <Button
            type="button"
            variant="secondary"
            onclick={testAiConnection}
            disabled={aiTestBusy}
          >
            {aiTestBusy ? t("common.busy.testing") : t("settings.testConnection")}
          </Button>
          </div>
        </article>
      {/if}
      {#if aiProvider !== "managed" && managedInstalled.length > 0}
        <!-- Ticket 186: Off keeps downloads for later reuse — and keeps their
             deliberate removal reachable. Without this the managed section
             above unmounts on Off and stranded files have no UI. -->
        <article class="knob knob--stack" hidden={!knobVisible("ai-model")}>
          <div class="knob__body">
            <span class="knob__label">{t("settings.ai-downloads.label")}</span>
            <p class="knob__hint">
              {t("settings.ai-downloads.hint")}
            </p>
          </div>
          <div class="managed__list">
            {#each managedInstalled as model (model.id)}
              <div class="managed__option">
                <div class="managed__texts">
                  <p class="managed__name">{managedTierLabel(model.id)} — {managedModelParams(model.id)}</p>
                  <p class="managed__meta">{t("managed.installedMeta").replace("{size}", managedSize(model.download_size_bytes))}</p>
                </div>
                <IconButton
                  icon="dots"
                  label={t("packet.actionsFor").replace("{name}", model.id)}
                  quiet
                  data-ctx-trigger
                  disabled={managedBusy}
                  onclick={(e) =>
                    openModelMenu(
                      model.id,
                      e.currentTarget as HTMLButtonElement,
                      e.detail === 0
                    )}
                />
              </div>
            {/each}
          </div>
          {#if managedBusy}
            <p class="knob__status" role="status">{t("managed.removing")}</p>
          {/if}
          {#if managedNotice}
            <p class="knob__status" role="status">{managedNotice}</p>
          {/if}
        </article>
      {/if}
        </GroupAccordion>
      {/if}
    </form>
    <!-- Per-model ⋯ menu host: page-root so collapsing the AI group can never
         unmount an open menu from under keyboard focus (0006 pattern 4 — the
         trigger stays at its object, the overlay lives once at the page). -->
    <ContextMenu ctx={modelMenu} onclose={() => (modelMenu = null)} />
    {/if}
    {#if isDirty}
      <!-- Ticket 115: fixed bottom bar — warning text + Save/Discard, pinned
           regardless of scroll, until saved or reverted. Text + color, never
           color alone; announce via polite live region (below). -->
      <div class="dirty-bar" role="region" aria-label={t("settings.dirtyTitle")}>
        <p class="dirty-bar__text">
          <span class="dirty-bar__dot" aria-hidden="true"></span>
          {t("settings.dirtyBar")}
        </p>
        <div class="dirty-bar__actions">
          <Button variant="secondary" onclick={discard} type="button" disabled={saving}
            >{t("common.discard")}</Button
          >
          <Button onclick={save} disabled={saving}>{saving ? t("common.busy.saving") : t("common.save")}</Button>
        </div>
      </div>
    {/if}
    <!-- Polite live region: announces appearance/disappearance without moving focus or scrolling (0004 rule 5). -->
    <p class="sr-only" aria-live="polite" aria-atomic="true">{dirtyLiveMessage}</p>
    <!-- Ticket 116: guard leaving dirty Settings — rail + window close share one three-way alertdialog. -->
    <Dialog
      open={guardOpen}
      title={t("settings.dirtyTitle")}
      role="alertdialog"
      width={480}
      focusTarget="#guard-keep"
      onclose={handleKeepEditing}
    >
      <div class="guard">
        <p class="guard__body">
          {t("settings.guardBody")}
        </p>
        <div class="guard__actions">
          <Button variant="secondary" id="guard-keep" onclick={handleKeepEditing} disabled={guardSaving || saving}
            >{t("settings.guardKeep")}</Button
          >
          <Button variant="secondary" onclick={handleDiscardGuard} disabled={guardSaving || saving}
            >{t("settings.guardDiscard")}</Button
          >
          <Button onclick={handleSaveGuard} disabled={guardSaving || saving}
            >{guardSaving || saving ? t("common.busy.saving") : t("common.saveChanges")}</Button
          >
        </div>
      </div>
    </Dialog>
  {/if}
</section>

<Dialog
  open={managedDetailsOpen}
  title={managedReviewModel && !managedReviewModel.installed ? t("managed.installTitle") : t("managed.detailsTitle")}
  onclose={() => {
    managedDetailsOpen = false;
    managedConfirmRemoveId = null;
  }}
>
  {#if managedDetailsOpen && managedCatalog}
    <div class="guard">
      {#if !managedReviewModel}
        <p class="guard__body">{t("managed.noVerifiedPair")}</p>
      {/if}
      <section class="knob__body">
        <h3 class="knob__label">{t("managed.runtimeHead")}</h3>
        <p class="guard__body">{managedCatalog.runtime.name} {managedCatalog.runtime.version} · {managedCatalog.runtime.qualified ? t("managed.verified") : t("managed.awaitingVerify")}</p>
        <p class="guard__body">{managedCatalog.runtime.license}</p>
        <p class="guard__body managed__source">{t("managed.srcLabel").replace("{value}", managedCatalog.runtime.source)}</p>
        {#if managedCatalog.runtime.download_size_bytes !== null}
          <p class="guard__body">{t("managed.runtimeDownload").replace("{value}", managedSize(managedCatalog.runtime.download_size_bytes))}</p>
        {/if}
        {#if !managedCatalog.runtime.qualified && managedCatalog.runtime.blocker}
          <p class="guard__body">{managedCatalog.runtime.blocker}</p>
        {/if}
      </section>
      {#each managedReviewModel ? [managedReviewModel] : managedCatalog.models as model (model.id)}
        <section class="knob__body">
          <h3 class="knob__label managed__source">{model.artifact}</h3>
          {#if model.installed}
            <p class="guard__body">{t("managed.installedNote")}</p>
          {:else if !model.installable}
            <p class="guard__body">{t("managed.awaitingVerify")}</p>
          {/if}
          {#if model.blocker}<p class="guard__body">{model.blocker}</p>{/if}
          <p class="guard__body">{t("managed.licenseLabel").replace("{value}", model.license)}</p>
          <p class="guard__body managed__source">{t("managed.licenseSrc").replace("{value}", model.license_source)}</p>
          <p class="guard__body managed__source">{t("managed.modelSrc").replace("{value}", model.source)}</p>
          {#if model.revision}<p class="guard__body managed__source">{t("managed.revision").replace("{value}", model.revision)}</p>{/if}
          {#if model.quantization}<p class="guard__body">{t("managed.quantization").replace("{value}", model.quantization)}</p>{/if}
          {#if model.download_size_bytes !== null}<p class="guard__body">{t("managed.modelDownload").replace("{value}", managedSize(model.download_size_bytes))}</p>{/if}
          {#if model.memory_needs_mb !== null}<p class="guard__body">{t("managed.workingMem").replace("{value}", new Intl.NumberFormat().format(model.memory_needs_mb))}</p>{/if}
          {#if model.context_limit_tokens !== null}<p class="guard__body">{t("managed.ctxLimit").replace("{value}", new Intl.NumberFormat().format(model.context_limit_tokens))}</p>{/if}
          {#if model.minimum_runtime_version}<p class="guard__body">{t("managed.minRuntime").replace("{value}", model.minimum_runtime_version)}</p>{/if}
          {#if model.sha256}<p class="guard__body managed__source">SHA-256: {model.sha256}</p>{/if}
          {#if model.installed && model.installed_dir}
            <p class="guard__body managed__source">{t("managed.installedAt").replace("{value}", model.installed_dir)}</p>
            <p class="guard__body">
              {t("managed.weightsNote")}
            </p>
            <div class="managed__actions">
              <Button variant="secondary" onclick={copyInstallPath}>{t("managed.copyPath")}</Button>
              {#if copyPathFeedback}
                <span class="knob__status" role="status">{copyPathFeedback}</span>
              {/if}
            </div>
          {/if}
        </section>
      {/each}
      {#if managedReviewModel && installInFlight && managedInstall.modelId === managedReviewModel.id}
        <!-- Ticket 193: dialog full bar for the same flight the row shows. -->
        <div class="managed__progress" role="status" aria-label={t("managed.installProgress")}>
          <div class="managed__bar">
            <div class="managed__fill" style="width: {installPercent ?? 0}%"></div>
          </div>
          <p class="guard__body">{installProgressLine()}</p>
        </div>
      {/if}
      {#if managedReviewModel && installGuardHit && managedInstall.modelId === managedReviewModel.id}
        <Notice tone="error">{managedInstall.error}</Notice>
        <div class="managed__actions">
          <Button variant="secondary" onclick={() => void retryAfterRemove(managedReviewModel!.id)}>
            {t("managed.removeRetry")}
          </Button>
          <Button variant="ghost" onclick={keepInstallFiles}>{t("managed.keepFiles")}</Button>
        </div>
      {/if}
      {#if managedBusy}
        <p class="guard__body" role="status">{t("managed.removing")}</p>
      {:else if managedNotice}
        <p class="guard__body" role="status">{managedNotice}</p>
      {/if}
      {#if managedError}<Notice tone="error">{managedError}</Notice>{/if}
      {#if managedReviewModel?.installed}
        <section class="knob__body">
          <h3 class="knob__label">{t("managed.removeHead")}</h3>
          <p class="guard__body">{managedRemovePreview(managedReviewModel.id)}</p>
          {#if managedConfirmRemoveId === managedReviewModel.id}
            <div class="managed__actions">
              <Button variant="secondary" disabled={managedBusy} onclick={() => (managedConfirmRemoveId = null)}>{t("managed.keepIt")}</Button>
              <Button disabled={managedBusy} onclick={() => void removeManaged(managedReviewModel!.id)}>{managedBusy ? t("common.busy.removing") : t("managed.confirmRemove")}</Button>
            </div>
          {:else}
            <div class="managed__actions">
              <Button variant="secondary" disabled={managedBusy} onclick={() => (managedConfirmRemoveId = managedReviewModel!.id)}>{t("managed.removeModelBtn")}</Button>
            </div>
          {/if}
        </section>
      {/if}
      <div class="managed__actions">
        <Button variant="secondary" onclick={() => { managedDetailsOpen = false; managedConfirmRemoveId = null; }}>{t("common.close")}</Button>
        {#if managedReviewModel && installInFlight && managedInstall.modelId === managedReviewModel.id}
          <Button variant="secondary" onclick={cancelManagedInstall}>{t("managed.cancelInstall")}</Button>
        {:else if !managedBusy && !installInFlight && managedInstall.status === "error" && !installGuardHit && managedInstall.modelId === managedReviewModel?.id}
          <Button variant="secondary" onclick={retryManagedInstall}>{t("managed.retryInstall")}</Button>
        {:else if !managedBusy && !installInFlight && managedReviewModel?.installable && !managedReviewModel.installed}
          <Button onclick={() => void installManaged(managedReviewModel!.id)}>{t("managed.installModelRuntime")}</Button>
        {/if}
      </div>
    </div>
  {/if}
</Dialog>

<ConfirmDialog
  open={installConfirmOpen}
  title={t("update.available")}
  confirmLabel={updateState.installing ? t("common.busy.installing") : t("update.installRestart")}
  onconfirm={applyInstall}
  oncancel={() => (installConfirmOpen = false)}
>
  <p>{t("update.confirmBody").replace("{version}", updateState.available?.version ?? "")}</p>
  <p>{t("update.confirmNote")}</p>
  {#if installError}
    <Notice tone="error">{installError}</Notice>
  {/if}
</ConfirmDialog>

<ConfirmDialog
  open={restoreCounts !== null}
  title={t("settings.restoreTitle")}
  confirmLabel={t("chrome.restore")}
  onconfirm={() => {
    const file = restoreFile;
    restoreCounts = null;
    restoreFile = "";
    if (file) void importBackupFile(file);
  }}
  oncancel={() => {
    restoreCounts = null;
    restoreFile = "";
  }}
>
  {#if restoreCounts && describeCounts(restoreCounts)}
    <p>
      <strong>{restoreFile.split(/[\\/]/).pop()}</strong>{t("settings.restoreContains").replace("{counts}", describeCounts(restoreCounts))}
    </p>
    <p>{t("settings.restoreKept")}</p>
  {:else}
    <p>{t("settings.restoreEmpty")}</p>
  {/if}
</ConfirmDialog>

<ConfirmDialog
  open={exportOpen}
  title={t("settings.exportTitle")}
  confirmLabel={t("settings.exportConfirm")}
  confirmDisabled={!anyIncluded}
  onconfirm={() => {
    exportOpen = false;
    void exportSelected();
  }}
  oncancel={() => (exportOpen = false)}
>
  <p>{t("settings.exportHint")}</p>
  <div class="export-picker" role="group" aria-label={t("settings.exportGroup")}>
    {#each EXPORT_ORDER as key (key)}
      <Checkbox
        title={t(
          key === "launch_entries"
            ? "nav.launch"
            : key === "quick_actions"
              ? "nav.actions"
              : key === "clips"
                ? "nav.clips"
                : key === "products"
                  ? "nav.products"
                  : "nav.presets",
        )}
        checked={include[key]}
        onchange={(v) => (include[key] = v)}
      />
    {/each}
  </div>
</ConfirmDialog>

<style>
  /* Managed list knobs: the default .knob is a body/input row; a pick list
     needs full width, so the stack variant flows vertically while every
     other knob keeps the shared row rhythm (0005 rules 5/6). Tokens only.
     WHY the chained selector: .knob--stack alone ties .knob at 0-1-0, and
     .knob sits ~300 lines later — so align-items:center,
     justify-content:space-between and gap:space-5 silently won, centering
     every child and spreading them apart. .knob.knob--stack (0-2-0) wins
     regardless of order; never "simplify" it back to one class. */
  .knob.knob--stack {
    flex-direction: column;
    align-items: stretch;
    justify-content: flex-start;
    /* List-grade rhythm (0005 rule 6 — owned here, the one place a vertical
       knob exists): 16px between sections, 8px inside divided rows. */
    gap: var(--space-4);
  }

  .knob.knob--stack > .knob__body {
    flex: 1 1 auto;
  }

  .managed__actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    margin-top: var(--space-2);
  }

  /* Ticket 190: lazy Details disclosure — the shared Disclosure header plus
     one tooltip-grade line. Tokens only; no transition or gauge, so Animation
     off and reduced-motion change nothing (numbers update as text). */
  .managed__details {
    margin-top: var(--space-2);
  }

  .managed__resource {
    margin-top: var(--space-1);
  }

  /* Tickets 189/193: static Running/Stopped dot (no pulse — stillness is
     the honest state for a status) plus the install progress bar. Tokens
     only; no transition, so the Animation switch and reduced-motion have
     nothing to gate — the fill jumps to each reported percent. */
  .managed__status {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .managed__dot {
    width: 8px;
    height: 8px;
    border-radius: var(--radius-pill);
    background: var(--text-muted);
    flex-shrink: 0;
  }

  .managed__dot--running {
    background: var(--status-applied-text);
  }

  .managed__running {
    color: var(--status-applied-text);
    font-weight: 500;
  }

  .managed__stopped {
    color: var(--text-muted);
  }

  .managed__progress {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin-top: var(--space-2);
  }

  .managed__bar {
    height: 8px;
    border-radius: var(--radius-pill);
    background: var(--border);
    overflow: hidden;
  }

  .managed__fill {
    height: 100%;
    background: var(--accent);
    border-radius: var(--radius-pill);
  }

  .managed__choice {
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
  }

  /* Sibling rows divide with the shared hairline so each list scans as one
     flat grammar (0014 flat rows; 0004:2 frequency split — installed picks
     first, installs second). Tokens only. */
  .managed__option + .managed__option,
  .managed__choice + .managed__choice {
    border-top: 1px solid var(--border);
    padding-top: var(--space-2);
  }

  /* Ticket 191: the explicit Active-model radio. Native fieldset + radios in
     the shared find-row idiom (flat stacked option rows — 0014 keeps
     Settings rows flat, never nested cards). The radio itself carries state;
     the focus ring is the only highlight, no transition, so the Animation
     switch and reduced-motion have nothing to gate. 16px box matches the
     shared Checkbox control scale (0022 verdict 3). The fieldset needs
     min-width:0 or flex shrinks it past its radios (web-design-guidelines:
     flex children need min-w-0). */
  /* One list grammar for every model list in every provider mode
     (0005 rule 5 — Active picks, Available installs, and kept downloads
     share row rhythm): 8px rows, hairline dividers, 8px under each line. */
  .managed__list {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
  }

  .managed__active {
    border: none;
    padding: 0;
    margin: 0;
    min-width: 0;
  }

  /* One row: pick label left, ⋯ trigger right. The wrapping label is the
     single hit target for box + texts (web-design-guidelines: no dead
     zones); the menu trigger stays a sibling so opening it never toggles
     the pick — the same split the shared Checkbox uses for its InfoTip. */
  .managed__option {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2);
    min-width: 0;
  }

  .managed__pick {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2);
    flex: 1;
    min-width: 0;
    cursor: pointer;
  }

  .managed__pick > input {
    margin: 0;
    margin-top: 2px;
    accent-color: var(--accent);
    flex: none;
    width: 16px;
    height: 16px;
    cursor: pointer;
  }

  .managed__pick > input:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }

  .managed__texts {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
    flex: 1;
  }

  .managed__name {
    margin: 0;
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text);
    overflow-wrap: anywhere;
  }

  .managed__meta {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-muted);
    overflow-wrap: anywhere;
    font-variant-numeric: tabular-nums;
  }

  .managed__status,
  .managed__resource {
    font-variant-numeric: tabular-nums;
  }

  .managed__source {
    overflow-wrap: anywhere;
  }

  .settings {
    max-width: 680px;
    margin: 0 auto;
  }

  .settings--dirty {
    /* Reserve space so the fixed bar never hides the last knob — the page
       itself never scrolls to deliver the warning (ticket 115). */
    padding-bottom: 72px;
  }

  /* Ticket 115: fixed bottom bar — warning text + at most Save + Discard,
     pinned regardless of scroll, until saved or reverted. Text + color, never
     color alone (warn tint + explicit text + dot). Reuses Button primitives
     and tokens only; no ad-hoc colors. */
  .dirty-bar {
    position: fixed;
    bottom: 0;
    left: 200px;
    right: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
    padding: var(--space-3) var(--space-5);
    background: var(--bg-surface);
    border-top: 1px solid var(--warn-tint-border);
    box-shadow: var(--shadow-dialog);
    z-index: 20;
  }

  .dirty-bar__text {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    color: var(--warn-text);
  }

  .dirty-bar__dot {
    width: 8px;
    height: 8px;
    border-radius: var(--radius-pill);
    background: var(--warn-text);
    flex-shrink: 0;
  }

  .dirty-bar__actions {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    flex-shrink: 0;
  }

  /* Ticket 116: guard dialog — alertdialog with consequence-named actions */
  .guard {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
  }

  .guard__body {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--text);
    line-height: var(--leading-body);
  }

  .guard__actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
  }

  .sifting {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }

  .mono {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
  }

  .empty-cta {
    margin-top: var(--space-4);
  }

  .form {
    display: flex;
    flex-direction: column;
    /* No own gap: sections arrive with the accordion's component-owned
       separation, so collapsed rows sit as one list, not adrift. */
    gap: 0;
  }

  .knob {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-5);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-surface);
    padding: var(--space-4);
  }

  /* Section chrome (headers, caret anatomy, section spacing) belongs to the
     shared accordion — the page only keeps its knobs' rhythm inside the
     rows container, which carries none of its own. */
  .form :global(.group__rows) {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  /* Class-based display overrides `hidden`'s UA rule, so filtered-out knobs
     need the explicit collapse. */
  .knob[hidden],
  .companion-manager[hidden] {
    display: none;
  }

  .filter-count {
    margin: 0 0 0 var(--space-3);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
    white-space: nowrap;
  }

  /* Programmatic focus target for a failing save — the error Notice itself
     is the visual indicator, so the anchor draws no ring of its own. */
  .settings-error-anchor:focus {
    outline: none;
  }

  .companion-manager {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-5);
    padding: var(--space-2) var(--space-1) 0;
  }

  .per-monitor {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    width: 100%;
  }

  .per-monitor__header {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: 0 var(--space-1);
  }

  .knob__body {
    display: flex;
    flex-direction: column;
    /* Hint sits below the label (research 0021 spacing round): 8px owns
       the label → hint step, matching dialog field rhythm. */
    gap: var(--space-2);
    min-width: 0;
  }

  .knob__label {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--accent);
  }

  .knob__hint {
    margin: 0;
    font-size: var(--text-xs);
    line-height: var(--leading-tight);
    color: var(--text-muted);
  }

  .knob__status {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }

  .knob__input {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    flex-shrink: 0;
  }

  .knob__input--wide {
    flex: 1 0 auto;
    min-width: 0;
  }

  .field__input--dir {
    width: auto;
    min-width: 220px;
    flex: 1;
    text-align: left;
  }

  .theme-picker {
    display: flex;
    gap: var(--space-1);
    flex-shrink: 0;
    background: var(--bg-sunken);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: var(--space-1);
  }

  .theme-picker__option {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--text-muted);
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    padding: var(--space-1) var(--space-2);
    cursor: pointer;
  }

  .theme-picker__option:hover {
    color: var(--text);
  }

  .theme-picker__option--active {
    background: var(--accent-tint);
    color: var(--accent);
  }

  .field__input {
    width: 110px;
    font-family: var(--font-mono);
    font-size: var(--text-base);
    color: var(--text);
    /* The shared filled frame (research 0021, ticket 204). */
    background: var(--bg-sunken);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: var(--space-2) var(--space-3);
    text-align: right;
    /* WHY three properties: the shared progressive input frame — quiet rest,
       hover wash, glowing focus (research 0020 enhancement). */
    transition: border-color var(--dur-fast) var(--ease-out),
      background-color var(--dur-fast) var(--ease-out),
      box-shadow var(--dur-fast) var(--ease-out);
  }

  .field__input:hover {
    background-color: var(--bg-hover);
    border-color: var(--border-strong);
  }

  .field__input:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: var(--ring-glow);
  }

  .knob__unit {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
    width: 2.5em;
  }

  /* Ticket 128: the width slider fills its row (no ad-hoc px — flex only);
     the readout keeps the mono unit treatment with room for "% · ~NNN px". */
  .knob__range {
    flex: 1;
    min-width: 0;
    accent-color: var(--accent);
  }

  .knob__range:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }

  .knob__range:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  /* The disabled-width hint's jump link to the per-monitor widths below:
     accent text with an underline, glowing on focus like the shared
     select — tokens only, no ad-hoc color. */
  .knob__hint a {
    color: var(--accent);
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  .knob__hint a:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }

  .knob__unit--auto {
    width: auto;
    white-space: nowrap;
  }

  .export-picker {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
</style>
