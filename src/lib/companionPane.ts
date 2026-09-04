export type CompanionFrameRect = Pick<DOMRect, "left" | "top" | "width" | "height">;

const COMPANION_COMFORTABLE_LAYOUT_WIDTH = 320;
const COMPANION_MIN_ZOOM = 0.7;

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
