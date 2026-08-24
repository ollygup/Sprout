<script lang="ts">
  import { onMount } from "svelte";
  import type {
    LaunchCandidate,
    LaunchEntry,
    LaunchReport,
    VirtualDesktop,
  } from "$lib/types";
  import {
    candidateIcon,
    createLaunchEntry,
    createVirtualDesktop,
    deleteLaunchEntry,
    listLaunchCandidates,
    listLaunchEntries,
    listVirtualDesktops,
    moveLaunchEntry,
    startQuickLaunch,
    updateLaunchEntry,
  } from "$lib/api";
  import { listen } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";
  import Button from "$lib/components/Button.svelte";
  import Disclosure from "$lib/components/Disclosure.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import IconButton from "$lib/components/IconButton.svelte";
  import PageHeader from "$lib/components/PageHeader.svelte";
  import SearchInput from "$lib/components/SearchInput.svelte";
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import CommandFormDialog from "$lib/components/CommandFormDialog.svelte";
  import ContextMenu, {
    type ContextMenuItem,
    type ContextMenuState,
  } from "$lib/components/ContextMenu.svelte";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import Notice from "$lib/components/Notice.svelte";

  let entries = $state<LaunchEntry[]>([]);
  let loading = $state(true);
  let loadFailed = $state(false);
  let busy = $state(false);
  let launching = $state(false);
  let error = $state("");
  let notice = $state("");
  let deleting: LaunchEntry | null = $state(null);
  let commandOpen = $state(false);

  // The added-entries filter (ticket 46): the shared search input on top
  // narrows the rack client-side; the installed-app search lives in the Add
  // panel below.
  let filter = $state("");

  // Add panel (ticket 46): one "Add" button reveals the inline panel with the
  // installed-app search plus pick-a-file and add-command. The candidate
  // snapshot is walked once per tab visit, on first open — no cache.
  let addOpen = $state(false);
  let candidatesLoaded = $state(false);
  let query = $state("");
  let candidates = $state<LaunchCandidate[]>([]);
  let candidatesLoading = $state(false);
  let candidatesFailed = $state(false);
  let searchInput = $state<HTMLInputElement>();

  // Virtual-desktop grouping (ticket 44/46): the whole surface — the row
  // menus, the group accordions, "New desktop…" — is hidden below Windows
  // 11 24H2.
  let desktops = $state<VirtualDesktop[]>([]);
  let desktopSupported = $state(false);
  let menu: (ContextMenuState & { entryId: number }) | null = $state(null);

  // Accordion collapse state (ticket 46): only user-collapsed group keys;
  // everything else — including newly appearing groups — defaults open.
  let collapsed = $state<Set<string>>(new Set());

  onMount(() => {
    load();
    loadVirtualDesktops();
    // Ticket 42: the run finishes on the backend's background thread; the
    // summary lands as a system notification, and this event clears the
    // button and mirrors the summary in-page.
    let unlisten: (() => void) | undefined;
    listen<LaunchReport>("launch-run-done", (event) => {
      launching = false;
      flash(reportSummary(event.payload));
    }).then((fn) => (unlisten = fn));
    return () => unlisten?.();
  });

  // Focus the installed-app search when the Add panel opens.
  $effect(() => {
    if (addOpen) requestAnimationFrame(() => searchInput?.focus());
  });

  async function loadVirtualDesktops() {
    try {
      const surface = await listVirtualDesktops();
      desktops = surface.desktops;
      desktopSupported = surface.supported;
    } catch (e) {
      console.error(e);
      desktops = [];
      desktopSupported = false;
    }
  }

  async function load() {
    loading = true;
    try {
      entries = await listLaunchEntries();
      loadFailed = false;
    } catch (e) {
      console.error(e);
      loadFailed = true;
    } finally {
      loading = false;
    }
  }

  async function loadCandidates() {
    candidatesLoading = true;
    try {
      candidates = await listLaunchCandidates();
      candidatesFailed = false;
    } catch (e) {
      console.error(e);
      candidatesFailed = true;
    } finally {
      candidatesLoading = false;
    }
  }

  const matches = $derived(
    query.trim()
      ? candidates.filter((c) => {
          const q = query.trim().toLowerCase();
          return (
            c.name.toLowerCase().includes(q) ||
            (c.publisher ?? "").toLowerCase().includes(q)
          );
        })
      : []
  );

  // Ticket 40: icons are fetched per visible row and held in memory only.
  let icons = $state<Record<string, string>>({});
  let iconRequests = new Set<string>();

  function lazyIcon(node: HTMLElement, target: string) {
    const io = new IntersectionObserver((entries) => {
      if (!entries.some((e) => e.isIntersecting)) return;
      io.disconnect();
      fetchIcon(target);
    });
    io.observe(node);
    return {
      update(next: string) {
        target = next;
      },
      destroy() {
        io.disconnect();
      },
    };
  }

  async function fetchIcon(target: string) {
    if (iconRequests.has(target)) return;
    iconRequests.add(target);
    try {
      const url = await candidateIcon(target);
      if (url) icons[target] = url;
    } catch (e) {
      console.error(e);
    }
  }

  function flash(message: string) {
    notice = message;
    setTimeout(() => (notice = ""), 3200);
  }

  // Ticket 42: the same wording the system notification carries. Ticket 44:
  // the desktop-assignment notes ride along after the counts. Ticket 48:
  // skipped entries list their reason, so a no-op run is never silent.
  function reportSummary(report: LaunchReport): string {
    const counts = [
      `started ${report.started.length}`,
      `skipped ${report.skipped.length}`,
      `failed ${report.failed.length}`,
    ];
    const skipped =
      report.skipped.length > 0 ? ` Skipped: ${report.skipped.join(", ")}.` : "";
    const failed =
      report.failed.length > 0 ? ` Failed: ${report.failed.join(", ")}.` : "";
    const notes = report.notes.length > 0 ? ` ${report.notes.join(". ")}.` : "";
    return `Quick Launch done — ${counts.join(", ")}.${skipped}${failed}${notes}`;
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

  function toggleAdd() {
    addOpen = !addOpen;
    if (addOpen && !candidatesLoaded) {
      candidatesLoaded = true;
      loadCandidates();
    }
  }

  async function addApp() {
    const picked = await open({
      title: "Pick an application to add",
      multiple: false,
      directory: false,
      filters: [{ name: "Applications", extensions: ["exe", "lnk"] }],
    });
    if (typeof picked !== "string") return;
    busy = true;
    error = "";
    try {
      const name = picked
        .split(/[\\/]/)
        .pop()
        ?.replace(/\.(exe|lnk)$/i, "");
      await createLaunchEntry({
        name: name ?? picked,
        kind: "app",
        target: picked,
        shell: null,
        show_window: false,
        desktop_id: null,
      });
      flash(`${name ?? picked} added to Quick Launch.`);
      await load();
    } catch (e) {
      console.error(e);
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function addCandidate(candidate: LaunchCandidate) {
    busy = true;
    error = "";
    try {
      await createLaunchEntry({
        name: candidate.name,
        kind: "app",
        target: candidate.target,
        shell: null,
        show_window: false,
        desktop_id: null,
      });
      flash(`${candidate.name} added to Quick Launch.`);
      query = "";
      searchInput?.focus();
      await load();
    } catch (e) {
      console.error(e);
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function remove() {
    if (!deleting) return;
    const entry = deleting;
    deleting = null;
    busy = true;
    error = "";
    try {
      await deleteLaunchEntry(entry.id);
      flash(`${entry.name} removed from Quick Launch.`);
      await load();
    } catch (e) {
      console.error(e);
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function move(id: number, toPosition: number) {
    busy = true;
    error = "";
    try {
      await moveLaunchEntry(id, toPosition);
      await load();
    } catch (e) {
      console.error(e);
      error = String(e);
    } finally {
      busy = false;
    }
  }

  // ------------------- added-entries filter (ticket 46) ------------------

  const filterQ = $derived(filter.trim().toLowerCase());
  const visible = $derived(
    filterQ
      ? entries.filter(
          (e) =>
            e.name.toLowerCase().includes(filterQ) ||
            e.target.toLowerCase().includes(filterQ)
        )
      : entries
  );

  // ------------------- virtual-desktop grouping (ticket 46) --------------

  /** The label shown for an assignment: the desktop's name, or "Current
   * desktop" for no assignment. An id the list no longer knows (the desktop
   * was deleted) reads as "Desktop ?" — the launch falls back with a note. */
  function desktopName(id: string | null): string {
    if (!id) return "Current desktop";
    return desktops.find((d) => d.id === id)?.name ?? "Desktop ?";
  }

  interface LaunchGroup {
    key: string;
    label: string;
    entries: LaunchEntry[];
  }

  /** The rack grouped by desktop assignment (ticket 44/46): unassigned
   * entries under "Current desktop" first, then one group per desktop in Task
   * View order, then stale assignments (desktop no longer exists) in list
   * order — each labelled "Desktop ?". Empty groups never render, and the
   * whole grouping surface is hidden below Windows 11 24H2. */
  const groups = $derived(buildGroups());

  function buildGroups(): LaunchGroup[] {
    if (!desktopSupported) return [];
    const out: LaunchGroup[] = [];
    const unassigned = visible.filter((e) => e.desktop_id === null);
    if (unassigned.length > 0) {
      out.push({ key: "current", label: "Current desktop", entries: unassigned });
    }
    for (const d of desktops) {
      const list = visible.filter((e) => e.desktop_id === d.id);
      if (list.length > 0) {
        out.push({ key: d.id, label: d.name, entries: list });
      }
    }
    for (const e of visible) {
      if (
        e.desktop_id &&
        !desktops.some((d) => d.id === e.desktop_id) &&
        !out.some((g) => g.key === e.desktop_id)
      ) {
        out.push({
          key: e.desktop_id,
          label: "Desktop ?",
          entries: visible.filter((x) => x.desktop_id === e.desktop_id),
        });
      }
    }
    return out;
  }

  function toggleGroup(key: string) {
    const next = new Set(collapsed);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    collapsed = next;
  }

  function isOpen(key: string): boolean {
    return !collapsed.has(key);
  }

  function groupOf(key: string | null): LaunchGroup | null {
    if (key === null) {
      const list = visible;
      return list.length > 0 ? { key: "", label: "", entries: list } : null;
    }
    return groups.find((g) => g.key === key) ?? null;
  }

  /** One ⋯ menu per row (ticket 46): the desktop assignment list with a check
   * on the current assignment, then Move up / Move down / Remove. Move runs
   * within the row's own group — a desktop group stays put while its order
   * changes. */
  function openRowMenu(
    entry: LaunchEntry,
    anchor: HTMLButtonElement,
    groupKey: string | null,
    viaKeyboard: boolean
  ) {
    if (menu?.entryId === entry.id) {
      menu = null;
      return;
    }
    const group = groupOf(groupKey);
    const groupEntries = group?.entries ?? visible;
    const gi = groupEntries.indexOf(entry);
    const upDisabled = gi <= 0;
    const downDisabled = gi >= groupEntries.length - 1;
    const items: ContextMenuItem[] = [];
    if (desktopSupported) {
      items.push(
        {
          label: "Current desktop",
          icon: entry.desktop_id === null ? "check" : undefined,
          onselect: () => assignDesktop(entry, null),
        },
        ...desktops.map((d) => ({
          label: d.name,
          icon: entry.desktop_id === d.id ? "check" : undefined,
          onselect: () => assignDesktop(entry, d.id),
        })),
        {
          label: "New desktop…",
          icon: "plus",
          onselect: () => newDesktop(entry),
        },
        { label: "", separator: true, onselect: () => {} }
      );
    }
    items.push(
      {
        label: "Move up",
        icon: "chevron-up",
        disabled: upDisabled,
        onselect: () => move(entry.id, entries.indexOf(groupEntries[gi - 1])),
      },
      {
        label: "Move down",
        icon: "chevron-down",
        disabled: downDisabled,
        onselect: () => move(entry.id, entries.indexOf(groupEntries[gi + 1])),
      },
      {
        label: "Remove",
        icon: "trash",
        danger: true,
        onselect: () => (deleting = entry),
      }
    );
    menu = {
      entryId: entry.id,
      open: true,
      label: `Actions for ${entry.name}`,
      anchor,
      focusFirst: viaKeyboard,
      returnTo: anchor,
      items,
    };
  }

  async function assignDesktop(entry: LaunchEntry, id: string | null) {
    busy = true;
    error = "";
    try {
      await updateLaunchEntry({ ...entry, desktop_id: id });
      flash(
        id
          ? `${entry.name} will open on ${desktopName(id)}.`
          : `${entry.name} will open on the current desktop.`
      );
      await load();
    } catch (e) {
      console.error(e);
      error = String(e);
    } finally {
      busy = false;
    }
  }

  /** "New desktop…" (ticket 44): the OS creates the desktop on the user's
   * behalf, the entry is assigned to it, and the list refreshes so the new
   * desktop's label is available immediately. */
  async function newDesktop(entry: LaunchEntry) {
    busy = true;
    error = "";
    try {
      const id = await createVirtualDesktop();
      if (!id) {
        error = "Windows did not create the desktop — try again.";
        return;
      }
      await updateLaunchEntry({ ...entry, desktop_id: id });
      await Promise.all([loadVirtualDesktops(), load()]);
      flash(`${entry.name} will open on the new desktop.`);
    } catch (e) {
      console.error(e);
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<svelte:head>
  <title>Quick Launch — Sprout</title>
</svelte:head>

<section class="launch" aria-labelledby="launch-title">
  <PageHeader titleId="launch-title" title="Quick Launch">
    {#snippet actions()}
      <Button
        onclick={start}
        disabled={busy || launching || entries.length === 0}
      >
        <Icon name="play" size={15} />
        {launching ? "Starting…" : "Start"}
      </Button>
      <Button
        variant="secondary"
        onclick={toggleAdd}
        aria-expanded={addOpen}
        disabled={busy}
      >
        <Icon name="plus" size={15} />
        Add
      </Button>
    {/snippet}
    {#snippet subtitle()}
      {visible.length} {visible.length === 1 ? "entry" : "entries"}
      {filter.trim()
        ? visible.length === 1
          ? " matches your filter."
          : " match your filter."
        : "."}
      The tray's left-click opens Quick Launch — one click starts them together.
    {/snippet}
    {#snippet toolbar()}
      <SearchInput
        value={filter}
        placeholder="Filter Quick Launch…"
        ariaLabel="Filter Quick Launch"
        onchange={(v) => (filter = v)}
      />
    {/snippet}
  </PageHeader>

  {#if error}
    <Notice tone="error">{error}</Notice>
  {/if}
  {#if notice}
    <Notice tone="ok">{notice}</Notice>
  {/if}

  {#if addOpen}
    <section class="add-panel" aria-label="Add to Quick Launch">
      <div class="add-panel__search">
        <span class="add-panel__search-icon" aria-hidden="true">
          <Icon name="search" size={14} />
        </span>
        <input
          bind:this={searchInput}
          class="add-panel__search-input"
          type="search"
          placeholder="Search installed apps…"
          aria-label="Search installed apps"
          autocomplete="off"
          bind:value={query}
          onkeydown={(e) => {
            if (e.key === "Escape" && query) {
              e.preventDefault();
              query = "";
            }
          }}
        />
        {#if query}
          <button
            type="button"
            class="add-panel__search-clear"
            aria-label="Clear search"
            title="Clear search"
            onclick={() => {
              query = "";
              searchInput?.focus();
            }}
          >
            <Icon name="x" size={12} />
          </button>
        {/if}
      </div>

      {#if query.trim()}
        <div class="hits" aria-label="Installed-app search results">
          {#if candidatesLoading}
            <p class="hits__hint">Scanning installed apps…</p>
          {:else if candidatesFailed}
            <Notice tone="error">
              Could not scan this machine's installed apps. Try again.
            </Notice>
          {:else if matches.length === 0}
            <p class="hits__hint">
              Nothing installed matches “{query.trim()}”.
            </p>
          {:else}
            <p class="hits__hint">
              {matches.length} {matches.length === 1 ? "match" : "matches"} — pick one
              to add.
            </p>
            <ul class="hits__list">
              {#each matches as candidate (candidate.target)}
                <li>
                  <button
                    type="button"
                    class="hits__row"
                    disabled={busy}
                    title={`Add ${candidate.name}`}
                    onclick={() => addCandidate(candidate)}
                    use:lazyIcon={candidate.target}
                  >
                    {#if icons[candidate.target]}
                      <img
                        class="hits__icon"
                        src={icons[candidate.target]}
                        alt=""
                        width={24}
                        height={24}
                      />
                    {/if}
                    <span class="hits__name">{candidate.name}</span>
                    {#if candidate.publisher}
                      <span class="hits__publisher">{candidate.publisher}</span>
                    {/if}
                    <span class="hits__add" aria-hidden="true">
                      <Icon name="plus" size={13} />
                    </span>
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
        </div>
      {/if}

      <div class="add-panel__alt">
        <Button variant="ghost" onclick={addApp} disabled={busy}>
          <Icon name="folder" size={14} />
          Pick a file…
        </Button>
        <Button variant="ghost" onclick={() => (commandOpen = true)} disabled={busy}>
          <Icon name="terminal" size={14} />
          Add command…
        </Button>
      </div>
    </section>
  {/if}

  {#if loading && entries.length === 0}
    <p class="sifting" aria-live="polite">Loading…</p>
  {:else if loadFailed}
    <Notice tone="error">Could not load the Quick Launch list.</Notice>
  {:else if visible.length === 0 && !filter.trim()}
    <EmptyState icon="rocket" title="Nothing to launch yet">
      <p>
        Press <strong>Add</strong> to search this machine's installed apps,
        pick a file, or write a custom command. The tray's left-click opens
        Quick Launch — one click starts them together.
      </p>
    </EmptyState>
  {:else if visible.length === 0}
    <EmptyState title={`Nothing matches “${filter.trim()}”`}>
      <p>Try a different name, or clear the filter to see every entry.</p>
      <div class="empty-cta">
        <Button variant="secondary" onclick={() => (filter = "")}>
          Clear filter
        </Button>
      </div>
    </EmptyState>
  {:else if desktopSupported}
    {#each groups as group (group.key)}
      <section class="group" aria-label={group.label}>
        <header class="group__head">
          <Disclosure
            open={isOpen(group.key)}
            controls={`group-${group.key}`}
            ariaLabel={`Toggle the ${group.label} group`}
            onclick={() => toggleGroup(group.key)}
          />
          <h2 class="group__name">{group.label}</h2>
          <span class="group__count">
            {group.entries.length} {group.entries.length === 1 ? "entry" : "entries"}
          </span>
        </header>
        {#if isOpen(group.key)}
          <ul id={`group-${group.key}`} class="rack">
            {#each group.entries as entry (entry.id)}
              <li class="rack__row">
                <span class="rack__badge" aria-hidden="true">
                  <Icon
                    name={entry.kind === "app" ? "rocket" : "terminal"}
                    size={14}
                  />
                </span>
                <span class="rack__name">{entry.name}</span>
                <span class="rack__kind">
                  {entry.kind}
                  {#if entry.kind === "command" && entry.shell}
                    · {entry.shell}
                  {/if}
                </span>
                <span class="rack__target" title={entry.target}>{entry.target}</span>
                <IconButton
                  icon="dots"
                  label={`Actions for ${entry.name}`}
                  quiet
                  data-ctx-trigger
                  onclick={(e) =>
                    openRowMenu(
                      entry,
                      e.currentTarget as HTMLButtonElement,
                      group.key,
                      e.detail === 0
                    )}
                />
              </li>
            {/each}
          </ul>
        {/if}
      </section>
    {/each}
  {:else}
    <ul class="rack">
      {#each visible as entry (entry.id)}
        <li class="rack__row">
          <span class="rack__badge" aria-hidden="true">
            <Icon
              name={entry.kind === "app" ? "rocket" : "terminal"}
              size={14}
            />
          </span>
          <span class="rack__name">{entry.name}</span>
          <span class="rack__kind">
            {entry.kind}
            {#if entry.kind === "command" && entry.shell}
              · {entry.shell}
            {/if}
          </span>
          <span class="rack__target" title={entry.target}>{entry.target}</span>
          <IconButton
            icon="dots"
            label={`Actions for ${entry.name}`}
            quiet
            data-ctx-trigger
            onclick={(e) =>
              openRowMenu(
                entry,
                e.currentTarget as HTMLButtonElement,
                null,
                e.detail === 0
              )}
          />
        </li>
      {/each}
    </ul>
  {/if}
</section>

<ConfirmDialog
  open={deleting !== null}
  title="Remove from Quick Launch?"
  confirmLabel="Remove"
  danger
  onconfirm={remove}
  oncancel={() => (deleting = null)}
>
  <p>
    <strong>{deleting?.name}</strong> will no longer be started by Quick
    Launch. The app itself is untouched.
  </p>
</ConfirmDialog>

<CommandFormDialog
  open={commandOpen}
  onsave={async (message) => {
    commandOpen = false;
    flash(message);
    await load();
  }}
  oncancel={() => (commandOpen = false)}
/>

<ContextMenu ctx={menu} onclose={() => (menu = null)} />

<style>
  .launch {
    max-width: 1080px;
    margin: 0 auto;
  }

  .sifting {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }

  .empty-cta {
    margin-top: var(--space-4);
  }

  .add-panel {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-4);
    margin-bottom: var(--space-5);
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }

  .add-panel__search {
    position: relative;
    display: flex;
    align-items: center;
  }

  .add-panel__search-icon {
    position: absolute;
    left: 10px;
    display: inline-flex;
    color: var(--text-muted);
    pointer-events: none;
  }

  .add-panel__search-input {
    width: 100%;
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    color: var(--text);
    background: var(--bg-surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    padding: 9px 32px 9px 32px;
    transition: border-color var(--dur-fast) var(--ease-out),
      box-shadow var(--dur-fast) var(--ease-out);
  }

  .add-panel__search-input::placeholder {
    color: var(--text-muted);
    opacity: 0.8;
  }

  .add-panel__search-input:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: var(--ring-glow);
  }

  .add-panel__search-input::-webkit-search-cancel-button {
    display: none;
  }

  .add-panel__search-clear {
    position: absolute;
    right: 6px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
  }

  .add-panel__search-clear:hover {
    background: var(--bg-hover);
    color: var(--text);
  }

  .add-panel__alt {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }

  .hits {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .hits__hint {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .hits__list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .hits__row {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    width: 100%;
    text-align: left;
    padding: var(--space-3) var(--space-4);
    background: var(--bg-surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    color: inherit;
    font: inherit;
    cursor: pointer;
    transition: border-color var(--dur-fast) var(--ease-out),
      background var(--dur-fast) var(--ease-out);
  }

  .hits__row:hover:not(:disabled) {
    border-color: var(--accent);
    background: var(--bg-hover);
  }

  .hits__row:focus-visible {
    outline: none;
    border-color: var(--accent);
    box-shadow: var(--ring-glow);
  }

  .hits__row:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .hits__icon {
    flex-shrink: 0;
    width: 24px;
    height: 24px;
    border-radius: var(--radius-sm);
    background: var(--bg-surface);
  }

  .hits__name {
    font-family: var(--font-display);
    font-size: var(--text-base);
    font-weight: 600;
    color: var(--text);
    white-space: nowrap;
  }

  .hits__publisher {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .hits__add {
    flex-shrink: 0;
    display: inline-flex;
    color: var(--accent);
  }

  .group {
    margin-bottom: var(--space-5);
  }

  .group__head {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-bottom: var(--space-2);
  }

  .group__name {
    margin: 0;
    font-family: var(--font-display);
    font-size: var(--text-base);
    font-weight: 600;
    color: var(--text);
  }

  .group__count {
    padding: 2px 8px;
    border-radius: 999px;
    background: var(--bg-surface);
    border: 1px solid var(--border);
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }

  .rack {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .rack__row {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }

  .rack__badge {
    display: inline-flex;
    color: var(--accent);
  }

  .rack__name {
    font-family: var(--font-display);
    font-size: var(--text-base);
    font-weight: 600;
    color: var(--text);
    white-space: nowrap;
  }

  .rack__kind {
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    padding: 2px 8px;
    border-radius: 999px;
    background: var(--accent-tint);
    color: var(--accent);
  }

  .rack__target {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-muted);
  }
</style>
