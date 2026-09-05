<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import type {
    Clip,
    CompanionAudioState,
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
    startLaunchEntry,
    startQuickLaunch,
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
  let companionUrlList: string[] = $state([]);
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
  let companionSyncRunning = false;
  let companionSyncPending = false;
  let companionWebviewFailed = $state(false);
  let companionFailedUrl: string | null = $state(null);
  let companionFailureDetail = $state("");
  let companionOpeningExternal = $state(false);
  let companionMixerOpening = $state(false);
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
  // Splitter follows the 0.25–0.60 clamp (ticket 125) — single source with settings.rs
  function clampCompanionRatio(v: number): number {
    return Math.min(0.60, Math.max(0.25, v));
  }
  async function persistCompanionRatio() {
    try {
      const clamped = clampCompanionRatio(companionRatio);
      await setCompanionHeightRatio(clamped);
      try {
        const displays = await listDisplays();
        if (displays.length > 0) {
          await setCompanionHeightRatioForDisplay(displays[0].device_name, clamped);
        }
      } catch {}
    } catch (e) {
      console.error(e);
    }
  }
  async function refreshCompanion() {
    try {
      const s = await getSettings();
      companionUrl = s.companion_url;
      companionUrlList = s.companion_url_list ?? [];
      const globalRatio = s.companion_height_ratio ?? 0.40;
      // Per-monitor override: query the monitor the window sits on
      let perMonitor: number | null = null;
      try {
        const displays = await listDisplays();
        // Simplest: use first display as proxy for current monitor; the backend's
        // dock memory already keys per-monitor, and the frontend's drag persists
        // per monitor via setCompanionHeightRatioForDisplay with that display.
        if (displays.length > 0) {
          perMonitor = await getCompanionHeightRatio(displays[0].device_name);
        }
      } catch {}
      companionRatio = clampCompanionRatio(perMonitor ?? globalRatio);
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
      // Ensure native webview reflects new URL / ratio (direct navigation, not iframe, so X-Frame-Options never blocks)
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
    companionWebviewFailed = false;
    companionFailedUrl = null;
    companionFailureDetail = "";
    if (webview) {
      try { await webview.close(); } catch {}
    }
    await syncCompanionWebview();
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
      if (companionWebview) {
        try { await companionWebview.close(); } catch {}
        companionWebview = null;
      }
      companionWebviewFailed = false;
      companionFailedUrl = null;
      companionFailureDetail = "";
      return;
    }
    if (!companionFrameWrapEl) return;
    const bounds = companionWebviewBounds(companionFrameWrapEl.getBoundingClientRect());
    // Recreate if URL changed or not yet created
    const needsCreate = !companionWebview;
    // For URL changes, easiest is to recreate the webview (Tauri Webview has no navigate API)
    // We track lastUrl via a hidden prop
    const lastUrl = (companionWebview as any)?._companionUrl as string | undefined;
    const urlChanged = lastUrl !== companionUrl;
    if (needsCreate || urlChanged) {
      if (companionWebview) {
        try { await companionWebview.close(); } catch {}
        companionWebview = null;
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
          console.log("companion webview created", targetUrl);
          void wv.setZoom(companionZoomForWidth(bounds.width)).catch((e) => {
            console.error("syncCompanionWebview zoom failed", e);
          });
          companionWebviewFailed = false;
          companionFailedUrl = null;
          companionFailureDetail = "";
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
          companionFailedUrl = targetUrl;
          companionFailureDetail = String(e.payload ?? "");
          companionWebviewFailed = true;
        });
        companionWebview = wv;
        companionWebviewFailed = false;
      } catch (e) {
        console.error("syncCompanionWebview create failed", e);
        companionWebview = null;
        companionFailedUrl = targetUrl;
        companionFailureDetail = String(e);
        companionWebviewFailed = true;
      }
      return;
    }
    // Existing webview: update bounds live
    const webview = companionWebview;
    if (!webview) return;
    try {
      await webview.setPosition(new LogicalPosition(bounds.x, bounds.y));
      await webview.setSize(new LogicalSize(bounds.width, bounds.height));
      await webview.setZoom(companionZoomForWidth(bounds.width));
    } catch (e) {
      console.error("syncCompanionWebview bounds failed", e);
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
      // Cleanup companion webview on unmount
      if (companionWebview) {
        void companionWebview.close().catch(() => {});
        companionWebview = null;
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
      // it was selected, land on Launch rather than a dead selection.
      if (clips.length === 0 && tab === "clips") tab = "launch";
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
  // set in Icon.svelte (rocket / terminal / copy).
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
    if (clips.length > 0) {
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
      await startQuickLaunch();
    } catch (e) {
      console.error(e);
      error = String(e);
      launching = false;
    }
  }

  // ------------------- ticket 93/97: clickable entries + groups ----------

  /** Sections exist only once at least one group does — and in this
   *  read-only surface a group with no members renders nothing at all
   *  (research 0004 rule 2): there is no ⋯ menu here to fill it from. */
  const launchGrouped = $derived(launchGroupsOn && launchGroups.length > 0);

  const launchUngrouped = $derived(
    entries.filter((e) => e.group_id === null)
  );

  const launchSections = $derived(
    launchGroups
      .map((g) => ({
        group: g,
        rows: entries.filter((e) => e.group_id === g.id),
      }))
      .filter((s) => s.rows.length > 0)
  );

  const actionsGrouped = $derived(actionGroupsOn && actionGroups.length > 0);

  const actionsUngrouped = $derived(
    actions.filter((a) => a.group_id === null)
  );

  const actionSections = $derived(
    actionGroups
      .map((g) => ({
        group: g,
        rows: actions.filter((a) => a.group_id === g.id),
      }))
      .filter((s) => s.rows.length > 0)
  );

  const clipsGrouped = $derived(clipGroupsOn && clipGroups.length > 0);

  const clipsUngrouped = $derived(clips.filter((c) => c.group_id === null));

  const clipSections = $derived(
    clipGroups
      .map((g) => ({
        group: g,
        rows: clips.filter((c) => c.group_id === g.id),
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
          {#if loading && entries.length === 0}
            <p class="qlw__sifting" aria-live="polite">Loading…</p>
          {:else if entries.length === 0}
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
                {entries.length} {entries.length === 1 ? "entry" : "entries"}
                in the Quick Launch list.
              </p>
              <Button onclick={start} disabled={startInFlight}>
                <Icon name="play" size={15} />
                {launching ? "Starting…" : "Start all"}
              </Button>
              <div class="qlw__list">
                {#if !launchGrouped}
                  <ul class="qlw__entries">
                    {#each entries as entry (entry.id)}
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
          {#if loading && actions.length === 0}
            <p class="qlw__sifting" aria-live="polite">Loading…</p>
          {:else if actions.length === 0}
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
                  {#each actions as action (action.id)}
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
          {#if loading && clips.length === 0}
            <p class="qlw__sifting" aria-live="polite">Loading…</p>
          {:else}
            <!-- Ticket 97: same Groups mirror as the other two tabs. -->
            <div class="qlw__list qlw__list--padded">
              {#if !clipsGrouped}
                <ul class="qlw__clips">
                  {#each clips as clip (clip.id)}
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
              <p class="qlw__companion-placeholder-text">Loading {companionUrl}…</p>
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
