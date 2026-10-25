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
    openClipViewer,
    placeItem,
    updateClip,
    updateClipImage,
  } from "$lib/api";
  import {
    createCollectionGroups,
  } from "$lib/collectionGroups.svelte";
  import {
    collectionTree,
    countMembers,
    subtreeCount,
  } from "$lib/collectionTree";
  import {
    isDockVisible,
    isFilterActive,
    isReorderBlocked,
    matchesDockVisibility,
    normalizeQuery,
    shouldShowDockFilter,
    type DockVisibility,
  } from "$lib/dockVisibility";
  import { createCollectionDrag } from "$lib/collectionDrag.svelte";
  import {
    resolveGroupDrop,
    resolveItemDrop,
    type ItemDropSuccess,
  } from "$lib/dropTargets";
  import { clipTitle, formatBytes } from "$lib/format";
  import Button from "$lib/components/Button.svelte";
  import Checkbox from "$lib/components/Checkbox.svelte";
  import Dialog from "$lib/components/Dialog.svelte";
  import TextInput from "$lib/components/TextInput.svelte";
  import InfoTip from "$lib/components/InfoTip.svelte";
  import GroupNameDialog from "$lib/components/GroupNameDialog.svelte";
  import GroupColorDialog from "$lib/components/GroupColorDialog.svelte";
  import GroupAccordion from "$lib/components/GroupAccordion.svelte";
  import DragGrip from "$lib/components/DragGrip.svelte";
  import DragOverlay from "$lib/components/DragOverlay.svelte";
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
  import { t, tCount } from "$lib/copy";

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
      ? clip.name.trim() || t("common.imageName")
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

  // Drag & drop (tickets 232–233): the shared controller, one per page.
  const drag = createCollectionDrag({
    enabled: () => !reorderBlocked,
    resolveItem: (dragId, target) =>
      resolveItemDrop({
        dragId,
        order: clips.map((clip) => clip.id),
        filtering: reorderBlocked,
        overRow: target.overRow,
        hasAppendTarget: target.hasAppendTarget,
        appendGroupId: target.appendGroupId,
      }),
    resolveGroup: (dragId, target) =>
      resolveGroupDrop({
        dragId,
        groups: groups.groups,
        filtering: reorderBlocked,
        overHeaderId: target.overHeaderId,
        overSlot: target.overSlot,
        overTopLevelArea: target.overTopLevelArea,
      }),
    commitItem: (dragId, drop) => void placeClip(dragId, drop),
    commitGroup: (dragId, drop) =>
      void groups.reorderGroup(dragId, drop.parentId, drop.position),
  });

  /** One transactional clip drop; a refusal reloads the stored list. */
  async function placeClip(itemId: number, drop: ItemDropSuccess) {
    busy = true;
    error = "";
    try {
      await placeItem("clip", itemId, drop.groupId, drop.position);
    } catch (e) {
      console.error(e);
      error = String(e);
    } finally {
      await load();
      busy = false;
    }
  }

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
      copiedAnnouncement = t("common.copiedName").replace("{name}", clipName(clip));
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
      flash(t("clips.removedFlash"));
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
          ? t("clips.dockShown").replace("{title}", title)
          : t("clips.dockHidden").replace("{title}", title),
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
        reject(new Error(t("clips.imgErrReadShort")));
      reader.onload = () => resolve(String(reader.result ?? ""));
      reader.readAsDataURL(file);
    });
  }

  /** Takes a pasted/picked File: instant type+size feedback here, preview on
   *  success — the save path re-validates through the backend either way. */
  async function takeImageFile(file: File) {
    imgError = "";
    if (file.type !== "image/png" && file.type !== "image/jpeg") {
      imgError = t("clips.imgErrType");
      return;
    }
    if (file.size > CLIP_IMAGE_MAX_BYTES) {
      imgError = t("clips.imgErrSize");
      return;
    }
    if (file.size === 0) {
      imgError = t("clips.imgErrEmpty");
      return;
    }
    try {
      imgDataUrl = await readFileAsDataUrl(file);
    } catch (e) {
      console.error(e);
      imgError = t("clips.imgErrRead").replace("{detail}", e instanceof Error ? e.message : String(e));
      return;
    }
    imgBytes = imgDataUrl.split(",", 2)[1] ?? "";
    if (!imgBytes) {
      imgError = t("clips.imgErrReadRetry");
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
      imgError = t("clips.imgErrNoClip");
      return;
    }
    e.preventDefault();
    void takeImageFile(file);
  }

  async function saveImage() {
    imgError = "";
    if (!imgBytes) {
      imgError = imgEditing
        ? t("clips.imgErrLost")
        : t("clips.imgErrFirst");
      return;
    }
    imgSaving = true;
    try {
      if (imgEditing) {
        await updateClipImage(imgEditing.id, imgName.trim(), imgShowInDock);
        imgOpen = false;
        flash(t("clips.imgSavedFlash").replace("{name}", imgName.trim() || t("common.imageName")));
      } else {
        const created = await createClipImage(imgName.trim(), imgBytes);
        imgOpen = false;
        flash(t("clips.imgAddedFlash").replace("{name}", clipName(created)));
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
      label: t("features.groupsLabel"),
      description: t("features.groupsDesc").replace("{noun}", t("collection.clips.many")),
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

  /** Opens the one clip viewer for an image Clip — the row's
   *  right-click verb and its menu's first item. Only the id travels; the
   *  viewer reads the Clip itself. */
  function openViewer(clip: Clip) {
    openClipViewer(clip.id, "main").catch((e) => {
      console.error(e);
      error = String(e);
    });
  }

  /** Right-click on a row: an image Clip opens the viewer;
   *  text Clips keep their existing behavior. The mouse button is the
   *  discriminator — a right-click carries `button === 2`, while a
   *  keyboard-triggered contextmenu (Shift+F10 / Menu key) has no button and
   *  no cursor position, so it always opens the row menu and never the
   *  viewer. (`detail` is 0 for both paths in Chromium, so it cannot tell
   *  them apart.) */
  function openRowContextMenu(e: MouseEvent, clip: Clip) {
    if (e.button !== 2) {
      const target = (e.target as Element | null)?.closest<HTMLElement>("button");
      if (!target) return;
      e.preventDefault();
      openRowMenu(clip, target, true);
      return;
    }
    if (!clip.image) return;
    e.preventDefault();
    openViewer(clip);
  }

  /** One ⋯ menu per clip row, on the round's ordering standard (ticket
   *  106): Edit first (the row's primary verb), then the Move to group
   *  flyout while Groups is on, the dock visibility toggle, Move up /
   *  Move down over the visible slice, Remove danger-last behind a
   *  separator. "Open image" sits ahead of Edit for image Clips —
   *  the menu is the keyboard path to the viewer. */
  function openRowMenu(
    clip: Clip,
    anchor: HTMLElement,
    viaKeyboard: boolean
  ) {
    if (menu?.clipId === clip.id) {
      menu = null;
      return;
    }
    const title = clipName(clip);
    const slice = moveSlice(clip);
    const index = slice.indexOf(clip);
    const items: ContextMenuItem[] = [];
    if (clip.image) {
      items.push({
        label: t("menu.openImage"),
        icon: "image",
        onselect: () => openViewer(clip),
      });
    }
    items.push({
      label: t("common.edit"),
      icon: "pencil",
      onselect: () => (clip.image ? openImgEdit(clip) : openEdit(clip)),
    });
    if (groups.enabled) {
      items.push({
        label: t("menu.moveToGroup"),
        icon: "folder",
        children: groups.moveToGroupChildren(clip, title),
      });
    }
    items.push({
      label: (clip.show_in_dock ?? true) ? t("menu.hideFromDock") : t("common.showInDock"),
      icon: (clip.show_in_dock ?? true) ? "eye-off" : "eye",
      onselect: () => toggleDockVisibility(clip),
    });
    items.push(
      {
        label: t("menu.moveUp"),
        icon: "chevron-up",
        // Filtered neighbors are not saved neighbors: refuse to reorder
        // through them rather than write a surprising order.
        disabled: index <= 0 || reorderBlocked,
        onselect: () => move(clip.id, clips.indexOf(slice[index - 1])),
      },
      {
        label: t("menu.moveDown"),
        icon: "chevron-down",
        disabled: index >= slice.length - 1 || reorderBlocked,
        onselect: () => move(clip.id, clips.indexOf(slice[index + 1])),
      },
      { label: "", separator: true, onselect: () => {} },
      {
        label: t("common.remove"),
        icon: "trash",
        danger: true,
        onselect: () => (deleting = clip),
      },
    );
    menu = {
      clipId: clip.id,
      open: true,
      label: t("packet.actionsFor").replace("{name}", title),
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
    // Reparenting changes sibling order, so "Move into…" pauses with them.
    menu = reorderBlocked
      ? {
          ...groupMenu,
          items: groupMenu.items.map((item) =>
            item.label === t("menu.moveUp") ||
            item.label === t("menu.moveDown") ||
            item.label === t("menu.moveInto")
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

  const tree = $derived(
    collectionTree(groups.groups, clips, matchesBoth, isFiltering)
  );
</script>

<svelte:head>
  <title>{t("nav.clips")} — Sprout</title>
</svelte:head>

{#snippet groupActions(group: Group)}
  <IconButton
    icon="dots"
    label={t("groups.menuLabel").replace("{name}", group.name)}
    quiet
    data-ctx-trigger
    onclick={(e) =>
      openGroupMenu(
        group,
        e.currentTarget as HTMLButtonElement,
        e.detail === 0
      )}
  />
{/snippet}

{#snippet clipRow(clip: Clip)}
  {@const title = clipName(clip)}
  <li
    class="rack__row"
    data-drop-row={clip.id}
    data-drop-group={clip.group_id ?? ""}
    oncontextmenu={(e) => openRowContextMenu(e, clip)}
  >
    <DragGrip
      label={t("drag.grip").replace("{name}", title)}
      disabled={reorderBlocked}
      onstart={(event) =>
        drag.start(event, { kind: "item", id: clip.id, label: title })}
      onmenu={(event) =>
        openRowMenu(clip, event.currentTarget as HTMLButtonElement, true)}
    />
    <button
      type="button"
      class="rack__main"
      aria-label={t("clips.copyName").replace("{title}", title)}
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
        <span class="rack__copied">{t("common.copied")}</span>
      {:else if clip.image}
        <span class="rack__content">{clipImageMeta(clip.image)}</span>
      {:else}
        <span class="rack__content">{clip.content}</span>
      {/if}
    </button>
    {#if !isDockVisible(clip)}
      <!-- The dock-hidden annotation (ADR-0028): informational only — the
           clip stays fully copyable here; only the dock filters it out. -->
      <span class="rack__dock" title={t("common.hiddenFromDock")}>
        <Icon name="eye-off" size={12} />
        <span>{t("common.hiddenFromDock")}</span>
      </span>
    {/if}
    <IconButton
      icon="dots"
      label={t("packet.actionsFor").replace("{name}", title)}
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
  <PageHeader titleId="clips-title" title={t("nav.clips")}>
    {#snippet actions()}
      <Button onclick={openAdd} disabled={busy}>
        <Icon name="plus" size={15} />
        {t("common.add")}
      </Button>
      <!-- The second create stays secondary: one primary per header row
           (research 0005 rule 2) — Add is this page's main verb. -->
      <Button variant="secondary" onclick={openImgAdd} disabled={busy}>
        <Icon name="plus" size={15} />
        {t("clips.addImage")}
      </Button>
    {/snippet}
    {#snippet subtitle()}
      {clips.length === 1 ? tCount("clips.countOne", clips.length) : tCount("clips.countMany", clips.length)}
      {t("clips.subBody")}
    {/snippet}
    {#snippet toolbar()}
      <div class="toolbar">
        <SearchInput
          value={filter}
          placeholder={t("clips.searchPh")}
          ariaLabel={t("clips.searchLabel")}
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
      <PageFeaturesButton label={t("clips.featuresLabel")} items={featureItems} />
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
      {t("launch.reorderPaused")}
      <button
        type="button"
        class="reorder-note__clear"
        onclick={clearFilters}
      >
        {t("common.clearFilters")}
      </button>
      {t("launch.reorderTail")}
    </p>
  {/if}

  {#if loading && clips.length === 0}
    <p class="sifting" aria-live="polite">Loading…</p>
  {:else if loadFailed}
    <Notice tone="error">{t("clips.loadFail")}</Notice>
  {:else if clips.length === 0}
    <EmptyState icon="copy" title={t("clips.noClips")}>
      <p>
        {t("common.emptyPress")}<strong>{t("common.add")}</strong>{t("clips.emptyBodyB")}<strong>{t("clips.addImage")}</strong>{t("clips.emptyBodyC")}
      </p>
    </EmptyState>
  {:else if matchedCount === 0 && filter.trim() !== ""}
    <EmptyState icon="search" title={t("common.noMatchFor").replace("{query}", filter.trim())}>
      <p>{t("clips.searchLooks")}</p>
      {#if dockVisibility !== "all"}
        <div class="empty-cta">
          <Button variant="secondary" onclick={() => (dockVisibility = "all")}>
            {t("common.showAll")}
          </Button>
        </div>
      {/if}
    </EmptyState>
  {:else if matchedCount === 0}
    <EmptyState icon="search" title={t("clips.noFilterTitle")}>
      {#if dockVisibility === "hidden"}
        <p>{t("clips.noHiddenNow")}</p>
      {:else}
        <p>{t("clips.everyHidden")}</p>
      {/if}
      <div class="empty-cta">
        <Button variant="secondary" onclick={() => (dockVisibility = "all")}>
          {t("common.showAll")}
        </Button>
      </div>
    </EmptyState>
  {:else}
    <div class="rack-area" data-drop-top-level>
      {#if grouped}
        {#if tree.ungrouped.length > 0}
          <ul class="rack">
            {#each tree.ungrouped as clip (clip.id)}
              {@render clipRow(clip)}
            {/each}
          </ul>
        {/if}
        {#each tree.sections as section (section.group.id)}
          {#snippet grip()}
            <DragGrip
              label={t("drag.grip").replace("{name}", section.group.name)}
              disabled={reorderBlocked}
              onstart={(event) =>
                drag.start(event, {
                  kind: "group",
                  id: section.group.id,
                  label: section.group.name,
                })}
              onmenu={(event) =>
                openGroupMenu(
                  section.group,
                  event.currentTarget as HTMLButtonElement,
                  true,
                )}
            />
          {/snippet}
          <GroupAccordion
            grip={grip}
            open={sectionOpen(section.group.id)}
            controls={`clip-group-${section.group.id}`}
            name={section.group.name}
            count={subtreeCount(clips, groups.groups, section.group.id)}
            color={section.group.color}
            groupId={section.group.id}
            onToggle={() => groups.collapse.toggle(section.group.id)}
          >
            {#snippet actions()}
              {@render groupActions(section.group)}
            {/snippet}
            {#if section.rows.length > 0 || section.children.length === 0}
              <ul class="rack">
                {#each section.rows as clip (clip.id)}
                  {@render clipRow(clip)}
                {/each}
                {#if section.rows.length === 0}
                  <li class="rack__hint" data-drop-empty-group={section.group.id}>
                    {t("clips.emptyGroupHint")}
                  </li>
                {/if}
              </ul>
            {/if}
            {#each section.children as child (child.group.id)}
              {#snippet grip()}
                <DragGrip
                  label={t("drag.grip").replace("{name}", child.group.name)}
                  disabled={reorderBlocked}
                  onstart={(event) =>
                    drag.start(event, {
                      kind: "group",
                      id: child.group.id,
                      label: child.group.name,
                    })}
                  onmenu={(event) =>
                    openGroupMenu(
                      child.group,
                      event.currentTarget as HTMLButtonElement,
                      true,
                    )}
                />
              {/snippet}
              <GroupAccordion
                grip={grip}
                nested
                open={sectionOpen(child.group.id)}
                controls={`clip-group-${child.group.id}`}
                name={child.group.name}
                count={countMembers(clips, child.group.id)}
                color={child.group.color}
                groupId={child.group.id}
                parentId={section.group.id}
                onToggle={() => groups.collapse.toggle(child.group.id)}
              >
                {#snippet actions()}
                  {@render groupActions(child.group)}
                {/snippet}
                <ul class="rack">
                  {#each child.rows as clip (clip.id)}
                    {@render clipRow(clip)}
                  {/each}
                  {#if child.rows.length === 0}
                    <li class="rack__hint" data-drop-empty-group={child.group.id}>
                      {t("clips.emptyGroupHint")}
                    </li>
                  {/if}
                </ul>
              </GroupAccordion>
            {/each}
          </GroupAccordion>
        {/each}
      {:else}
        <ul class="rack">
          {#each matchingClips as clip (clip.id)}
            {@render clipRow(clip)}
          {/each}
        </ul>
      {/if}
    </div>
  {/if}
</section>

<DragOverlay drag={drag.drag} message={drag.message} />

<div class="sr-only" role="status" aria-live="polite">
  {copiedAnnouncement}
</div>

<ConfirmDialog
  open={deleting !== null}
  title={t("clips.deleteTitle")}
  confirmLabel={t("common.delete")}
  danger
  onconfirm={remove}
  oncancel={() => (deleting = null)}
>
  <p>
    <strong>{deleting ? clipName(deleting) : ""}</strong>{t("clips.deleteBodyTail")}
    {#if deleting?.image}{t("clips.deleteImgNote")}{:else}{t("clips.deleteTextNote")}{/if}
  </p>
</ConfirmDialog>

<ConfirmDialog
  open={groups.removing !== null}
  title={t("dialog.removeGroupTitle")}
  confirmLabel={t("common.remove")}
  danger
  onconfirm={() => groups.removeGroup()}
  oncancel={() => groups.cancelRemove()}
>
  <p>
    <strong>{groups.removing?.name}</strong>{groups.removingHasChildren
      ? t("dialog.removeParentBody")
      : t("dialog.removeChildBody")}
  </p>
</ConfirmDialog>

<GroupNameDialog
  naming={groups.naming}
  draft={groups.nameDraft}
  error={groups.nameError}
  saving={groups.savingName}
  inputId="clip-group-name"
  placeholder={t("clips.groupEx")}
  ondraft={(v) => (groups.nameDraft = v)}
  onsubmit={() => groups.submitName()}
  onclose={() => groups.cancelNaming()}
/>

<GroupColorDialog
  group={groups.coloring}
  draft={groups.colorDraft}
  error={groups.colorError}
  saving={groups.savingColor}
  inputId="clip-group-color"
  onpick={(color) => groups.applyColor(color)}
  ondraft={(v) => (groups.colorDraft = v)}
  onsubmit={() => groups.applyColor(groups.colorDraft)}
  onclose={() => groups.cancelColor()}
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
  title={imgEditing ? t("clips.imgTitleEdit") : t("clips.imgTitleAdd")}
  onclose={() => (imgOpen = false)}
  width={560}
  focusTarget="#clip-image-name"
>
  <div class="imgform" onpaste={handleImgPaste}>
    {#if !imgEditing}
      <p class="imgform__hint">
        {t("clips.imgHint")}
      </p>
      <div class="imgform__pick">
        <Button
          variant="secondary"
          onclick={() => imgFile?.click()}
          disabled={imgSaving}
        >
          <Icon name="plus" size={15} />
          {t("clips.chooseFile")}
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
          ? t("clips.imgAltSaved").replace("{name}", imgEditing.name.trim() || t("common.imageName"))
          : t("clips.imgPreviewAlt")}
      />
      {#if imgMeta}
        <p class="imgform__meta">{imgMeta}</p>
      {/if}
    {/if}

    <TextInput
      id="clip-image-name"
      label={t("common.name")}
      placeholder={t("clips.imgNamePh").replace("{image}", t("common.imageName"))}
      value={imgName}
      onchange={(v) => (imgName = v)}
      info={t("dialog.namingHow")}
    >
      {#snippet infobody()}
        <p>
          {t("clips.imgNamingBody").replace("{image}", t("common.imageName"))}
        </p>
      {/snippet}
    </TextInput>

    <Checkbox
      checked={imgShowInDock}
      onchange={(v) => (imgShowInDock = v)}
      title={t("common.showInDock")}
      hint={t("common.showInDockHint")}
    />

    {#if imgError}
      <p class="imgform__error" role="alert">{imgError}</p>
    {/if}

    <div class="imgform__actions">
      <Button
        variant="secondary"
        onclick={() => (imgOpen = false)}
        disabled={imgSaving}
      >
        {t("common.cancel")}
      </Button>
      <Button onclick={() => void saveImage()} disabled={imgSaving}>
        {imgSaving
          ? imgEditing
            ? t("common.busy.saving")
            : t("common.busy.adding")
          : imgEditing
            ? t("common.saveChanges")
            : t("clips.imgAdd")}
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

  /* Room below the tree for the top-level drop target (ticket 233). */
  .rack-area {
    min-height: 96px;
    padding-bottom: var(--space-6);
  }

  .rack__row {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: calc(var(--space-3) - 2px) var(--space-3)
      calc(var(--space-3) - 2px) var(--space-4);
    /* The card carries its group's tint; a neutral group publishes none, so a
       neutral row falls back to the page surface (ticket 240). */
    background: var(--group-row-bg, transparent);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    --row-text: var(--text-muted);
    transition: background-color var(--dur-fast) var(--ease-out);
  }

  /* A colored group publishes its hover surface and hover text (ticket 240,
     research 0028); neutral groups and ungrouped rows fall back to the
     neutral tokens. */
  .rack__row:hover {
    background: var(--group-row-hover, var(--bg-hover));
    --row-text: var(--group-row-hover-text, var(--text-muted));
  }

  /* The card's drag grip fades in from its leading gutter (ticket 232). The
     grip's strip is a descendant of the row, so hovering the strip keeps this
     `:hover` alive: the card stays lit while the pointer travels to the
     handle, and the handle stays visible. */
  .rack__row:hover,
  .rack__row:focus-within {
    --grip-opacity: 1;
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
    color: var(--row-text);
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
