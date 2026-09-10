<script lang="ts">
  import { onMount, tick } from "svelte";
  import { beforeNavigate, goto } from "$app/navigation";
  import type { BackupCounts, CompanionSite, DisplayInfo, ManagedCatalogStatus, Settings } from "$lib/types";
  import { aiProviderLabel } from "$lib/types";
  import type { AiProvider } from "$lib/types";
  import { companionDisplayName, normalizeCompanionSites } from "$lib/companion";
  import {
    aiCheckExistingLocal,
    aiCancelManagedInstall,
    aiInstallManaged,
    aiManagedStatus,
    exportBackup,
    getDisplayDockEdge,
    getDisplayDockMode,
    getDisplayDockWidthPct,
    getSettings,
    importBackup,
    inspectBackup,
    listDisplays,
    setCompanionHeightRatioForDisplay,
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
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import GroupAccordion from "$lib/components/GroupAccordion.svelte";
  import Notice from "$lib/components/Notice.svelte";
  import PageHeader from "$lib/components/PageHeader.svelte";
  import SearchInput from "$lib/components/SearchInput.svelte";
  import Select from "$lib/components/Select.svelte";
  import { open, save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { theme, restoreTheme, selectTheme } from "$lib/theme.svelte";
  import type { ThemeMode } from "$lib/theme.svelte";
  import {
    checkForUpdates,
    installNow,
    updateState,
  } from "$lib/updateState.svelte";
  import { COLLECTIONS, EXPORT_ORDER } from "$lib/collections";
  import type { CollectionKey } from "$lib/collections";
  import {
    buildSettingsSearchIndex,
    resolveSettingsFilter,
    SETTINGS_GROUP_KNOBS,
    type SettingsGroupKey,
  } from "$lib/settingsSearch";

  const themeOptions: { mode: ThemeMode; label: string }[] = [
    { mode: "system", label: "System" },
    { mode: "light", label: "Light" },
    { mode: "dark", label: "Dark" },
  ];

  const dockModeOptions: { value: string; label: string }[] = [
    { value: "auto-hide", label: "Auto-hide" },
    { value: "fixed", label: "Fixed" },
  ];

  const dockEdgeOptions: { value: string; label: string }[] = [
    { value: "left", label: "Left" },
    { value: "right", label: "Right" },
  ];

  const dockStateOptions: { value: string; label: string }[] = [
    { value: "floating", label: "Floating" },
    { value: "docked", label: "Docked" },
  ];

  const dockDensityOptions: { value: string; label: string }[] = [
    { value: "compact", label: "Compact" },
    { value: "default", label: "Default" },
    { value: "large", label: "Large" },
  ];

  const autostartOptions: { value: string; label: string }[] = [
    { value: "on", label: "On" },
    { value: "off", label: "Off" },
  ];

  const aiProviderOptions: { value: AiProvider; label: string }[] = [
    { value: "off", label: aiProviderLabel.off },
    { value: "existing-local", label: aiProviderLabel["existing-local"] },
    { value: "managed", label: aiProviderLabel.managed },
    { value: "cloud", label: aiProviderLabel.cloud },
  ];

  const AI_BASE_URL_DEFAULT = "http://127.0.0.1:11434";

  const SEAM_REASON = "Borders another display — cursor can't stop there";

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
  // workspace, ADR-0011) while auto-hide may apply to 60 (it overlays).
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
  let managedBusy = $state(false);
  let managedError = $state("");
  let managedNotice = $state("");
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
  let displayErrors = $state<Record<string, string>>({});

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
  /** The slider cap for a dock mode: fixed stays a strip (30), auto-hide may
   * overlay wide (60). Unknown modes take the fixed cap — the conservative
   * reservation assumption. */
  function dockWidthMaxForMode(mode: string): number {
    return mode === "auto-hide" ? DOCK_WIDTH_MAX_PCT_AUTOHIDE : DOCK_WIDTH_MAX_PCT_FIXED;
  }
  /** Clamps a width % into a mode's applied range, so a mode switch re-clamps
   * honestly: 55 in auto-hide becomes 30 in fixed. */
  function clampWidthPctForMode(v: number, mode: string): number {
    const n = Math.floor(Number(v));
    if (!Number.isFinite(n)) return DOCK_WIDTH_DEFAULT_PCT;
    return Math.min(dockWidthMaxForMode(mode), Math.max(DOCK_WIDTH_MIN_PCT, n));
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
          ? `Connected — the service exposes 1 model: ${exposed[0]}.`
          : `Connected — the service exposes ${exposed.length} models: ${exposed.join(", ")}.`;
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
    } catch (cause) {
      managedCatalog = null;
      managedError = String(cause);
    }
  }

  async function installManaged(modelId: string) {
    if (managedBusy) return;
    managedBusy = true;
    managedError = "";
    managedNotice = "";
    try {
      const result = await aiInstallManaged(modelId);
      aiModel = result.model_id;
      managedNotice = result.message;
      await loadManagedCatalog();
    } catch (cause) {
      managedError = String(cause);
    } finally {
      managedBusy = false;
    }
  }

  function cancelManagedInstall() {
    void aiCancelManagedInstall().catch(() => {});
    managedNotice = "Cancelling installation; staged files will not be activated.";
  }

  function managedSize(bytes: number | null): string {
    if (bytes === null) return "Not qualified";
    const gib = bytes / 1024 / 1024 / 1024;
    return `${new Intl.NumberFormat().format(bytes)} bytes (${gib.toFixed(2)} GB download)`;
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
    }
    return false;
  });

  const filtering = $derived(filter.trim().length > 0);
  // Update state in user terms, feeding the search index so update queries
  // land on the Sprout updates knob.
  const backupSummary = $derived.by(() => {
    if (updateState.available) return `Sprout ${updateState.available.version} ready to install`;
    if (checkResult === "current") return "Up to date";
    return "Whole-app backup";
  });
  const searchIndex = $derived.by(() =>
    buildSettingsSearchIndex({
      themeMode: theme.mode,
      themeLabel: themeOptions.find((o) => o.mode === theme.mode)?.label ?? "System",
      installDir,
      autostart,
      timeoutMinutes: clampTimeout(timeout),
      retentionDays: clampRetention(retention),
      launchConcurrency: clampConcurrency(launchConcurrency),
      dockMode,
      dockEdge,
      dockState,
      dockWidthPct: clampWidthPctForMode(dockWidthPct, dockMode),
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
      aiProviderLabel: aiProviderOptions.find((o) => o.value === aiProvider)?.label ?? "Off",
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
      dirtyLiveMessage = "You have unsaved changes — Save or Discard.";
    } else if (dirtyLiveMessage.startsWith("You have")) {
      dirtyLiveMessage = "All changes saved or discarded.";
      const t = setTimeout(() => {
        dirtyLiveMessage = "";
      }, 1500);
      return () => clearTimeout(t);
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
      }
      displayEdges = nextEdges;
      displayModes = nextModes;
      displayWidths = nextWidths;
      baselineDisplayEdges = { ...nextEdges };
      baselineDisplayModes = { ...nextModes };
      baselineDisplayWidths = { ...nextWidths };
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
   * the fixed slider can never hold above 30%. */
  function changeDockMode(mode: string) {
    dockMode = mode;
    dockWidthPct = clampWidthPctForMode(dockWidthPct, mode);
  }

  function changeDisplayWidth(device: string, pct: number) {
    displayErrors = { ...displayErrors, [device]: "" };
    const mode = displayModes[device] ?? dockMode;
    displayWidths = { ...displayWidths, [device]: clampWidthPctForMode(pct, mode) };
  }

  // Ticket 125 companion list helpers — dedup trimmed case-insensitive on host+path, machine-local
  async function pick(mode: ThemeMode) {
    saved = "";
    error = "";
    try {
      await selectTheme(mode);
    } catch {
      error = "Couldn't save the theme — it applies for now, but won't survive a restart.";
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
      error =
        "Couldn't change the start-with-Windows registration — try again.";
    }
  }

  async function browseInstallDir() {
    error = "";
    try {
      const picked = await open({
        title: "Default install directory",
        multiple: false,
        directory: true,
      });
      if (typeof picked === "string" && picked) installDir = picked;
    } catch {
      error = "Couldn't open the folder picker — type the path directly instead.";
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
      error = "Name the model your local service exposes — Sprout never substitutes another one.";
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
          throw new Error(
            "The Companion active site changed elsewhere — Discard and re-apply your edits."
          );
        }
        if (!companionRatioTouched) {
          saveCompanionRatio = clampCompanionRatio(fresh.companion_height_ratio ?? 0.40);
        } else if (
          baseline &&
          clampCompanionRatio(fresh.companion_height_ratio ?? 0.40) !== baseline.companionHeightRatio &&
          clampCompanionRatio(fresh.companion_height_ratio ?? 0.40) !== clampedPageRatio
        ) {
          throw new Error(
            "The Companion pane height changed in the dock while Settings was open — Discard and re-apply your edits."
          );
        }
        saveCompanionList = normalizeCompanionList(fresh.companion_url_list ?? []);
      } catch (e) {
        if (e instanceof Error && e.message.includes("Discard and re-apply")) throw e;
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
          companionHeightError = `Couldn't save the Companion height for this screen — ${String(e)}`;
        }
      }
      if (perMonitorError) {
        error = "Some per-monitor choices couldn't be saved — see the rows below.";
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
        saved = "Saved — the next run honors these.";
      }
      // Reflect any clamping back into the fields.
      timeout = Math.max(1, Math.floor(timeout) || 1);
      retention = Math.max(1, Math.floor(retention) || 1);
      launchConcurrency = Math.min(50, Math.max(1, Math.floor(launchConcurrency) || 1));
      dockWidthPct = clampWidthPctForMode(dockWidthPct, dockMode);
      dockDensity = validDensity(dockDensity);
      for (const d of displays) {
        if (displayWidths[d.device_name] !== undefined) {
          const rowMode = displayModes[d.device_name] ?? dockMode;
          displayWidths[d.device_name] = clampWidthPctForMode(displayWidths[d.device_name], rowMode);
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
        ? `Couldn't save the settings — ${detail}`
        : "Couldn't save the settings — try again. If it keeps failing, close Sprout and relaunch.";
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
      phrase(counts.products, COLLECTIONS.products),
      phrase(counts.presets, COLLECTIONS.presets),
      phrase(counts.launch_entries, COLLECTIONS.launch_entries),
      phrase(counts.quick_actions, COLLECTIONS.quick_actions),
      phrase(counts.clips, COLLECTIONS.clips),
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
        title: "Back up Sprout",
        defaultPath: "sprout-backup.json",
        filters: [{ name: "Sprout backup", extensions: ["json"] }],
      });
      if (!path) return;
      backupBusy = true;
      const counts = await exportBackup(path, include);
      backupStatus = `Backed up ${describeCounts(counts)} to ${path}`;
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
        title: "Open a Sprout backup",
        multiple: false,
        directory: false,
        filters: [{ name: "Sprout backup", extensions: ["json"] }],
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
        backupStatus = "Nothing to restore — everything in the file already exists.";
      } else if (skipped > 0) {
        backupStatus =
          `Restored ${restored}. ${skipped} item${skipped === 1 ? " was" : "s were"} already present and kept.`;
      } else {
        backupStatus = `Restored ${restored}.`;
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
  <PageHeader titleId="settings-title" title="Settings">
    {#snippet subtitle()}
      Defaults for authoring and housekeeping, persisted in the Library database and honored by
      every run.
    {/snippet}
    {#snippet toolbar()}
      <SearchInput
        value={filter}
        placeholder="Filter settings…"
        ariaLabel="Filter settings"
        onchange={(v) => (filter = v)}
      />
      {#if filtering}
        <p class="filter-count" role="status">
          {matchCount === 1 ? "1 match" : `${matchCount} matches`}
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
    <EmptyState icon="x" title="Couldn't read the settings">
      <p>
        Couldn't read the settings from
        <span class="mono">%LOCALAPPDATA%\Sprout\sprout.db</span> — the file may be locked or
        missing.
      </p>
      <p>Try again; if it keeps failing, close the app and relaunch.</p>
      <div class="empty-cta">
        <Button variant="secondary" onclick={load}>Try again</Button>
      </div>
    </EmptyState>
  {:else}
    {#if noGroupVisible}
      <EmptyState icon="search" title={`Nothing matches “${filter.trim()}”`}>
        <p>Try a different name, or clear the filter to see every setting.</p>
        <div class="empty-cta">
          <Button variant="secondary" onclick={() => (filter = "")}>Clear filter</Button>
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
        <!-- General: theme, install directory, auto-start, and run defaults. -->
        <GroupAccordion
          open={groupEffectiveOpen("general")}
          controls="group-general-body"
          name="General"
          count={groupKnobCount("general")}
          onToggle={() => toggleGroup("general")}
        >
      <article class="knob" hidden={!knobVisible("theme")}>
        <div class="knob__body">
          <span class="knob__label">Theme</span>
          <p class="knob__hint">
            Follows Windows, or pins one look. Applies immediately; no save needed.
          </p>
        </div>
        <div class="theme-picker" role="radiogroup" aria-label="Theme">
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

      <article class="knob" hidden={!knobVisible("install-dir")}>
        <div class="knob__body">
          <label class="knob__label" for="install-dir">Install directory</label>
          <p class="knob__hint">
            Where installs and upgrades land. Empty = the installer's default; use an
            absolute path like D:\Apps. Installers that ignore it are reported on
            the Plan. Never shared with exported presets.
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
            placeholder="(winget default)"
            value={installDir}
            oninput={(e) => (installDir = (e.target as HTMLInputElement).value)}
          />
          <Button type="button" variant="secondary" onclick={browseInstallDir}>Browse…</Button>
          {#if installDir}
            <Button type="button" variant="ghost" onclick={() => (installDir = "")}>Clear</Button>
          {/if}
        </div>
      </article>

      <article class="knob" hidden={!knobVisible("autostart")}>
        <div class="knob__body">
          <span class="knob__label">Start with Windows</span>
          <p class="knob__hint">
            Starts Sprout at login, tray-only: the main window stays closed and a
            docked bar reappears on its own. Turning it off removes the
            registration immediately; no restart needed.
          </p>
        </div>
        <div class="knob__input">
          <Select
            id="autostart"
            variant="small"
            value={autostart}
            onchange={(v) => pickAutostart(v)}
          >
            {#each autostartOptions as option (option.value)}
              <option value={option.value}>{option.label}</option>
            {/each}
          </Select>
        </div>
      </article>

      <article class="knob" hidden={!knobVisible("default-timeout")}>
        <div class="knob__body">
          <label class="knob__label" for="default-timeout">Default timeout</label>
          <p class="knob__hint">
            Minutes a requirement may take before its installer is killed. New
            requirements start with this value; each one can override it.
            1–1440 min, default 10 min.
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
          <label class="knob__label" for="log-retention">Log retention</label>
          <p class="knob__hint">
            How long a finished run's raw logs are kept. Pruning runs after every
            run and at app start; the runs list itself is never deleted.
            1–3650 days, default 30 days.
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
          <label class="knob__label" for="launch-concurrency">Launch concurrency</label>
          <p class="knob__hint">
            How many Quick Launch apps may start at once; the rest queue.
            1–50 apps, default 8.
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
          name="Dock"
          count={groupKnobCount("dock")}
          onToggle={() => toggleGroup("dock")}
        >
      <article class="knob" hidden={!knobVisible("dock-state")}>
        <div class="knob__body">
          <label class="knob__label" for="dock-state">Quick Launch window</label>
          <p class="knob__hint">
            The Quick Launch window floats as a palette or docks as a bar. Applies
            to an open window on save and is remembered; the window's dock toggle
            writes back here.
          </p>
        </div>
        <div class="knob__input">
          <Select
            id="dock-state"
            variant="small"
            value={dockState}
            onchange={(v) => (dockState = v)}
          >
            {#each dockStateOptions as option (option.value)}
              <option value={option.value}>{option.label}</option>
            {/each}
          </Select>
        </div>
      </article>

      <article class="knob" hidden={!knobVisible("dock-mode")}>
        <div class="knob__body">
          <label class="knob__label" for="dock-mode">Dock mode</label>
          <p class="knob__hint">
            Fixed keeps a visible strip and squeezes other windows. Auto-hide hides
            completely — push into that screen's edge and hold to call it back;
            otherwise windows keep their full size.
          </p>
        </div>
        <div class="knob__input">
          <Select id="dock-mode" variant="small" value={dockMode} onchange={(v) => changeDockMode(v)}>
            {#each dockModeOptions as option (option.value)}
              <option value={option.value}>{option.label}</option>
            {/each}
          </Select>
        </div>
      </article>

      <article class="knob" hidden={!knobVisible("dock-edge")}>
        <div class="knob__body">
          <label class="knob__label" for="dock-edge">Default dock edge</label>
          <p class="knob__hint">
            The edge a dock uses until its display remembers its own. The dock's
            left/right switch overrides per monitor.
          </p>
        </div>
        <div class="knob__input">
          <Select id="dock-edge" variant="small" value={dockEdge} onchange={(v) => (dockEdge = v)}>
            {#each dockEdgeOptions as option (option.value)}
              <option value={option.value}>{option.label}</option>
            {/each}
          </Select>
        </div>
      </article>

      <article class="knob" hidden={!knobVisible("dock-width")}>
        <div class="knob__body">
          <label class="knob__label" for="dock-width">Dock width</label>
          <p class="knob__hint">
            Wider fits longer names. Fixed caps at 30% — it reserves workspace;
            auto-hide may run to 60% — it overlays instead.
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
          <label class="knob__label" for="dock-density">List density</label>
          <p class="knob__hint">
            Text size across the Quick Launch window's three tabs. Compact fits
            more rows; Large reads easier.
          </p>
        </div>
        <div class="knob__input">
          <Select id="dock-density" variant="small" value={dockDensity} onchange={(v) => (dockDensity = v)}>
            {#each dockDensityOptions as option (option.value)}
              <option value={option.value}>{option.label}</option>
            {/each}
          </Select>
        </div>
      </article>

      {#if displays.length > 1 && knobVisible("per-monitor")}
        <!-- Per-monitor: flat knobs per display (flattened to reuse .knob, 0005 rule 5). No nested card. Global defaults above stay fallback. -->
        <div class="per-monitor" aria-labelledby="per-monitor-title">
          <div class="per-monitor__header">
            <span class="knob__label" id="per-monitor-title">Per-monitor dock</span>
            <p class="knob__hint">
              Each display remembers its own edge, mode, and width; the defaults
              above cover the rest. Choices save with the button below and apply
              next time that display docks.
            </p>
          </div>
          {#each displays as d (d.device_name)}
            {@const edgeId = `per-monitor-edge-${d.device_name.replace(/[^a-zA-Z0-9]/g, "-")}`}
            {@const modeId = `per-monitor-mode-${d.device_name.replace(/[^a-zA-Z0-9]/g, "-")}`}
            {@const widthId = `per-monitor-width-${d.device_name.replace(/[^a-zA-Z0-9]/g, "-")}`}
            {@const reasonId = `per-monitor-reason-${d.device_name.replace(/[^a-zA-Z0-9]/g, "-")}`}
            {@const hasSeam = !d.left_eligible || !d.right_eligible}
            {@const perMode = displayModes[d.device_name] ?? dockMode}
            {@const widthPct = clampWidthPctForMode(displayWidths[d.device_name] ?? dockWidthPct, perMode)}
            <article class="knob" hidden={!knobVisible("per-monitor")}>
              <div class="knob__body">
                <span class="knob__label">{d.label} · {d.resolution}</span>
                {#if hasSeam}
                  <p class="knob__hint" id={reasonId} style="color: var(--warn-text)">{SEAM_REASON}</p>
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
                  aria-label={`Dock edge on ${d.label}`}
                  aria-describedby={hasSeam ? reasonId : undefined}
                >
                  <option value="left" disabled={!d.left_eligible}>Left</option>
                  <option value="right" disabled={!d.right_eligible}>Right</option>
                </Select>
                <Select
                  id={modeId}
                  variant="small"
                  value={displayModes[d.device_name] ?? dockMode}
                  onchange={(v) => changeDisplayMode(d.device_name, v)}
                  aria-label={`Dock mode on ${d.label}`}
                >
                  <option value="auto-hide">Auto-hide</option>
                  <option value="fixed">Fixed</option>
                </Select>
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
            <label class="knob__label" for="reveal-dwell">Reveal delay</label>
            <p class="knob__hint">
              Hold time at the edge before the hidden dock slides out. Shorter
              may fire on grazes; longer needs a deliberate hold. 0–1000 ms,
              default 200 ms.
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
            <label class="knob__label" for="reveal-sensitivity">Reveal sensitivity</label>
            <p class="knob__hint">
              How far the cursor must push into the edge before the hold timer
              starts. Lower is immediate; higher ignores brushes. 0–50 px,
              default 12 px.
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
          name="Companion"
          count={groupKnobCount("companion")}
          onToggle={() => toggleGroup("companion")}
        >
          <article class="knob" hidden={!knobVisible("companion-active")}>
            <div class="knob__body">
              <label class="knob__label" for="companion-url">Active site</label>
              <p class="knob__hint">
                The site shown while Quick Launch is docked. Off removes the pane completely.
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
              >
                <option value="">Off</option>
                {#each companionUrlList as site (site.url)}
                  <option value={site.url}>{companionDisplayName(site)}</option>
                {/each}
              </Select>
            </div>
          </article>

          <article class="knob" hidden={!knobVisible("companion-height")}>
            <div class="knob__body">
              <label class="knob__label" for="companion-ratio">Pane height</label>
              <p class="knob__hint">
                Starting height; drag the divider in the dock to resize.
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
              <span class="knob__label">Saved sites</span>
              <p class="knob__hint">
                {companionUrlList.length === 1 ? "1 site saved on this PC." : `${companionUrlList.length} sites saved on this PC.`}
              </p>
            </div>
            <Button variant="secondary" onclick={() => goto("/companion")}>Manage sites</Button>
          </div>
        </GroupAccordion>
      {/if}

      {#if groupVisible("backup")}
        <!-- Backup & housekeeping: whole-app backup and Sprout updates. -->
        <GroupAccordion
          open={groupEffectiveOpen("backup")}
          controls="group-backup-body"
          name="Backup & housekeeping"
          count={groupKnobCount("backup")}
          onToggle={() => toggleGroup("backup")}
        >
      <article class="knob" hidden={!knobVisible("backup")}>
        <div class="knob__body">
          <span class="knob__label">Backup</span>
          <p class="knob__hint">
            Writes your collections into one JSON file you pick — choose what to include when you
            export; restoring adds what's missing and keeps what's already here. Run history, logs,
            settings, dock memory, and install directories never leave this PC.
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
            Export…
          </Button>
          <Button variant="secondary" onclick={restoreViaDialog} disabled={backupBusy}>
            Restore…
          </Button>
        </div>
      </article>

      <article class="knob" hidden={!knobVisible("updates")}>
        <div class="knob__body">
          <span class="knob__label">Sprout updates</span>
          <p class="knob__hint">
            Checks GitHub releases for a newer build; installing downloads it
            and restarts Sprout.
          </p>
          {#if updateState.installing}
            <p class="knob__status" role="status">
              Installing Sprout {updateState.available?.version} — Sprout
              restarts when it finishes.
            </p>
          {:else if updateState.available}
            <Notice tone="ok">Sprout {updateState.available.version} is available.</Notice>
          {:else if checking}
            <p class="knob__status" role="status">Checking…</p>
          {:else if checkResult === "current"}
            <Notice tone="ok">You're up to date.</Notice>
          {:else if checkResult === "failed"}
            <Notice tone="warn">
              Couldn't reach the release feed just now — try again later.
            </Notice>
          {/if}
        </div>
        <div class="knob__input">
          {#if updateState.installing}
            <Button disabled>Installing…</Button>
          {:else if updateState.available}
            <Button
              onclick={() => {
                installError = "";
                installConfirmOpen = true;
              }}
            >
              Install {updateState.available.version}
            </Button>
          {:else}
            <Button variant="secondary" onclick={runUpdateCheck} disabled={checking}>
              {checking ? "Checking…" : "Check for updates"}
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
          name="AI assistance"
          count={groupKnobCount("ai")}
          onToggle={() => toggleGroup("ai")}
        >
      <article class="knob" hidden={!knobVisible("ai-provider")}>
        <div class="knob__body">
          <label class="knob__label" for="ai-provider">AI provider</label>
          <p class="knob__hint">
            Optional help drafting Quick Action commands. Off installs nothing;
            existing-local uses your own on-device service as-is. Drafts never
            run — you review and save each one.
          </p>
          {#if aiProvider === "managed"}
            <p class="knob__hint">
              Enabling this provider downloads nothing. A separate Install action appears only for a fully qualified bundled recommendation.
            </p>
          {/if}
          {#if aiProvider === "cloud"}
            <p class="knob__hint">
              Cloud providers aren't in this build yet — nothing here sends
              anything anywhere.
            </p>
          {/if}
        </div>
        <div class="knob__input">
          <Select
            id="ai-provider"
            variant="small"
            value={aiProvider}
            onchange={(v) => (aiProvider = v as AiProvider)}
          >
            {#each aiProviderOptions as option (option.value)}
              <option value={option.value}>{option.label}</option>
            {/each}
          </Select>
        </div>
      </article>

      {#if aiProvider === "managed"}
        <article class="knob" hidden={!knobVisible("ai-model")}>
          <div class="knob__body">
            <p class="knob__label">Managed recommendation</p>
            {#if managedCatalog}
              <p class="knob__hint">
                Runtime: {managedCatalog.runtime.name} {managedCatalog.runtime.version} · {managedCatalog.runtime.license} · {managedSize(managedCatalog.runtime.download_size_bytes)}
              </p>
              <p class="knob__hint">Source: {managedCatalog.runtime.source}</p>
              {#if !managedCatalog.runtime.qualified}
                <Notice tone="warn">{managedCatalog.runtime.blocker}</Notice>
              {/if}
              {#each managedCatalog.models as model (model.id)}
                <p class="knob__label">{model.artifact}</p>
                <p class="knob__hint">
                  Status: {model.status}. Revision: {model.revision ?? "Not qualified"}. Download: {managedSize(model.download_size_bytes)}.
                </p>
                <p class="knob__hint">
                  Working memory: {model.memory_needs_mb === null ? "Not qualified" : `${new Intl.NumberFormat().format(model.memory_needs_mb)} MB RAM/VRAM`}. Context: {model.context_limit_tokens ?? "Not qualified"}. Runtime minimum: {model.minimum_runtime_version ?? "Not qualified"}.
                </p>
                <p class="knob__hint">License: {model.license} · Source: {model.license_source}</p>
                {#if model.blocker}
                  <Notice tone="warn">{model.blocker}</Notice>
                {/if}
                {#if model.installed}
                  <Notice tone="ok">Installed for this user. It stays stopped until Generate and unloads after 5 idle minutes.</Notice>
                {/if}
              {/each}
              {#if managedNotice}
                <p class="knob__status" role="status">{managedNotice}</p>
              {/if}
              {#if managedError}
                <Notice tone="error">{managedError}</Notice>
              {/if}
            {:else if managedError}
              <Notice tone="error">{managedError}</Notice>
            {:else}
              <p class="knob__status" role="status">Reading bundled recommendation…</p>
            {/if}
          </div>
          <div class="knob__input">
            {#if managedBusy}
              <Button type="button" variant="secondary" onclick={cancelManagedInstall}>Cancel Install</Button>
            {:else if managedCatalog}
              {#each managedCatalog.models.filter((model) => model.installable && !model.installed) as model (model.id)}
                <Button type="button" onclick={() => void installManaged(model.id)}>Install {model.artifact}</Button>
              {/each}
            {/if}
          </div>
        </article>
      {/if}

      {#if aiProvider === "existing-local"}
      <article class="knob" hidden={!knobVisible("ai-endpoint")}>
        <div class="knob__body">
          <label class="knob__label" for="ai-base-url">Local service address</label>
          <p class="knob__hint">
            Your service's loopback address, e.g. http://127.0.0.1:11434.
            Sprout connects to this machine only and never follows redirects
            elsewhere; it never starts, stops, or reconfigures your service.
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
          <label class="knob__label" for="ai-model">Local model</label>
          <p class="knob__hint">
            The exact model name your service exposes. Sprout never substitutes
            another one — an unknown name fails instead. Test connection checks
            it without saving anything.
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
            placeholder="e.g. qwen2.5-coder:7b"
            value={aiModel}
            oninput={(e) => (aiModel = (e.target as HTMLInputElement).value)}
          />
          <Button
            type="button"
            variant="secondary"
            onclick={testAiConnection}
            disabled={aiTestBusy}
          >
            {aiTestBusy ? "Testing…" : "Test connection"}
          </Button>
        </div>
      </article>
      {/if}
        </GroupAccordion>
      {/if}
    </form>
    {/if}
    {#if isDirty}
      <!-- Ticket 115: fixed bottom bar — warning text + Save/Discard, pinned
           regardless of scroll, until saved or reverted. Text + color, never
           color alone; announce via polite live region (below). -->
      <div class="dirty-bar" role="region" aria-label="Unsaved changes">
        <p class="dirty-bar__text">
          <span class="dirty-bar__dot" aria-hidden="true"></span>
          Unsaved changes — Save or Discard
        </p>
        <div class="dirty-bar__actions">
          <Button variant="secondary" onclick={discard} type="button" disabled={saving}
            >Discard</Button
          >
          <Button onclick={save} disabled={saving}>{saving ? "Saving…" : "Save"}</Button>
        </div>
      </div>
    {/if}
    <!-- Polite live region: announces appearance/disappearance without moving focus or scrolling (0004 rule 5). -->
    <p class="sr-only" aria-live="polite" aria-atomic="true">{dirtyLiveMessage}</p>
    <!-- Ticket 116: guard leaving dirty Settings — rail + window close share one three-way alertdialog. -->
    <Dialog
      open={guardOpen}
      title="Unsaved changes"
      role="alertdialog"
      width={480}
      focusTarget="#guard-keep"
      onclose={handleKeepEditing}
    >
      <div class="guard">
        <p class="guard__body">
          You have unsaved changes. Save them, discard them, or keep editing — no changes are saved until you say so.
        </p>
        <div class="guard__actions">
          <Button variant="secondary" id="guard-keep" onclick={handleKeepEditing} disabled={guardSaving || saving}
            >Keep editing</Button
          >
          <Button variant="secondary" onclick={handleDiscardGuard} disabled={guardSaving || saving}
            >Discard changes</Button
          >
          <Button onclick={handleSaveGuard} disabled={guardSaving || saving}
            >{guardSaving || saving ? "Saving…" : "Save changes"}</Button
          >
        </div>
      </div>
    </Dialog>
  {/if}
</section>

<ConfirmDialog
  open={installConfirmOpen}
  title="Update available"
  confirmLabel={updateState.installing ? "Installing…" : "Install and restart"}
  onconfirm={applyInstall}
  oncancel={() => (installConfirmOpen = false)}
>
  <p>Install Sprout {updateState.available?.version} now?</p>
  <p>Sprout restarts when the installer finishes.</p>
  {#if installError}
    <Notice tone="error">{installError}</Notice>
  {/if}
</ConfirmDialog>

<ConfirmDialog
  open={restoreCounts !== null}
  title="Restore backup?"
  confirmLabel="Restore"
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
      <strong>{restoreFile.split(/[\\/]/).pop()}</strong> contains
      {describeCounts(restoreCounts)}.
    </p>
    <p>Items that already exist here are kept — nothing is overwritten.</p>
  {:else}
    <p>This file contains no items to restore.</p>
  {/if}
</ConfirmDialog>

<ConfirmDialog
  open={exportOpen}
  title="Export backup"
  confirmLabel="Export selected…"
  confirmDisabled={!anyIncluded}
  onconfirm={() => {
    exportOpen = false;
    void exportSelected();
  }}
  oncancel={() => (exportOpen = false)}
>
  <p>Everything is included by default — untick what should stay out of the file.</p>
  <div class="export-picker" role="group" aria-label="Collections to include">
    {#each EXPORT_ORDER as key (key)}
      <label class="export-picker__item">
        <input type="checkbox" bind:checked={include[key]} />
        <span>{COLLECTIONS[key].label}</span>
      </label>
    {/each}
  </div>
</ConfirmDialog>

<style>
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
    gap: var(--space-1);
    padding: 0 var(--space-1);
  }

  .knob__body {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
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
    background: var(--bg-page);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    padding: var(--space-2) var(--space-3);
    text-align: right;
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

  .knob__unit--auto {
    width: auto;
    white-space: nowrap;
  }

  .export-picker {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .export-picker__item {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    cursor: pointer;
  }

  .export-picker__item input[type="checkbox"] {
    margin: 0;
    accent-color: var(--accent);
    width: 14px;
    height: 14px;
  }
</style>
