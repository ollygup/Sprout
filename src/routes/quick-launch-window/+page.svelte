<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import type {
    Clip,
    CompanionAudioState,
    CompanionSite,
    Group,
    LaunchEntry,
    LaunchReport,
    QuickAction,
  } from "$lib/types";
  import {
    copyClip,
    getCompanionAudioState,
    getQuickLaunchDockState,
    getSettings,
    listClips,
    listGroups,
    listLaunchEntries,
    listQuickActions,
    openCompanionExternal,
    openSprout,
    openVolumeMixer,
    runQuickAction,
    setCompanionMuted,
    startDockQuickLaunch,
    startLaunchEntry,
    switchQuickLaunchDockEdge,
    toggleQuickLaunchDock,
    setCompanionHeightRatio,
    setCompanionHeightRatioForDisplay,
    getCompanionHeightRatio,
    listDisplays,
    COMPANION_MOBILE_UA,
  } from "$lib/api";
  import {
    quickActionRuns,
    stopActionRun,
    syncQuickActionRuns,
  } from "$lib/quickActionRuns.svelte";
  import QuickActionRunControl from "$lib/components/QuickActionRunControl.svelte";
  import QuickActionDetailsDialog from "$lib/components/QuickActionDetailsDialog.svelte";
  import QuickLaunchRow from "$lib/components/QuickLaunchRow.svelte";
  import { clipTitle, launchReportSummary } from "$lib/format";
  import { hasNote } from "$lib/noteFormat";
  import { appIcons, lazyIcon } from "$lib/lazyIcon.svelte";
  import { createGroupCollapse } from "$lib/groupCollapse.svelte";
  import type { QuickLaunchDockState } from "$lib/types";
  import { restoreTheme, type ThemeMode } from "$lib/theme.svelte";
  import { titleBarDragRegion } from "$lib/quickLaunchTitleBar";
  import {
    companionWebviewBounds,
    companionZoomForWidth,
  } from "$lib/companionPane";
  import Button from "$lib/components/Button.svelte";
  import GroupAccordion from "$lib/components/GroupAccordion.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import IconButton from "$lib/components/IconButton.svelte";
  import SproutMark from "$lib/components/SproutMark.svelte";
  import Tabs from "$lib/components/Tabs.svelte";
  // Companion WebView2 (ticket 125): direct navigation so X-Frame-Options never blocks
  // (learn.microsoft.com/webview2/concepts/frames). Use Webview API when in Tauri.
  import { LogicalPosition, LogicalSize } from "@tauri-apps/api/dpi";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { Webview } from "@tauri-apps/api/webview";

  // The Quick Launch window (ticket 52): the tray's left-click target — a
  // miniature, frameless, read-only window. The backend owns its life cycle
  // (ticket 56: blur does nothing; the × button / Alt+F4 destroy it and the
  // tray reopens it at a fixed centered size — no geometry is remembered);
  // this page only renders and fires the existing runners.
  // Docking (ticket 53) is controlled from this header: the toggle pins the
  // window to the current monitor's remembered (or Settings-default) edge as
  // a Win32 AppBar, and the arrows move it left↔right while docked.
  // Quick Clips (ticket 79) joins as a third tab only while at least one
  // clip exists — an empty feature must not occupy chrome (research 0004
  // rule 2); the main app's /clips page is its discoverability home and its
  // only editing surface.
  // Ticket 93: the Launch tab lists its entries and every row starts just
  // that entry; Start all stays pinned above the list. The list mirrors the
  // main page's Groups toggle (flat when off; ungrouped-first plus
  // default-expanded accordions with count badges when on) — the window
  // itself has no configuration surface (CONTEXT: all configuration happens
  // in the main app).

  let entries = $state<LaunchEntry[]>([]);
  let actions = $state<QuickAction[]>([]);
  let clips = $state<Clip[]>([]);
  // Ticket 93: every tab mirrors its collection's Groups feature live from
  // Settings (`launch_groups` / `action_groups` / `clip_groups`), plus each
  // collection's groups in user order — the window has no configuration
  // surface (CONTEXT: all configuration happens in the main app).
  let launchGroupsOn = $state(false);
  let actionGroupsOn = $state(false);
  let clipGroupsOn = $state(false);
  // The window lists' density (Compact/Default/Large): picked on the main
  // app's Quick Launch page, read here from the same Settings. Default is
  // today's sizing; anything unrecognized falls back to it so a broken value
  // never leaves the lists unstyled.
  let density = $state("default");
  let launchGroups = $state<Group[]>([]);
  let actionGroups = $state<Group[]>([]);
  let clipGroups = $state<Group[]>([]);
  const launchCollapse = createGroupCollapse();
  const actionCollapse = createGroupCollapse();
  const clipCollapse = createGroupCollapse();
  let loading = $state(true);
  let launching = $state(false);
  // Ticket 93: the entries with a single-entry start in flight — set on
  // click, cleared only by `launch-run-done` (research 0004 rule 5: silence
  // reads as breakage). The backend's runs are single-flight, so while
  // anything is in flight every start affordance waits.
  let startingEntries = $state<Set<number>>(new Set());
  // Ticket 93: the finished run's summary line — the same wording the system
  // notification and the main page's flash carry, auto-cleared like them.
  let runNotice = $state("");
  let runNoticeTimer: ReturnType<typeof setTimeout> | undefined;
  let error = $state("");
  let tab = $state("launch");
  const SEAM_REASON = "Borders another display — cursor can't stop there";

  // Ticket 59: the dock state is never null — while the window floats it
  // carries the target edge/mode the toggle would dock to (`docked: false`),
  // so the toggle's icon tells the truth before the first dock. Ticket 63:
  // `blocked` carries the shell's auto-hide refusal while docked — transient,
  // only ever set by the backend.
  let dock = $state<QuickLaunchDockState>({
    edge: "left",
    mode: "auto-hide",
    docked: false,
    blocked: null,
    left_eligible: true,
    right_eligible: true,
    monitor: null,
    monitor_identity: null,
  });
  // Ticket 119 Study A: already-docked middle line (seam) reuses the blocked
  // banner — same wall rule and same reason line as Settings.
  const seamBlocked = $derived(
    dock.docked &&
      ((dock.edge === "left" && !dock.left_eligible) ||
        (dock.edge === "right" && !dock.right_eligible))
  );
  const showBlocked = $derived(dock.docked && (dock.blocked !== null || seamBlocked));

  // Ticket 125 Companion: single-tab mobile web view in the dock's bottom ~40%.
  // Content-gated (0004:2 / 0006:11) — companionUrl==null → no Webview, no splitter, no chrome;
  // floating never shows the pane; per-monitor height ratio falls back to settings.
  let companionUrl: string | null = $state(null);
  let companionUrlList: CompanionSite[] = $state([]);
  let companionRatio = $state(0.40);
  let companionCanGoBack = $state(false);
  let companionCanGoForward = $state(false);
  let companionHistory: string[] = $state([]);
  let companionHistoryIndex = $state(-1);
  let companionDragging = $state(false);
  let qlwMainEl: HTMLDivElement | null = $state(null);
  let companionFrameEl: HTMLIFrameElement | null = $state(null);
  let companionFrameWrapEl: HTMLDivElement | null = $state(null);
  let companionWebview: Webview | null = $state(null);
  // Whether the cached handle finished backend registration (its created
  // event fired). Distinguishes a child that died (drop the stale handle so
  // the next pass recreates) from one still registering (leave it alone —
  // nulling it now would fork a duplicate creation).
  let companionWebviewBorn = false;
  // How long a fresh child may take to finish backend registration before its
  // silence turns loud — cold WebView2 profile init is slow, but a birth that
  // never lands must say so instead of hanging forever with no pane and no
  // word anywhere (ADR-0022 Companion is a native WebView2 child).
  const COMPANION_BORN_TIMEOUT_MS = 10_000;
  const COMPANION_BORN_TIMEOUT_MSG =
    "Couldn't show the companion pane — it never finished loading.";
  let companionSyncRunning = false;
  let companionSyncPending = false;
  // Monotonic id for refreshCompanion runs — a run superseded by a newer one
  // discards its results instead of assigning stale state (e.g. a pre-save
  // read landing after an off-save and resurrecting the old URL for good).
  // Plain counter, never reactive: only compared, never rendered.
  let companionRefreshGen = 0;
  // The off-transition orphan poller below — one live at most; re-arming
  // clears the previous so rapid off/on cycles never stack pollers.
  // Bare-timer typed like the audio poll below (this tsconfig resolves the
  // bare setInterval to Node's Timeout).
  let companionSweeper: ReturnType<typeof setInterval> | undefined;
  let companionWebviewFailed = $state(false);
  let companionFailedUrl: string | null = $state(null);
  let companionFailureDetail = $state("");
  let companionOpeningExternal = $state(false);
  let companionMixerOpening = $state(false);
  // Ticket 161: Reload recreates the child WebView (bad login / dead-end
  // recovery) — busy flag drives the bar idiom (disabled + label switch).
  let companionReloading = $state(false);
  // Companion audio: persisted global mute plus the live playing read. The
  // toggle and the indicator render only inside the docked pane's own
  // toolbar, so floating and no-URL states gain no audio chrome.
  let companionMuted = $state(false);
  let companionPlaying = $state(false);
  let companionMuteBusy = $state(false);
  let companionAudioTimer: ReturnType<typeof setInterval> | undefined;
  function hasCompanionUrl(url: string | null): url is string {
    return typeof url === "string" && url.trim().length > 0;
  }
  const companionVisible = $derived(dock.docked && hasCompanionUrl(companionUrl));
  // Browser preview uses an iframe; the Windows runtime uses WebView2 or the stable failure surface.
  // Detect Tauri reliably — __TAURI_IPC__ is always present in Tauri webviews, __TAURI__ may be delayed
  const isTauri = typeof window !== "undefined" && !!((window as any).__TAURI__ || (window as any).__TAURI_IPC__ || (window as any).__TAURI_INTERNALS__);
  const useWebview = $derived(companionVisible && isTauri && !companionWebviewFailed);
  // Splitter follows the 0.25–0.60 clamp — single source with settings.rs
  function clampCompanionRatio(v: number): number {
    const f = Number(v);
    if (!Number.isFinite(f)) return 0.40;
    return Math.min(0.60, Math.max(0.25, f));
  }
  // The launch gate: the native child is only created once the saved ratio
  // for the actual dock monitor has resolved, so the first paint never sizes
  // from the 0.40 init. After that the ratio always holds a resolved value.
  let companionRatioReady = $state(false);
  // The real dock monitor: the backend's live dock state names the device the
  // dock is attached to, matched against the arrangement — per-monitor memory
  // wins, the global ratio covers every miss (floating, unknown device).
  async function resolveCompanionDisplay(): Promise<string | null> {
    if (!dock.docked || !dock.monitor) return null;
    try {
      const displays = await listDisplays();
      const match =
        displays.find((d) => d.device_name === dock.monitor) ??
        (dock.monitor_identity
          ? displays.find((d) => d.identity === dock.monitor_identity)
          : undefined);
      return match?.device_name ?? null;
    } catch {
      return null;
    }
  }
  async function persistCompanionRatio() {
    try {
      const clamped = clampCompanionRatio(companionRatio);
      await setCompanionHeightRatio(clamped);
      try {
        // The drag happened on the live dock's monitor — remember it there,
        // not on a proxy, so moving screens recalls each screen's height.
        await refreshDock();
        const display = await resolveCompanionDisplay();
        if (display) {
          await setCompanionHeightRatioForDisplay(display, clamped);
        }
      } catch (e) {
        // The global save above landed but this screen forgot — say so
        // instead of looking applied (research 0004 rule 5: silence reads
        // as breakage).
        console.error(e);
        error = `Couldn't save the Companion height for this screen — ${String(e)}`;
      }
    } catch (e) {
      console.error(e);
      error = `Couldn't save the Companion height — ${String(e)}`;
    }
  }
  async function refreshCompanion() {
    const gen = ++companionRefreshGen;
    // Ordered launch reads: dock state first (which monitor the dock sits
    // on), then the global ratio, then that monitor's override — the native
    // child is created only after the resolved ratio lands.
    await refreshDock();
    let url: string | null = null;
    let globalRatio = 0.40;
    try {
      const s = await getSettings();
      url = s.companion_url ?? null;
      companionUrlList = s.companion_url_list ?? [];
      globalRatio = s.companion_height_ratio ?? 0.40;
    } catch (e) {
      console.error(e);
    }
    // The backend state can lag the window right after a dock toggle — one
    // shot at the per-monitor read turns that transient into a permanent
    // fallback. Retry boundedly while the monitor is unknown; a genuinely
    // monitorless dock still ends promptly, and a missing entry (not an
    // unknown screen) stays a silent global fallback as before.
    const RESOLVE_ATTEMPTS = 3;
    const RESOLVE_RETRY_MS = 150;
    const RESOLVE_FAILED_MSG =
      "Couldn't read the Companion height for this screen — using the Settings height.";
    let perMonitor: number | null = null;
    let displayResolved = false;
    for (let attempt = 0; ; attempt++) {
      try {
        const display = await resolveCompanionDisplay();
        if (display) {
          displayResolved = true;
          if (error === RESOLVE_FAILED_MSG) error = "";
          try {
            perMonitor = await getCompanionHeightRatio(display);
          } catch (e) {
            console.error(e);
            error = `Couldn't read the Companion height for this screen — ${String(e)}`;
          }
          break;
        }
      } catch (e) {
        console.error(e);
      }
      if (attempt + 1 >= RESOLVE_ATTEMPTS || !dock.docked) break;
      await new Promise((r) => setTimeout(r, RESOLVE_RETRY_MS));
      await refreshDock();
    }
    if (!displayResolved && dock.docked && url) {
      error = RESOLVE_FAILED_MSG;
    }
    // A newer refresh started while this one was awaiting — its results win.
    // Applying these stale ones here would resurrect a pre-save URL after an
    // off-save (or clobber a rapid off→on), so drop them silently.
    if (gen !== companionRefreshGen) return;
    companionUrl = url;
    companionRatio = clampCompanionRatio(perMonitor ?? globalRatio);
    companionRatioReady = true;
    // Init history when url changes
    if (companionUrl) {
      if (companionHistory.length === 0 || companionHistory[0] !== companionUrl) {
        companionHistory = [companionUrl];
        companionHistoryIndex = 0;
        companionCanGoBack = false;
        companionCanGoForward = false;
      }
    } else {
      companionHistory = [];
      companionHistoryIndex = -1;
      companionCanGoBack = false;
      companionCanGoForward = false;
    }
    // The resolved ratio is in — re-sync so the first paint never measures a
    // pre-layout rect at the init value (the ratio assignment above also
    // re-fires the tracking effect; this covers the child already created).
    void syncCompanionWebview();
    // The persisted mute is the source of truth — reading heals a fresh
    // WebView toward it, so a recreated pane never comes back loud.
    try {
      const audio = await getCompanionAudioState();
      companionMuted = audio.muted;
      companionPlaying = audio.playing;
    } catch (e) {
      console.error(e);
    }
  }
  function companionGoBack() {
    if (!companionFrameEl || companionHistoryIndex <= 0) return;
    companionHistoryIndex -= 1;
    const url = companionHistory[companionHistoryIndex];
    companionCanGoBack = companionHistoryIndex > 0;
    companionCanGoForward = companionHistoryIndex < companionHistory.length - 1;
    if (companionFrameEl) companionFrameEl.src = url;
  }
  function companionGoForward() {
    if (!companionFrameEl || companionHistoryIndex >= companionHistory.length - 1) return;
    companionHistoryIndex += 1;
    const url = companionHistory[companionHistoryIndex];
    companionCanGoBack = companionHistoryIndex > 0;
    companionCanGoForward = companionHistoryIndex < companionHistory.length - 1;
    if (companionFrameEl) companionFrameEl.src = url;
  }
  async function companionOpenExternal() {
    if (!companionUrl || companionOpeningExternal) return;
    companionOpeningExternal = true;
    try {
      await openCompanionExternal(companionUrl);
    } catch (e) {
      error = `Couldn't open the companion in your browser — ${String(e)}`;
    } finally {
      companionOpeningExternal = false;
    }
  }
  // The toolbar's volume-mixer shortcut: loud/soft lives in the OS mixer, so
  // the pane links straight there instead of describing the way in words.
  async function openMixer() {
    if (companionMixerOpening) return;
    companionMixerOpening = true;
    try {
      await openVolumeMixer();
    } catch (e) {
      error = `Couldn't open the volume mixer — ${String(e)}`;
    } finally {
      companionMixerOpening = false;
    }
  }
  // The toolbar's mute toggle: persists globally, silences the live WebView,
  // and reads back at once so silence never feels broken.
  async function toggleCompanionMute() {
    if (companionMuteBusy) return;
    const wasMuted = companionMuted;
    companionMuteBusy = true;
    try {
      const next = await setCompanionMuted(!wasMuted);
      companionMuted = next.muted;
      companionPlaying = next.playing;
    } catch (e) {
      console.error(e);
      error = `Couldn't ${wasMuted ? "unmute" : "mute"} the companion — ${String(e)}`;
    } finally {
      companionMuteBusy = false;
    }
  }
  async function companionRetry() {
    const webview = companionWebview;
    companionWebview = null;
    companionWebviewBorn = false;
    companionWebviewFailed = false;
    companionFailedUrl = null;
    companionFailureDetail = "";
    if (webview) {
      try { await webview.close(); } catch {}
    }
    await syncCompanionWebview();
  }
  // Ticket 161: Reload reuses the recreate path above (null handle, close,
  // born/failed flags reset, resync) — the saved site URL is untouched, so a
  // stuck login recovers to a fresh page at the same address. Busy-guarded
  // like the bar's mixer/external buttons; never navigation chrome (ADR-0022).
  async function companionReload() {
    if (companionReloading || !companionUrl) return;
    companionReloading = true;
    try {
      await companionRetry();
    } finally {
      companionReloading = false;
    }
  }
  function handleCompanionLoad() {
    // Track in-pane navigation for Back/Forward (0004:2 show-if-you-can).
    // For cross-origin iframes we cannot read contentWindow.location, so we
    // synthesize history growth via load count — after first nav, Back appears.
    // The current Tauri child-WebView surface does not expose native history state.
    try {
      const current = companionFrameEl?.src ?? companionUrl ?? "";
      if (current && companionHistory[companionHistoryIndex] !== current) {
        // Truncate forward history on new nav
        companionHistory = companionHistory.slice(0, companionHistoryIndex + 1);
        companionHistory.push(current);
        companionHistoryIndex = companionHistory.length - 1;
      }
    } catch {}
    companionCanGoBack = companionHistoryIndex > 0;
    companionCanGoForward = companionHistoryIndex < companionHistory.length - 1;
    // Simulate after first real navigation inside pane, Back appears — if we
    // have only one entry, keep false until second load
  }
  function onCompanionSplitterPointerDown(e: PointerEvent) {
    if (!qlwMainEl) return;
    companionDragging = true;
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    const onMove = (ev: PointerEvent) => {
      if (!qlwMainEl || !companionDragging) return;
      const rect = qlwMainEl.getBoundingClientRect();
      const offsetFromTop = ev.clientY - rect.top;
      // Companion is bottom pane: ratio = 1 - (splitterPosition / height)
      // Approximate splitter at 40%: top part height = total*(1-ratio)
      const total = rect.height;
      if (total <= 0) return;
      let newRatio = 1 - offsetFromTop / total;
      newRatio = clampCompanionRatio(newRatio);
      companionRatio = newRatio;
    };
    const onUp = () => {
      companionDragging = false;
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      void persistCompanionRatio();
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    // Keep native WebView bounds in sync during drag
    void syncCompanionWebview();
  }

  function onCompanionSplitterKeyDown(e: KeyboardEvent) {
    let next: number;
    switch (e.key) {
      case "ArrowUp":
        next = companionRatio + 0.05;
        break;
      case "ArrowDown":
        next = companionRatio - 0.05;
        break;
      case "Home":
        next = 0.60;
        break;
      case "End":
        next = 0.25;
        break;
      default:
        return;
    }
    e.preventDefault();
    companionRatio = clampCompanionRatio(next);
    void syncCompanionWebview();
    void persistCompanionRatio();
  }

  // Native WebView2 uses direct navigation so X-Frame-Options never blocks.
  async function syncCompanionWebview() {
    companionSyncPending = true;
    if (companionSyncRunning) return;
    companionSyncRunning = true;
    try {
      while (companionSyncPending) {
        companionSyncPending = false;
        await syncCompanionWebviewOnce();
      }
    } finally {
      companionSyncRunning = false;
      if (companionSyncPending) void syncCompanionWebview();
    }
  }

  async function syncCompanionWebviewOnce() {
    // In browser preview without Tauri, skip native webview and use iframe fallback
    if (!isTauri) return;
    if (companionWebviewFailed && companionFailedUrl === companionUrl) return;
    if (companionFailedUrl !== companionUrl) {
      companionWebviewFailed = false;
      companionFailedUrl = null;
      companionFailureDetail = "";
    }
    if (!companionVisible || !companionUrl) {
      const hadWebview = companionWebview !== null;
      if (companionWebview) {
        try { await companionWebview.close(); } catch {}
        companionWebview = null;
        companionWebviewBorn = false;
      }
      // A creation racing this teardown can register its native child after
      // the cached handle died (cold WebView2 profile init is slow) — sweep
      // by label so no ownerless child keeps a renderer alive after Off.
      // The backend destroys by label on save too; this catches a child that
      // lands after that destroy. The late re-sweep below only arms when a
      // child actually existed, so idle off-states schedule no timers.
      try {
        const orphan = await Webview.getByLabel("companion");
        if (orphan) await orphan.close();
      } catch {}
      if (hadWebview) {
        // A child created just now can still land after the kills above
        // (cold WebView2 profile init is slow, slower still on weak devices,
        // where this read as white → bare site → gone at ~15–20 s). Re-sweep
        // on a short poll while the pane stays gone (off or undocked with the
        // same URL) instead of once after 12 s, so an orphan lives ~1 s, not
        // ~12 s. Every tick re-checks the gate: a legitimately returned pane
        // (rapid off→on) stops the poller untouched, as does the hard cap —
        // stale pollers from superseded transitions self-terminate.
        const sweptUrl = companionUrl;
        clearInterval(companionSweeper);
        companionSweeper = setInterval(() => {
          if ((companionUrl ?? null) !== (sweptUrl ?? null) || useWebview) {
            clearInterval(companionSweeper);
            companionSweeper = undefined;
            return;
          }
          void (async () => {
            try {
              const late = await Webview.getByLabel("companion");
              if (late) await late.close();
            } catch {
              clearInterval(companionSweeper);
              companionSweeper = undefined;
            }
          })();
        }, 1000);
        // Hard stop past the cold-init window: a birth that never lands is
        // the birth-timeout banner's job, not this poller's.
        window.setTimeout(() => {
          clearInterval(companionSweeper);
          companionSweeper = undefined;
        }, COMPANION_BORN_TIMEOUT_MS + 5000);
      }
      companionWebviewFailed = false;
      companionFailedUrl = null;
      companionFailureDetail = "";
      return;
    }
    if (!companionFrameWrapEl) return;
    // Recreate if URL changed or not yet created
    const needsCreate = !companionWebview;
    // For URL changes, easiest is to recreate the webview (Tauri WebView has no navigate API)
    // We track lastUrl via a hidden prop
    const lastUrl = (companionWebview as any)?._companionUrl as string | undefined;
    const urlChanged = lastUrl !== companionUrl;
    // Launch gate: never create (or recreate) the native child before the
    // saved ratio for the actual dock monitor has resolved — the re-sync
    // after it lands performs the first sizing.
    if ((needsCreate || urlChanged) && !companionRatioReady) return;
    const bounds = companionWebviewBounds(companionFrameWrapEl.getBoundingClientRect());
    if (needsCreate || urlChanged) {
      if (companionWebview) {
        try { await companionWebview.close(); } catch {}
        companionWebview = null;
        companionWebviewBorn = false;
      }
      const targetUrl = companionUrl;
      try {
        const win = getCurrentWindow();
        // Ensure previous companion webview removed (idempotent)
        try {
          const existing = await Webview.getByLabel("companion");
          if (existing) await existing.close();
        } catch {}
        const wv = new Webview(win, "companion", {
          url: targetUrl,
          x: bounds.x,
          y: bounds.y,
          width: bounds.width,
          height: bounds.height,
          userAgent: COMPANION_MOBILE_UA,
          incognito: false,
          dataDirectory: "companion",
          transparent: false,
          focus: false,
          dragDropEnabled: false,
        });
        (wv as any)._companionUrl = targetUrl;
        wv.once("tauri://created", () => {
          if (companionWebview !== wv) return;
          companionWebviewBorn = true;
          console.log("companion webview created", targetUrl);
          void wv.setZoom(companionZoomForWidth(bounds.width)).catch((e) => {
            console.error("syncCompanionWebview zoom failed", e);
          });
          // A child born while the details dialog sits above starts yielded —
          // the synchronous hide after construction can lose to backend
          // creation, and a native child paints above all web content, so the
          // created callback re-asserts the yield (ADR-0022 Companion is a
          // native WebView2 child).
          if (detailsAction !== null) {
            // Registration may still be landing — the created callback above
            // retries with the error line attached, so a race here stays quiet.
            void wv.hide().catch((e) => console.error(e));
          }
          companionWebviewFailed = false;
          companionFailedUrl = null;
          companionFailureDetail = "";
          // A late birth still heals — the timeout banner claimed the error
          // line only because nothing had arrived yet.
          if (error === COMPANION_BORN_TIMEOUT_MSG) error = "";
          // A fresh WebView starts unmuted — push the persisted choice back
          // in before anything audible can leak through.
          void getCompanionAudioState()
            .then((audio) => {
              companionMuted = audio.muted;
              companionPlaying = audio.playing;
            })
            .catch((e) => {
              console.error("companion audio sync failed", e);
            });
        });
        wv.once("tauri://error", (e) => {
          if (companionWebview !== wv) return;
          console.error("companion webview error", e);
          void wv.close().catch(() => {});
          companionWebview = null;
          companionWebviewBorn = false;
          companionFailedUrl = targetUrl;
          companionFailureDetail = String(e.payload ?? "");
          companionWebviewFailed = true;
        });
        companionWebview = wv;
        companionWebviewBorn = false;
        companionWebviewFailed = false;
        // A child created while the details dialog sits above starts yielded
        // at birth — the created callback above re-asserts the hide there, so
        // nothing hides here: the child cannot paint before registration, and
        // every call until then only throws WebviewNotFound noise.
        // The quiet passes below stay silent by design, so a birth that never
        // completes expires here instead of hanging forever with no pane and
        // no word anywhere (ADR-0022 Companion is a native WebView2 child).
        window.setTimeout(() => {
          if (companionWebview === wv && !companionWebviewBorn && !companionWebviewFailed) {
            console.error("companion webview creation timed out", targetUrl);
            if (!error) error = COMPANION_BORN_TIMEOUT_MSG;
          }
        }, COMPANION_BORN_TIMEOUT_MS);
      } catch (e) {
        console.error("syncCompanionWebview create failed", e);
        companionWebview = null;
        companionWebviewBorn = false;
        companionFailedUrl = targetUrl;
        companionFailureDetail = String(e);
        companionWebviewFailed = true;
      }
      return;
    }
    // Existing webview: update bounds live
    const webview = companionWebview;
    if (!webview) return;
    // A child still registering answers every call with WebviewNotFound until
    // its created event lands — queue nothing, log nothing; the creation
    // timeout above turns a never-landing registration loud (ADR-0022
    // Companion is a native WebView2 child).
    if (!companionWebviewBorn) return;
    try {
      await webview.setPosition(new LogicalPosition(bounds.x, bounds.y));
      await webview.setSize(new LogicalSize(bounds.width, bounds.height));
      await webview.setZoom(companionZoomForWidth(bounds.width));
    } catch (e) {
      // A registered child answering this way is gone without its null-out
      // landing — drop the stale handle so the next pass recreates instead of
      // throwing on a dead label forever. An as-yet-unregistered child keeps
      // its handle: its created callback settles it (ADR-0022 Companion is a
      // native WebView2 child).
      console.error("syncCompanionWebview bounds failed", e);
      if (companionWebview === webview && companionWebviewBorn) {
        companionWebview = null;
        companionWebviewBorn = false;
        companionSyncPending = true;
      }
    }
  }

  // Ticket 79: one-click copy feedback — the copied row flashes "Copied"
  // for ~1.2 s and a polite live region announces it; silence reads as
  // breakage (research 0004 rule 5).
  let copiedId = $state<number | null>(null);
  // Ticket 130: the row's text side opens the action's details read-only
  // (research 0006:13 one grammar per surface; 0004:3 level 1 here, full
  // configuration stays in the main app) — the icon button alone runs/stops.
  let detailsAction: QuickAction | null = $state(null);
  let copiedAnnouncement = $state("");
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;

  onMount(() => {
    load();
    refreshDock();
    refreshCompanion();
    // Ticket 42: the run finishes on the backend's background thread — the
    // summary lands as a system notification, this event releases the start
    // affordances (Start all plus ticket 93's entry rows) and posts the
    // summary line.
    // Ticket 57: the backend emits `quick-launch-changed` after every command
    // that mutates what this window renders — Launch entry mutations, Quick
    // Action mutations, Clip mutations, `update_settings`, `update_theme`.
    // The window listens once and re-runs its loads plus its dock-state
    // refresh, so entries/actions/clips/settings changed in the main app —
    // including the theme and the Groups toggle — appear without reopening
    // it.
    const unlisteners: (() => void)[] = [];
    listen<LaunchReport>("launch-run-done", (event) => {
      launching = false;
      startingEntries = new Set();
      flashRun(launchReportSummary(event.payload));
    }).then((fn) => unlisteners.push(fn));
    listen("quick-launch-changed", () => {
      load();
      refreshDock();
      refreshCompanion();
    }).then((fn) => unlisteners.push(fn));
    listen("displays-changed", () => {
      refreshDock();
      refreshCompanion();
    }).then((fn) => unlisteners.push(fn));
    // Ticket 61: a background dock failure — a shell-initiated re-assert
    // (ABN_POSCHANGED) or the drift watchdog — surfaces in the window's error
    // banner instead of leaving a half-docked bar.
    listen<string>("quick-launch-dock-error", (e) => {
      error = e.payload;
    }).then((fn) => unlisteners.push(fn));
    // The mute toggle fans its state out so the indicator follows at once,
    // without waiting for the next poll below.
    listen<CompanionAudioState>("companion-audio-changed", (e) => {
      companionMuted = e.payload.muted;
      companionPlaying = e.payload.playing;
    }).then((fn) => unlisteners.push(fn));
    return () => {
      unlisteners.forEach((fn) => fn());
      clearTimeout(copiedTimer);
      clearTimeout(runNoticeTimer);
      clearInterval(companionAudioTimer);
      clearInterval(companionSweeper);
      companionSweeper = undefined;
      // Cleanup companion webview on unmount
      if (companionWebview) {
        void companionWebview.close().catch(() => {});
        companionWebview = null;
        companionWebviewBorn = false;
      }
    };
  });

  // Keep native WebView bounds + url in sync whenever its inputs change (also handles X-Frame-Options avoidance)
  $effect(() => {
    void companionVisible;
    void companionUrl;
    void companionRatio;
    void dock.docked;
    void qlwMainEl;
    void companionFrameWrapEl;
    void syncCompanionWebview();
  });

  // Resolves the backend's live Companion child by label. The cached handle
  // can go stale — a close racing its null-out leaves a dead label behind —
  // and every call through it then fails with "webview not found" while the
  // pane stays bricked. Adopting the live child heals that; the URL tag rides
  // along so the next bounds pass sees no change to recreate (ADR-0022
  // Companion is a native WebView2 child).
  async function liveCompanionChild(): Promise<Webview | null> {
    if (!isTauri || !companionUrl) return null;
    try {
      const live = await Webview.getByLabel("companion");
      if (!live) return null;
      (live as any)._companionUrl = companionUrl;
      companionWebview = live;
      companionWebviewBorn = true;
      return live;
    } catch (e) {
      console.error(e);
      return null;
    }
  }

  // The details dialog always sits above the Companion pane. The pane is a
  // native child window, which paints above all web content — no CSS z-index
  // can cover it — so the child yields while the dialog is open and is
  // restored after (ADR-0022 Companion is a native WebView2 child).
  $effect(() => {
    if (!isTauri) return;
    const webview = companionWebview;
    const dialogOpen = detailsAction !== null;
    if (!webview) return;
    // A child still registering has nothing to hide or show yet — its created
    // callback re-asserts the yield when the dialog sits above, so attempting
    // here only throws WebviewNotFound noise (ADR-0022 Companion is a native
    // WebView2 child).
    if (!companionWebviewBorn) return;
    if (dialogOpen) {
      // A refused hide must reach the error line, not just the console — a
      // native child paints above all web content, so a silent denial reads
      // as the dialog sliding behind the pane (ADR-0022 Companion is a
      // native WebView2 child).
      void webview.hide().catch(async (e) => {
        console.error(e);
        const live = await liveCompanionChild();
        if (live && live !== webview) {
          try {
            await live.hide();
          } catch (e2) {
            console.error(e2);
            error = `Couldn't hide the companion pane — ${String(e2)}`;
          }
        } else if (!live && companionWebview === webview && companionWebviewBorn) {
          // Nothing alive under the label and the cached handle was
          // registered — it died without its null-out landing. Drop it so the
          // next pass recreates; silence is correct, nothing was covering.
          companionWebview = null;
          companionWebviewBorn = false;
          void syncCompanionWebview();
        }
      });
    } else {
      void (async () => {
        try {
          await webview.show();
        } catch (e) {
          console.error(e);
          const live = await liveCompanionChild();
          if (live && live !== webview) {
            try {
              await live.show();
            } catch (e2) {
              console.error(e2);
              error = `Couldn't restore the companion pane — ${String(e2)}`;
            }
          } else if (!live && companionWebview === webview && companionWebviewBorn) {
            // The reported case: the cached handle outlived its child. Drop
            // it — the trailing sync recreates silently, so no error line.
            companionWebview = null;
            companionWebviewBorn = false;
          } else if (live) {
            error = `Couldn't restore the companion pane — ${String(e)}`;
          }
        }
        await syncCompanionWebview();
      })();
    }
  });

  // The playing indicator polls the live WebView while the pane shows — the
  // cheapest honest read, with no animation to honor reduced motion.
  $effect(() => {
    if (!companionVisible) return;
    const sync = async () => {
      try {
        const audio = await getCompanionAudioState();
        companionMuted = audio.muted;
        companionPlaying = audio.playing;
      } catch (e) {
        console.error(e);
      }
    };
    void sync();
    companionAudioTimer = setInterval(() => void sync(), 2500);
    return () => clearInterval(companionAudioTimer);
  });

  $effect(() => {
    const frame = companionFrameWrapEl;
    if (!frame || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => void syncCompanionWebview());
    observer.observe(frame);
    return () => observer.disconnect();
  });

  async function refreshDock() {
    try {
      dock = await getQuickLaunchDockState();
    } catch (e) {
      console.error(e);
    }
  }

  async function toggleDock() {
    error = "";
    try {
      await toggleQuickLaunchDock();
      await refreshDock();
      // A fresh dock can sit on another screen — re-resolve the Companion
      // ratio for the monitor it actually landed on.
      await refreshCompanion();
    } catch (e) {
      console.error(e);
      error = String(e);
    }
  }

  async function switchEdge(edge: "left" | "right") {
    error = "";
    try {
      await switchQuickLaunchDockEdge(edge);
      // The backend settles the blocked state during the switch (ticket 63) —
      // re-read instead of merging locally.
      await refreshDock();
    } catch (e) {
      console.error(e);
      error = String(e);
    }
  }

  // The window's one hard promise to itself: never an eternal "Loading…".
  // The ticket-79 freeze presented exactly that way — healthy commands,
  // dead paint — so any startup load outliving this budget reads as failed
  // and surfaces the error line with a Try again affordance.
  const LOAD_TIMEOUT_MS = 10_000;

  function withTimeout<T>(pending: Promise<T>, what: string): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(
        () => reject(new Error(`${what} did not answer in time`)),
        LOAD_TIMEOUT_MS
      );
      pending.then(
        (value) => {
          clearTimeout(timer);
          resolve(value);
        },
        (e) => {
          clearTimeout(timer);
          reject(e);
        }
      );
    });
  }

  async function load() {
    loading = true;
    try {
      const [entriesResult, actionsResult, clipsResult, settings, lgs, ags, cgs] =
        await Promise.all([
          withTimeout(listLaunchEntries(), "The launch list"),
          withTimeout(listQuickActions(), "The quick actions list"),
          withTimeout(listClips(), "The clips list"),
          withTimeout(getSettings(), "The settings"),
          withTimeout(listGroups("launch"), "The launch groups list"),
          withTimeout(listGroups("action"), "The action groups list"),
          withTimeout(listGroups("clip"), "The clip groups list"),
          // Ticket 98: the shared run-state store — the same one the Quick
          // Actions page reads — seeds itself from the registry here.
          withTimeout(syncQuickActionRuns(), "The running-actions check"),
        ]);
      entries = entriesResult;
      actions = actionsResult;
      clips = clipsResult;
      // Ticket 130: keep an open details dialog live across background
      // reloads (`quick-launch-changed` fires on main-app edits) — a
      // deleted action closes it instead of showing a ghost.
      if (detailsAction) {
        const openId = detailsAction.id;
        detailsAction = actionsResult.find((a) => a.id === openId) ?? null;
      }
      // The same settings read carries the theme and all three Groups
      // features — every one live-updates through `quick-launch-changed`.
      launchGroupsOn = settings.launch_groups === "on";
      actionGroupsOn = settings.action_groups === "on";
      clipGroupsOn = settings.clip_groups === "on";
      // The same read carries the list density the main page's features menu
      // writes — the window owns no configuration surface of its own, so it
      // re-reads here on every `quick-launch-changed`.
      density =
        settings.dock_density === "compact" ||
        settings.dock_density === "large"
          ? settings.dock_density
          : "default";
      const mode = settings.theme as ThemeMode;
      if (mode === "system" || mode === "light" || mode === "dark") {
        restoreTheme(mode);
      }
      launchGroups = lgs;
      actionGroups = ags;
      clipGroups = cgs;
      launchCollapse.prune(lgs.map((g) => g.id));
      actionCollapse.prune(ags.map((g) => g.id));
      clipCollapse.prune(cgs.map((g) => g.id));
      // Deleting the last clip removes the third tab again (accepted) — if
      // it was selected, land on Launch rather than a dead selection. Hiding
      // every clip hides the tab the same way — the dock shows only what is
      // visible.
      if (clips.filter((c) => (c.show_in_dock ?? true) !== false).length === 0 && tab === "clips") tab = "launch";
      error = "";
    } catch (e) {
      console.error(e);
      error = String(e);
    } finally {
      loading = false;
    }
  }

  // Ticket 79: base two tabs, plus Quick Clips iff at least one clip
  // exists. Short labels and icons feed the strip's measured degradation
  // chain (research 0004 rule 4); `title` keeps every stage named for
  // tooltips and assistive tech. Icon names verified against the existing
  // set in Icon.svelte (rocket / terminal / copy). Hidden clips never count
  // — the dock tab appears only while a visible clip exists.
  const qlTabs = $derived.by(() => {
    const tabs = [
      {
        id: "launch",
        label: "Quick Launch",
        shortLabel: "Launch",
        icon: "rocket",
        title: "Quick Launch",
      },
      {
        id: "actions",
        label: "Quick Actions",
        shortLabel: "Actions",
        icon: "terminal",
        title: "Quick Actions",
      },
    ];
    if (clips.filter((c) => (c.show_in_dock ?? true) !== false).length > 0) {
      tabs.push({
        id: "clips",
        label: "Quick Clips",
        shortLabel: "Clips",
        icon: "copy",
        title: "Quick Clips",
      });
    }
    return tabs;
  });

  async function start() {
    launching = true;
    error = "";
    try {
      await startDockQuickLaunch();
    } catch (e) {
      console.error(e);
      error = String(e);
      launching = false;
    }
  }

  // ------------------- ticket 93/97: clickable entries + groups ----------

  /** Per-item dock visibility: hidden items never reach this surface — each
   *  Start-all starts exactly what its surface shows. Missing (legacy) means
   *  visible. */
  function isDockVisible(item: { show_in_dock?: boolean | null }): boolean {
    return (item.show_in_dock ?? true) !== false;
  }

  const dockEntries = $derived(entries.filter(isDockVisible));
  const dockActions = $derived(actions.filter(isDockVisible));
  const dockClips = $derived(clips.filter(isDockVisible));

  /** Sections exist only once at least one group does — and in this
   *  read-only surface a group with no members renders nothing at all
   *  (research 0004 rule 2): there is no ⋯ menu here to fill it from. A
   *  group whose members are all hidden drops its section here. */
  const launchGrouped = $derived(launchGroupsOn && launchGroups.length > 0);

  const launchUngrouped = $derived(
    dockEntries.filter((e) => e.group_id === null)
  );

  const launchSections = $derived(
    launchGroups
      .map((g) => ({
        group: g,
        rows: dockEntries.filter((e) => e.group_id === g.id),
      }))
      .filter((s) => s.rows.length > 0)
  );

  const actionsGrouped = $derived(actionGroupsOn && actionGroups.length > 0);

  const actionsUngrouped = $derived(
    dockActions.filter((a) => a.group_id === null)
  );

  const actionSections = $derived(
    actionGroups
      .map((g) => ({
        group: g,
        rows: dockActions.filter((a) => a.group_id === g.id),
      }))
      .filter((s) => s.rows.length > 0)
  );

  const clipsGrouped = $derived(clipGroupsOn && clipGroups.length > 0);

  const clipsUngrouped = $derived(dockClips.filter((c) => c.group_id === null));

  const clipSections = $derived(
    clipGroups
      .map((g) => ({
        group: g,
        rows: dockClips.filter((c) => c.group_id === g.id),
      }))
      .filter((s) => s.rows.length > 0)
  );

  /** True while any launch run is in flight — Start all and the entry rows
   *  share one backend pipeline whose runs are single-flight, so every
   *  start affordance waits together rather than inviting a rejection. */
  const startInFlight = $derived(launching || startingEntries.size > 0);

  /** Starts just this entry through the same pipeline as Start all
   *  (ticket 93). The row says "Starting…" until `launch-run-done` lands;
   *  a rejection (single-flight guard, vanished entry) releases immediately
   *  and surfaces its reason in the error line. */
  async function startEntry(entry: LaunchEntry) {
    error = "";
    const next = new Set(startingEntries);
    next.add(entry.id);
    startingEntries = next;
    try {
      await startLaunchEntry(entry.id);
    } catch (e) {
      console.error(e);
      error = String(e);
      const recovered = new Set(startingEntries);
      recovered.delete(entry.id);
      startingEntries = recovered;
    }
  }

  /** Ticket 93: the finished run's summary as a quiet status line — visible
   *  feedback for both Start all and single-entry starts (research 0004
   *  rule 5), auto-cleared on the main page's flash cadence. */
  function flashRun(message: string) {
    runNotice = message;
    clearTimeout(runNoticeTimer);
    runNoticeTimer = setTimeout(() => (runNotice = ""), 3200);
  }

  async function run(action: QuickAction) {
    error = "";
    try {
      await runQuickAction(action.id);
    } catch (e) {
      console.error(e);
      error = String(e);
    }
  }

  /** Stop (tickets 62 & 92) via the shared store's lifecycle (ticket 98):
   *  Stopping is set and cleared there; only a refusal surfaces here. */
  async function stop(action: QuickAction) {
    error = "";
    try {
      await stopActionRun(action.id);
    } catch (e) {
      console.error(e);
      error = String(e);
    }
  }

  /** Copies via the clipboard command and flashes the row only once the
   *  write has honestly landed (ticket 78's command contract). */
  async function copy(clip: Clip) {
    error = "";
    try {
      await copyClip(clip.id);
      copiedId = clip.id;
      copiedAnnouncement = `${clipTitle(clip.name, clip.content)} copied.`;
      clearTimeout(copiedTimer);
      copiedTimer = setTimeout(() => (copiedId = null), 1200);
    } catch (e) {
      console.error(e);
      error = String(e);
    }
  }

  function close() {
    invoke("close_quick_launch_window").catch((e) => console.error(e));
  }

  // Ticket 123: the dock header's mark doubles as a home affordance — a
  // second entry point to the main window beside the tray's right-click menu.
  // Reuses `tray::open_sprout` (`open_main_window` + `open_if_docked`, single
  // seam for tray, dock and single-instance) so dock and tray share the same
  // main-thread foreground and `800ms` zombie handling; failure is logged.
  let openingMain = $state(false);
  async function openMain() {
    if (openingMain) return;
    openingMain = true;
    try {
      await openSprout();
    } catch (e) {
      console.error(e);
    } finally {
      openingMain = false;
    }
  }
</script>

<svelte:head>
  <title>Quick Launch</title>
</svelte:head>

<div
  class="qlw"
  class:qlw--docked={dock.docked}
  class:qlw--docked-left={dock.docked && dock.edge === "left"}
  class:qlw--docked-right={dock.docked && dock.edge === "right"}
  class:qlw--density-compact={density === "compact"}
  class:qlw--density-large={density === "large"}
>
  <header
    class="qlw__bar"
    data-tauri-drag-region={titleBarDragRegion(dock.docked)}
  >
    <button
      class="qlw__mark"
      type="button"
      aria-label="Open Sprout"
      title="Open Sprout"
      aria-busy={openingMain}
      disabled={openingMain}
      data-tauri-drag-region="false"
      onclick={openMain}
    >
      <SproutMark size={16} />
    </button>
    {#if !dock.docked}
      <h1 class="qlw__title">Quick Launch</h1>
    {:else}
      <span class="qlw__spacer" aria-hidden="true"></span>
      <IconButton
        icon="chevron-left"
        label={dock.left_eligible ? "Dock to the left edge" : SEAM_REASON}
        quiet
        disabled={dock.edge === "left" || !dock.left_eligible}
        onclick={() => switchEdge("left")}
      />
      <IconButton
        icon="chevron-right"
        label={dock.right_eligible ? "Dock to the right edge" : SEAM_REASON}
        quiet
        disabled={dock.edge === "right" || !dock.right_eligible}
        onclick={() => switchEdge("right")}
      />
    {/if}
    <IconButton
      icon={dock.docked
        ? "undock"
        : dock.edge === "left"
          ? "dock-left"
          : "dock-right"}
      label={dock.docked
        ? "Undock — float again"
        : dock.edge === "left"
          ? "Dock to the left edge"
          : "Dock to the right edge"}
      quiet
      onclick={toggleDock}
    />
    <IconButton icon="x" label="Close window" onclick={close} />
  </header>

  {#if showBlocked}
    <!-- Ticket 63: auto-hide was refused by the shell — say why and offer the
         free edge instead of silently pinning the strip forever.
         Ticket 119 reuses the same banner for a seam-docked strip. -->
    <div class="qlw__blocked" role="status">
      <div class="qlw__blocked-top">
        <span class="qlw__blocked-icon" aria-hidden="true">
          <Icon name="warn" size={15} />
        </span>
        <p class="qlw__blocked-text">
          {#if seamBlocked}
            {SEAM_REASON}
          {:else}
            {dock.blocked}. Hiding still works — the strip slides on its own
            while that edge stays busy.
          {/if}
        </p>
      </div>
      <Button
        variant="ghost"
        onclick={() => switchEdge(dock.edge === "left" ? "right" : "left")}
      >
        {#if seamBlocked}
          Move to outer edge
        {:else}
          Move to the {dock.edge === "left" ? "right" : "left"} edge
        {/if}
      </Button>
    </div>
  {/if}

  {#snippet launchRow(entry: LaunchEntry)}
    <!-- Ticket 93: one entry, one click. The accessible name carries the verb so screen readers hear what the
         click does ("Start Spotify", not "Spotify, button").
         Ticket 134: thin adapter over the shared QuickLaunchRow shell — badge, name and Starting…
         stay collection content; the shell owns the card box, states and layout. The lazy icon
         observes the badge (a stable ancestor of the icon slot); no tooltip, as before. -->
    <QuickLaunchRow
      mainLabel={`Start ${entry.name}`}
      disabled={startInFlight}
      onmain={() => startEntry(entry)}
    >
      <span
        class="qlw__entry-badge"
        aria-hidden="true"
        use:lazyIcon={entry.kind === "app" ? entry.target : ""}
      >
        {#if entry.kind === "app" && appIcons[entry.target]}
          <!-- Ticket 97: the app's real icon, lazily extracted; kind
               glyphs stay for commands and unresolvable targets. -->
          <img
            class="qlw__entry-icon"
            src={appIcons[entry.target]}
            alt=""
            width={16}
            height={16}
          />
        {:else}
          <Icon
            name={entry.kind === "app" ? "rocket" : "terminal"}
            size={14}
          />
        {/if}
      </span>
      <span
        class="qlw__entry-name"
        class:qlw__entry-name--muted={startInFlight}
      >
        {entry.name}
      </span>
      {#if startingEntries.has(entry.id)}
        <span class="qlw__entry-starting">Starting…</span>
      {/if}
    </QuickLaunchRow>
  {/snippet}

  {#snippet actionRow(action: QuickAction)}
    <!-- Ticket 130: `[flex text | fixed full-height icon Run/Stop]` — the text side opens the
         details dialog, the icon button alone runs/stops. Two sibling buttons, never nested.
         Ticket 134: thin adapter over the shared QuickLaunchRow shell — the details verb, the
         content-gated note glyph and the tooltip text stay collection content; the shell owns
         the card box, the split layout and the tip anchoring. -->
    <QuickLaunchRow
      mainLabel={hasNote(action.note)
        ? `About ${action.name} (has note)`
        : `About ${action.name}`}
      tipId={`qlw-tip-action-${action.id}`}
      tipName={action.name}
      tipBody={action.command}
      onmain={() => (detailsAction = action)}
    >
      <span class="qlw__action-name">{action.name}</span>
      {#if hasNote(action.note)}
        <!-- Content-gated note glyph only — no note content on constrained surfaces (research 0004 rule 3, 0006 pattern 14) -->
        <span class="qlw__note" aria-hidden="true" title="Has note">
          <Icon name="note" size={12} />
        </span>
      {/if}
      {#snippet trailing()}
        <!-- Ticket 98: the three-state control is shared with the main app's
             Quick Actions page — one vocabulary, one spinner; here in ticket
             130's compact icon-only form while the roomy page keeps icon+text. -->
        <QuickActionRunControl
          compact
          name={action.name}
          stoppable={action.stoppable}
          running={quickActionRuns.running.has(action.id)}
          stopping={quickActionRuns.stopping.has(action.id)}
          onrun={() => run(action)}
          onstop={() => stop(action)}
          describedby={`qlw-tip-action-${action.id}`}
        />
      {/snippet}
    </QuickLaunchRow>
  {/snippet}

  {#snippet clipRow(clip: Clip)}
    {@const title = clipTitle(clip.name, clip.content)}
    <!-- Ticket 134: thin adapter over the shared QuickLaunchRow shell — badge, title, excerpt
         and the tooltip text stay collection content; the shell owns the card box and tip. -->
    <QuickLaunchRow
      mainLabel={`Copy ${title} to the clipboard`}
      tipId={`qlw-tip-clip-${clip.id}`}
      tipName={title}
      tipBody={clip.content}
      onmain={() => copy(clip)}
    >
      <span class="qlw__clip-badge" aria-hidden="true">
        <Icon name={copiedId === clip.id ? "check" : "copy"} size={14} />
      </span>
      <span class="qlw__clip-name">{title}</span>
      {#if copiedId === clip.id}
        <span class="qlw__clip-copied">Copied</span>
      {:else}
        <span class="qlw__clip-excerpt">{clip.content}</span>
      {/if}
    </QuickLaunchRow>
  {/snippet}

  <!-- Ticket 125 companion: single-tab mobile web view in dock's bottom ~40% — content-gated, dock-only -->
  <div class="qlw__main" bind:this={qlwMainEl}>
    <div
      class="qlw__tabs-wrap"
      style:flex={companionVisible ? (1 - companionRatio) + " 1 0%" : "1 1 0%"}
    >
      <div class="qlw__tabs">
        <Tabs
          tabs={qlTabs}
          selected={tab}
          onselect={(id) => (tab = id)}
          ariaLabel="Quick Launch window sections"
        >
      {#snippet panel(id)}
        {#if id === "launch"}
          {#if loading && dockEntries.length === 0}
            <p class="qlw__sifting" aria-live="polite">Loading…</p>
          {:else if dockEntries.length === 0}
            <div class="qlw__empty">
              <span class="qlw__empty-icon" aria-hidden="true">
                <Icon name="rocket" size={22} />
              </span>
              <p class="qlw__empty-title">Nothing to launch</p>
              <p class="qlw__empty-body">
                Add entries in the main window's Quick Launch page — the
                tray's left-click opens this window, where Start all lives.
              </p>
            </div>
          {:else}
            <!-- Ticket 93: Start all stays pinned on top; the entry list
                 scrolls beneath it. -->
            <div class="qlw__launch">
              <p class="qlw__count">
                {dockEntries.length} {dockEntries.length === 1 ? "entry" : "entries"}
                in the Quick Launch list.
              </p>
              <Button onclick={start} disabled={startInFlight}>
                <Icon name="play" size={15} />
                {launching ? "Starting…" : "Start all"}
              </Button>
              <div class="qlw__list">
                {#if !launchGrouped}
                  <ul class="qlw__entries">
                    {#each dockEntries as entry (entry.id)}
                      {@render launchRow(entry)}
                    {/each}
                  </ul>
                {:else}
                  {#if launchUngrouped.length > 0}
                    <ul class="qlw__entries">
                      {#each launchUngrouped as entry (entry.id)}
                        {@render launchRow(entry)}
                      {/each}
                    </ul>
                  {/if}
                  {#each launchSections as section (section.group.id)}
                    <!-- The shared GroupAccordion in its flush strip variant:
                         sections exist only while they have members —
                         nothing here can fill an empty one (research 0004
                         rule 2). -->
                    <GroupAccordion
                      flush
                      open={launchCollapse.isOpen(section.group.id)}
                      controls={`qlw-group-${section.group.id}`}
                      name={section.group.name}
                      count={section.rows.length}
                      onToggle={() => launchCollapse.toggle(section.group.id)}
                    >
                      <ul class="qlw__entries">
                        {#each section.rows as entry (entry.id)}
                          {@render launchRow(entry)}
                        {/each}
                      </ul>
                    </GroupAccordion>
                  {/each}
                {/if}
              </div>
            </div>
          {/if}
        {:else if id === "actions"}
          {#if loading && dockActions.length === 0}
            <p class="qlw__sifting" aria-live="polite">Loading…</p>
          {:else if dockActions.length === 0}
            <div class="qlw__empty">
              <span class="qlw__empty-icon" aria-hidden="true">
                <Icon name="terminal" size={22} />
              </span>
              <p class="qlw__empty-title">No quick actions</p>
              <p class="qlw__empty-body">
                Compose PowerShell commands in the main window's Quick Actions
                page — they run here, hidden, as the current user.
              </p>
            </div>
          {:else}
            <!-- Ticket 97: the tab mirrors the collection's Groups toggle,
                 exactly like the Launch list. -->
            <div class="qlw__list qlw__list--padded">
              {#if !actionsGrouped}
                <ul class="qlw__actions">
                  {#each dockActions as action (action.id)}
                    {@render actionRow(action)}
                  {/each}
                </ul>
              {:else}
                {#if actionsUngrouped.length > 0}
                  <ul class="qlw__actions">
                    {#each actionsUngrouped as action (action.id)}
                      {@render actionRow(action)}
                    {/each}
                  </ul>
                {/if}
                {#each actionSections as section (section.group.id)}
                  <GroupAccordion
                    flush
                    open={actionCollapse.isOpen(section.group.id)}
                    controls={`qlw-actions-group-${section.group.id}`}
                    name={section.group.name}
                    count={section.rows.length}
                    onToggle={() => actionCollapse.toggle(section.group.id)}
                  >
                    <ul class="qlw__actions">
                      {#each section.rows as action (action.id)}
                        {@render actionRow(action)}
                      {/each}
                    </ul>
                  </GroupAccordion>
                {/each}
              {/if}
            </div>
          {/if}
        {:else}
          {#if loading && dockClips.length === 0}
            <p class="qlw__sifting" aria-live="polite">Loading…</p>
          {:else}
            <!-- Ticket 97: same Groups mirror as the other two tabs. -->
            <div class="qlw__list qlw__list--padded">
              {#if !clipsGrouped}
                <ul class="qlw__clips">
                  {#each dockClips as clip (clip.id)}
                    {@render clipRow(clip)}
                  {/each}
                </ul>
              {:else}
                {#if clipsUngrouped.length > 0}
                  <ul class="qlw__clips">
                    {#each clipsUngrouped as clip (clip.id)}
                      {@render clipRow(clip)}
                    {/each}
                  </ul>
                {/if}
                {#each clipSections as section (section.group.id)}
                  <GroupAccordion
                    flush
                    open={clipCollapse.isOpen(section.group.id)}
                    controls={`qlw-clips-group-${section.group.id}`}
                    name={section.group.name}
                    count={section.rows.length}
                    onToggle={() => clipCollapse.toggle(section.group.id)}
                  >
                    <ul class="qlw__clips">
                      {#each section.rows as clip (clip.id)}
                        {@render clipRow(clip)}
                      {/each}
                    </ul>
                  </GroupAccordion>
                {/each}
              {/if}
            </div>
          {/if}
        {/if}
      {/snippet}
        </Tabs>
      </div>
    </div>
    {#if companionVisible}
      <!-- Splitter: horizontal draggable divider 0006:7 Disclosure-like but horizontal, clamped 25–60% -->
      <!-- svelte-ignore a11y_no_noninteractive_tabindex (a focusable ARIA separator is a range widget when it exposes aria-valuenow) -->
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions (the separator supports both pointer drag and keyboard arrows) -->
      <div
        class="qlw__splitter"
        role="separator"
        aria-orientation="horizontal"
        aria-label="Resize companion pane"
        aria-valuemin="25"
        aria-valuemax="60"
        aria-valuenow={Math.round(companionRatio * 100)}
        tabindex="0"
        title="Drag or use arrow keys to resize companion pane (25%–60%)"
        onpointerdown={onCompanionSplitterPointerDown}
        onkeydown={onCompanionSplitterKeyDown}
      ></div>
      <div class="qlw__companion" style:flex={companionRatio + " 1 0%"}>
        <div class="qlw__companion-bar">
          {#if companionCanGoBack}
            <IconButton icon="chevron-left" label="Back" quiet onclick={companionGoBack} />
          {/if}
          {#if companionCanGoForward}
            <IconButton icon="chevron-right" label="Forward" quiet onclick={companionGoForward} />
          {/if}
          <!-- Ticket 161: Reload recreates the child WebView (bad login /
               dead-end recovery) — left cluster, before the URL; mute/mixer/
               external order untouched. Icon "refresh" is the verified Icon.svelte name. -->
          <IconButton
            icon="refresh"
            label={companionReloading ? "Reloading companion" : "Reload companion"}
            quiet
            disabled={companionReloading}
            onclick={() => void companionReload()}
          />
          <span class="qlw__companion-url" title={companionUrl ?? ""}>{companionUrl}</span>
          {#if companionPlaying}
            <!-- The playing indicator: status only, never a control — the
                 mute toggle beside it owns the action. Tooltip-grade per the
                 row-glyph grammar; static, so reduced motion stays still. -->
            <span
              class="qlw__companion-playing"
              role="img"
              aria-label={companionMuted ? "Playing audio (muted)" : "Playing audio"}
              title={companionMuted ? "Playing audio (muted)" : "Playing audio"}
            >
              <Icon name={companionMuted ? "volume-muted" : "volume"} size={13} />
            </span>
          {/if}
          <span class="qlw__companion-spacer" aria-hidden="true"></span>
          <IconButton
            icon={companionMuted ? "volume-muted" : "volume"}
            label={companionMuted ? "Unmute companion audio" : "Mute companion audio"}
            quiet
            disabled={companionMuteBusy}
            aria-pressed={companionMuted}
            onclick={toggleCompanionMute}
          />
          <IconButton
            icon="sliders"
            label={companionMixerOpening ? "Opening volume mixer" : "Open volume mixer"}
            quiet
            disabled={companionMixerOpening}
            onclick={openMixer}
          />
          <IconButton
            icon="external"
            label={companionOpeningExternal ? "Opening externally" : "Open externally"}
            quiet
            disabled={companionOpeningExternal}
            onclick={companionOpenExternal}
          />
        </div>
        <div class="qlw__companion-frame-wrap" bind:this={companionFrameWrapEl}>
          {#if companionWebviewFailed}
            <div class="qlw__companion-failure" role="status">
              <Icon name="monitor" size={20} />
              <p>This site couldn’t load in Companion.</p>
              <span>
                {import.meta.env.DEV && companionFailureDetail
                  ? companionFailureDetail
                  : "It may block embedded browsers."}
              </span>
              <div class="qlw__companion-failure-actions">
                <Button variant="secondary" onclick={() => void companionRetry()}>Try again</Button>
                <Button onclick={() => void companionOpenExternal()}>Open externally</Button>
              </div>
            </div>
          {:else if useWebview}
            <!-- The placeholder reserves the content area while the native WebView2 is created. -->
            <div class="qlw__companion-placeholder" aria-hidden="true">
              {#if detailsAction === null}
                <p class="qlw__companion-placeholder-text">Loading {companionUrl}…</p>
              {/if}
            </div>
          {:else}
            <!-- Fallback iframe for browser preview / when Tauri not available.
                 Note: many sites (YouTube, Spotify) send X-Frame-Options: SAMEORIGIN and will show
                 "refused to connect" here — the native WebView (dock on Windows) does direct
                 navigation and is not blocked. Use Open externally for those sites in preview. -->
            <iframe
              bind:this={companionFrameEl}
              class="qlw__companion-frame"
              src={companionUrl}
              title="Companion"
              allow="clipboard-read; clipboard-write; autoplay; encrypted-media; fullscreen"
              sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-popups-to-escape-sandbox"
              onload={handleCompanionLoad}
            ></iframe>
          {/if}
        </div>
      </div>
    {/if}
  </div>

  {#if runNotice}
    <!-- Ticket 93: the finished launch run's summary (Start all and
         single-entry starts alike) — visible feedback, not just the system
         notification (research 0004 rule 5). -->
    <p class="qlw__status" role="status">{runNotice}</p>
  {/if}

  {#if error}
    <div class="qlw__error-row">
      <p class="qlw__error" role="alert">{error}</p>
      <Button variant="ghost" onclick={() => { load(); refreshDock(); }}>
        Try again
      </Button>
    </div>
  {/if}

  <div class="sr-only" role="status" aria-live="polite">
    {copiedAnnouncement}
  </div>

  <!-- Ticket 130: the row's text side lands here — read-only details (no
       Edit; full configuration lives in the main app per research 0004:3),
       with the same Run/Stop control the row carries. -->
  <QuickActionDetailsDialog
    open={detailsAction !== null}
    action={detailsAction}
    onclose={() => (detailsAction = null)}
    onrun={(a) => run(a)}
    onstop={(a) => stop(a)}
    running={detailsAction ? quickActionRuns.running.has(detailsAction.id) : false}
    stopping={detailsAction ? quickActionRuns.stopping.has(detailsAction.id) : false}
  />
</div>

<style>
  .qlw {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--bg-page);
    border: 1px solid var(--border);
    /* List density: one type-token step down/up from today's sizing —
       existing `--text-*` tokens only, never ad-hoc sizes. The base is
       Default (no modifier); Compact and Large only re-point these three
       aliases, so every row below rescales together. Row geometry
       (truncation, `min-w-0`, badges, controls) is untouched — larger text
       truncates earlier, it never clips. */
    --qlw-name: var(--text-sm);
    --qlw-meta: var(--text-xs);
    --qlw-micro: var(--text-2xs);
  }

  .qlw--density-compact {
    --qlw-name: var(--text-xs);
    --qlw-meta: var(--text-2xs);
    --qlw-micro: var(--text-2xs);
  }

  .qlw--density-large {
    --qlw-name: var(--text-base);
    --qlw-meta: var(--text-sm);
    --qlw-micro: var(--text-xs);
  }

  .qlw__bar {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-2) var(--space-2) var(--space-4);
    flex-shrink: 0;
    user-select: none;
  }

  .qlw__mark {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    width: 30px;
    height: 30px;
    padding: 0;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    background: transparent;
    color: inherit;
    cursor: pointer;
    transition:
      background var(--dur-fast) var(--ease-out),
      border-color var(--dur-fast) var(--ease-out);
  }

  .qlw__mark:hover {
    background: var(--bg-hover);
    border-color: var(--border);
  }

  .qlw__mark:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }

  .qlw__mark:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .qlw__spacer {
    flex: 1;
    min-width: 0;
  }

  /* The docked strip (ticket 53) gets a distinct edge: a slightly deeper
     page background so the pinned bar reads as one surface against the
     desktop. Ticket 59 mirrored the padding (wider inset on the screen-edge
     side) for environment-symmetric gaps; ticket 123 follow-up makes the
     docked header symmetric (8px both sides) so the mark + controls are
     centered within the 340px strip — left auto-hide was 4px off-center with
     the mirrored 16/8, reported as "not centered" — and drops the decorative
     `dock-left`/`dock-right` hint (redundant with window position + disabled
     chevron, wasted 21px at 340). */
  .qlw--docked {
    background: var(--bg-card);
  }

  .qlw--docked-left .qlw__bar,
  .qlw--docked-right .qlw__bar {
    padding-left: var(--space-2);
    padding-right: var(--space-2);
  }

  .qlw__title {
    flex: 1;
    min-width: 0;
    font-family: var(--font-display);
    font-size: var(--text-base);
    font-weight: 600;
    letter-spacing: var(--tracking-display);
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .qlw__sifting {
    margin: 0;
    padding: var(--space-5) var(--space-4);
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }

  /* Ticket 93: Start all and the count stay pinned; the entry list scrolls
     beneath them inside the tab panel. */
  .qlw__launch {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    flex: 1;
    min-height: 0;
    padding: var(--space-4);
  }

  /* Ticket 93/97: the scroll container every tab's list lives in. Launch
     nests it inside the pinned Start-all head; Actions/Clips use it directly
     with their own padding (`--padded`). The bottom runway is where
     below-anchored tooltips land at full scroll. */
  .qlw__list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: var(--space-3);
  }

  .qlw__list--padded {
    padding: var(--space-3);
  }

  .qlw__list > :last-child {
    padding-bottom: calc(var(--space-7) + var(--space-6));
  }

  .qlw__count {
    margin: 0;
    font-size: var(--qlw-name);
    color: var(--text-muted);
  }

  .qlw__entries {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .qlw__entry-badge {
    display: inline-flex;
    flex-shrink: 0;
    color: var(--text-muted);
  }

  /* Ticket 97: the entry's real app icon, where one resolves. */
  .qlw__entry-icon {
    width: 16px;
    height: 16px;
    flex-shrink: 0;
  }

  .qlw__entry-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-display);
    font-size: var(--qlw-name);
    font-weight: 600;
    color: var(--text);
  }

  /* Single-flight (ticket 93): every start affordance waits together — the
     name mutes with the disabled control, exactly as before. */
  .qlw__entry-name--muted {
    color: var(--text-muted);
  }

  .qlw__entry-starting {
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: var(--qlw-meta);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }

  .qlw__actions {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .qlw__action-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-display);
    font-size: var(--qlw-name);
    font-weight: 600;
    color: var(--text);
  }

  /* Content-gated note glyph — token color only (research 0006 pattern 14) */
  .qlw__note {
    display: inline-flex;
    flex-shrink: 0;
    color: var(--text-muted);
  }

  /* Ticket 79: the read-only Quick Clips rows — whole-row click-to-copy,
   * same visual language as the actions list. No editing affordances here:
   * all CRUD stays on the main app's /clips page (research 0004 rule 3). */
  .qlw__clips {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .qlw__clip-badge {
    display: inline-flex;
    flex-shrink: 0;
    color: var(--accent);
  }

  .qlw__clip-name {
    flex-shrink: 0;
    max-width: 45%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-display);
    font-size: var(--qlw-name);
    font-weight: 600;
    color: var(--text);
  }

  .qlw__clip-excerpt {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: var(--qlw-meta);
    color: var(--text-muted);
  }

  .qlw__clip-copied {
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: var(--qlw-meta);
    letter-spacing: var(--tracking-mono);
    color: var(--accent);
  }

  /* The live region is the shared `.sr-only` utility (tokens.css). */

  .qlw__empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-7) var(--space-5);
    text-align: center;
  }

  .qlw__empty-icon {
    display: inline-flex;
    color: var(--accent);
    margin-bottom: var(--space-2);
  }

  .qlw__empty-title {
    margin: 0;
    font-family: var(--font-display);
    font-size: var(--text-lg);
    font-weight: 600;
    color: var(--text);
  }

  .qlw__empty-body {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  /* Ticket 93: the finished run's summary line — quiet visible feedback
     beside (not replacing) the system notification. */
  .qlw__status {
    margin: 0;
    padding: 0 var(--space-4) var(--space-2);
    font-size: var(--text-sm);
    color: var(--text-muted);
    overflow-wrap: anywhere;
  }

  .qlw__error-row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-4) var(--space-4);
  }

  .qlw__error {
    flex: 1;
    min-width: 0;
    margin: 0;
    font-size: var(--text-sm);
    color: var(--danger-text);
    overflow-wrap: anywhere;
  }

  /* Ticket 63: the blocked-auto-hide banner — the shell refused the edge
     registration, so this says what was refused. Shared warn tokens; `status`
     (not `alert`) because nothing is broken — the driver still slides the
     strip on its own while the edge stays busy (research 0004: fit must hold
     at real device DPI). Stacked — same copy, same order, shared
     Button untouched and full-width via stretch. */
  .qlw__blocked {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: var(--space-2);
    margin: 0 var(--space-4) var(--space-2);
    padding: var(--space-2) var(--space-3);
    background: var(--warn-tint);
    border: 1px solid var(--warn-tint-border);
    border-radius: var(--radius);
  }

  .qlw__blocked-top {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .qlw__blocked-icon {
    display: inline-flex;
    flex-shrink: 0;
    color: var(--warn-text);
  }

  .qlw__blocked-text {
    flex: 1;
    min-width: 0;
    margin: 0;
    font-size: var(--text-sm);
    color: var(--warn-text);
    overflow-wrap: anywhere;
  }

  /* The tab strip fills the window below the header; the active panel
     stretches and lets its list scroll internally. The panel itself is a
     flex column: it is a plain block in Tabs.svelte, and a block panel
     clipped its direct-child lists (Actions/Clips grew past it with no
     scrollbar — ticket 102's root cause); flexing it lets every tab's
     `flex: 1; min-height: 0` scroll container actually resolve. */
  .qlw__tabs {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .qlw__tabs :global(.tabs) {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .qlw__tabs :global(.tabs__panel) {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  /* This page's flexed panel ties with Tabs' own `.tabs__panel[hidden]`
     once both carry their scope hashes, so re-state hiding here explicitly —
     otherwise stylesheet order decides and every tab renders at once. */
  .qlw__tabs :global(.tabs__panel[hidden]) {
    display: none;
  }

  /* Ticket 125 companion: horizontal splitter + web view pane (0006:7 Disclosure-like but horizontal).
     Content-gated — no URL means no pane, no splitter, no chrome; floating never shows it (constants/window.rs 460).
     Ratio clamped 25–60% live while dragging, persists per monitor, survives toggleQuickLaunchDock + restart. */
  .qlw__main {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .qlw__tabs-wrap {
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .qlw__splitter {
    display: block;
    width: 100%;
    flex-shrink: 0;
    height: 6px;
    margin: 0;
    padding: 0;
    border: 0;
    background: var(--border);
    cursor: row-resize;
    touch-action: none;
    transition: background var(--dur-fast) var(--ease-out);
  }

  .qlw__splitter:hover,
  .qlw__splitter:active {
    background: var(--accent-tint-border);
  }

  .qlw__splitter:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: -2px;
  }

  .qlw__companion {
    min-height: 0;
    display: flex;
    flex-direction: column;
    background: var(--bg-surface);
    border-top: 1px solid var(--border);
    overflow: hidden;
  }

  .qlw__companion-bar {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    padding: var(--space-1) var(--space-2);
    flex-shrink: 0;
    border-bottom: 1px solid var(--border);
    background: var(--bg-card);
  }

  .qlw__companion-url {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  /* The playing indicator: token color only, no motion — it must stay still
     under reduced motion, so status reads without animation. */
  .qlw__companion-playing {
    display: inline-flex;
    flex-shrink: 0;
    color: var(--text-muted);
  }

  .qlw__companion-spacer {
    flex: 1;
    min-width: 0;
  }

  .qlw__companion-frame-wrap {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    background: var(--bg-page);
  }

  .qlw__companion-placeholder,
  .qlw__companion-failure {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
    padding: var(--space-5);
    color: var(--text-muted);
    text-align: center;
  }

  .qlw__companion-placeholder-text,
  .qlw__companion-failure p,
  .qlw__companion-failure span {
    margin: 0;
    font-size: var(--text-xs);
  }

  .qlw__companion-failure p {
    color: var(--text);
    font-weight: 600;
  }

  .qlw__companion-failure-actions {
    display: flex;
    gap: var(--space-2);
    margin-top: var(--space-2);
  }

  .qlw__companion-frame {
    width: 100%;
    height: 100%;
    border: 0;
    display: block;
    background: var(--bg-page);
  }
</style>
