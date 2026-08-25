<script lang="ts">
  import { onMount } from "svelte";
  import type { Group, QuickAction } from "$lib/types";
  import {
    deleteQuickAction,
    getSettings,
    listQuickActions,
    moveQuickAction,
    runQuickAction,
  } from "$lib/api";
  import {
    countMembers,
    createCollectionGroups,
    groupView,
  } from "$lib/collectionGroups.svelte";
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
  import ContextMenu, {
    type ContextMenuItem,
    type ContextMenuState,
  } from "$lib/components/ContextMenu.svelte";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import Notice from "$lib/components/Notice.svelte";
  import PageFeaturesButton from "$lib/components/PageFeaturesButton.svelte";
  import PageHeader from "$lib/components/PageHeader.svelte";
  import SearchInput from "$lib/components/SearchInput.svelte";

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

  // The compose dialog (ticket 51): `formAction` null = adding a new action,
  // set = editing that action.
  let formOpen = $state(false);
  let formAction: QuickAction | null = $state(null);
  let deleting: QuickAction | null = $state(null);

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
    formAction = action;
    formOpen = true;
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
    // While searching, every section opens so no match hides behind a
    // chevron.
    return filter.trim() !== "" || groups.collapse.isOpen(groupId);
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
   *  flyout while Groups is on, Move up / Move down over the visible slice,
   *  Remove danger-last behind a separator. */
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
    items.push(
      {
        label: "Move up",
        icon: "chevron-up",
        disabled: index <= 0,
        onselect: () => move(action.id, quickActions.indexOf(slice[index - 1])),
      },
      {
        label: "Move down",
        icon: "chevron-down",
        disabled: index >= slice.length - 1,
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
    menu = groups.groupMenu(group, anchor, viaKeyboard);
  }

  function matchesAction(a: QuickAction): boolean {
    const q = filter.trim().toLowerCase();
    return (
      a.name.toLowerCase().includes(q) || a.command.toLowerCase().includes(q)
    );
  }

  const matchedCount = $derived(quickActions.filter(matchesAction).length);
  const listView = $derived(
    groupView(groups.groups, quickActions, matchesAction, filter.trim() !== "")
  );
</script>

<svelte:head>
  <title>Quick Actions — Sprout</title>
</svelte:head>

{#snippet actionRow(action: QuickAction)}
  <li class="rack__row">
    <span class="rack__badge" aria-hidden="true">
      <Icon name="terminal" size={14} />
    </span>
    <span class="rack__name">{action.name}</span>
    <span class="rack__command" title={action.command}>
      {action.command}
    </span>
    {#if action.cwd}
      <span class="rack__cwd" title={action.cwd}>{action.cwd}</span>
    {/if}
    <!-- Ticket 98: the three-state control is shared with the Quick Launch
         window's Actions tab — one markup, one spinner, one vocabulary. -->
    <QuickActionRunControl
      name={action.name}
      stoppable={action.stoppable}
      running={quickActionRuns.running.has(action.id)}
      stopping={quickActionRuns.stopping.has(action.id)}
      onrun={() => run(action)}
      onstop={() => stop(action)}
    />
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
      <SearchInput
        value={filter}
        placeholder="Search name or command…"
        ariaLabel="Search quick actions"
        onchange={(v) => (filter = v)}
      />
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

  {#if loading && quickActions.length === 0}
    <p class="sifting" aria-live="polite">Loading…</p>
  {:else if loadFailed}
    <Notice tone="error">Could not load the Quick Actions list.</Notice>
  {:else if quickActions.length === 0}
    <EmptyState icon="terminal" title="No quick actions yet">
      <p>
        Press <strong>Add</strong> to write a named PowerShell command with an
        optional working directory. Run each action right here or from the
        Quick Launch window — hidden, as the current user, with no status UI.
      </p>
    </EmptyState>
  {:else if matchedCount === 0}
    <EmptyState icon="search" title={`Nothing matches “${filter.trim()}”`}>
      <p>Search looks at action names and their commands.</p>
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
      {#each quickActions.filter(matchesAction) as action (action.id)}
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
  onsave={async (message) => {
    formOpen = false;
    flash(message);
    await load();
  }}
  oncancel={() => (formOpen = false)}
/>

<ContextMenu ctx={menu} onclose={() => (menu = null)} />

<style>
  .qa {
    max-width: 1080px;
    margin: 0 auto;
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
