<script lang="ts">
  import { onMount } from "svelte";
  import type { Group, QuickAction } from "$lib/types";
  import {
    aiManagedStatus,
    deleteQuickAction,
    exportQuickAction,
    getSettings,
    listQuickActions,
    moveQuickAction,
    runQuickAction,
    updateQuickAction,
  } from "$lib/api";
  import {
    countMembers,
    createCollectionGroups,
    groupView,
  } from "$lib/collectionGroups.svelte";
  import {
    isDockVisible,
    isFilterActive,
    isReorderBlocked,
    matchesDockVisibility,
    normalizeQuery,
    shouldShowDockFilter,
    type DockVisibility,
  } from "$lib/dockVisibility";
  import {
    quickActionRuns,
    stopActionRun,
    syncQuickActionRuns,
  } from "$lib/quickActionRuns.svelte";
  import QuickActionRunControl from "$lib/components/QuickActionRunControl.svelte";
  import Button from "$lib/components/Button.svelte";
  import GroupNameDialog from "$lib/components/GroupNameDialog.svelte";
  import GroupAccordion from "$lib/components/GroupAccordion.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import IconButton from "$lib/components/IconButton.svelte";
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import QuickActionFormDialog from "$lib/components/QuickActionFormDialog.svelte";
  import QuickActionDetailsDialog from "$lib/components/QuickActionDetailsDialog.svelte";
  import ContextMenu, {
    type ContextMenuItem,
    type ContextMenuState,
  } from "$lib/components/ContextMenu.svelte";
  import DockVisibilityFilter from "$lib/components/DockVisibilityFilter.svelte";
  import { save as saveDialog } from "@tauri-apps/plugin-dialog";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import Notice from "$lib/components/Notice.svelte";
  import PageFeaturesButton from "$lib/components/PageFeaturesButton.svelte";
  import PageHeader from "$lib/components/PageHeader.svelte";
  import SearchInput from "$lib/components/SearchInput.svelte";
  import { hasNote } from "$lib/noteFormat";

  let quickActions = $state<QuickAction[]>([]);
  let loading = $state(true);
  let loadFailed = $state(false);
  let busy = $state(false);
  let error = $state("");
  let notice = $state("");

  // The shared search input narrows the rack client-side, over name and
  // command (ticket 84) — the same filter pattern as every other list page.
  // It filters across every section, so grouping never hides a match.
  let filter = $state("");

  // The dock visibility choice (ADR-0028): page-local, default All. Plain
  // component state, so leaving the page resets it and nothing persists it
  // to Settings, backup, or browser storage. Typing, refreshes, and edits
  // leave it alone.
  let dockVisibility = $state<DockVisibility>("all");

  // The compose dialog (ticket 51): `formAction` null = adding a new action,
  // set = editing that action.
  let formOpen = $state(false);
  let formAction: QuickAction | null = $state(null);
  let deleting: QuickAction | null = $state(null);
  // Detail peek — centered dialog matching Product-details grammar (research 0006 pattern 13)
  let details: QuickAction | null = $state(null);

  // Whether the authoring dialog may offer AI drafting: true only while an
  // existing-local route is configured with a named model (ADR-0031 keeps
  // assistance off until deliberately configured). Fail-closed — loading or
  // failed settings read as not ready, so the dialog renders zero AI chrome.
  let aiReady = $state(false);

  // Groups (tickets 89/90): the page-features gear menu is the feature's
  // only switch (research 0008 — ticket 88's bare toolbar checkbox was
  // rejected there, and this note names these toggles as its next
  // application). Off is fully dormant: stored groups and memberships are
  // never shown or touched, they simply wait. The feature's logic is owned
  // once by the shared manager (ticket 95); this page contributes only its
  // collection key and feedback channels.
  const groups = createCollectionGroups({
    collection: "action",
    noun: "actions",
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

  // One menu serves action rows and group headers; which kind it belongs to
  // rides on whichever id is set.
  let menu: (ContextMenuState & { actionId?: number; groupId?: number }) | null =
    $state(null);

  onMount(() => {
    load();
    loadGroupsSetting();
    return () => clearTimeout(noticeTimer);
  });

  async function load() {
    loading = true;
    try {
      const [actions] = await Promise.all([
        listQuickActions(),
        // Ticket 98: the shared run-state store seeds itself from the
        // registry here and stays current through the backend events —
        // the same store the Quick Launch window reads.
        syncQuickActionRuns(),
        // Ticket 95: the groups fetch lives in the shared manager; running
        // it inside this Promise.all keeps both loads parallel as before.
        groups.refresh(),
      ]);
      quickActions = actions;
      loadFailed = false;
    } catch (e) {
      console.error(e);
      loadFailed = true;
    } finally {
      loading = false;
    }
  }

  async function loadGroupsSetting() {
    try {
      const s = await getSettings();
      groups.setEnabledFromSettings(s.action_groups === "on");
      aiReady = s.ai_provider === "existing-local" && s.ai_model.trim() !== "";
      if (s.ai_provider === "managed" && s.ai_model.trim() !== "") {
        const catalog = await aiManagedStatus();
        aiReady = catalog.models.some(
          (model) => model.id === s.ai_model.trim() && model.installed,
        );
      }
    } catch (e) {
      console.error(e);
    }
  }

  let noticeTimer: ReturnType<typeof setTimeout> | undefined;

  function flash(message: string) {
    notice = message;
    clearTimeout(noticeTimer);
    noticeTimer = setTimeout(() => (notice = ""), 3200);
  }

  /** Runs the action through the same tracked spawn as the Quick Launch
   *  window (ticket 94); the running state itself lives in the shared
   *  quickActionRuns store. A rejection surfaces in the error line — never
   *  silent. */
  async function run(action: QuickAction) {
    error = "";
    try {
      await runQuickAction(action.id);
    } catch (e) {
      console.error(e);
      error = String(e);
    }
  }

  /** Stop via the shared store's lifecycle (tickets 62 & 92): Stopping is
   *  set and cleared there; only a refusal surfaces here. */
  async function stop(action: QuickAction) {
    error = "";
    try {
      await stopActionRun(action.id);
    } catch (e) {
      console.error(e);
      error = String(e);
    }
  }

  function openAdd() {
    formAction = null;
    formOpen = true;
  }

  function openEdit(action: QuickAction) {
    details = null;
    formAction = action;
    formOpen = true;
  }

  function openDetails(action: QuickAction) {
    details = action;
  }

  async function remove() {
    if (!deleting) return;
    const action = deleting;
    deleting = null;
    busy = true;
    error = "";
    try {
      await deleteQuickAction(action.id);
      flash(`${action.name} removed.`);
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
      await moveQuickAction(id, toPosition);
      await load();
    } catch (e) {
      console.error(e);
      error = String(e);
    } finally {
      busy = false;
    }
  }

  /** Per-item dock visibility (research 0006 pattern 4: the control lives on
   *  its object): hidden actions stay fully listed here and runnable — only
   *  the dock filters them out. */
  async function toggleDockVisibility(action: QuickAction) {
    busy = true;
    error = "";
    try {
      const visible = !(action.show_in_dock ?? true);
      await updateQuickAction({ ...action, show_in_dock: visible });
      flash(
        visible
          ? `${action.name} will show in the dock.`
          : `${action.name} hidden from the dock — still here and runnable.`
      );
      await load();
    } catch (e) {
      console.error(e);
      error = String(e);
    } finally {
      busy = false;
    }
  }

  /** One row's export through the moment-of-use save picker (research 0007):
   *  the file is the unchanged backup envelope with a one-element array, so
   *  it restores through Settings → Backup with honest counts. The notice
   *  states the payload rule up front: the same shell, command and working
   *  directory restores as skipped under any name, never duplicated. */
  async function exportViaDialog(action: QuickAction) {
    const path = await saveDialog({
      title: `Export ${action.name} as backup`,
      defaultPath: `${action.name}.json`,
      filters: [{ name: "Sprout backup", extensions: ["json"] }],
    });
    if (!path) return;
    try {
      await exportQuickAction(path, action.id);
      flash(
        `Exported ${action.name} to ${path}. Restore it through Settings → Backup — it adds the action unless the same shell, command and working directory already exists under any name, in which case it is skipped, not duplicated.`
      );
    } catch (e) {
      console.error(e);
      error = String(e);
    }
  }

  const featureItems = $derived([
    {
      label: "Groups",
      description:
        "Bucket actions into named sections you order yourself. Groups and assignments are kept while off.",
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
    return isFiltering || groups.collapse.isOpen(groupId);
  }

  /** Move up/down reorders within what the user can see: the whole list when
   *  flat, otherwise the ungrouped block or the action's own group. The
   *  positions passed down stay global list order. */
  function moveSlice(action: QuickAction): QuickAction[] {
    if (!grouped) return quickActions;
    return quickActions.filter((a) => a.group_id === action.group_id);
  }

  /** One ⋯ menu per action row, on the round's ordering standard (ticket
   *  106): Edit first (the row's primary verb), then the Move to group
   *  flyout while Groups is on, the dock visibility toggle, single-action
   *  Export, Move up / Move down over the visible slice, Remove danger-last
   *  behind a separator. */
  function openRowMenu(
    action: QuickAction,
    anchor: HTMLButtonElement,
    viaKeyboard: boolean
  ) {
    if (menu?.actionId === action.id) {
      menu = null;
      return;
    }
    const slice = moveSlice(action);
    const index = slice.indexOf(action);
    const items: ContextMenuItem[] = [
      {
        label: "Edit",
        icon: "pencil",
        onselect: () => openEdit(action),
      },
    ];
    if (groups.enabled) {
      items.push({
        label: "Move to group",
        icon: "folder",
        children: groups.moveToGroupChildren(action, action.name),
      });
    }
    items.push({
      label: (action.show_in_dock ?? true) ? "Hide from dock" : "Show in dock",
      icon: (action.show_in_dock ?? true) ? "eye-off" : "eye",
      onselect: () => toggleDockVisibility(action),
    });
    // Single-action Export lands here, between the visibility toggle and
    // the Move verbs (pinned row order across 159/160).
    items.push({
      label: "Export",
      icon: "download",
      onselect: () => exportViaDialog(action),
    });
    items.push(
      {
        label: "Move up",
        icon: "chevron-up",
        // Filtered neighbors are not saved neighbors: refuse to reorder
        // through them rather than write a surprising order.
        disabled: index <= 0 || reorderBlocked,
        onselect: () => move(action.id, quickActions.indexOf(slice[index - 1])),
      },
      {
        label: "Move down",
        icon: "chevron-down",
        disabled: index >= slice.length - 1 || reorderBlocked,
        onselect: () => move(action.id, quickActions.indexOf(slice[index + 1])),
      },
      { label: "", separator: true, onselect: () => {} },
      {
        label: "Remove",
        icon: "trash",
        danger: true,
        onselect: () => (deleting = action),
      },
    );
    menu = {
      actionId: action.id,
      open: true,
      label: `Actions for ${action.name}`,
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
    const groupMenu = groups.groupMenu(group, anchor, viaKeyboard);
    // Group order is order too: refuse it under filters like action moves.
    menu = reorderBlocked
      ? {
          ...groupMenu,
          items: groupMenu.items.map((item) =>
            item.label === "Move up" || item.label === "Move down"
              ? { ...item, disabled: true }
              : item
          ),
        }
      : groupMenu;
  }

  function matchesText(a: QuickAction): boolean {
    const q = normalizeQuery(filter);
    if (q === "") return true;
    return (
      a.name.toLowerCase().includes(q) || a.command.toLowerCase().includes(q)
    );
  }

  // Text search AND dock visibility intersect (ADR-0028): the one matching
  // collection below drives display — no second predicate anywhere.
  function matchesBoth(a: QuickAction): boolean {
    return matchesText(a) && matchesDockVisibility(a, dockVisibility);
  }

  /** Either filter narrows the list — section opening and the reorder gate
   *  read this one flag. */
  const isFiltering = $derived(isFilterActive(filter, dockVisibility));

  const matchingActions = $derived(quickActions.filter(matchesBoth));
  const matchedCount = $derived(matchingActions.length);

  /** Content gate from the full collection: a query never hides the trigger. */
  const showDockFilter = $derived(
    shouldShowDockFilter(quickActions, dockVisibility)
  );

  const reorderBlocked = $derived(isReorderBlocked(filter, dockVisibility));

  /** Restores ordinary ordering controls after the reorder pause. */
  function clearFilters() {
    filter = "";
    dockVisibility = "all";
  }
  const listView = $derived(
    groupView(groups.groups, quickActions, matchesBoth, isFiltering)
  );
</script>

<svelte:head>
  <title>Quick Actions — Sprout</title>
</svelte:head>

{#snippet actionRow(action: QuickAction)}
  <!-- Row click opens centered details dialog — same grammar Products use (research 0006 pattern 13).
       Content-gated note glyph appears when note exists (research 0006 pattern 14);
       compact window surfaces show glyph only (research 0004 rule 3). -->
  <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <li
    class="rack__row"
    role="button"
    tabindex="0"
    aria-label={`About ${action.name}`}
    onclick={() => openDetails(action)}
    onkeydown={(e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        openDetails(action);
      }
    }}
  >
    <span class="rack__badge" aria-hidden="true">
      <Icon name="terminal" size={14} />
    </span>
    <span class="rack__name">{action.name}</span>
    {#if hasNote(action.note)}
      <span class="rack__note" aria-label="Has note" title="Has note">
        <Icon name="note" size={12} />
      </span>
    {/if}
    <span class="rack__shell" title={`Runs under ${action.shell === "cmd" ? "cmd" : "PowerShell"}`}>{action.shell === "cmd" ? "cmd" : "PowerShell"}</span>
    {#if !isDockVisible(action)}
      <!-- The dock-hidden annotation (ADR-0028): informational only — the
           action stays fully runnable here; only the dock filters it out. -->
      <span class="rack__dock" title="Hidden from dock">
        <Icon name="eye-off" size={12} />
        <span>Hidden from dock</span>
      </span>
    {/if}
    <span class="rack__command" title={action.command}>
      {action.command}
    </span>
    {#if action.cwd}
      <span class="rack__cwd" title={action.cwd}>{action.cwd}</span>
    {/if}
    <!-- Ticket 98: the three-state control is shared with the Quick Launch
         window's Actions tab — one markup, one spinner, one vocabulary. -->
    <span class="rack__controls" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="presentation">
      <QuickActionRunControl
        name={action.name}
        stoppable={action.stoppable}
        running={quickActionRuns.running.has(action.id)}
        stopping={quickActionRuns.stopping.has(action.id)}
        onrun={() => run(action)}
        onstop={() => stop(action)}
      />
    </span>
    <span class="rack__menu" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="presentation">
      <IconButton
        icon="dots"
        label={`Actions for ${action.name}`}
        quiet
        data-ctx-trigger
        onclick={(e) =>
          openRowMenu(
            action,
            e.currentTarget as HTMLButtonElement,
            e.detail === 0
          )}
      />
    </span>
  </li>
{/snippet}

<section class="qa" aria-labelledby="qa-title">
  <PageHeader titleId="qa-title" title="Quick Actions">
    {#snippet actions()}
      <Button onclick={openAdd} disabled={busy}>
        <Icon name="plus" size={15} />
        Add
      </Button>
    {/snippet}
    {#snippet subtitle()}
      {quickActions.length} {quickActions.length === 1 ? "action" : "actions"}.
      Run each one right here or from the Quick Launch window — hidden, as
      the current user.
    {/snippet}
    {#snippet toolbar()}
      <div class="toolbar">
        <SearchInput
          value={filter}
          placeholder="Search name or command…"
          ariaLabel="Search quick actions"
          onchange={(v) => (filter = v)}
        />
        {#if showDockFilter}
          <DockVisibilityFilter
            value={dockVisibility}
            onchange={(v) => (dockVisibility = v)}
          />
        {/if}
      </div>
    {/snippet}
    {#snippet features()}
      <PageFeaturesButton label="Quick Actions features" items={featureItems} />
    {/snippet}
  </PageHeader>

  {#if error}
    <Notice tone="error">{error}</Notice>
  {/if}
  {#if notice}
    <Notice tone="ok">{notice}</Notice>
  {/if}

  {#if reorderBlocked && quickActions.length > 0}
    <p class="reorder-note">
      Reordering is paused while filters are active.
      <button
        type="button"
        class="reorder-note__clear"
        onclick={clearFilters}
      >
        Clear filters
      </button>
      to reorder.
    </p>
  {/if}

  {#if loading && quickActions.length === 0}
    <p class="sifting" aria-live="polite">Loading…</p>
  {:else if loadFailed}
    <Notice tone="error">Could not load the Quick Actions list.</Notice>
  {:else if quickActions.length === 0}
    <EmptyState icon="terminal" title="No quick actions yet">
      <p>
        Press <strong>Add</strong> to write a named PowerShell or cmd command
        with an optional working directory. Run each action right here or from
        the Quick Launch window — hidden, as the current user, with no status UI.
      </p>
    </EmptyState>
  {:else if matchedCount === 0 && filter.trim() !== ""}
    <EmptyState icon="search" title={`Nothing matches “${filter.trim()}”`}>
      <p>Search looks at action names and their commands.</p>
      {#if dockVisibility !== "all"}
        <div class="empty-cta">
          <Button variant="secondary" onclick={() => (dockVisibility = "all")}>
            Show all
          </Button>
        </div>
      {/if}
    </EmptyState>
  {:else if matchedCount === 0}
    <EmptyState icon="search" title="No actions match this filter.">
      {#if dockVisibility === "hidden"}
        <p>No actions are hidden from the dock right now.</p>
      {:else}
        <p>Every action is hidden from the dock.</p>
      {/if}
      <div class="empty-cta">
        <Button variant="secondary" onclick={() => (dockVisibility = "all")}>
          Show all
        </Button>
      </div>
    </EmptyState>
  {:else if grouped}
    {#if listView.ungrouped.length > 0}
      <ul class="rack">
        {#each listView.ungrouped as action (action.id)}
          {@render actionRow(action)}
        {/each}
      </ul>
    {/if}
    {#each listView.sections as section (section.group.id)}
      <GroupAccordion
        open={sectionOpen(section.group.id)}
        controls={`qa-group-${section.group.id}`}
        name={section.group.name}
        count={countMembers(quickActions, section.group.id)}
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
          {#each section.rows as action (action.id)}
            {@render actionRow(action)}
          {/each}
          {#if section.rows.length === 0}
            <li class="rack__hint">
              No actions here yet — use an action's ⋯ menu to move one in.
            </li>
          {/if}
        </ul>
      </GroupAccordion>
    {/each}
  {:else}
    <ul class="rack">
      {#each matchingActions as action (action.id)}
        {@render actionRow(action)}
      {/each}
    </ul>
  {/if}
</section>

<ConfirmDialog
  open={deleting !== null}
  title="Remove quick action?"
  confirmLabel="Remove"
  danger
  onconfirm={remove}
  oncancel={() => (deleting = null)}
>
  <p>
    <strong>{deleting?.name}</strong> will no longer run from this page or
    the Quick Launch window. The script is deleted.
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
    <strong>{groups.removing?.name}</strong> will be deleted. Its actions will
    not be — they return to the ungrouped list.
  </p>
</ConfirmDialog>

<GroupNameDialog
  naming={groups.naming}
  draft={groups.nameDraft}
  error={groups.nameError}
  saving={groups.savingName}
  inputId="group-name"
  placeholder="e.g. Docker maintenance"
  ondraft={(v) => (groups.nameDraft = v)}
  onsubmit={() => groups.submitName()}
  onclose={() => groups.cancelNaming()}
/>

<QuickActionFormDialog
  open={formOpen}
  action={formAction}
  groups={groups.groups}
  groupsEnabled={groups.enabled}
  aiReady={aiReady}
  onsave={async (message) => {
    formOpen = false;
    flash(message);
    await load();
  }}
  oncancel={() => (formOpen = false)}
/>

<QuickActionDetailsDialog
  open={details !== null}
  action={details}
  onclose={() => (details = null)}
  onedit={(a) => openEdit(a)}
  onrun={(a) => run(a)}
  onstop={(a) => stop(a)}
  running={details ? quickActionRuns.running.has(details.id) : false}
  stopping={details ? quickActionRuns.stopping.has(details.id) : false}
/>

<ContextMenu ctx={menu} onclose={() => (menu = null)} />

<style>
  .qa {
    max-width: 1080px;
    margin: 0 auto;
  }

  /* The toolbar lane: search plus the dock filter, wrapping on narrow
     main-window widths instead of squeezing. */
  .toolbar {
    display: flex;
    flex: 1;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }

  /* Why filtering pauses reordering, with the way back inline. */
  .reorder-note {
    margin: 0 0 var(--space-4);
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .reorder-note__clear {
    padding: 0;
    border: none;
    background: transparent;
    color: var(--accent);
    font: inherit;
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
  }

  .reorder-note__clear:hover {
    color: var(--accent-hover);
  }

  .reorder-note__clear:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }

  .empty-cta {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    margin-top: var(--space-4);
  }

  .sifting {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
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
    cursor: pointer;
    transition: border-color var(--dur-fast) var(--ease-out);
  }

  .rack__row:hover {
    border-color: var(--border-strong);
  }

  .rack__row:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: -2px;
  }

  /* Content-gated note glyph (research 0006 pattern 14) — token color only. */
  .rack__note {
    display: inline-flex;
    flex-shrink: 0;
    color: var(--text-muted);
  }

  .rack__controls,
  .rack__menu {
    display: inline-flex;
    flex-shrink: 0;
  }

  .rack__badge {
    display: inline-flex;
    flex-shrink: 0;
    color: var(--accent);
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

  .rack__command {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  /* The dock-hidden annotation: a quiet muted pill in the same language as
     the shell badge — informational only, never dimmed or disabled. */
  .rack__dock {
    display: inline-flex;
    flex-shrink: 0;
    align-items: center;
    gap: 4px;
    min-width: 0;
    max-width: 220px;
    overflow: hidden;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    padding: 2px 8px;
    border-radius: 999px;
    background: var(--bg-surface);
    border: 1px solid var(--border);
    color: var(--text-muted);
  }

  .rack__shell {
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    padding: 2px 8px;
    border-radius: 999px;
    background: var(--bg-surface);
    border: 1px solid var(--border);
    color: var(--text-muted);
  }

  .rack__cwd {
    flex-shrink: 0;
    max-width: 220px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    padding: 2px 8px;
    border-radius: 999px;
    background: var(--bg-surface);
    border: 1px solid var(--border);
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
