<script lang="ts">
  import { onMount } from "svelte";
  import type { Snippet } from "svelte";
  import { page } from "$app/state";
  import "../lib/styles/tokens.css";
  import { goto } from "$app/navigation";
  import { listen } from "@tauri-apps/api/event";
  import NavRail from "$lib/components/NavRail.svelte";
  import RunBanner from "$lib/components/RunBanner.svelte";
  import UnifiedHeader from "$lib/components/UnifiedHeader.svelte";
  import {
    closeMainWindow,
    mainWindowIsMaximized,
    mainWindowReady,
    minimizeMainWindow,
    setPresenceStatus,
    takePendingImport,
    takePendingRoute,
    toggleMainWindowMaximize,
  } from "$lib/api";
  import { nativeFrame, startNativeFrame } from "$lib/nativeFrame.svelte";
  import {
    SECTION_LABEL_BY_ROUTE,
    sectionLabelForRoute,
  } from "$lib/windowChrome";
  import { englishCopy, t } from "$lib/copy";
  import { startLocale } from "$lib/copy/locale.svelte";
  import { launchImport } from "$lib/launchImport.svelte";
  import { startRunAwareness } from "$lib/runAwareness.svelte";
  import { startTheme } from "$lib/theme.svelte";
  import { startAnimation } from "$lib/animation.svelte";
  import { watchUpdates } from "$lib/updateState.svelte";

  let { children }: { children: Snippet } = $props();

  // The Quick Launch window (ticket 52) and the clip viewer (ticket 226) are
  // separate webviews pointed at their own routes: they get the shared tokens,
  // fonts, and theme — but none of the main window's chrome (NavRail, run
  // banner) or import/run-awareness plumbing, which are meaningless in a
  // miniature surface.
  const isMiniWindow = $derived(
    page.route.id === "/quick-launch-window" || page.route.id === "/clip-viewer"
  );

  // WebView2 can paint its native window before this shell exists. Rust keeps
  // only the main window transparent until this mounted acknowledgement.
  onMount(() => {
    if (isMiniWindow) return;
    mainWindowReady().catch((error) => console.error(error));
  });

  // The theme store (ticket 31) applies its cached mode at import, before the
  // first paint; this wires the OS listener and backend reconciliation. The
  // animation store applies its cached switch the same way. The locale store
  // applies its cached language and re-reads the backend, in every window.
  $effect(() => {
    startTheme();
    startAnimation();
    startNativeFrame();
    startLocale();
  });

  // The run-awareness poller (ticket 18) lives at the layout level, so the
  // banner survives navigation and completion is announced exactly once —
  // page-local polling never had a vote. The Quick Launch window has no
  // banner; its Start button listens for `launch-run-done` itself.
  $effect(() => {
    if (isMiniWindow) return;
    startRunAwareness();
  });

  // The self-update watcher (ticket 74) feeds the rail pill and the Settings
  // row from one store; the Quick Launch window renders neither.
  $effect(() => {
    if (isMiniWindow) return;
    watchUpdates();
  });

  $effect(() => {
    if (isMiniWindow) return;
    takePendingImport()
      .then((path) => {
        if (path) {
          launchImport.path = path;
          goto("/presets");
        }
      })
      .catch(() => {});
    // The dock Companion picker's management row asks for the full manager:
    // a recreated window consumes the recorded route here, an already-open
    // one navigates through the event below.
    takePendingRoute()
      .then((route) => {
        if (route) {
          goto(route);
        }
      })
      .catch(() => {});
  });

  // The Discord section report (ADR-0033) reads the shared route map — the
  // same keys the unified header breadcrumbs, so presence and chrome can
  // never disagree. The pipe vocabulary stays English while the header
  // translates: wording-locked fixed text only, session clock untouched.
  let lastPresenceState = "";
  $effect(() => {
    if (isMiniWindow) return;
    const state =
      SECTION_LABEL_BY_ROUTE[page.route.id ?? "/"] ??
      englishCopy["presence.state.presets"];
    if (state === lastPresenceState) return;
    lastPresenceState = state;
    setPresenceStatus(englishCopy["presence.details"], state).catch(() => {});
  });

  // The unified header reflects the live maximized state instead of tracking
  // it — refreshed on mount, after every toggle, and whenever the window
  // regains focus (an OS shortcut may have changed it behind our back).
  let maximized = $state(false);
  async function refreshMaximized() {
    try {
      maximized = await mainWindowIsMaximized();
    } catch (error) {
      console.error(error);
    }
  }
  async function minimizeMain() {
    try {
      await minimizeMainWindow();
    } catch (error) {
      console.error(error);
    }
  }
  async function toggleMaximize() {
    try {
      await toggleMainWindowMaximize();
      await refreshMaximized();
    } catch (error) {
      console.error(error);
    }
  }
  // Close reuses the existing destroy path — the tray stays resident, so
  // this is hide-to-tray in effect, never quit (ADR-0010/0013 unchanged).
  async function closeMain() {
    try {
      await closeMainWindow();
    } catch (error) {
      console.error(error);
    }
  }
  $effect(() => {
    if (isMiniWindow) return;
    void refreshMaximized();
    const onFocus = () => {
      void refreshMaximized();
    };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  });

  // A .sprout.json double-clicked while Sprout is already running arrives as
  // an event from the single-instance hook (ticket 10) — route it to the same
  // import flow as a first-launch argument.
  $effect(() => {
    if (isMiniWindow) return;
    let unlistenImport: (() => void) | undefined;
    let unlistenRoute: (() => void) | undefined;
    listen<string>("pending-import", (event) => {
      launchImport.path = event.payload;
      goto("/presets");
    }).then((fn) => (unlistenImport = fn));
    // The dock picker's management row while this window is already open —
    // the recreated-window case is covered by the pending route above.
    listen("open-companion-manager", () => {
      goto("/companion");
    }).then((fn) => (unlistenRoute = fn));
    return () => {
      unlistenImport?.();
      unlistenRoute?.();
    };
  });
</script>

{#if isMiniWindow}
  <div class="mini">
    {@render children()}
  </div>
{:else}
  <div
    class="app"
    class:app--framed={nativeFrame.mode === "off"}
    class:app--maximized={maximized}
  >
    {#if nativeFrame.mode === "off"}
      <UnifiedHeader
        section={sectionLabelForRoute(page.route.id)}
        {maximized}
        onMinimize={minimizeMain}
        onToggleMaximize={toggleMaximize}
        onClose={closeMain}
      />
    {/if}
    <div class="shell">
      <a class="skip-link" href="#main">{t("chrome.skipLink")}</a>
      <NavRail />
      <div class="stage">
        <RunBanner />
        <main id="main" class="main" tabindex="-1">
          {@render children()}
        </main>
      </div>
    </div>
  </div>
{/if}

<style>
  /* The whole app scrolls inside exactly one container — `.main` in the
     shell, the tab lists in the mini window. The document itself must never
     grow a scrollbar: a single stray pixel at document level puts a second
     thumb at the window edge and scrolling it drags the rail and chrome
     along (ticket 108). */
  :global(html),
  :global(body) {
    height: 100%;
    overflow: hidden;
    overscroll-behavior: none;
  }

  .mini {
    height: 100vh;
    overflow: hidden;
  }

  .shell {
    display: flex;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
    /* The chrome surface behind everything (research 0025): rail + header
       paint it, the content panel sits on it. */
    background: var(--bg-surface);
  }

  .stage {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    /* The content always paints the page surface — including under the
       native frame, where no header covers the chrome behind it. */
    background: var(--bg-page);
  }

  /* The content panel (research 0025): rounded top corners over the chrome
     surface in the frameless mode — Discord's panel grammar in Ledger
     tokens, no ad-hoc radius. Maximized windows meet the screen edge
     square, like every native window. */
  .app--framed .stage {
    border-top-left-radius: var(--radius-lg);
    border-top-right-radius: var(--radius-lg);
    overflow: hidden;
  }

  .app--maximized .stage {
    border-radius: 0;
  }

  .main {
    flex: 1;
    overflow-y: auto;
    padding: var(--space-6) var(--space-7) var(--space-7);
  }

  .main:focus {
    outline: none;
  }

  .skip-link {
    position: fixed;
    top: var(--space-2);
    left: var(--space-2);
    z-index: 100;
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius);
    background: var(--accent);
    color: var(--on-accent);
    font-family: var(--font-body);
    font-size: var(--text-sm);
    font-weight: 600;
    text-decoration: none;
    transform: translateY(-200%);
    transition: transform var(--dur) var(--ease-out);
  }

  .skip-link:focus-visible {
    transform: translateY(0);
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }
</style>
