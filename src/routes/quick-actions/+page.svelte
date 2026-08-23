<script lang="ts">
  import { onMount } from "svelte";
  import type { QuickAction } from "$lib/types";
  import {
    deleteQuickAction,
    listQuickActions,
    moveQuickAction,
  } from "$lib/api";
  import Button from "$lib/components/Button.svelte";
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
  let filter = $state("");

  // The compose dialog (ticket 51): `formAction` null = adding a new action,
  // set = editing that action.
  let formOpen = $state(false);
  let formAction: QuickAction | null = $state(null);
  let deleting: QuickAction | null = $state(null);
  let menu: (ContextMenuState & { actionId: number }) | null = $state(null);

  onMount(() => {
    load();
  });

  async function load() {
    loading = true;
    try {
      quickActions = await listQuickActions();
      loadFailed = false;
    } catch (e) {
      console.error(e);
      loadFailed = true;
    } finally {
      loading = false;
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

  /** One ⋯ menu per row (ticket 51, prior art: the Launch page): Edit, Move
   *  up / Move down, Remove. */
  function openRowMenu(
    action: QuickAction,
    anchor: HTMLButtonElement,
    viaKeyboard: boolean
  ) {
    if (menu?.actionId === action.id) {
      menu = null;
      return;
    }
    const index = quickActions.indexOf(action);
    const upDisabled = index <= 0;
    const downDisabled = index >= quickActions.length - 1;
    const items: ContextMenuItem[] = [
      {
        label: "Edit",
        icon: "pencil",
        onselect: () => openEdit(action),
      },
      {
        label: "Move up",
        icon: "chevron-up",
        disabled: upDisabled,
        onselect: () => move(action.id, index - 1),
      },
      {
        label: "Move down",
        icon: "chevron-down",
        disabled: downDisabled,
        onselect: () => move(action.id, index + 1),
      },
      {
        label: "Remove",
        icon: "trash",
        danger: true,
        onselect: () => (deleting = action),
      },
    ];
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

  const filterQ = $derived(filter.trim().toLowerCase());
  const visible = $derived(
    filterQ
      ? quickActions.filter(
          (a) =>
            a.name.toLowerCase().includes(filterQ) ||
            a.command.toLowerCase().includes(filterQ)
        )
      : quickActions
  );
</script>

<svelte:head>
  <title>Quick Actions — Sprout</title>
</svelte:head>

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
  {:else if visible.length === 0}
    <EmptyState icon="search" title={`Nothing matches “${filter.trim()}”`}>
      <p>Search looks at action names and their commands.</p>
    </EmptyState>
  {:else}
    <ul class="rack">
      {#each visible as action (action.id)}
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
</style>
