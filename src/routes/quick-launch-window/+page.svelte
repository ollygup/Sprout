<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import type { LaunchEntry, LaunchReport, QuickAction } from "$lib/types";
  import {
    getQuickLaunchDockState,
    getSettings,
    listLaunchEntries,
    listQuickActions,
    runQuickAction,
    startQuickLaunch,
    switchQuickLaunchDockEdge,
    toggleQuickLaunchDock,
  } from "$lib/api";
  import type { QuickLaunchDockState } from "$lib/types";
  import { restoreTheme, type ThemeMode } from "$lib/theme.svelte";
  import Button from "$lib/components/Button.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import IconButton from "$lib/components/IconButton.svelte";
  import SproutMark from "$lib/components/SproutMark.svelte";
  import Tabs from "$lib/components/Tabs.svelte";

  // The Quick Launch window (ticket 52): the tray's left-click target — a
  // miniature, frameless, read-only window with two tabs. The backend owns
  // its life cycle (ticket 56: blur does nothing; the × button / Alt+F4
  // destroy it and the tray reopens it at a fixed centered size — no
  // geometry is remembered); this page only renders and fires the existing
  // runners.
  // Docking (ticket 53) is controlled from this header: the toggle pins the
  // window to the current monitor's remembered (or Settings-default) edge as
  // a Win32 AppBar, and the arrows move it left↔right while docked.

  let entries = $state<LaunchEntry[]>([]);
  let actions = $state<QuickAction[]>([]);
  let loading = $state(true);
  let launching = $state(false);
  let error = $state("");
  let tab = $state("launch");
  // Ticket 59: the dock state is never null — while the window floats it
  // carries the target edge/mode the toggle would dock to (`docked: false`),
  // so the toggle's icon tells the truth before the first dock.
  let dock = $state<QuickLaunchDockState>({
    edge: "left",
    mode: "auto-hide",
    docked: false,
  });

  onMount(() => {
    load();
    refreshDock();
    // Ticket 42: the run finishes on the backend's background thread — the
    // summary lands as a system notification, and this event just releases
    // the Start button.
    // Ticket 57: the backend emits `quick-launch-changed` after every command
    // that mutates what this window renders — Launch entry mutations, Quick
    // Action mutations, `update_settings`, `update_theme`. The window listens
    // once and re-runs its loads plus its dock-state refresh and theme
    // re-apply, so entries/actions/settings added in the main app appear
    // without reopening it.
    const unlisteners: (() => void)[] = [];
    listen<LaunchReport>("launch-run-done", () => {
      launching = false;
    }).then((fn) => unlisteners.push(fn));
    listen("quick-launch-changed", () => {
      load();
      refreshDock();
      applyPersistedTheme();
    }).then((fn) => unlisteners.push(fn));
    // Ticket 61: a background dock failure — a shell-initiated re-assert
    // (ABN_POSCHANGED) or the drift watchdog — surfaces in the window's error
    // banner instead of leaving a half-docked bar.
    listen<string>("quick-launch-dock-error", (e) => {
      error = e.payload;
    }).then((fn) => unlisteners.push(fn));
    return () => unlisteners.forEach((fn) => fn());
  });

  async function applyPersistedTheme() {
    try {
      const s = await getSettings();
      const mode = s.theme as ThemeMode;
      if (mode === "system" || mode === "light" || mode === "dark") {
        restoreTheme(mode);
      }
    } catch {
      // The cached theme still applies; nothing to reconcile.
    }
  }

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
      dock = { ...dock, edge };
    } catch (e) {
      console.error(e);
      error = String(e);
    }
  }

  async function load() {
    loading = true;
    try {
      const [entriesResult, actionsResult] = await Promise.all([
        listLaunchEntries(),
        listQuickActions(),
      ]);
      entries = entriesResult;
      actions = actionsResult;
      error = "";
    } catch (e) {
      console.error(e);
      error = String(e);
    } finally {
      loading = false;
    }
  }

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

  async function run(action: QuickAction) {
    error = "";
    try {
      await runQuickAction(action.id);
    } catch (e) {
      console.error(e);
      error = String(e);
    }
  }

  function close() {
    invoke("close_quick_launch_window").catch((e) => console.error(e));
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
>
  <header class="qlw__bar" data-tauri-drag-region="deep">
    <span class="qlw__mark" aria-hidden="true"><SproutMark size={16} /></span>
    <h1 class="qlw__title">Quick Launch</h1>
    {#if dock.docked}
      <span class="qlw__dock-hint" aria-hidden="true">
        <Icon
          name={dock.edge === "left" ? "dock-left" : "dock-right"}
          size={13}
        />
      </span>
      <IconButton
        icon="chevron-left"
        label="Dock to the left edge"
        quiet
        disabled={dock.edge === "left"}
        onclick={() => switchEdge("left")}
      />
      <IconButton
        icon="chevron-right"
        label="Dock to the right edge"
        quiet
        disabled={dock.edge === "right"}
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

  <div class="qlw__tabs">
    <Tabs
      tabs={[
        { id: "launch", label: "Quick Launch" },
        { id: "actions", label: "Quick Actions" },
      ]}
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
            <div class="qlw__launch">
              <p class="qlw__count">
                {entries.length} {entries.length === 1 ? "entry" : "entries"}
                in the Quick Launch list.
              </p>
              <Button onclick={start} disabled={launching}>
                <Icon name="play" size={15} />
                {launching ? "Starting…" : "Start all"}
              </Button>
            </div>
          {/if}
        {:else}
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
            <ul class="qlw__actions">
              {#each actions as action (action.id)}
                <li class="qlw__action">
                  <span class="qlw__action-name" title={action.command}>
                    {action.name}
                  </span>
                  <Button variant="secondary" onclick={() => run(action)}>
                    <Icon name="play" size={13} />
                    Run
                  </Button>
                </li>
              {/each}
            </ul>
          {/if}
        {/if}
      {/snippet}
    </Tabs>
  </div>

  {#if error}
    <p class="qlw__error" role="alert">{error}</p>
  {/if}
</div>

<style>
  .qlw {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--bg-page);
    border: 1px solid var(--border);
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
    flex-shrink: 0;
  }

  /* The docked strip (ticket 53) gets a distinct edge: a hint in the header
     and a slightly deeper page background so the pinned bar reads as one
     surface against the desktop. Ticket 59: the header padding mirrors on
     both docked edges — the wider inset lands on the screen-edge side, so a
     left- and right-docked strip have identical side spacing (no gap
     asymmetry, no crowding against the neighboring application's space). */
  .qlw--docked {
    background: var(--bg-card);
  }

  .qlw--docked-left .qlw__bar {
    padding-left: var(--space-4);
    padding-right: var(--space-2);
  }

  .qlw--docked-right .qlw__bar {
    padding-left: var(--space-2);
    padding-right: var(--space-4);
  }

  .qlw__dock-hint {
    display: inline-flex;
    flex-shrink: 0;
    color: var(--accent);
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

  .qlw__launch {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-5) var(--space-4);
  }

  .qlw__count {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .qlw__actions {
    list-style: none;
    margin: 0;
    padding: var(--space-3) var(--space-3) var(--space-4);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    overflow-y: auto;
  }

  .qlw__action {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-2) var(--space-2) var(--space-3);
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }

  .qlw__action-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-display);
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text);
  }

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

  .qlw__error {
    margin: 0;
    padding: 0 var(--space-4) var(--space-4);
    font-size: var(--text-sm);
    color: var(--danger-text);
    overflow-wrap: anywhere;
  }

  /* The tab strip fills the window below the header; the active panel
     stretches and lets its list scroll internally. */
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
    overflow: hidden;
  }
</style>