/** Clip viewer zoom/pan math.
 *
 *  One pure model behind the viewer's mouse and keyboard controls. The zoom
 *  value is a physical-pixel ratio: 100 % means one image pixel per physical
 *  display pixel, so the CSS scale it produces is `zoom / devicePixelRatio`.
 *  Pan is measured in CSS pixels from the viewport centre, which keeps the
 *  anchor math free of scroll offsets and element geometry.
 *
 *  The page owns events and rendering; this module owns clamping, fitting,
 *  cursor-anchored zoom, and pan bounds, so the behaviors are testable without
 *  a window (spec 221 §B.5).
 */

/** Interactive zoom range, as a physical-pixel ratio. */
export const ZOOM_MIN = 0.25;
export const ZOOM_MAX = 8;

/** One +/− step: multiplicative so large images and small ones both feel ok. */
export const ZOOM_STEP = 1.25;

/** Breathing room between the fitted image and the stage edges, in CSS px.
 *  The stage itself carries no padding so these margins are the only inset. */
export const FIT_MARGIN_PX = 16;

/** Wheel zoom sensitivity: `exp(-deltaY * rate)` per pixel of wheel travel.
 *  A 120-unit Windows notch moves one step of roughly 16 %. */
export const WHEEL_ZOOM_RATE = 0.0015;

/** Arrow-key pan distance in CSS px. */
export const KEY_PAN_PX = 48;

export interface ImageSize {
  width: number;
  height: number;
}

export interface Viewport {
  /** Stage width in CSS px. */
  width: number;
  /** Stage height in CSS px. */
  height: number;
  /** `window.devicePixelRatio`; values below 1 are treated as 1. */
  dpr: number;
}

export interface ZoomState {
  zoom: number;
  panX: number;
  panY: number;
}

/** A point in CSS px relative to the viewport centre. */
export interface Anchor {
  x: number;
  y: number;
}

function safeDpr(dpr: number): number {
  return Number.isFinite(dpr) && dpr > 0 ? dpr : 1;
}

function clamp(value: number, min: number, max: number): number {
  const clamped = Math.min(max, Math.max(min, value));
  // Normalize -0 so a pinned axis compares equal to its centred state.
  return clamped === 0 ? 0 : clamped;
}

/** Clamps a zoom value into the supported range. NaN reads as 100 % so a
 *  broken number never freezes the viewer between steps. */
export function clampZoom(zoom: number): number {
  if (Number.isNaN(zoom)) return 1;
  return clamp(zoom, ZOOM_MIN, ZOOM_MAX);
}

/** The CSS scale for a zoom value: 100 % at a 1.5 DPR display renders at
 *  two-thirds CSS size, which is one image pixel per physical pixel. */
export function cssScale(zoom: number, dpr: number): number {
  return clampZoom(zoom) / safeDpr(dpr);
}

/** The image's rendered size in CSS px at `zoom`. */
export function displayedSize(
  image: ImageSize,
  zoom: number,
  dpr: number
): ImageSize {
  const scale = cssScale(zoom, dpr);
  return { width: image.width * scale, height: image.height * scale };
}

/** Contain-with-margin: the largest zoom that keeps the whole image inside
 *  the stage minus `margin` on every side. The result is clamped to the zoom
 *  range, so an image too large for the 25 % floor at this DPI shows clipped
 *  rather than at an unsupported scale — the range is the contract. */
export function fitZoom(
  image: ImageSize,
  viewport: Viewport,
  margin: number = FIT_MARGIN_PX
): number {
  const availableWidth = viewport.width - margin * 2;
  const availableHeight = viewport.height - margin * 2;
  if (
    image.width <= 0 ||
    image.height <= 0 ||
    availableWidth <= 0 ||
    availableHeight <= 0
  ) {
    return ZOOM_MIN;
  }
  const dpr = safeDpr(viewport.dpr);
  const contain = Math.min(
    (availableWidth * dpr) / image.width,
    (availableHeight * dpr) / image.height
  );
  return clampZoom(contain);
}

/** How far the image may travel from centre on each axis; zero means the
 *  axis is already fully visible and cannot pan. */
export function panBounds(
  image: ImageSize,
  zoom: number,
  viewport: Viewport
): { x: number; y: number } {
  const size = displayedSize(image, zoom, viewport.dpr);
  return {
    x: Math.max(0, (size.width - viewport.width) / 2),
    y: Math.max(0, (size.height - viewport.height) / 2),
  };
}

/** Clamps a state's pan into the visible image so an edge never detaches.
 *  An axis smaller than the viewport is pinned to centre. */
export function clampPan(
  state: ZoomState,
  image: ImageSize,
  viewport: Viewport
): ZoomState {
  const bounds = panBounds(image, state.zoom, viewport);
  return {
    zoom: clampZoom(state.zoom),
    panX: clamp(state.panX, -bounds.x, bounds.x),
    panY: clamp(state.panY, -bounds.y, bounds.y),
  };
}

/** Fit and centre — the state behind Fit, `0`, and Reset/Ctrl+0. */
export function fitState(
  image: ImageSize,
  viewport: Viewport,
  margin: number = FIT_MARGIN_PX
): ZoomState {
  return { zoom: fitZoom(image, viewport, margin), panX: 0, panY: 0 };
}

/** Reset is fit-and-centre; the name is kept separate because the control and
 *  the shortcut promise a fresh view, not just a scale change. */
export function resetState(
  image: ImageSize,
  viewport: Viewport,
  margin: number = FIT_MARGIN_PX
): ZoomState {
  return fitState(image, viewport, margin);
}

/** Changes zoom while keeping the image point under `anchor` stationary.
 *  `pan' = anchor - (anchor - pan) * (zoom' / zoom)`, then bounds apply. */
export function zoomAt(
  state: ZoomState,
  requestedZoom: number,
  anchor: Anchor,
  image: ImageSize,
  viewport: Viewport
): ZoomState {
  const zoom = clampZoom(requestedZoom);
  const ratio = state.zoom > 0 ? zoom / state.zoom : 1;
  return clampPan(
    {
      zoom,
      panX: anchor.x - (anchor.x - state.panX) * ratio,
      panY: anchor.y - (anchor.y - state.panY) * ratio,
    },
    image,
    viewport
  );
}

export function panBy(
  state: ZoomState,
  dx: number,
  dy: number,
  image: ImageSize,
  viewport: Viewport
): ZoomState {
  return clampPan(
    { zoom: state.zoom, panX: state.panX + dx, panY: state.panY + dy },
    image,
    viewport
  );
}

/** True when any axis overflows, i.e. drag/arrow panning can do anything. */
export function canPan(
  image: ImageSize,
  zoom: number,
  viewport: Viewport
): boolean {
  const bounds = panBounds(image, zoom, viewport);
  return bounds.x > 0.5 || bounds.y > 0.5;
}

/** Double-click and percentage-button toggle: 100 % at the cursor when not
 *  already actual size, otherwise back to fit and centre. */
export function toggleActualSize(
  state: ZoomState,
  anchor: Anchor,
  image: ImageSize,
  viewport: Viewport
): ZoomState {
  return Math.abs(state.zoom - 1) < 0.001
    ? fitState(image, viewport)
    : zoomAt(state, 1, anchor, image, viewport);
}

/** The multiplier for one ctrl+wheel event; scrolling down zooms out. */
export function wheelZoomFactor(deltaY: number): number {
  if (!Number.isFinite(deltaY)) return 1;
  return Math.exp(-deltaY * WHEEL_ZOOM_RATE);
}

/** Normalizes `WheelEvent.deltaMode` to CSS pixels so mice, trackpads, and
 *  line-mode events move the view by comparable amounts. */
export function wheelPixels(
  delta: number,
  deltaMode: number,
  pageSize: number
): number {
  if (!Number.isFinite(delta)) return 0;
  if (deltaMode === 1) return delta * 16;
  if (deltaMode === 2) return delta * pageSize;
  return delta;
}

/** Rounded percentage for the readout and its announcement. */
export function zoomPercent(zoom: number): number {
  return Math.round(clampZoom(zoom) * 100);
}

export function zoomLabel(zoom: number): string {
  return `${zoomPercent(zoom)}%`;
}

/** Whether a zoom value is the current fit scale (used to re-fit on resize
 *  only while the user has not taken manual control). */
export function isFit(
  zoom: number,
  image: ImageSize,
  viewport: Viewport,
  margin: number = FIT_MARGIN_PX
): boolean {
  return Math.abs(zoom - fitZoom(image, viewport, margin)) < 0.001;
}
