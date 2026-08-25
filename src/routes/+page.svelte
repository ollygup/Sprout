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
    assignToGroup,
    createGroup,
    createLaunchEntry,
    createVirtualDesktop,
    deleteGroup,
    deleteLaunchEntry,
    getSettings,
    listGroups,
    listLaunchCandidates,
    listLaunchEntries,
    listVirtualDesktops,
    moveGroup,
    moveLaunchEntry,
    renameGroup,
    startQuickLaunch,
    unassignFromGroup,
    updateDesktopAssignments,
    updateGroupsEnabled,
    updateLaunchEntry,
  } from "$lib/api";
  import { launchReportSummary } from "$lib/format";
  import { appIcons, lazyIcon } from "$lib/lazyIcon.svelte";
  import { createGroupCollapse } from "$lib/groupCollapse.svelte";
  import { listen } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";
  import Button from "$lib/components/Button.svelte";
  import Dialog from "$lib/components/Dialog.svelte";
  import GroupAccordion from "$lib/components/GroupAccordion.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import IconButton from "$lib/components/IconButton.svelte";
  import PageHeader from "$lib/components/PageHeader.svelte";
  import SearchInput from "$lib/components/SearchInput.svelte";
  import TextInput from "$lib/components/TextInput.svelte";
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
  // never structures this list; Groups own the structure when enabled.
  let groupsOn = $state(false);
  let groups = $state<Group[]>([]);
  const collapse = createGroupCollapse();

  // One menu serves entry rows and group headers; which kind it belongs to
  // rides on whichever id is set.
  let menu: (ContextMenuState & { entryId?: number; groupId?: number }) | null =
    $state(null);
  let deletingGroup: Group | null = $state(null);

  // The create/rename dialog: mode "rename" carries the group being renamed.
  let nameDialog: { mode: "create" } | { mode: "rename"; group: Group } | null =
    $state(null);
  let groupNameDraft = $state("");
  let nameError = $state("");
  let savingName = $state(false);

  onMount(() => {
    load();
    loadVirtualDesktops();
    loadGroupingSetting();
    loadGroupsFeature();
    // Ticket 42: the run finishes on the backend's background thread; the
    // summary lands as a system notification, and this event clears the
    // button and mirrors the summary in-page.
    let unlisten: (() => void) | undefined;
    listen<LaunchReport>("launch-run-done", (event) => {
      launching = false;
      flash(launchReportSummary(event.payload));
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

  async function loadGroupingSetting() {
    try {
      const s = await getSettings();
      desktopGrouping = s.desktop_assignments === "on";
    } catch (e) {
      console.error(e);
    }
  }

  async function loadGroupsFeature() {
    try {
      const s = await getSettings();
      groupsOn = s.launch_groups === "on";
    } catch (e) {
      console.error(e);
    }
  }

  async function load() {
    loading = true;
    try {
      const [es, gs] = await Promise.all([
        listLaunchEntries(),
        listGroups("launch"),
      ]);
      entries = es;
      groups = gs;
      collapse.prune(gs.map((g) => g.id));
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
  function flash(message: string) {
    notice = message;
    setTimeout(() => (notice = ""), 3200);
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
      value: groupsOn,
      onchange: () => toggleGroups(),
    },
  ]);

  /** The Groups switch behind the page-features menu (research 0008):
   *  persisted for this collection through the settings store. Optimistic —
   *  reverted when the save fails. */
  async function toggleGroups() {
    const next = !groupsOn;
    groupsOn = next;
    try {
      await updateGroupsEnabled("launch", next);
      flash(
        next
          ? "Groups on — organize entries into named sections."
          : "Groups off — groups and assignments are kept but hidden."
      );
    } catch (e) {
      console.error(e);
      groupsOn = !next;
      error = "Couldn't save the Groups setting — try again.";
    }
  }

  /** Sections exist only once at least one group does (absent-until-content,
   *  research 0004 rule 2) — until then every affordance but the switch and
   *  the New group button stays hidden. */
  const grouped = $derived(groupsOn && groups.length > 0);

  function openCreate() {
    groupNameDraft = "";
    nameError = "";
    nameDialog = { mode: "create" };
  }

  function openRename(group: Group) {
    groupNameDraft = group.name;
    nameError = "";
    nameDialog = { mode: "rename", group };
  }

  async function submitName() {
    if (!nameDialog) return;
    const name = groupNameDraft.trim();
    if (!name) {
      nameError = "Group name must not be empty.";
      return;
    }
    savingName = true;
    nameError = "";
    try {
      if (nameDialog.mode === "create") {
        await createGroup("launch", name);
        flash(`Group “${name}” created.`);
      } else {
        await renameGroup(nameDialog.group.id, name);
        flash(`Group renamed to “${name}”.`);
      }
      nameDialog = null;
      await load();
    } catch (e) {
      console.error(e);
      nameError = String(e);
    } finally {
      savingName = false;
    }
  }

  function sectionOpen(groupId: number): boolean {
    // While filtering, every section opens so no match hides behind a
    // chevron.
    return filter.trim() !== "" || collapse.isOpen(groupId);
  }

  function toggleSection(groupId: number) {
    collapse.toggle(groupId);
  }

  function groupSize(groupId: number): number {
    return entries.filter((e) => e.group_id === groupId).length;
  }

  async function assignEntry(entry: LaunchEntry, groupId: number | null) {
    busy = true;
    error = "";
    try {
      if (groupId === null) {
        await unassignFromGroup("launch", entry.id);
        flash(`${entry.name} moved to the ungrouped list.`);
      } else {
        await assignToGroup("launch", entry.id, groupId);
        const target = groups.find((g) => g.id === groupId);
        flash(`${entry.name} moved to ${target?.name ?? "the group"}.`);
      }
      await load();
    } catch (e) {
      console.error(e);
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function reorderGroup(id: number, toPosition: number) {
    busy = true;
    error = "";
    try {
      await moveGroup(id, toPosition);
      await load();
    } catch (e) {
      console.error(e);
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function removeGroup() {
    if (!deletingGroup) return;
    const group = deletingGroup;
    deletingGroup = null;
    busy = true;
    error = "";
    try {
      await deleteGroup(group.id);
      flash(`Group “${group.name}” removed — its entries are back in the ungrouped list.`);
      await load();
    } catch (e) {
      console.error(e);
      error = String(e);
    } finally {
      busy = false;
    }
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
          onselect: () => assignEntry(entry, null),
        },
        ...groups.map((g) => ({
          label: g.name,
          icon: entry.group_id === g.id ? "check" : undefined,
          onselect: () => assignEntry(entry, g.id),
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

  /** One ⋯ menu per group header: Rename, order, Remove. */
  function openGroupMenu(
    group: Group,
    anchor: HTMLButtonElement,
    viaKeyboard: boolean
  ) {
    if (menu?.groupId === group.id) {
      menu = null;
      return;
    }
    const index = groups.indexOf(group);
    menu = {
      groupId: group.id,
      open: true,
      label: `Actions for group ${group.name}`,
      anchor,
      focusFirst: viaKeyboard,
      returnTo: anchor,
      items: [
        {
          label: "Rename",
          icon: "pencil",
          onselect: () => openRename(group),
        },
        {
          label: "Move up",
          icon: "chevron-up",
          disabled: index <= 0,
          onselect: () => reorderGroup(group.id, index - 1),
        },
        {
          label: "Move down",
          icon: "chevron-down",
          disabled: index >= groups.length - 1,
          onselect: () => reorderGroup(group.id, index + 1),
        },
        {
          label: "Remove",
          icon: "trash",
          danger: true,
          onselect: () => (deletingGroup = group),
        },
      ],
    };
  }

  const ungroupedRows = $derived(
    entries.filter((e) => e.group_id === null && matchesEntry(e))
  );
  const groupSections = $derived(
    groups
      .map((g) => ({
        group: g,
        rows: entries.filter((e) => e.group_id === g.id && matchesEntry(e)),
      }))
      .filter((s) => filter.trim() === "" || s.rows.length > 0)
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
        disabled={busy || launching || entries.length === 0}
      >
        <Icon name="play" size={15} />
        {launching ? "Starting…" : "Start"}
      </Button>
      {#if groupsOn}
        <Button variant="secondary" onclick={openCreate} disabled={busy}>
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
    {#if ungroupedRows.length > 0}
      <ul class="rack">
        {#each ungroupedRows as entry (entry.id)}
          {@render entryRow(entry)}
        {/each}
      </ul>
    {/if}
    {#each groupSections as section (section.group.id)}
      <GroupAccordion
        open={sectionOpen(section.group.id)}
        controls={`ql-group-${section.group.id}`}
        name={section.group.name}
        count={groupSize(section.group.id)}
        onToggle={() => toggleSection(section.group.id)}
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
  open={deletingGroup !== null}
  title="Remove group?"
  confirmLabel="Remove"
  danger
  onconfirm={removeGroup}
  oncancel={() => (deletingGroup = null)}
>
  <p>
    <strong>{deletingGroup?.name}</strong> will be deleted. Its entries will
    not be — they return to the ungrouped list.
  </p>
</ConfirmDialog>

<Dialog
  open={nameDialog !== null}
  title={nameDialog?.mode === "rename" ? "Rename group" : "New group"}
  onclose={() => (nameDialog = null)}
  width={380}
>
  <form
    class="name-form"
    onsubmit={(e) => {
      e.preventDefault();
      submitName();
    }}
  >
    <TextInput
      label="Name"
      id="launch-group-name"
      value={groupNameDraft}
      placeholder="e.g. Daily drivers"
      required
      onchange={(v) => (groupNameDraft = v)}
    />
    {#if nameError}
      <Notice tone="error">{nameError}</Notice>
    {/if}
    <div class="name-form__buttons">
      <Button variant="secondary" onclick={() => (nameDialog = null)}>
        Cancel
      </Button>
      <Button kind="submit" disabled={savingName}>
        {nameDialog?.mode === "rename" ? "Rename" : "Create"}
      </Button>
    </div>
  </form>
</Dialog>

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
    color: var(--accent);
  }

  /* Ticket 97: the entry's real app icon, where one resolves. */
  .rack__icon {
    width: 16px;
    height: 16px;
    flex-shrink: 0;
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

  /* A group with no members yet keeps its place in the user's order without
     pretending to have content. */
  .rack__hint {
    padding: var(--space-3) var(--space-4);
    border: 1px dashed var(--border-strong);
    border-radius: var(--radius);
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .name-form {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .name-form__buttons {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
  }
</style>
