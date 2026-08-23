<script lang="ts">
  import { onMount } from "svelte";
  import type { Clip } from "$lib/types";
  import {
    copyClip,
    deleteClip,
    listClips,
    moveClip,
  } from "$lib/api";
  import { clipTitle } from "$lib/format";
  import Button from "$lib/components/Button.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import IconButton from "$lib/components/IconButton.svelte";
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import ClipFormDialog from "$lib/components/ClipFormDialog.svelte";
  import ContextMenu, {
    type ContextMenuItem,
    type ContextMenuState,
  } from "$lib/components/ContextMenu.svelte";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import Notice from "$lib/components/Notice.svelte";
  import PageHeader from "$lib/components/PageHeader.svelte";
  import SearchInput from "$lib/components/SearchInput.svelte";

  let clips = $state<Clip[]>([]);
  let loading = $state(true);
  let loadFailed = $state(false);
  let busy = $state(false);
  let error = $state("");
  let notice = $state("");

  // The compose dialog: `formClip` null = adding a new clip, set = editing.
  let formOpen = $state(false);
  let formClip: Clip | null = $state(null);
  let deleting: Clip | null = $state(null);
  let menu: (ContextMenuState & { clipId: number }) | null = $state(null);

  // The shared search input narrows the list client-side, over name and
  // content (ticket 78).
  let filter = $state("");

  // One-click re-copy feedback: the id whose row flashes "Copied", plus the
  // polite live region both this page and the window tab (ticket 79) rely
  // on — silence is a bug (research 0004 rule 5).
  let copiedId = $state<number | null>(null);
  let copiedAnnouncement = $state("");
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;

  onMount(() => {
    load();
    return () => clearTimeout(copiedTimer);
  });

  async function load() {
    loading = true;
    try {
      clips = await listClips();
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
    formClip = null;
    formOpen = true;
  }

  function openEdit(clip: Clip) {
    formClip = clip;
    formOpen = true;
  }

  async function copy(clip: Clip) {
    try {
      await copyClip(clip.id);
      // The write landed — now the flash may honestly say Copied.
      copiedId = clip.id;
      copiedAnnouncement = `${clipTitle(clip.name, clip.content)} copied.`;
      clearTimeout(copiedTimer);
      copiedTimer = setTimeout(() => (copiedId = null), 1200);
    } catch (e) {
      console.error(e);
      error = String(e);
    }
  }

  async function remove() {
    if (!deleting) return;
    const clip = deleting;
    deleting = null;
    busy = true;
    error = "";
    try {
      await deleteClip(clip.id);
      flash("Clip deleted.");
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
      await moveClip(id, toPosition);
      await load();
    } catch (e) {
      console.error(e);
      error = String(e);
    } finally {
      busy = false;
    }
  }

  /** One ⋯ menu per row, identical in feel to Quick Actions (ticket 78):
   *  Edit, Move up / Move down, Remove. */
  function openRowMenu(
    clip: Clip,
    anchor: HTMLButtonElement,
    viaKeyboard: boolean
  ) {
    if (menu?.clipId === clip.id) {
      menu = null;
      return;
    }
    const index = clips.indexOf(clip);
    const upDisabled = index <= 0;
    const downDisabled = index >= clips.length - 1;
    const items: ContextMenuItem[] = [
      {
        label: "Edit",
        icon: "pencil",
        onselect: () => openEdit(clip),
      },
      {
        label: "Move up",
        icon: "chevron-up",
        disabled: upDisabled,
        onselect: () => move(clip.id, index - 1),
      },
      {
        label: "Move down",
        icon: "chevron-down",
        disabled: downDisabled,
        onselect: () => move(clip.id, index + 1),
      },
      {
        label: "Remove",
        icon: "trash",
        danger: true,
        onselect: () => (deleting = clip),
      },
    ];
    menu = {
      clipId: clip.id,
      open: true,
      label: `Actions for ${clipTitle(clip.name, clip.content)}`,
      anchor,
      focusFirst: viaKeyboard,
      returnTo: anchor,
      items,
    };
  }

  const filterQ = $derived(filter.trim().toLowerCase());
  const visible = $derived(
    filterQ
      ? clips.filter(
          (c) =>
            c.name.toLowerCase().includes(filterQ) ||
            c.content.toLowerCase().includes(filterQ)
        )
      : clips
  );
</script>

<svelte:head>
  <title>Quick Clips — Sprout</title>
</svelte:head>

<section class="clips" aria-labelledby="clips-title">
  <PageHeader titleId="clips-title" title="Quick Clips">
    {#snippet actions()}
      <Button onclick={openAdd} disabled={busy}>
        <Icon name="plus" size={15} />
        Add
      </Button>
    {/snippet}
    {#snippet subtitle()}
      {clips.length} {clips.length === 1 ? "clip" : "clips"}.
      Click a clip to put its text back on your clipboard. The Quick Launch
      window's Quick Clips tab copies them too, once any exist.
    {/snippet}
    {#snippet toolbar()}
      <SearchInput
        value={filter}
        placeholder="Search name or text…"
        ariaLabel="Search clips"
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

  {#if loading && clips.length === 0}
    <p class="sifting" aria-live="polite">Loading…</p>
  {:else if loadFailed}
    <Notice tone="error">Could not load the clip list.</Notice>
  {:else if clips.length === 0}
    <EmptyState icon="copy" title="No clips yet">
      <p>
        Press <strong>Add</strong> and paste the text you re-type most — support
        replies, commands, addresses. Clicking a clip puts its text back on
        your clipboard. Once one clip exists, a Quick Clips tab appears in the
        Quick Launch window for two-click copying from the tray.
      </p>
    </EmptyState>
  {:else if visible.length === 0}
    <EmptyState icon="search" title={`Nothing matches “${filter.trim()}”`}>
      <p>Search looks at clip names and their text.</p>
    </EmptyState>
  {:else}
    <ul class="rack">
      {#each visible as clip (clip.id)}
        {@const title = clipTitle(clip.name, clip.content)}
        <li class="rack__row">
          <button
            type="button"
            class="rack__main"
            aria-label={`Copy ${title} to the clipboard`}
            onclick={() => copy(clip)}
          >
            <span class="rack__badge" aria-hidden="true">
              <Icon name={copiedId === clip.id ? "check" : "copy"} size={14} />
            </span>
            <span class="rack__name">{title}</span>
            {#if copiedId === clip.id}
              <span class="rack__copied">Copied</span>
            {:else}
              <span class="rack__content">{clip.content}</span>
            {/if}
          </button>
          <IconButton
            icon="dots"
            label={`Actions for ${title}`}
            quiet
            data-ctx-trigger
            onclick={(e) =>
              openRowMenu(
                clip,
                e.currentTarget as HTMLButtonElement,
                e.detail === 0
              )}
          />
        </li>
      {/each}
    </ul>
  {/if}
</section>

<div class="sr-only" role="status" aria-live="polite">
  {copiedAnnouncement}
</div>

<ConfirmDialog
  open={deleting !== null}
  title="Delete clip?"
  confirmLabel="Delete"
  danger
  onconfirm={remove}
  oncancel={() => (deleting = null)}
>
  <p>
    <strong>{deleting ? clipTitle(deleting.name, deleting.content) : ""}</strong>
    will be removed from this page and from the Quick Launch window's Quick
    Clips tab. The text is deleted.
  </p>
</ConfirmDialog>

<ClipFormDialog
  open={formOpen}
  clip={formClip}
  onsave={async (message) => {
    formOpen = false;
    flash(message);
    await load();
  }}
  oncancel={() => (formOpen = false)}
/>

<ContextMenu ctx={menu} onclose={() => (menu = null)} />

<style>
  .clips {
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
    gap: var(--space-2);
    padding: calc(var(--space-3) - 2px) var(--space-3)
      calc(var(--space-3) - 2px) var(--space-4);
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }

  .rack__main {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    flex: 1;
    min-width: 0;
    padding: 0;
    border: none;
    background: transparent;
    text-align: left;
    cursor: pointer;
    color: inherit;
    font: inherit;
  }

  .rack__main:focus-visible {
    outline-offset: -2px;
    border-radius: var(--radius-sm);
  }

  .rack__badge {
    display: inline-flex;
    flex-shrink: 0;
    color: var(--accent);
  }

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

  .rack__content {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .rack__copied {
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: var(--tracking-mono);
    color: var(--accent);
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
    white-space: nowrap;
    border: 0;
  }
</style>
