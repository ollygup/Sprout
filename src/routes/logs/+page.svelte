<script lang="ts">
  import { onMount } from "svelte";
  import type { LogEntry, LogLocations } from "$lib/types";
  import { formatBytes, formatTimestamp } from "$lib/format";
  import { listLogs, openFolder } from "$lib/api";
  import Button from "$lib/components/Button.svelte";
  import Disclosure from "$lib/components/Disclosure.svelte";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import Notice from "$lib/components/Notice.svelte";
  import PageHeader from "$lib/components/PageHeader.svelte";

  /** Rows visible before "Show all" takes over (ticket 83): every family's
   *  preview stays within a fraction of a viewport, so no header buries the
   *  next one however many folders pile up. */
  const PREVIEW_ROWS = 3;

  type FamilyKey = "runs" | "actions" | "launch";

  let locations: LogLocations | null = $state(null);
  let loading = $state(true);
  let loadFailed = $state(false);
  let error = $state("");
  let opening = $state<string | null>(null);

  // Session-only disclosure state (ticket 83): every family starts expanded
  // at its preview cap; nothing persists across visits.
  let expanded = $state<Record<FamilyKey, boolean>>({
    runs: true,
    actions: true,
    launch: true,
  });
  let showAll = $state<Record<FamilyKey, boolean>>({
    runs: false,
    actions: false,
    launch: false,
  });

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

  function totalBytes(entries: LogEntry[]) {
    return entries.reduce((sum, entry) => sum + entry.size_bytes, 0);
  }
</script>

{#snippet logSection(
  key: FamilyKey,
  label: string,
  entries: LogEntry[],
  emptyCopy: string,
)}
  {@const visible = showAll[key] ? entries : entries.slice(0, PREVIEW_ROWS)}
  <section class="family" aria-labelledby={`family-${key}-title`}>
    <header class="family__head">
      <Disclosure
        open={expanded[key]}
        controls={`family-${key}`}
        ariaLabel={`Toggle the ${label} list`}
        onclick={() => (expanded[key] = !expanded[key])}
      />
      <h2 id={`family-${key}-title`} class="family__name">{label}</h2>
      <span class="family__count">
        {entries.length} folder{entries.length === 1 ? "" : "s"} · {formatBytes(totalBytes(entries))}
      </span>
    </header>
    {#if expanded[key]}
      <div id={`family-${key}`} class="family__body">
        {#if entries.length === 0}
          <p class="family__empty">{emptyCopy}</p>
        {:else}
          <ul class="family__list">
            {#each visible as entry (entry.name)}
              <li class="family__row">
                <span class="family__folder">{entry.name}</span>
                <span class="family__stats">
                  {formatBytes(entry.size_bytes)}
                  {#if entry.modified_at}
                    <span class="family__date">{formatTimestamp(entry.modified_at)}</span>
                  {/if}
                </span>
                <Button variant="secondary" onclick={() => open(entry.path)} disabled={opening !== null}>
                  <Icon name="folder" size={13} /> Open
                </Button>
              </li>
            {/each}
          </ul>
          {#if entries.length > PREVIEW_ROWS}
            <button
              type="button"
              class="expander"
              onclick={() => (showAll[key] = !showAll[key])}
            >
              {showAll[key] ? "Show fewer" : `Show all ${entries.length}`}
            </button>
          {/if}
        {/if}
      </div>
    {/if}
  </section>
{/snippet}

<section class="logs" aria-labelledby="logs-title">
  <PageHeader titleId="logs-title" title="Logs">
    {#snippet actions()}
      <Button variant="secondary" onclick={load} disabled={loading}>
        <Icon name="refresh" size={13} /> Refresh
      </Button>
    {/snippet}
    {#snippet subtitle()}
      Log content is not rendered in the app; these are raw files for you and support staff.
      Every run keeps its own folder; expired folders are pruned per the retention setting.
    {/snippet}
  </PageHeader>

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
            {loc.quick_launch_runs.length} Quick Launch run{loc.quick_launch_runs.length === 1 ? "" : "s"} ·{" "}
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

    {@render logSection(
      "runs",
      "Run folders",
      loc.runs,
      "No run folders yet. Each run's raw output lands in its own folder.",
    )}
    {@render logSection(
      "actions",
      "Quick Action runs",
      loc.quick_action_runs,
      "No Quick Action runs yet. Each run's live output and its stop/exit lines land in its own folder.",
    )}
    {@render logSection(
      "launch",
      "Quick Launch runs",
      loc.quick_launch_runs,
      "No Quick Launch runs yet. Each run's started, skipped, and failed entries land in its own folder.",
    )}
  {/if}
</section>

<style>
  .logs {
    max-width: 920px;
    margin: 0 auto;
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

  /* One rhythm per family (ticket 83): hairline + generous space above each
     section, so a capped preview never bleeds into the next header. */
  .family {
    margin-top: var(--space-6);
    padding-top: var(--space-5);
    border-top: 1px solid var(--border);
  }

  .family__head {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-bottom: var(--space-3);
  }

  .family__name {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--accent);
  }

  .family__count {
    margin-left: auto;
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
    white-space: nowrap;
  }

  .family__empty {
    margin: 0;
    font-size: var(--text-xs);
    font-style: italic;
    color: var(--text-muted);
  }

  .family__list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .family__row {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-surface);
    padding: var(--space-2) var(--space-3);
  }

  .family__folder {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text);
    overflow-wrap: anywhere;
    flex: 1;
    min-width: 0;
  }

  .family__stats {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .family__date {
    color: var(--warm-text);
  }

  .expander {
    display: inline-block;
    margin-top: var(--space-2);
    padding: var(--space-1) var(--space-2);
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--text-muted);
    cursor: pointer;
  }

  .expander:hover {
    color: var(--accent);
    background: var(--bg-hover);
  }
</style>
