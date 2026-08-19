<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import type { LaunchEntry, LaunchReport, QuickAction } from "$lib/types";
  import {
    listLaunchEntries,
    listQuickActions,
    runQuickAction,
    startQuickLaunch,
  } from "$lib/api";
  import Button from "$lib/components/Button.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import IconButton from "$lib/components/IconButton.svelte";
  import SproutMark from "$lib/components/SproutMark.svelte";
  import Tabs from "$lib/components/Tabs.svelte";

  // The Quick Launch window (ticket 52): the tray's left-click target — a
  // miniature, frameless, read-only window with two tabs. The backend owns
  // its life cycle (blur/close destroy it, the tray reopens it, the geometry
  // is remembered); this page only renders and fires the existing runners.

  let entries = $state<LaunchEntry[]>([]);
  let actions = $state<QuickAction[]>([]);
  let loading = $state(true);
  let launching = $state(false);
  let error = $state("");
  let tab = $state("launch");

  onMount(() => {
    load();
    // Ticket 42: the run finishes on the backend's background thread — the
    // summary lands as a system notification, and this event just releases
    // the Start button.
    let unlisten: (() => void) | undefined;
    listen<LaunchReport>("launch-run-done", () => {
      launching = false;
    }).then((fn) => (unlisten = fn));
    return () => unlisten?.();
  });

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

<div class="qlw">
  <header class="qlw__bar" data-tauri-drag-region="deep">
    <span class="qlw__mark" aria-hidden="true"><SproutMark size={16} /></span>
    <h1 class="qlw__title">Quick Launch</h1>
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