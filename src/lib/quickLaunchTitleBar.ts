/** Value for the Quick Launch title bar's `data-tauri-drag-region`
 *  attribute, per Tauri's injected drag script: `"deep"` lets a press
 *  anywhere in the bar start dragging while the window floats. While docked
 *  (ADR-0011) the bar is a Win32 AppBar — no OS mechanism ever moves an
 *  appbar, motion is always the app's own — so `"false"` blocks the drag
 *  (and double-click maximize) outright instead of walking the strip out of
 *  its reserved edge slot. */
export function titleBarDragRegion(docked: boolean): "deep" | "false" {
  return docked ? "false" : "deep";
}
