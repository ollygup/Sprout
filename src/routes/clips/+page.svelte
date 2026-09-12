<script lang="ts">
  import { onMount } from "svelte";
  import type { Clip, Group } from "$lib/types";
  import {
    clipImageMeta,
    clipImageUrl,
    CLIP_IMAGE_MAX_BYTES,
    copyClip,
    copyClipImage,
    createClipImage,
    decodeImageToRgba,
    deleteClip,
    getSettings,
    listClips,
    moveClip,
    updateClip,
    updateClipImage,
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
  import { clipTitle, formatBytes } from "$lib/format";
  import Button from "$lib/components/Button.svelte";
  import Dialog from "$lib/components/Dialog.svelte";
  import TextInput from "$lib/components/TextInput.svelte";
  import InfoTip from "$lib/components/InfoTip.svelte";
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
  import DockVisibilityFilter from "$lib/components/DockVisibilityFilter.svelte";
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

  // The dock visibility choice (ADR-0028): page-local, default All. Plain
  // component state, so leaving the page resets it and nothing persists it
  // to Settings, backup, or browser storage. Typing, refreshes, and edits
  // leave it alone.
  let dockVisibility = $state<DockVisibility>("all");

  // One-click re-copy feedback: the id whose row flashes "Copied", plus the
  // polite live region both this page and the window tab (ticket 79) rely
  // on — silence is a bug (research 0004 rule 5).
  let copiedId = $state<number | null>(null);
  let copiedAnnouncement = $state("");
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;

  /** Image-aware row title (ticket 178): named images show their name,
   *  untitled ones read "Image" — clipTitle's first-line fallback needs text
   *  that image Clips don't carry. Text rows keep clipTitle untouched. */
  function clipName(clip: Clip): string {
    return clip.image
      ? clip.name.trim() || "Image"
      : clipTitle(clip.name, clip.content);
  }

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
      if (clip.image) {
        // The pixels decode here (canvas); the write itself stays behind
        // the Rust clipboard command — the flash below only runs once the
        // write landed, so it stays honest like the text path.
        const rgba = await decodeImageToRgba(clipImageUrl(clip.image));
        await copyClipImage(clip.id, rgba.rgbaBase64, rgba.width, rgba.height);
      } else {
        await copyClip(clip.id);
      }
      // The write landed — now the flash may honestly say Copied.
      copiedId = clip.id;
      copiedAnnouncement = `${clipName(clip)} copied.`;
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

  /** Per-item dock visibility (research 0006 pattern 4: the control lives on
   *  its object): hidden clips stay fully listed here and copyable — only
   *  the dock filters them out. Image Clips flip through their own update
   *  so the text path stays untouched. */
  async function toggleDockVisibility(clip: Clip) {
    busy = true;
    error = "";
    try {
      const visible = !(clip.show_in_dock ?? true);
      if (clip.image) {
        await updateClipImage(clip.id, clip.name, visible);
      } else {
        await updateClip({ ...clip, show_in_dock: visible });
      }
      const title = clipName(clip);
      flash(
        visible
          ? `"${title}" will show in the dock.`
          : `"${title}" hidden from the dock — still here and copyable.`
      );
      await load();
    } catch (e) {
      console.error(e);
      error = String(e);
    } finally {
      busy = false;
    }
  }

  // Image Clips (ticket 178): the add/edit dialog below reuses the shared
  // Dialog + TextInput + Button foundation — ClipFormDialog stays the
  // text-clip form untouched. Paste lands from the clipboard, Choose-file
  // opens the OS picker; both preview before saving, and the backend
  // re-validates authoritatively (PNG/JPEG, 5 MB cap).
  let imgOpen = $state(false);
  let imgEditing: Clip | null = $state(null);
  let imgName = $state("");
  let imgDataUrl = $state("");
  let imgBytes = $state("");
  let imgMeta = $state("");
  let imgShowInDock = $state(true);
  let imgSaving = $state(false);
  let imgError = $state("");
  let imgFile: HTMLInputElement | null = $state(null);

  function openImgAdd() {
    imgEditing = null;
    imgName = "";
    imgDataUrl = "";
    imgBytes = "";
    imgMeta = "";
    imgShowInDock = true;
    imgSaving = false;
    imgError = "";
    imgOpen = true;
  }

  function openImgEdit(clip: Clip) {
    if (!clip.image) return;
    imgEditing = clip;
    imgName = clip.name;
    imgDataUrl = clipImageUrl(clip.image);
    imgBytes = clip.image.bytes_base64;
    imgMeta = clipImageMeta(clip.image);
    imgShowInDock = clip.show_in_dock ?? true;
    imgSaving = false;
    imgError = "";
    imgOpen = true;
  }

  function readFileAsDataUrl(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onerror = () =>
        reject(new Error("that file couldn't be read"));
      reader.onload = () => resolve(String(reader.result ?? ""));
      reader.readAsDataURL(file);
    });
  }

  /** Takes a pasted/picked File: instant type+size feedback here, preview on
   *  success — the save path re-validates through the backend either way. */
  async function takeImageFile(file: File) {
    imgError = "";
    if (file.type !== "image/png" && file.type !== "image/jpeg") {
      imgError = "Only PNG and JPEG images can be kept as clips.";
      return;
    }
    if (file.size > CLIP_IMAGE_MAX_BYTES) {
      imgError = "That image is over the 5 MB limit — pick a smaller file.";
      return;
    }
    if (file.size === 0) {
      imgError = "That file has no image data.";
      return;
    }
    try {
      imgDataUrl = await readFileAsDataUrl(file);
    } catch (e) {
      console.error(e);
      imgError = `That file couldn't be read — ${e instanceof Error ? e.message : String(e)}`;
      return;
    }
    imgBytes = imgDataUrl.split(",", 2)[1] ?? "";
    if (!imgBytes) {
      imgError = "That file couldn't be read — try again.";
      imgDataUrl = "";
      return;
    }
    const kind = file.type === "image/png" ? "PNG" : "JPEG";
    imgMeta = `${kind} · ${formatBytes(file.size)}`;
  }

  /** Paste lands anywhere inside the dialog: the first clipboard image wins;
   *  anything else is refused plainly, never silently. */
  function handleImgPaste(e: ClipboardEvent) {
    const file = [...(e.clipboardData?.files ?? [])].find((f) =>
      f.type.startsWith("image/")
    );
    if (!file) {
      imgError = "No image in the clipboard — copy a PNG or JPEG first.";
      return;
    }
    e.preventDefault();
    void takeImageFile(file);
  }

  async function saveImage() {
    imgError = "";
    if (!imgBytes) {
      imgError = imgEditing
        ? "That clip lost its image — close and try again."
        : "Paste an image or choose a file first.";
      return;
    }
    imgSaving = true;
    try {
      if (imgEditing) {
        await updateClipImage(imgEditing.id, imgName.trim(), imgShowInDock);
        imgOpen = false;
        flash(`"${imgName.trim() || "Image"}" saved.`);
      } else {
        const created = await createClipImage(imgName.trim(), imgBytes);
        imgOpen = false;
        flash(`"${clipName(created)}" added to Quick Clips.`);
      }
      await load();
    } catch (e) {
      console.error(e);
      imgError = String(e);
    } finally {
      imgSaving = false;
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
    // While filtering, every section opens so no match hides behind a
    // chevron.
    return isFiltering || groups.collapse.isOpen(groupId);
  }

  /** Move up/down reorders within what the user can see: the whole list when
   *  flat, otherwise the ungrouped block or the clip's own group. The
   *  positions passed down stay global list order. */
  function moveSlice(clip: Clip): Clip[] {
    if (!grouped) return clips;
    return clips.filter((c) => c.group_id === clip.group_id);
  }

  /** One ⋯ menu per clip row, on the round's ordering standard (ticket
   *  106): Edit first (the row's primary verb), then the Move to group
   *  flyout while Groups is on, the dock visibility toggle, Move up /
   *  Move down over the visible slice, Remove danger-last behind a
   *  separator. */
  function openRowMenu(
    clip: Clip,
    anchor: HTMLButtonElement,
    viaKeyboard: boolean
  ) {
    if (menu?.clipId === clip.id) {
      menu = null;
      return;
    }
    const title = clipName(clip);
    const slice = moveSlice(clip);
    const index = slice.indexOf(clip);
    const items: ContextMenuItem[] = [
      {
        label: "Edit",
        icon: "pencil",
        onselect: () => (clip.image ? openImgEdit(clip) : openEdit(clip)),
      },
    ];
    if (groups.enabled) {
      items.push({
        label: "Move to group",
        icon: "folder",
        children: groups.moveToGroupChildren(clip, title),
      });
    }
    items.push({
      label: (clip.show_in_dock ?? true) ? "Hide from dock" : "Show in dock",
      icon: (clip.show_in_dock ?? true) ? "eye-off" : "eye",
      onselect: () => toggleDockVisibility(clip),
    });
    items.push(
      {
        label: "Move up",
        icon: "chevron-up",
        // Filtered neighbors are not saved neighbors: refuse to reorder
        // through them rather than write a surprising order.
        disabled: index <= 0 || reorderBlocked,
        onselect: () => move(clip.id, clips.indexOf(slice[index - 1])),
      },
      {
        label: "Move down",
        icon: "chevron-down",
        disabled: index >= slice.length - 1 || reorderBlocked,
        onselect: () => move(clip.id, clips.indexOf(slice[index + 1])),
      },
      { label: "", separator: true, onselect: () => {} },
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
    const groupMenu = groups.groupMenu(group, anchor, viaKeyboard);
    // Group order is order too: refuse it under filters like clip moves.
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

  function matchesText(c: Clip): boolean {
    const q = normalizeQuery(filter);
    if (q === "") return true;
    return (
      c.name.toLowerCase().includes(q) || c.content.toLowerCase().includes(q)
    );
  }

  // Text search AND dock visibility intersect (ADR-0028): the one matching
  // collection below drives display — no second predicate anywhere.
  function matchesBoth(c: Clip): boolean {
    return matchesText(c) && matchesDockVisibility(c, dockVisibility);
  }

  /** Either filter narrows the list — section opening and the reorder gate
   *  read this one flag. */
  const isFiltering = $derived(isFilterActive(filter, dockVisibility));

  const matchingClips = $derived(clips.filter(matchesBoth));
  const matchedCount = $derived(matchingClips.length);

  /** Content gate from the full collection: a query never hides the trigger. */
  const showDockFilter = $derived(shouldShowDockFilter(clips, dockVisibility));

  const reorderBlocked = $derived(isReorderBlocked(filter, dockVisibility));

  /** Restores ordinary ordering controls after the reorder pause. */
  function clearFilters() {
    filter = "";
    dockVisibility = "all";
  }

  const listView = $derived(
    groupView(groups.groups, clips, matchesBoth, isFiltering)
  );
</script>

<svelte:head>
  <title>Quick Clips — Sprout</title>
</svelte:head>

{#snippet clipRow(clip: Clip)}
  {@const title = clipName(clip)}
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
      {#if clip.image}
        <!-- Decorative thumbnail (the name beside it carries the meaning);
             full image lives in the edit dialog, copy is the row's verb. -->
        <img
          class="rack__thumb"
          src={clipImageUrl(clip.image)}
          alt=""
          width={32}
          height={32}
        />
      {/if}
      <span class="rack__name">{title}</span>
      {#if copiedId === clip.id}
        <span class="rack__copied">Copied</span>
      {:else if clip.image}
        <span class="rack__content">{clipImageMeta(clip.image)}</span>
      {:else}
        <span class="rack__content">{clip.content}</span>
      {/if}
    </button>
    {#if !isDockVisible(clip)}
      <!-- The dock-hidden annotation (ADR-0028): informational only — the
           clip stays fully copyable here; only the dock filters it out. -->
      <span class="rack__dock" title="Hidden from dock">
        <Icon name="eye-off" size={12} />
        <span>Hidden from dock</span>
      </span>
    {/if}
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
      <Button onclick={openAdd} disabled={busy}>
        <Icon name="plus" size={15} />
        Add
      </Button>
      <!-- The second create stays secondary: one primary per header row
           (research 0005 rule 2) — Add is this page's main verb. -->
      <Button variant="secondary" onclick={openImgAdd} disabled={busy}>
        <Icon name="plus" size={15} />
        Add image
      </Button>
    {/snippet}
    {#snippet subtitle()}
      {clips.length} {clips.length === 1 ? "clip" : "clips"}.
      Click a clip to put it back on your clipboard — text or image. The Quick Launch
      window's Quick Clips tab copies them too, once any exist.
    {/snippet}
    {#snippet toolbar()}
      <div class="toolbar">
        <SearchInput
          value={filter}
          placeholder="Search name or text…"
          ariaLabel="Search clips"
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
      <PageFeaturesButton label="Quick Clips features" items={featureItems} />
    {/snippet}
  </PageHeader>

  {#if error}
    <Notice tone="error">{error}</Notice>
  {/if}
  {#if notice}
    <Notice tone="ok">{notice}</Notice>
  {/if}

  {#if reorderBlocked && clips.length > 0}
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

  {#if loading && clips.length === 0}
    <p class="sifting" aria-live="polite">Loading…</p>
  {:else if loadFailed}
    <Notice tone="error">Could not load the clip list.</Notice>
  {:else if clips.length === 0}
    <EmptyState icon="copy" title="No clips yet">
      <p>
        Press <strong>Add</strong> and paste the text you re-type most — support
        replies, commands, addresses — or <strong>Add image</strong> for a
        PNG/JPEG picture. Clicking a clip puts it back on
        your clipboard. Once one clip exists, a Quick Clips tab appears in the
        Quick Launch window for two-click copying from the tray.
      </p>
    </EmptyState>
  {:else if matchedCount === 0 && filter.trim() !== ""}
    <EmptyState icon="search" title={`Nothing matches “${filter.trim()}”`}>
      <p>Search looks at clip names and their text (images by name).</p>
      {#if dockVisibility !== "all"}
        <div class="empty-cta">
          <Button variant="secondary" onclick={() => (dockVisibility = "all")}>
            Show all
          </Button>
        </div>
      {/if}
    </EmptyState>
  {:else if matchedCount === 0}
    <EmptyState icon="search" title="No clips match this filter.">
      {#if dockVisibility === "hidden"}
        <p>No clips are hidden from the dock right now.</p>
      {:else}
        <p>Every clip is hidden from the dock.</p>
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
      {#each matchingClips as clip (clip.id)}
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
    <strong>{deleting ? clipName(deleting) : ""}</strong>
    will be removed from this page and from the Quick Launch window's Quick
    Clips tab. {#if deleting?.image}The image is deleted.{:else}The text is deleted.{/if}
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

<!-- Image Clip dialog (ticket 178): paste or pick one PNG/JPEG, preview it,
     then name it. Editing shows the full saved image read-only above the
     name — the details surface for image Clips on this page. -->
<Dialog
  open={imgOpen}
  title={imgEditing ? "Edit image clip" : "Add an image clip"}
  onclose={() => (imgOpen = false)}
  width={560}
  focusTarget="#clip-image-name"
>
  <div class="imgform" onpaste={handleImgPaste}>
    {#if !imgEditing}
      <p class="imgform__hint">
        Paste a PNG or JPEG from your clipboard anywhere in this dialog, or
        choose a file — up to 5 MB, one picture per clip.
      </p>
      <div class="imgform__pick">
        <Button
          variant="secondary"
          onclick={() => imgFile?.click()}
          disabled={imgSaving}
        >
          <Icon name="plus" size={15} />
          Choose file…
        </Button>
        <input
          bind:this={imgFile}
          type="file"
          accept="image/png,image/jpeg"
          class="imgform__file"
          tabindex="-1"
          aria-hidden="true"
          onchange={(e) => {
            const picked = (e.currentTarget as HTMLInputElement).files?.[0];
            e.currentTarget.value = "";
            if (picked) void takeImageFile(picked);
          }}
        />
      </div>
    {/if}

    {#if imgDataUrl}
      <img
        class="imgform__preview"
        src={imgDataUrl}
        alt={imgEditing
          ? `Image saved as ${imgEditing.name.trim() || "Image"}`
          : "Preview of the image to save"}
      />
      {#if imgMeta}
        <p class="imgform__meta">{imgMeta}</p>
      {/if}
    {/if}

    <TextInput
      id="clip-image-name"
      label="Name"
      placeholder="Optional — untitled images show as “Image”"
      value={imgName}
      onchange={(v) => (imgName = v)}
      info="How naming works"
    >
      {#snippet infobody()}
        <p>
          Optional. An unnamed image is listed as “Image”, so you never have
          to invent a name.
        </p>
      {/snippet}
    </TextInput>

    <label class="imgform__dockvis">
      <input
        type="checkbox"
        class="imgform__dockvis-check"
        checked={imgShowInDock}
        onchange={(e) =>
          (imgShowInDock = (e.target as HTMLInputElement).checked)}
      />
      <span class="imgform__dockvis-title">Show in dock</span>
      <InfoTip label="What showing in the dock does">
        <p>Uncheck to hide this clip from the Quick Launch dock. It stays here and stays copyable.</p>
      </InfoTip>
    </label>

    {#if imgError}
      <p class="imgform__error" role="alert">{imgError}</p>
    {/if}

    <div class="imgform__actions">
      <Button
        variant="secondary"
        onclick={() => (imgOpen = false)}
        disabled={imgSaving}
      >
        Cancel
      </Button>
      <Button onclick={() => void saveImage()} disabled={imgSaving}>
        {imgSaving
          ? imgEditing
            ? "Saving…"
            : "Adding…"
          : imgEditing
            ? "Save changes"
            : "Add image clip"}
      </Button>
    </div>
  </div>
</Dialog>

<ContextMenu ctx={menu} onclose={() => (menu = null)} />

<style>
  .clips {
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

  /* Row thumbnail (ticket 178): a token-sized square that never stretches
     the row — cover-crop keeps any aspect ratio inside it, and the row's
     existing ellipsis still absorbs narrow widths. */
  .rack__thumb {
    width: var(--space-6);
    height: var(--space-6);
    flex-shrink: 0;
    object-fit: cover;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--bg-surface);
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

  /* The dock-hidden annotation: a quiet muted pill in the same language as
     the other metadata — informational only, never dimmed or disabled. */
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

  /* A group with no members yet keeps its place in the user's order without
     pretending to have content. */
  .rack__hint {
    padding: var(--space-3) var(--space-4);
    border: 1px dashed var(--border-strong);
    border-radius: var(--radius);
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  /* Image Clip dialog (ticket 178): the text form's vertical rhythm with the
     same tokens — hint, preview, name, dock toggle, actions. */
  .imgform {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    min-width: 0;
  }

  .imgform__hint {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .imgform__pick {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }

  /* The OS picker opens from the button above — the input itself is never
     a keyboard or screen-reader stop. */
  .imgform__file {
    display: none;
  }

  .imgform__preview {
    display: block;
    max-width: 100%;
    max-height: calc(var(--space-7) * 10);
    width: auto;
    margin: 0 auto;
    object-fit: contain;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: var(--bg-surface);
  }

  .imgform__meta {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .imgform__dockvis {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    cursor: pointer;
  }

  .imgform__dockvis-check {
    margin: 0;
    accent-color: var(--accent);
    width: 14px;
    height: 14px;
  }

  .imgform__dockvis-title {
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text);
  }

  .imgform__error {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--danger-text);
    overflow-wrap: anywhere;
  }

  .imgform__actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
    margin-top: var(--space-2);
  }

  /* The live region is the shared `.sr-only` utility (tokens.css) — its
     local copy here lacked explicit offsets and stretched the document
     (ticket 108). */
</style>
