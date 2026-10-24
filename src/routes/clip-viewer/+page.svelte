<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import type { Clip } from "$lib/types";
  import {
    clipImageMeta,
    clipImageUrl,
    closeClipViewer,
    copyClipImage,
    decodeImageToRgba,
    getClip,
    takeClipViewerRequest,
  } from "$lib/api";
  import { t } from "$lib/copy";
  import Button from "$lib/components/Button.svelte";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import IconButton from "$lib/components/IconButton.svelte";

  // The clip viewer: one reusable floating window showing an
  // image Clip. The window is created/retargeted by Rust (`open_clip_viewer`);
  // this page only reads the Clip by id, so image bytes never ride a URL or
  // an event. Closing goes back through Rust, which remembers the size and
  // returns focus to the window that invoked the viewer.

  let clip = $state<Clip | null>(null);
  let requestedId = $state<number | null>(null);
  let loading = $state(true);
  let loadFailed = $state(false);
  let copyError = $state("");
  let copied = $state(false);
  let copiedAnnouncement = $state("");
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;

  const image = $derived(clip?.image ?? null);
  const title = $derived(clip ? clip.name.trim() || t("common.imageName") : "");

  /** Reads the requested Clip. `null` is the friendly missing state (deleted
   *  while the viewer was open, or gone before it opened); a thrown error is
   *  the honest read-failure state. */
  async function load(id: number) {
    requestedId = id;
    loading = true;
    loadFailed = false;
    copyError = "";
    copied = false;
    try {
      const found = await getClip(id);
      // A newer open request won while this read was in flight.
      if (requestedId !== id) return;
      clip = found?.image ? found : null;
    } catch (e) {
      console.error(e);
      if (requestedId === id) {
        clip = null;
        loadFailed = true;
      }
    } finally {
      if (requestedId === id) loading = false;
    }
  }

  function close() {
    closeClipViewer().catch((e) => console.error(e));
  }

  /** The same copy path and feedback as the Clip row: decode the
   *  stored bytes, write through the Rust clipboard command, then flash
   *  "Copied" only once the write honestly landed. */
  async function copy() {
    if (!clip?.image) return;
    copyError = "";
    try {
      const rgba = await decodeImageToRgba(clipImageUrl(clip.image));
      await copyClipImage(clip.id, rgba.rgbaBase64, rgba.width, rgba.height);
      copied = true;
      copiedAnnouncement = t("common.copiedName").replace("{name}", title);
      clearTimeout(copiedTimer);
      copiedTimer = setTimeout(() => (copied = false), 1200);
    } catch (e) {
      console.error(e);
      copyError = String(e);
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key !== "Escape") return;
    e.preventDefault();
    close();
  }

  onMount(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;

    // The create → mount race: Rust records the id before this page exists,
    // so the pending read delivers it exactly once.
    takeClipViewerRequest()
      .then((id) => {
        if (disposed) return;
        if (id === null) loading = false;
        else void load(id);
      })
      .catch((e) => {
        console.error(e);
        if (disposed) return;
        loading = false;
        loadFailed = true;
      });

    // An already-open viewer retargets through the event — id only, never
    // image bytes.
    listen<number>("clip-viewer-open", (event) => {
      void load(event.payload);
    }).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });

    // A Clip deleted in the main app while the viewer shows it: re-read when
    // the viewer regains focus, replacing stale pixels with the empty state.
    const onFocus = () => {
      if (requestedId !== null) void load(requestedId);
    };
    window.addEventListener("focus", onFocus);

    return () => {
      disposed = true;
      unlisten?.();
      window.removeEventListener("focus", onFocus);
      clearTimeout(copiedTimer);
    };
  });
</script>

<svelte:window onkeydown={onKeydown} />

<main class="viewer" aria-labelledby="viewer-title">
  <!-- Compact frameless chrome: the whole bar drags the window; the close
       control opts out so it stays clickable. -->
  <header class="viewer__bar" data-tauri-drag-region>
    <span class="viewer__mark" aria-hidden="true" data-tauri-drag-region>
      <Icon name="image" size={15} />
    </span>
    <h1 class="viewer__title" id="viewer-title" title={title || t("viewer.title")} data-tauri-drag-region>
      {title || t("viewer.title")}
    </h1>
    <IconButton
      icon="x"
      label={t("viewer.close")}
      data-tauri-drag-region="false"
      onclick={close}
    />
  </header>

  {#if loading}
    <div class="viewer__stage viewer__stage--message">
      <p class="viewer__message" aria-live="polite">{t("viewer.loading")}</p>
    </div>
  {:else if image && clip}
    <div class="viewer__stage">
      <img
        class="viewer__image"
        src={clipImageUrl(image)}
        alt={t("viewer.imageAlt").replace("{name}", title)}
      />
    </div>
    <footer class="viewer__foot">
      <span class="viewer__meta">{clipImageMeta(image)}</span>
      <Button
        variant="secondary"
        aria-label={t("clips.copyName").replace("{title}", title)}
        onclick={copy}
      >
        <Icon name={copied ? "check" : "copy"} size={14} />
        {copied ? t("common.copied") : t("common.copy")}
      </Button>
    </footer>
  {:else}
    <div class="viewer__stage viewer__stage--message">
      <EmptyState
        icon="image"
        title={loadFailed ? t("viewer.errorTitle") : t("viewer.missingTitle")}
      >
        <p>{loadFailed ? t("viewer.errorBody") : t("viewer.missingBody")}</p>
      </EmptyState>
    </div>
  {/if}

  {#if copyError}
    <p class="viewer__error" role="alert">{copyError}</p>
  {/if}

  <span class="sr-only" role="status" aria-live="polite">
    {copiedAnnouncement}
  </span>
</main>

<style>
  /* One column: compact bar, the picture stage (the only flexible row), the
     meta/copy footer. The stage owns the remaining height so the image never
     pushes the controls out of the window. */
  .viewer {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
    background: var(--bg-page);
    color: var(--text);
  }

  .viewer__bar {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    flex-shrink: 0;
    padding: var(--space-2) var(--space-2) var(--space-2) var(--space-4);
    border-bottom: 1px solid var(--border);
    user-select: none;
  }

  .viewer__mark {
    display: inline-flex;
    flex-shrink: 0;
    color: var(--text-muted);
  }

  .viewer__title {
    flex: 1;
    min-width: 0;
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-display);
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text);
  }

  .viewer__stage {
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--space-4);
    background: var(--bg-sunken);
  }

  .viewer__stage--message {
    padding: var(--space-5);
    background: var(--bg-page);
  }

  .viewer__image {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-dialog);
  }

  .viewer__message {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .viewer__foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    flex-shrink: 0;
    padding: var(--space-2) var(--space-3) var(--space-2) var(--space-4);
    border-top: 1px solid var(--border);
  }

  .viewer__meta {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }

  .viewer__error {
    margin: 0;
    padding: var(--space-2) var(--space-4);
    border-top: 1px solid var(--border);
    font-size: var(--text-xs);
    color: var(--danger-text);
  }
</style>
