export type CompanionFrameRect = Pick<DOMRect, "left" | "top" | "width" | "height">;

const COMPANION_COMFORTABLE_LAYOUT_WIDTH = 320;
const COMPANION_MIN_ZOOM = 0.7;

/** Explicit user zoom bounds as factors: 50–200%. */
export const COMPANION_USER_ZOOM_MIN = 0.5;
export const COMPANION_USER_ZOOM_MAX = 2.0;
/** One `-`/`+` step: ten percentage points. */
export const COMPANION_USER_ZOOM_STEP = 0.1;

export function companionWebviewBounds(frame: CompanionFrameRect) {
  return {
    x: Math.round(frame.left),
    y: Math.round(frame.top),
    width: Math.max(1, Math.round(frame.width)),
    height: Math.max(1, Math.round(frame.height)),
  };
}

/**
 * Gives responsive sites enough effective CSS width to avoid their most
 * compressed layouts while keeping embedded text comfortably legible.
 */
export function companionZoomForWidth(width: number): number {
  return Math.min(
    1,
    Math.max(COMPANION_MIN_ZOOM, width / COMPANION_COMFORTABLE_LAYOUT_WIDTH),
  );
}

/** Clamps an explicit zoom into 50–200%; non-finite reads as auto (null). */
export function clampCompanionUserZoom(zoom: unknown): number | null {
  if (typeof zoom !== "number" || !Number.isFinite(zoom)) return null;
  return Math.min(COMPANION_USER_ZOOM_MAX, Math.max(COMPANION_USER_ZOOM_MIN, zoom));
}

/** The zoom the WebView actually uses: the explicit per-site choice when set,
 *  otherwise today's width-derived auto zoom. Never touches layout bounds —
 *  zoom and the height splitter stay independent. */
export function companionEffectiveZoom(autoZoom: number, userZoom: number | null): number {
  return userZoom ?? autoZoom;
}

/** One `-`/`+` step from the current effective zoom, clamped into 50–200%. */
export function stepCompanionUserZoom(
  effectiveZoom: number,
  direction: -1 | 1,
): number {
  const base = Number.isFinite(effectiveZoom) ? effectiveZoom : 1;
  const stepped = Math.round((base + direction * COMPANION_USER_ZOOM_STEP) * 100) / 100;
  return Math.min(COMPANION_USER_ZOOM_MAX, Math.max(COMPANION_USER_ZOOM_MIN, stepped));
}

/** Renders a zoom factor as a whole percent (`1` → `"100%"`). */
export function formatCompanionZoomPct(zoom: number): string {
  return `${Math.round(zoom * 100)}%`;
}
