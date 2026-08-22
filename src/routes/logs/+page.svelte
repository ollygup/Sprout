<script lang="ts">
  import { onMount } from "svelte";
  import type { LogLocations } from "$lib/types";
  import { formatBytes, formatTimestamp } from "$lib/format";
  import { listLogs, openFolder } from "$lib/api";
  import Button from "$lib/components/Button.svelte";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import Notice from "$lib/components/Notice.svelte";

  let locations: LogLocations | null = $state(null);
  let loading = $state(true);
  let loadFailed = $state(false);
  let error = $state("");
  let opening = $state<string | null>(null);

  onMount(() => {
    load();
  });

  async function load() {
    loading = true;
    loadFailed = false;
    error = "";
    try {
      locations = await listLogs();
      loadFailed = false;
    } catch {
      loadFailed = true;
    } finally {
      loading = false;
    }
  }

  async function open(path: string) {
    opening = path;
    error = "";
    try {
      await openFolder(path);
    } catch {
      error =
        "Couldn't open that folder — it may have been moved or deleted. Try refreshing the list.";
    } finally {
      opening = null;
    }
  }
</script>

<section class="logs" aria-labelledby="logs-title">
  <header class="logs__header">
    <div class="logs__head-row">
      <h1 id="logs-title" class="logs__title">Logs</h1>
      <Button variant="secondary" onclick={load} disabled={loading}>
        <Icon name="refresh" size={13} /> Refresh
      </Button>
    </div>
    <p class="logs__sub">
      Log content is not rendered in the app; these are raw files for you and support staff.
      Every run keeps its own folder; expired folders are pruned per the retention setting.
    </p>
  </header>

  {#if error}
    <Notice tone="error">{error}</Notice>
  {/if}

  {#if loading}
    <p class="sifting" aria-live="polite">Loading…</p>
  {:else if loadFailed || !locations}
    <EmptyState icon="x" title="Couldn't read the log locations">
      <p>
        Couldn't read the log locations from
        <span class="mono">%LOCALAPPDATA%\Sprout</span> — the folder may be missing or locked by
        another process.
      </p>
      <p>Try again; if it keeps failing, close the app and relaunch.</p>
      <div class="empty-cta">
        <Button variant="secondary" onclick={load}>Try again</Button>
      </div>
    </EmptyState>
  {:else}
    {@const loc = locations}
    <div class="roots">
      <article class="root">
        <div class="root__body">
          <p class="root__name">Logs root</p>
          <p class="root__path">{loc.logs_dir}</p>
          <p class="root__meta">
            {loc.runs.length} run folder{loc.runs.length === 1 ? "" : "s"} ·{" "}
            {loc.quick_action_runs.length} Quick Action run{loc.quick_action_runs.length === 1 ? "" : "s"} ·{" "}
            {formatBytes(loc.total_logs_bytes)}
          </p>
        </div>
        <div class="root__cta">
          <Button variant="secondary" onclick={() => open(loc.logs_dir)} disabled={opening !== null}>
            <Icon name="folder" size={13} /> Open folder
          </Button>
        </div>
      </article>

      <article class="root">
        <div class="root__body">
          <p class="root__name">Library database</p>
          <p class="root__path">{loc.db_path}</p>
          <p class="root__meta">{formatBytes(loc.db_size_bytes)}; holds the runs list, presets, and products</p>
        </div>
        <div class="root__cta">
          <Button variant="secondary" onclick={() => open(loc.data_dir)} disabled={opening !== null}>
            <Icon name="folder" size={13} /> Open data folder
          </Button>
        </div>
      </article>
    </div>

    <div class="runs">
      <p class="runs__label">Run folders</p>
      {#if loc.runs.length === 0}
        <p class="runs__none">No run folders yet. Each run's raw output lands in its own folder.</p>
      {:else}
        <ul class="runs__list">
          {#each loc.runs as entry (entry.name)}
            <li class="runs__row">
              <span class="runs__name">{entry.name}</span>
              <span class="runs__meta">
                {formatBytes(entry.size_bytes)}
                {#if entry.modified_at}
                  <span class="runs__date">{formatTimestamp(entry.modified_at)}</span>
                {/if}
              </span>
              <Button variant="secondary" onclick={() => open(entry.path)} disabled={opening !== null}>
                <Icon name="folder" size={13} /> Open
              </Button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>

    <div class="runs">
      <p class="runs__label">Quick Action runs</p>
      {#if loc.quick_action_runs.length === 0}
        <p class="runs__none">
          No Quick Action runs yet. Each run's live output and its stop/exit
          lines land in its own folder.
        </p>
      {:else}
        <ul class="runs__list">
          {#each loc.quick_action_runs as entry (entry.name)}
            <li class="runs__row">
              <span class="runs__name">{entry.name}</span>
              <span class="runs__meta">
                {formatBytes(entry.size_bytes)}
                {#if entry.modified_at}
                  <span class="runs__date">{formatTimestamp(entry.modified_at)}</span>
                {/if}
              </span>
              <Button variant="secondary" onclick={() => open(entry.path)} disabled={opening !== null}>
                <Icon name="folder" size={13} /> Open
              </Button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  {/if}
</section>

<style>
  .logs {
    max-width: 920px;
    margin: 0 auto;
  }

  .logs__header {
    margin-bottom: var(--space-5);
  }

  .logs__head-row {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--space-4);
  }

  .logs__title {
    font-family: var(--font-display);
    font-size: var(--text-2xl);
    line-height: 1.15;
    color: var(--text);
    text-wrap: balance;
  }

  .logs__sub {
    margin: var(--space-2) 0 0;
    font-size: var(--text-sm);
    color: var(--text-muted);
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

  .roots {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(360px, 1fr));
    gap: var(--space-3);
    margin-bottom: var(--space-6);
  }

  .root {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-surface);
    padding: var(--space-4);
  }

  .root__body {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    min-width: 0;
  }

  .root__name {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--accent);
  }

  .root__path {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text);
    overflow-wrap: anywhere;
  }

  .root__meta {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .root__cta {
    flex-shrink: 0;
  }

  .runs__label {
    margin: 0 0 var(--space-3);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--accent);
  }

  .runs__none {
    margin: 0;
    font-size: var(--text-xs);
    font-style: italic;
    color: var(--text-muted);
  }

  .runs__list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .runs__row {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-surface);
    padding: var(--space-2) var(--space-3);
  }

  .runs__name {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text);
    overflow-wrap: anywhere;
    flex: 1;
    min-width: 0;
  }

  .runs__meta {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .runs__date {
    color: var(--warm-text);
  }
</style>