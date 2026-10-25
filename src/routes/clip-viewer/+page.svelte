<script lang="ts">
  import { onMount, tick } from "svelte";
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
  import {
    KEY_PAN_PX,
    ZOOM_MAX,
    ZOOM_MIN,
    ZOOM_STEP,
    canPan,
    clampPan,
    cssScale,
    displayedSize,
    fitState,
    isFit,
    panBy,
    toggleActualSize,
    wheelPixels,
    wheelZoomFactor,
    zoomAt,
    zoomLabel,
    type Anchor,
    type ImageSize,
    type Viewport,
    type ZoomState,
  } from "$lib/clipViewerZoom";
  import Button from "$lib/components/Button.svelte";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import IconButton from "$lib/components/IconButton.svelte";

  // The clip viewer: one reusable floating window showing an
  // image Clip. The window is created/retargeted by Rust (`open_clip_viewer`);
  // this page reads the Clip by id, so image bytes never ride a URL or an
  // event. Closing goes back through Rust, which remembers the size and
  // returns focus to the window that invoked the viewer.
  //
  // Zoom is a physical-pixel ratio (100 % = one image pixel per physical
  // display pixel); the pure math lives in `$lib/clipViewerZoom` so the mouse
  // and keyboard controls share one model. The image is positioned by
  // transform rather than by scrolling, which is what makes cursor-anchored
  // zoom and bounded panning possible without layout thrash.

  let clip = $state<Clip | null>(null);
  let requestedId = $state<number | null>(null);
  let loading = $state(true);
  let loadFailed = $state(false);
  let copyError = $state("");
  let copied = $state(false);
  let copiedAnnouncement = $state("");
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;

  let stageEl: HTMLDivElement | undefined = $state();
  let viewport = $state<Viewport>({ width: 0, height: 0, dpr: 1 });
  let zoom = $state(1);
  let panX = $state(0);
  let panY = $state(0);
  // Fit is a live mode: while it holds, a window resize re-fits instead of
  // leaving the image at a scale chosen for the old stage.
  let fitMode = $state(true);
  let dragging = $state(false);
  let drag:
    | {
        pointerId: number;
        startX: number;
        startY: number;
        panX: number;
        panY: number;
      }
    | undefined;

  const image = $derived(clip?.image ?? null);
  const title = $derived(clip ? clip.name.trim() || t("common.imageName") : "");
  const imageSize = $derived<ImageSize>(
    image ? { width: image.width, height: image.height } : { width: 0, height: 0 }
  );
  const viewState = $derived<ZoomState>({ zoom, panX, panY });
  const pannable = $derived(
    image ? canPan(imageSize, zoom, viewport) : false
  );
  const imageTransform = $derived.by(() => {
    const size = displayedSize(imageSize, zoom, viewport.dpr);
    const x = viewport.width / 2 + panX - size.width / 2;
    const y = viewport.height / 2 + panY - size.height / 2;
    return `translate(${x}px, ${y}px) scale(${cssScale(zoom, viewport.dpr)})`;
  });

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
    if (requestedId !== id) return;
    // A new Clip starts fitted; the stage only exists after the tick.
    await tick();
    fitMode = true;
    measureStage();
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

  function apply(next: ZoomState) {
    zoom = next.zoom;
    panX = next.panX;
    panY = next.panY;
  }

  /** Reads the live stage box and DPR. A fit view re-derives its scale, a
   *  manual view keeps its zoom and only pulls the pan back inside bounds. */
  function measureStage() {
    if (!stageEl || !image) return;
    const rect = stageEl.getBoundingClientRect();
    viewport = {
      width: rect.width,
      height: rect.height,
      dpr: window.devicePixelRatio || 1,
    };
    if (fitMode) {
      apply(fitState(imageSize, viewport));
    } else {
      apply(clampPan(viewState, imageSize, viewport));
    }
  }

  function setFit() {
    fitMode = true;
    apply(fitState(imageSize, viewport));
  }

  function setActual() {
    fitMode = false;
    apply(zoomAt(viewState, 1, { x: 0, y: 0 }, imageSize, viewport));
  }

  function zoomStep(factor: number) {
    fitMode = false;
    apply(zoomAt(viewState, zoom * factor, { x: 0, y: 0 }, imageSize, viewport));
  }

  /** The percentage button is the keyboard equivalent of double-click: it
   *  toggles between fit and actual size at the viewport centre. */
  function toggleFromReadout() {
    const next = toggleActualSize(
      viewState,
      { x: 0, y: 0 },
      imageSize,
      viewport
    );
    fitMode = isFit(next.zoom, imageSize, viewport);
    apply(next);
  }

  /** CSS px from the stage's centre — the anchor space the pure module uses. */
  function anchorAt(clientX: number, clientY: number): Anchor {
    const rect = stageEl?.getBoundingClientRect();
    return {
      x: clientX - (rect?.left ?? 0) - viewport.width / 2,
      y: clientY - (rect?.top ?? 0) - viewport.height / 2,
    };
  }

  function onWheel(e: WheelEvent) {
    if (!image || !stageEl) return;
    e.preventDefault();
    const anchor = anchorAt(e.clientX, e.clientY);
    if (e.ctrlKey || e.metaKey) {
      // Trackpad pinch arrives as ctrl+wheel, so the same path serves both.
      const delta = wheelPixels(e.deltaY, e.deltaMode, viewport.height);
      fitMode = false;
      apply(
        zoomAt(viewState, zoom * wheelZoomFactor(delta), anchor, imageSize, viewport)
      );
      return;
    }
    const deltaX = wheelPixels(e.deltaX, e.deltaMode, viewport.width);
    const deltaY = wheelPixels(e.deltaY, e.deltaMode, viewport.height);
    const horizontal = e.shiftKey ? deltaY || deltaX : deltaX;
    const vertical = e.shiftKey ? 0 : deltaY;
    const next = panBy(viewState, -horizontal, -vertical, imageSize, viewport);
    if (next.panX !== panX || next.panY !== panY) fitMode = false;
    apply(next);
  }

  function onDoubleClick(e: MouseEvent) {
    if (!image) return;
    const next = toggleActualSize(
      viewState,
      anchorAt(e.clientX, e.clientY),
      imageSize,
      viewport
    );
    fitMode = isFit(next.zoom, imageSize, viewport);
    apply(next);
  }

  function startPan(e: PointerEvent) {
    if (e.button !== 0 || !image || !pannable || !stageEl) return;
    drag = {
      pointerId: e.pointerId,
      startX: e.clientX,
      startY: e.clientY,
      panX,
      panY,
    };
    dragging = true;
    // Capture keeps the gesture alive when the pointer leaves the stage.
    stageEl.setPointerCapture?.(e.pointerId);
    e.preventDefault();
  }

  function movePan(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.pointerId) return;
    const next = panBy(
      { zoom, panX: drag.panX, panY: drag.panY },
      e.clientX - drag.startX,
      e.clientY - drag.startY,
      imageSize,
      viewport
    );
    if (next.panX !== panX || next.panY !== panY) fitMode = false;
    apply(next);
  }

  function endPan(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.pointerId) return;
    if (stageEl?.hasPointerCapture?.(e.pointerId)) {
      stageEl.releasePointerCapture(e.pointerId);
    }
    drag = undefined;
    dragging = false;
  }

  function panKeys(dx: number, dy: number) {
    const next = panBy(viewState, dx, dy, imageSize, viewport);
    if (next.panX === panX && next.panY === panY) return;
    fitMode = false;
    apply(next);
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      close();
      return;
    }
    if (!image) return;
    if (e.altKey || e.metaKey) return;
    if (e.ctrlKey) {
      if (e.key === "0") {
        e.preventDefault();
        setFit();
      }
      return;
    }
    switch (e.key) {
      case "+":
      case "=":
        e.preventDefault();
        zoomStep(ZOOM_STEP);
        break;
      case "-":
      case "_":
        e.preventDefault();
        zoomStep(1 / ZOOM_STEP);
        break;
      case "0":
        e.preventDefault();
        setFit();
        break;
      case "1":
        e.preventDefault();
        setActual();
        break;
      case "ArrowLeft":
        e.preventDefault();
        panKeys(-KEY_PAN_PX, 0);
        break;
      case "ArrowRight":
        e.preventDefault();
        panKeys(KEY_PAN_PX, 0);
        break;
      case "ArrowUp":
        e.preventDefault();
        panKeys(0, -KEY_PAN_PX);
        break;
      case "ArrowDown":
        e.preventDefault();
        panKeys(0, KEY_PAN_PX);
        break;
    }
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
    // Window/display changes carry both a new stage size and a possible new
    // devicePixelRatio (a 100 % → 150 % move), so both are re-measured.
    window.addEventListener("resize", measureStage);

    // A scale change can also arrive without a resize of our own making (the
    // window lands on a differently scaled monitor, or Windows scaling
    // changes). `resolution` reports exactly that; re-arm it for the new value
    // and measure once the frame carrying the new DPR has landed.
    let dprQuery: MediaQueryList | undefined;
    const armDprQuery = () => {
      dprQuery?.removeEventListener("change", onDprChange);
      dprQuery = window.matchMedia(
        `(resolution: ${window.devicePixelRatio || 1}dppx)`
      );
      dprQuery.addEventListener("change", onDprChange);
    };
    const onDprChange = () => {
      armDprQuery();
      requestAnimationFrame(measureStage);
    };
    armDprQuery();

    return () => {
      disposed = true;
      unlisten?.();
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("resize", measureStage);
      dprQuery?.removeEventListener("change", onDprChange);
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
    <div
      class="viewer__stage"
      class:viewer__stage--pannable={pannable}
      class:viewer__stage--dragging={dragging}
      role="group"
      aria-label={t("viewer.canvas")}
      bind:this={stageEl}
      onpointerdown={startPan}
      onpointermove={movePan}
      onpointerup={endPan}
      onpointercancel={endPan}
      onwheel={onWheel}
      ondblclick={onDoubleClick}
    >
      <img
        class="viewer__image"
        src={clipImageUrl(image)}
        alt={t("viewer.imageAlt").replace("{name}", title)}
        width={image.width}
        height={image.height}
        draggable="false"
        style:transform={imageTransform}
        style:width="{image.width}px"
        style:height="{image.height}px"
      />
    </div>
    <footer class="viewer__foot">
      <span class="viewer__meta">{clipImageMeta(image)}</span>
      <div class="viewer__actions">
        <div class="viewer__zoom" role="group" aria-label={t("viewer.zoomControls")}>
          <div class="viewer__stepper">
            <IconButton
              icon="minus"
              label={t("viewer.zoomOut")}
              onclick={() => zoomStep(1 / ZOOM_STEP)}
              disabled={zoom <= ZOOM_MIN + 1e-9}
            />
            <Button
              variant="ghost"
              aria-label={t("viewer.zoomLevelAction").replace(
                "{percent}",
                zoomLabel(zoom)
              )}
              onclick={toggleFromReadout}
            >
              <output aria-live="polite" aria-atomic="true">{zoomLabel(zoom)}</output>
            </Button>
            <IconButton
              icon="plus"
              label={t("viewer.zoomIn")}
              onclick={() => zoomStep(ZOOM_STEP)}
              disabled={zoom >= ZOOM_MAX - 1e-9}
            />
          </div>
          <div class="viewer__presets">
            <Button
              variant="ghost"
              aria-label={t("viewer.fit")}
              onclick={setFit}
            >{t("viewer.fit")}</Button>
            <Button
              variant="ghost"
              aria-label={t("viewer.actualSize")}
              onclick={setActual}
            >100%</Button>
            <Button
              variant="ghost"
              aria-label={t("viewer.resetZoom")}
              onclick={setFit}
            >{t("viewer.reset")}</Button>
          </div>
        </div>
        <Button
          variant="secondary"
          aria-label={t("clips.copyName").replace("{title}", title)}
          onclick={copy}
        >
          <span class="viewer__copy-icon" aria-hidden="true">
            <Icon name={copied ? "check" : "copy"} size={14} />
          </span>
          {copied ? t("common.copied") : t("common.copy")}
        </Button>
      </div>
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
     meta/zoom/copy footer. The stage owns the remaining height so the image
     never pushes the controls out of the window. */
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

  /* The stage clips the panning image. It carries no padding: the fit margin
     is part of the zoom math (`FIT_MARGIN_PX`) so the image can use every
     pixel the stage has at 100 % and while panned. */
  .viewer__stage {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    background: var(--bg-sunken);
    user-select: none;
    touch-action: none;
  }

  .viewer__stage--pannable {
    cursor: grab;
  }

  .viewer__stage--dragging {
    cursor: grabbing;
  }

  .viewer__stage--message {
    padding: var(--space-5);
    background: var(--bg-page);
  }

  .viewer__image {
    position: absolute;
    top: 0;
    left: 0;
    max-width: none;
    max-height: none;
    transform-origin: 0 0;
    object-fit: contain;
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-dialog);
    will-change: transform;
  }

  /* Motion is an enhancement: without it (reduced motion, or the app's
     Animation switch) every zoom still lands, just without the in-between. */
  @media (prefers-reduced-motion: no-preference) {
    .viewer__image {
      transition: transform var(--dur-fast) var(--ease-out);
    }
  }

  /* A drag must track the pointer exactly, so it never animates. */
  .viewer__stage--dragging .viewer__image {
    transition: none;
  }

  .viewer__message {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .viewer__foot {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--space-2) var(--space-3);
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

  .viewer__actions {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--space-2);
    margin-left: auto;
    min-width: 0;
  }

  .viewer__zoom {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }

  .viewer__stepper,
  .viewer__presets {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    min-width: 0;
  }

  /* The window's floor is 480x360 physical — 320 CSS wide at 150 % scaling —
     which cannot hold the whole toolbar on one line. The file facts yield
     first; then the zoom group folds into its two semantic rows (stepper,
     then the fit / 100 % / reset presets) with Copy centred beside them. The
     image keeps whatever height is left, and nothing ever clips. */
  @media (max-width: 460px) {
    .viewer__meta {
      display: none;
    }

    .viewer__foot {
      padding-inline: var(--space-2);
    }

    .viewer__actions {
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto;
      align-items: center;
      margin-left: 0;
    }

    .viewer__zoom {
      flex-direction: column;
      align-items: flex-start;
    }

    .viewer__stepper,
    .viewer__presets {
      flex-wrap: wrap;
    }

    /* Copy keeps its word (clearer than a glyph) and drops the decorative
       icon, which buys the presets row the width it needs. */
    .viewer__copy-icon {
      display: none;
    }
  }

  .viewer__error {
    margin: 0;
    padding: var(--space-2) var(--space-4);
    border-top: 1px solid var(--border);
    font-size: var(--text-xs);
    color: var(--danger-text);
  }
</style>
