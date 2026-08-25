<script lang="ts">
  import { onMount } from "svelte";
  import type { Clip, Group } from "$lib/types";
  import {
    copyClip,
    deleteClip,
    getSettings,
    listClips,
    moveClip,
  } from "$lib/api";
  import {
    countMembers,
    createCollectionGroups,
    groupView,
  } from "$lib/collectionGroups.svelte";
  import { clipTitle } from "$lib/format";
  import Button from "$lib/components/Button.svelte";
  import GroupNameDialog from "$lib/components/GroupNameDialog.svelte";
  import GroupAccordion from "$lib/components/GroupAccordion.svelte";
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
  import PageFeaturesButton from "$lib/components/PageFeaturesButton.svelte";
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

  // The shared search input narrows the list client-side, over name and
  // content (ticket 78). It filters across every section, so grouping never
  // hides a match.
  let filter = $state("");

  // One-click re-copy feedback: the id whose row flashes "Copied", plus the
  // polite live region both this page and the window tab (ticket 79) rely
  // on — silence is a bug (research 0004 rule 5).
  let copiedId = $state<number | null>(null);
  let copiedAnnouncement = $state("");
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;

  // Groups (tickets 89/91): the same per-collection pattern as Quick Actions
  // (ticket 90) — the page-features gear menu is the feature's only switch
  // (research 0008). Off is fully dormant: stored groups and memberships are
  // never shown or touched, they simply wait. The feature's logic is owned
  // once by the shared manager (ticket 95); this page contributes only its
  // collection key and feedback channels.
  const groups = createCollectionGroups({
    collection: "clip",
    noun: "clips",
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

  // One menu serves clip rows and group headers; which kind it belongs to
  // rides on whichever id is set.
  let menu: (ContextMenuState & { clipId?: number; groupId?: number }) | null =
    $state(null);

  onMount(() => {
    load();
    loadGroupsSetting();
    return () => {
      clearTimeout(copiedTimer);
      clearTimeout(noticeTimer);
    };
  });

  async function load() {
    loading = true;
    try {
      const [cs] = await Promise.all([
        listClips(),
        // Ticket 95: the groups fetch lives in the shared manager; running
        // it inside this Promise.all keeps both loads parallel as before.
        groups.refresh(),
      ]);
      clips = cs;
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
      groups.setEnabledFromSettings(s.clip_groups === "on");
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

  /** The feature switch behind the page-features menu (research 0008):
   *  persisted for this collection through the settings store. Optimistic —
   *  reverted when the save fails. */
  const featureItems = $derived([
    {
      label: "Groups",
      description:
        "Bucket clips into named sections you order yourself. Groups and assignments are kept while off.",
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
   *  flat, otherwise the ungrouped block or the clip's own group. The
   *  positions passed down stay global list order. */
  function moveSlice(clip: Clip): Clip[] {
    if (!grouped) return clips;
    return clips.filter((c) => c.group_id === clip.group_id);
  }

  /** One ⋯ menu per clip row: group assignment first while grouping is on,
   *  then Edit, Move up / Move down scoped to the visible slice, Remove. */
  function openRowMenu(
    clip: Clip,
    anchor: HTMLButtonElement,
    viaKeyboard: boolean
  ) {
    if (menu?.clipId === clip.id) {
      menu = null;
      return;
    }
    const title = clipTitle(clip.name, clip.content);
    const slice = moveSlice(clip);
    const index = slice.indexOf(clip);
    const items: ContextMenuItem[] = [];
    if (grouped) {
      items.push(
        {
          label: "Ungrouped",
          icon: clip.group_id === null ? "check" : undefined,
          onselect: () =>
            groups.assign(clip, clipTitle(clip.name, clip.content), null),
        },
        ...groups.groups.map((g) => ({
          label: g.name,
          icon: clip.group_id === g.id ? "check" : undefined,
          onselect: () =>
            groups.assign(clip, clipTitle(clip.name, clip.content), g.id),
        })),
        { label: "", separator: true, onselect: () => {} }
      );
    }
    items.push(
      {
        label: "Edit",
        icon: "pencil",
        onselect: () => openEdit(clip),
      },
      {
        label: "Move up",
        icon: "chevron-up",
        disabled: index <= 0,
        onselect: () => move(clip.id, clips.indexOf(slice[index - 1])),
      },
      {
        label: "Move down",
        icon: "chevron-down",
        disabled: index >= slice.length - 1,
        onselect: () => move(clip.id, clips.indexOf(slice[index + 1])),
      },
      {
        label: "Remove",
        icon: "trash",
        danger: true,
        onselect: () => (deleting = clip),
      },
    );
    menu = {
      clipId: clip.id,
      open: true,
      label: `Actions for ${title}`,
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

  function matchesClip(c: Clip): boolean {
    const q = filter.trim().toLowerCase();
    return (
      c.name.toLowerCase().includes(q) || c.content.toLowerCase().includes(q)
    );
  }

  const matchedCount = $derived(clips.filter(matchesClip).length);
  const listView = $derived(
    groupView(groups.groups, clips, matchesClip, filter.trim() !== "")
  );
</script>

<svelte:head>
  <title>Quick Clips — Sprout</title>
</svelte:head>

{#snippet clipRow(clip: Clip)}
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
{/snippet}

<section class="clips" aria-labelledby="clips-title">
  <PageHeader titleId="clips-title" title="Quick Clips">
    {#snippet actions()}
      {#if groups.enabled}
        <Button variant="secondary" onclick={() => groups.openCreate()} disabled={busy}>
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
    {#snippet features()}
      <PageFeaturesButton label="Quick Clips features" items={featureItems} />
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
  {:else if matchedCount === 0}
    <EmptyState icon="search" title={`Nothing matches “${filter.trim()}”`}>
      <p>Search looks at clip names and their text.</p>
    </EmptyState>
  {:else if grouped}
    {#if listView.ungrouped.length > 0}
      <ul class="rack">
        {#each listView.ungrouped as clip (clip.id)}
          {@render clipRow(clip)}
        {/each}
      </ul>
    {/if}
    {#each listView.sections as section (section.group.id)}
      <GroupAccordion
        open={sectionOpen(section.group.id)}
        controls={`clip-group-${section.group.id}`}
        name={section.group.name}
        count={countMembers(clips, section.group.id)}
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
          {#each section.rows as clip (clip.id)}
            {@render clipRow(clip)}
          {/each}
          {#if section.rows.length === 0}
            <li class="rack__hint">
              No clips here yet — use a clip's ⋯ menu to move one in.
            </li>
          {/if}
        </ul>
      </GroupAccordion>
    {/each}
  {:else}
    <ul class="rack">
      {#each clips.filter(matchesClip) as clip (clip.id)}
        {@render clipRow(clip)}
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

<ConfirmDialog
  open={groups.removing !== null}
  title="Remove group?"
  confirmLabel="Remove"
  danger
  onconfirm={() => groups.removeGroup()}
  oncancel={() => groups.cancelRemove()}
>
  <p>
    <strong>{groups.removing?.name}</strong> will be deleted. Its clips will
    not be — they return to the ungrouped list.
  </p>
</ConfirmDialog>

<GroupNameDialog
  naming={groups.naming}
  draft={groups.nameDraft}
  error={groups.nameError}
  saving={groups.savingName}
  inputId="clip-group-name"
  placeholder="e.g. Support replies"
  ondraft={(v) => (groups.nameDraft = v)}
  onsubmit={() => groups.submitName()}
  onclose={() => groups.cancelNaming()}
/>

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

  /* A group with no members yet keeps its place in the user's order without
     pretending to have content. */
  .rack__hint {
    padding: var(--space-3) var(--space-4);
    border: 1px dashed var(--border-strong);
    border-radius: var(--radius);
    font-size: var(--text-xs);
    color: var(--text-muted);
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
