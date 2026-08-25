<script lang="ts">
  import { onMount } from "svelte";
  import type { Group, QuickAction } from "$lib/types";
  import {
    assignToGroup,
    createGroup,
    deleteGroup,
    deleteQuickAction,
    getSettings,
    listGroups,
    listQuickActions,
    moveGroup,
    moveQuickAction,
    renameGroup,
    unassignFromGroup,
    updateGroupsEnabled,
  } from "$lib/api";
  import { createGroupCollapse } from "$lib/groupCollapse.svelte";
  import Button from "$lib/components/Button.svelte";
  import Dialog from "$lib/components/Dialog.svelte";
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
  import TextInput from "$lib/components/TextInput.svelte";

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
  // never shown or touched, they simply wait.
  let groupsOn = $state(false);
  let groups = $state<Group[]>([]);
  const collapse = createGroupCollapse();

  // One menu serves action rows and group headers; which kind it belongs to
  // rides on whichever id is set.
  let menu: (ContextMenuState & { actionId?: number; groupId?: number }) | null =
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
    loadGroupsSetting();
  });

  async function load() {
    loading = true;
    try {
      const [actions, gs] = await Promise.all([
        listQuickActions(),
        listGroups("action"),
      ]);
      quickActions = actions;
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

  async function loadGroupsSetting() {
    try {
      const s = await getSettings();
      groupsOn = s.action_groups === "on";
    } catch (e) {
      console.error(e);
    }
  }

  function flash(message: string) {
    notice = message;
    setTimeout(() => (notice = ""), 3200);
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

  /** The feature switch behind the page-features menu: persisted per
   *  collection through the settings store. Optimistic — reverted when the
   *  save fails. */
  async function toggleGroups() {
    const next = !groupsOn;
    groupsOn = next;
    try {
      await updateGroupsEnabled("action", next);
      flash(
        next
          ? "Groups on — organize actions into named sections."
          : "Groups off — groups and assignments are kept but hidden."
      );
    } catch (e) {
      console.error(e);
      groupsOn = !next;
      error = "Couldn't save the Groups setting — try again.";
    }
  }

  const featureItems = $derived([
    {
      label: "Groups",
      description:
        "Bucket actions into named sections you order yourself. Groups and assignments are kept while off.",
      value: groupsOn,
      onchange: () => toggleGroups(),
    },
  ]);

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
        await createGroup("action", name);
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
    // While searching, every section opens so no match hides behind a
    // chevron.
    return filter.trim() !== "" || collapse.isOpen(groupId);
  }

  function toggleSection(groupId: number) {
    collapse.toggle(groupId);
  }

  function groupSize(groupId: number): number {
    return quickActions.filter((a) => a.group_id === groupId).length;
  }

  async function assign(action: QuickAction, groupId: number | null) {
    busy = true;
    error = "";
    try {
      if (groupId === null) {
        await unassignFromGroup("action", action.id);
        flash(`${action.name} moved to the ungrouped list.`);
      } else {
        await assignToGroup("action", action.id, groupId);
        const target = groups.find((g) => g.id === groupId);
        flash(`${action.name} moved to ${target?.name ?? "the group"}.`);
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
      flash(`Group “${group.name}” removed — its actions are back in the ungrouped list.`);
      await load();
    } catch (e) {
      console.error(e);
      error = String(e);
    } finally {
      busy = false;
    }
  }

  /** Move up/down reorders within what the user can see: the whole list when
   *  flat, otherwise the ungrouped block or the action's own group. The
   *  positions passed down stay global list order. */
  function moveSlice(action: QuickAction): QuickAction[] {
    if (!grouped) return quickActions;
    return quickActions.filter((a) => a.group_id === action.group_id);
  }

  /** One ⋯ menu per action row: group assignment first while grouping is on
   *  (prior art: the Launch page's desktop list), then Edit, Move up / Move
   *  down scoped to the visible slice, Remove. */
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
    const items: ContextMenuItem[] = [];
    if (grouped) {
      items.push(
        {
          label: "Ungrouped",
          icon: action.group_id === null ? "check" : undefined,
          onselect: () => assign(action, null),
        },
        ...groups.map((g) => ({
          label: g.name,
          icon: action.group_id === g.id ? "check" : undefined,
          onselect: () => assign(action, g.id),
        })),
        { label: "", separator: true, onselect: () => {} }
      );
    }
    items.push(
      {
        label: "Edit",
        icon: "pencil",
        onselect: () => openEdit(action),
      },
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

  function matchesAction(a: QuickAction): boolean {
    const q = filter.trim().toLowerCase();
    return (
      a.name.toLowerCase().includes(q) || a.command.toLowerCase().includes(q)
    );
  }

  const matchedCount = $derived(quickActions.filter(matchesAction).length);
  const ungroupedRows = $derived(
    quickActions.filter((a) => a.group_id === null && matchesAction(a))
  );
  const groupSections = $derived(
    groups
      .map((g) => ({
        group: g,
        rows: quickActions.filter((a) => a.group_id === g.id && matchesAction(a)),
      }))
      .filter((s) => filter.trim() === "" || s.rows.length > 0)
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
      {#if groupsOn}
        <Button variant="secondary" onclick={openCreate} disabled={busy}>
          <Icon name="plus" size={15} />
          New group
        </Button>
      {/if}
      <Button onclick={openAdd} disabled={busy}>
        <Icon name="plus" size={15} />
        Add
      </Button>
    {/snippet}
    {#snippet subtitle()}
      {quickActions.length} {quickActions.length === 1 ? "action" : "actions"}.
      The Quick Launch window's Quick Actions tab runs each one hidden, as the
      current user.
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
        optional working directory. The Quick Launch window's Quick Actions tab
        runs each action hidden, as the current user, with no status UI.
      </p>
    </EmptyState>
  {:else if matchedCount === 0}
    <EmptyState icon="search" title={`Nothing matches “${filter.trim()}”`}>
      <p>Search looks at action names and their commands.</p>
    </EmptyState>
  {:else if grouped}
    {#if ungroupedRows.length > 0}
      <ul class="rack">
        {#each ungroupedRows as action (action.id)}
          {@render actionRow(action)}
        {/each}
      </ul>
    {/if}
    {#each groupSections as section (section.group.id)}
      <GroupAccordion
        open={sectionOpen(section.group.id)}
        controls={`qa-group-${section.group.id}`}
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
    <strong>{deleting?.name}</strong> will no longer be available in the Quick
    Launch window's Quick Actions tab. The script is deleted.
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
    <strong>{deletingGroup?.name}</strong> will be deleted. Its actions will
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
      id="group-name"
      value={groupNameDraft}
      placeholder="e.g. Docker maintenance"
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
    color: var(--accent);
  }

  .rack__name {
    flex-shrink: 0;
    font-family: var(--font-display);
    font-size: var(--text-base);
    font-weight: 600;
    color: var(--text);
    white-space: nowrap;
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
