<script lang="ts">
  import { onMount } from "svelte";
  import type {
    Group,
    LaunchCandidate,
    LaunchEntry,
    LaunchReport,
    VirtualDesktop,
  } from "$lib/types";
  import {
    createLaunchEntry,
    createVirtualDesktop,
    deleteLaunchEntry,
    getSettings,
    listLaunchCandidates,
    listLaunchEntries,
    listVirtualDesktops,
    moveLaunchEntry,
    startLaunchEntry,
    startQuickLaunch,
    updateDesktopAssignments,
    updateLaunchEntry,
  } from "$lib/api";
  import {
    countMembers,
    createCollectionGroups,
    groupView,
  } from "$lib/collectionGroups.svelte";
  import { launchReportSummary } from "$lib/format";
  import { appIcons, lazyIcon } from "$lib/lazyIcon.svelte";
  import { listen } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";
  import Button from "$lib/components/Button.svelte";
  import GroupNameDialog from "$lib/components/GroupNameDialog.svelte";
  import GroupAccordion from "$lib/components/GroupAccordion.svelte";
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
  import PageFeaturesButton from "$lib/components/PageFeaturesButton.svelte";

  let entries = $state<LaunchEntry[]>([]);
  let loading = $state(true);
  let loadFailed = $state(false);
  let busy = $state(false);
  let launching = $state(false);
  // Ticket 94: the entries with a single-entry start in flight — set on
  // click, cleared only by `launch-run-done` (research 0004 rule 5: silence
  // reads as breakage). The backend's runs are single-flight, so while
  // anything is in flight every start affordance waits.
  let startingEntries = $state<Set<number>>(new Set());
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

  // Virtual-desktop assignments (tickets 44/88): an opt-in feature — the
  // page-features gear menu is its only switch (research 0008), default off
  // and fully dormant when off. The desktop list itself still loads so
  // turning the switch on works without a reload; below Windows 11 24H2
  // (`desktopSupported` false) the desktop item disappears from the menu.
  let desktops = $state<VirtualDesktop[]>([]);
  let desktopSupported = $state(false);
  let desktopGrouping = $state(false);

  // Groups (tickets 89/91): the same per-collection pattern as Quick Actions
  // (ticket 90) — its own namespace and its own gear-menu switch, fully
  // dormant while off. Desktop assignment stays badge-only beside it: it
  // never structures this list; Groups own the structure when enabled. The
  // feature's logic is owned once by the shared manager (ticket 95); this
  // page contributes only its collection key and feedback channels.
  const groups = createCollectionGroups({
    collection: "launch",
    noun: "entries",
    host: {
      begin() {
        error = "";
        busy = true;
      },
      end() {
        busy = false;
      },
      flash: (message) => flash(message),
      fail: (message) => (error = message),
      reload: () => load(),
    },
  });

  // One menu serves entry rows and group headers; which kind it belongs to
  // rides on whichever id is set.
  let menu: (ContextMenuState & { entryId?: number; groupId?: number }) | null =
    $state(null);

  onMount(() => {
    load();
    loadVirtualDesktops();
    loadFeatureSettings();
    // Ticket 42: the run finishes on the backend's background thread; the
    // summary lands as a system notification, and this event clears the
    // button and mirrors the summary in-page. Ticket 94: single-entry row
    // starts ride the same pipeline and event, so this also releases their
    // "Starting…" rows — one source of truth for every start affordance.
    let unlisten: (() => void) | undefined;
    listen<LaunchReport>("launch-run-done", (event) => {
      launching = false;
      startingEntries = new Set();
      flash(launchReportSummary(event.payload));
    }).then((fn) => (unlisten = fn));
    return () => {
      unlisten?.();
      clearTimeout(noticeTimer);
    };
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

  /** One Settings read carries both opt-in features (ticket 95 merged the
   *  two former single-key reads): the desktop switch and this page's
   *  Groups flag. */
  async function loadFeatureSettings() {
    try {
      const s = await getSettings();
      desktopGrouping = s.desktop_assignments === "on";
      groups.setEnabledFromSettings(s.launch_groups === "on");
    } catch (e) {
      console.error(e);
    }
  }

  async function load() {
    loading = true;
    try {
      const [es] = await Promise.all([
        listLaunchEntries(),
        // Ticket 95: the groups fetch lives in the shared manager; running
        // it inside this Promise.all keeps both loads parallel as before.
        groups.refresh(),
      ]);
      entries = es;
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

  // Ticket 40: icons are fetched per visible row and held in memory only —
  // the shared lazyIcon module (used here by the Add panel and the rack
  // rows, and by the Quick Launch window's entry rows).
  let noticeTimer: ReturnType<typeof setTimeout> | undefined;

  function flash(message: string) {
    notice = message;
    clearTimeout(noticeTimer);
    noticeTimer = setTimeout(() => (notice = ""), 3200);
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

  /** True while any launch run is in flight — the header Start button and
   *  ticket 94's row play affordances share one backend pipeline whose runs
   *  are single-flight, so every start affordance waits together rather
   *  than inviting a rejection. */
  const startInFlight = $derived(launching || startingEntries.size > 0);

  /** Starts just this entry through the same pipeline as Start (ticket 94).
   *  The row says "Starting…" until `launch-run-done` lands; a rejection
   *  (single-flight guard, vanished entry) releases immediately and
   *  surfaces its reason in the error line — never silent (research 0004
   *  rule 5). */
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

  function matchesEntry(e: LaunchEntry): boolean {
    const q = filter.trim().toLowerCase();
    return (
      e.name.toLowerCase().includes(q) || e.target.toLowerCase().includes(q)
    );
  }

  const matchedCount = $derived(entries.filter(matchesEntry).length);

  // ------------------- desktop assignments (tickets 44 & 88) -------------

  /** The label shown for an assignment: the desktop's name. An id the list
   *  no longer knows (the desktop was deleted) reads as "Desktop ?" — the
   *  launch falls back with a note when grouping is on. */
  function desktopName(id: string | null): string {
    if (!id) return "Current desktop";
    return desktops.find((d) => d.id === id)?.name ?? "Desktop ?";
  }

  /** The feature switch behind the page-features menu (ticket 88): persisted
   *  through the settings store so the runner and any live window obey the
   *  same value. Optimistic — reverted when the save fails. */
  async function toggleDesktopAssignments() {
    const next = !desktopGrouping;
    desktopGrouping = next;
    try {
      await updateDesktopAssignments(next);
      flash(
        next
          ? "Desktop grouping on — stored assignments apply again."
          : "Desktop grouping off — assignments are kept but ignored."
      );
    } catch (e) {
      console.error(e);
      desktopGrouping = !next;
      error = "Couldn't save the desktop grouping setting — try again.";
    }
  }

  // The gear menu carries one row per opt-in feature: desktop grouping below
  // its 24H2 gate only (an empty row would be chrome for a dead feature,
  // research 0004 rule 2), Groups always — every collection page offers it
  // (research 0008 rule 3: each row names the feature and explains both
  // states).
  const featureItems = $derived([
    ...(desktopSupported
      ? [
          {
            label: "Desktop grouping",
            description:
              "Assigned entries launch on their virtual desktop. Assignments are kept while grouping is off.",
            value: desktopGrouping,
            onchange: () => toggleDesktopAssignments(),
          },
        ]
      : []),
    {
      label: "Groups",
      description:
        "Bucket entries into named sections you order yourself. Groups and assignments are kept while off.",
      value: groups.enabled,
      onchange: () => groups.toggle(),
    },
  ]);

  /** Sections exist only once at least one group does (absent-until-content,
   *  research 0004 rule 2) — until then every affordance but the switch and
   *  the New group button stays hidden. */
  const grouped = $derived(groups.grouped);

  function sectionOpen(groupId: number): boolean {
    // While filtering, every section opens so no match hides behind a
    // chevron.
    return filter.trim() !== "" || groups.collapse.isOpen(groupId);
  }

  /** Move up/down reorders within what the user can see: the whole list when
   *  flat, otherwise the ungrouped block or the entry's own group. The
   *  positions passed down stay global list order. */
  function moveSlice(entry: LaunchEntry): LaunchEntry[] {
    if (!grouped) return entries;
    return entries.filter((e) => e.group_id === entry.group_id);
  }

  /** One ⋯ menu per row: while Groups is on, group assignment comes first
   *  (the structuring feature owns the top of the menu); the desktop
   *  assignment list follows only while that feature is on (ticket 88's
   *  dormancy); then Move up / Move down / Remove over the visible slice.
   *  Desktop assignment stays badge-only in the list itself — it never
   *  structures anything. */
  function openRowMenu(
    entry: LaunchEntry,
    anchor: HTMLButtonElement,
    viaKeyboard: boolean
  ) {
    if (menu?.entryId === entry.id) {
      menu = null;
      return;
    }
    const slice = moveSlice(entry);
    const index = slice.indexOf(entry);
    const items: ContextMenuItem[] = [];
    if (grouped) {
      items.push(
        {
          label: "Ungrouped",
          icon: entry.group_id === null ? "check" : undefined,
          onselect: () => groups.assign(entry, entry.name, null),
        },
        ...groups.groups.map((g) => ({
          label: g.name,
          icon: entry.group_id === g.id ? "check" : undefined,
          onselect: () => groups.assign(entry, entry.name, g.id),
        })),
        { label: "", separator: true, onselect: () => {} }
      );
    }
    if (desktopSupported && desktopGrouping) {
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
        disabled: index <= 0,
        onselect: () => move(entry.id, entries.indexOf(slice[index - 1])),
      },
      {
        label: "Move down",
        icon: "chevron-down",
        disabled: index >= slice.length - 1,
        onselect: () => move(entry.id, entries.indexOf(slice[index + 1])),
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

  /** One ⋯ menu per group header: Rename, order, Remove. The items live in
   *  the shared manager (ticket 95); this page owns only the toggle-off
   *  check against its own menu state. */
  function openGroupMenu(
    group: Group,
    anchor: HTMLButtonElement,
    viaKeyboard: boolean
  ) {
    if (menu?.groupId === group.id) {
      menu = null;
      return;
    }
    menu = groups.groupMenu(group, anchor, viaKeyboard);
  }

  const listView = $derived(
    groupView(groups.groups, entries, matchesEntry, filter.trim() !== "")
  );

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

{#snippet entryRow(entry: LaunchEntry)}
  <li class="rack__row" use:lazyIcon={entry.kind === "app" ? entry.target : ""}>
    <span class="rack__badge" aria-hidden="true">
      {#if entry.kind === "app" && appIcons[entry.target]}
        <!-- Ticket 97: the app's real icon, extracted lazily from its
             target; kind glyphs stay for commands and unresolvable targets. -->
        <img
          class="rack__icon"
          src={appIcons[entry.target]}
          alt=""
          width={16}
          height={16}
        />
      {:else}
        <Icon name={entry.kind === "app" ? "rocket" : "terminal"} size={14} />
      {/if}
    </span>
    <span class="rack__name">{entry.name}</span>
    <span class="rack__kind">
      {entry.kind}
      {#if entry.kind === "command" && entry.shell}
        · {entry.shell}
      {/if}
    </span>
    {#if desktopGrouping && desktopSupported && entry.desktop_id}
      <span
        class="rack__desk"
        title={`Opens on ${desktopName(entry.desktop_id)}`}
      >
        {desktopName(entry.desktop_id)}
      </span>
    {:else if desktopGrouping && desktopSupported && !entry.desktop_id}
      <span class="rack__desk rack__desk--empty" aria-hidden="true"></span>
    {/if}
    <span class="rack__target" title={entry.target}>{entry.target}</span>
    {#if startingEntries.has(entry.id)}
      <!-- Ticket 94: the run is in flight until the backend's
           `launch-run-done` lands (research 0004 rule 5 — silence reads as
           breakage). -->
      <span class="rack__starting">Starting…</span>
    {/if}
    <!-- Ticket 94: the row's own run affordance — a quiet icon control like
         the ⋯ beside it (research 0005 rule 5); the header's Start keeps the
         page's single accent-filled verb (research 0005 rule 2). -->
    <IconButton
      icon="play"
      label={`Start ${entry.name}`}
      quiet
      disabled={startInFlight}
      onclick={() => startEntry(entry)}
    />
    <IconButton
      icon="dots"
      label={`Actions for ${entry.name}`}
      quiet
      data-ctx-trigger
      onclick={(e) =>
        openRowMenu(
          entry,
          e.currentTarget as HTMLButtonElement,
          e.detail === 0
        )}
    />
  </li>
{/snippet}

<section class="launch" aria-labelledby="launch-title">
  <PageHeader titleId="launch-title" title="Quick Launch">
    {#snippet actions()}
      <Button
        onclick={start}
        disabled={busy || startInFlight || entries.length === 0}
      >
        <Icon name="play" size={15} />
        {launching ? "Starting…" : "Start"}
      </Button>
      {#if groups.enabled}
        <Button variant="secondary" onclick={() => groups.openCreate()} disabled={busy}>
          <Icon name="plus" size={15} />
          New group
        </Button>
      {/if}
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
      {matchedCount} {matchedCount === 1 ? "entry" : "entries"}
      {filter.trim()
        ? matchedCount === 1
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
    {#snippet features()}
      <PageFeaturesButton label="Quick Launch features" items={featureItems} />
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
                    {#if appIcons[candidate.target]}
                      <img
                        class="hits__icon"
                        src={appIcons[candidate.target]}
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
  {:else if entries.length === 0}
    <EmptyState icon="rocket" title="Nothing to launch yet">
      <p>
        Press <strong>Add</strong> to search this machine's installed apps,
        pick a file, or write a custom command. The tray's left-click opens
        Quick Launch — one click starts them together.
      </p>
    </EmptyState>
  {:else if matchedCount === 0}
    <EmptyState title={`Nothing matches “${filter.trim()}”`}>
      <p>Try a different name, or clear the filter to see every entry.</p>
      <div class="empty-cta">
        <Button variant="secondary" onclick={() => (filter = "")}>
          Clear filter
        </Button>
      </div>
    </EmptyState>
  {:else if grouped}
    {#if listView.ungrouped.length > 0}
      <ul class="rack">
        {#each listView.ungrouped as entry (entry.id)}
          {@render entryRow(entry)}
        {/each}
      </ul>
    {/if}
    {#each listView.sections as section (section.group.id)}
      <GroupAccordion
        open={sectionOpen(section.group.id)}
        controls={`ql-group-${section.group.id}`}
        name={section.group.name}
        count={countMembers(entries, section.group.id)}
        onToggle={() => groups.collapse.toggle(section.group.id)}
      >
        {#snippet actions()}
          <IconButton
            icon="dots"
            label={`Actions for group ${section.group.name}`}
            quiet
            data-ctx-trigger
            onclick={(e) =>
              openGroupMenu(
                section.group,
                e.currentTarget as HTMLButtonElement,
                e.detail === 0
              )}
          />
        {/snippet}
        <ul class="rack">
          {#each section.rows as entry (entry.id)}
            {@render entryRow(entry)}
          {/each}
          {#if section.rows.length === 0}
            <li class="rack__hint">
              No entries here yet — use an entry's ⋯ menu to move one in.
            </li>
          {/if}
        </ul>
      </GroupAccordion>
    {/each}
  {:else}
    <ul class="rack">
      {#each entries.filter(matchesEntry) as entry (entry.id)}
        {@render entryRow(entry)}
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

<ConfirmDialog
  open={groups.removing !== null}
  title="Remove group?"
  confirmLabel="Remove"
  danger
  onconfirm={() => groups.removeGroup()}
  oncancel={() => groups.cancelRemove()}
>
  <p>
    <strong>{groups.removing?.name}</strong> will be deleted. Its entries will
    not be — they return to the ungrouped list.
  </p>
</ConfirmDialog>

<GroupNameDialog
  naming={groups.naming}
  draft={groups.nameDraft}
  error={groups.nameError}
  saving={groups.savingName}
  inputId="launch-group-name"
  placeholder="e.g. Daily drivers"
  ondraft={(v) => (groups.nameDraft = v)}
  onsubmit={() => groups.submitName()}
  onclose={() => groups.cancelNaming()}
/>

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
    flex-shrink: 0;
    color: var(--accent);
  }

  /* Ticket 97: the entry's real app icon, where one resolves. */
  .rack__icon {
    width: 16px;
    height: 16px;
    flex-shrink: 0;
  }

  /* Long names ellipsize instead of pushing the row's other columns out of
     the card — the same treatment as every other rack (guidelines: text
     containers handle long content). */
  .rack__name {
    flex-shrink: 0;
    max-width: 40%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-display);
    font-size: var(--text-base);
    font-weight: 600;
    color: var(--text);
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

  /* The desktop-assignment badge (ticket 88): a quiet mono pill, same
   * language as the kind pill but muted — it names, it doesn't accent. The
   * empty twin reserves the column for unassigned rows while grouping is on,
   * so targets stay aligned; when grouping is off neither span renders. */
  .rack__desk {
    flex-shrink: 0;
    min-width: 88px;
    text-align: center;
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    padding: 2px 8px;
    border-radius: 999px;
    background: var(--bg-surface);
    border: 1px solid var(--border);
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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

  /* Ticket 94: the single-entry start's in-flight word — the same mono
     treatment as the Quick Launch window's entry rows. */
  .rack__starting {
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }

  /* A group with no members yet keeps its place in the user's order without
     pretending to have content. */
  .rack__hint {
    padding: var(--space-3) var(--space-4);
    border: 1px dashed var(--border-strong);
    border-radius: var(--radius);
    font-size: var(--text-xs);
    color: var(--text-muted);
  }
</style>
