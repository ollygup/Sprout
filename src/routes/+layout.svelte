<script lang="ts">
  import type { Snippet } from "svelte";
  import { page } from "$app/state";
  import "../lib/styles/tokens.css";
  import { goto } from "$app/navigation";
  import { listen } from "@tauri-apps/api/event";
  import NavRail from "$lib/components/NavRail.svelte";
  import RunBanner from "$lib/components/RunBanner.svelte";
  import { takePendingImport } from "$lib/api";
  import { launchImport } from "$lib/launchImport.svelte";
  import { startRunAwareness } from "$lib/runAwareness.svelte";
  import { startTheme } from "$lib/theme.svelte";
  import { watchUpdates } from "$lib/updateState.svelte";

  let { children }: { children: Snippet } = $props();

  // The Quick Launch window (ticket 52) is a second webview pointed at its
  // own route: it gets the shared tokens, fonts, and theme — but none of the
  // main window's chrome (NavRail, run banner) or import/run-awareness
  // plumbing, which are meaningless in a miniature read-only surface.
  const isQuickLaunchWindow = $derived(
    page.route.id === "/quick-launch-window"
  );

  // The theme store (ticket 31) applies its cached mode at import, before the
  // first paint; this wires the OS listener and backend reconciliation.
  $effect(() => {
    startTheme();
  });

  // The run-awareness poller (ticket 18) lives at the layout level, so the
  // banner survives navigation and completion is announced exactly once —
  // page-local polling never had a vote. The Quick Launch window has no
  // banner; its Start button listens for `launch-run-done` itself.
  $effect(() => {
    if (isQuickLaunchWindow) return;
    startRunAwareness();
  });

  // The self-update watcher (ticket 74) feeds the rail pill and the Settings
  // row from one store; the Quick Launch window renders neither.
  $effect(() => {
    if (isQuickLaunchWindow) return;
    watchUpdates();
  });

  $effect(() => {
    if (isQuickLaunchWindow) return;
    takePendingImport()
      .then((path) => {
        if (path) {
          launchImport.path = path;
          goto("/presets");
        }
      })
      .catch(() => {});
  });

  // A .sprout.json double-clicked while Sprout is already running arrives as
  // an event from the single-instance hook (ticket 10) — route it to the same
  // import flow as a first-launch argument.
  $effect(() => {
    if (isQuickLaunchWindow) return;
    let unlisten: (() => void) | undefined;
    listen<string>("pending-import", (event) => {
      launchImport.path = event.payload;
      goto("/presets");
    }).then((fn) => (unlisten = fn));
    return () => unlisten?.();
  });
</script>

{#if isQuickLaunchWindow}
  <div class="mini">
    {@render children()}
  </div>
{:else}
  <div class="shell">
    <a class="skip-link" href="#main">Skip to content</a>
    <NavRail />
    <div class="stage">
      <RunBanner />
      <main id="main" class="main" tabindex="-1">
        {@render children()}
      </main>
    </div>
  </div>
{/if}

<style>
  .mini {
    height: 100vh;
    overflow: hidden;
  }

  .shell {
    display: flex;
    height: 100vh;
    overflow: hidden;
  }

  .stage {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
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
