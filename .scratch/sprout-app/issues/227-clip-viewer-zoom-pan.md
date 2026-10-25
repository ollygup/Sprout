# 227 — Clip viewer zoom and pan

**Parent:** [221 — Collections & media (spec)](221-collections-media-spec.md)

**What to build:** The clip viewer becomes a real image inspector: 25–800 % zoom, fit/100 %/reset shortcuts, Ctrl+wheel zoom at the cursor, +/− controls, drag-pan, double-click fit ↔ 100 %, wheel/Shift+wheel panning, and arrow-key panning. 100 % means one image pixel per physical display pixel. The zoom readout and controls are keyboard- and screen-reader-accessible.

**Blocked by:** 226 — Clip viewer shell.

**Status:** complete - 25–800 % zoom, fit/100 %/reset, cursor-anchored Ctrl+wheel, +/−/keyboard, drag/wheel/arrow panning, accessible controls, and DPI-aware 100 % landed; gates and CDP evidence recorded below.

- [x] Zoom is clamped to 25–800 %. Fit is contain-with-margin; 100 % is one image pixel per physical display pixel (device-pixel-ratio aware); reset returns to fit and centers.
- [x] Ctrl+wheel zooms at the cursor; `+`/`−` zoom at the viewport center; `0` fits; `1` goes to 100 %; Ctrl+0 resets; double-click toggles fit ↔ 100 % at the cursor.
- [x] Drag-pan uses pointer capture when the image is larger than the viewport; plain wheel pans vertically; Shift+wheel pans horizontally; arrow keys pan when zoomed.
- [x] The zoom percentage is announced; −/percentage/+/Fit/100 %/Reset are labeled, focusable buttons with visible focus.
- [x] Reduced-motion mode keeps the controls functional without animated zoom transitions.
- [x] Pure tests cover clamp, fit, 100 %/DPR, reset, cursor-anchored zoom, and pan bounds. Manual evidence covers mouse wheel, trackpad, keyboard, and 100/150/200 % DPI.
- [x] The viewer still closes on Esc, copies correctly, and remains not always-on-top.
- [x] `cargo test`, `cargo check`, `npm run check`, `node tools/contrast-check.mjs`, and `node tools/ownership-gate.mjs` pass.

**Claims (recheck at dispatch):** viewer route; a new pure zoom/pan module; i18n. **Explicitly not built:** rotation, annotation, slideshow, next/previous navigation, a second window, always-on-top, or changing the shell's open/copy behavior.

## Evidence - 2026-10-24

**Backend.** None — this ticket is frontend-only; `cargo test` / `cargo check` are confirmation gates.

**Frontend.** `src/lib/clipViewerZoom.ts` (new) is the whole zoom/pan model, pure and window-free: `ZOOM_MIN` 0.25 / `ZOOM_MAX` 8 (physical-pixel ratio, so 100 % = one image pixel per physical display pixel), `ZOOM_STEP` 1.25, `FIT_MARGIN_PX` 16, `WHEEL_ZOOM_RATE` 0.0015, `KEY_PAN_PX` 48, plus `clampZoom`, `fitState`, `resetState`, `cssScale`, `displayedSize`, `panBounds`, `clampPan`, `canPan`, `zoomAt`, `panBy`, `toggleActualSize`, `wheelZoomFactor`, `wheelPixels`, `zoomPercent` / `zoomLabel`, `isFit`. The CSS scale is `zoom / dpr`; pan is measured in CSS px from the viewport centre, so anchor math never touches scroll offsets or element geometry. Fit is contain-with-margin and clamps into the range (the 25 % floor wins for huge images); `resetState` returns to fit, centred; `clampZoom` maps NaN to 100 % so a broken number can never freeze the viewer between steps.

`src/routes/clip-viewer/+page.svelte` renders the image by `translate(...) scale(...)` on an absolutely-positioned stage instead of by scrolling — that is what makes cursor-anchored zoom and bounded panning possible without layout thrash. Events: `onwheel` (Ctrl/meta → `zoomAt` at the cursor with `wheelZoomFactor`; plain → vertical pan; Shift → horizontal pan), `onpointerdown/move/up/cancel` drag with `setPointerCapture` gated on `canPan` (only when the image overflows), `ondblclick` → `toggleActualSize` at the cursor, and `svelte:window onkeydown` for `+`/`=` , `-`/`_`, `0`, `1`, `Ctrl+0`, arrows, and Esc. The footer group is `−`, the percentage, `+`, Fit, 100 %, Reset, all focusable with labels (`viewer.zoomOut`, `viewer.zoomLevelAction`, `viewer.zoomIn`, `viewer.fit`, `viewer.actualSize`, `viewer.resetZoom`). A fitted view re-fits on resize; a manual zoom keeps its scale and only pulls the pan back inside bounds. `measureStage` re-reads `devicePixelRatio` on `resize` **and** on the `(resolution: Ndppx)` media query (re-armed per value) — the CDP matrix below proved a scale change can arrive without a resize the page asked for, and without that watch the first 150 % → 100 % transition rendered 100 % at the old DPR. Motion is `transition: transform var(--dur-fast)` only under `prefers-reduced-motion: no-preference`; `tokens.css` already collapses it to 0.01 ms for reduced motion and for the app's Animation switch, so every control still lands instantly. Nine `viewer.*` keys were added to both locales (1221 keys each, parity kept); the unused `viewer.zoomLevel` was dropped.

**Decisions applied.** ADR-0021 (one size source): no new window dimensions — the stage is measured, never hard-coded. ADR-0026 (machine-local boundary): zoom/pan are session state only; nothing new is persisted, so exports and backups are untouched. ADR-0029 (one owner per Windows command): the viewer still opens/copies/closes through the existing `clip_viewer` commands; no second window or clipboard path was added. The percentage control is a `Button` wrapping the live `<output aria-live="polite">` — the ticket asks for the percentage to be an announced, focusable button, and making it the keyboard equivalent of double-click (fit ↔ 100 % at the centre) satisfies both halves with one control.

**Automated gates (all in `C:\Sprout`).**

- `cargo test` — 722 passed, 3 ignored, 0 failed (frontend-only change; confirmation gate).
- `cargo check` — pass in 7.1 s (run after `cargo test` so it reuses those artifacts; see `docs/agents/toolchain.md`).
- `npm.cmd run check` — 0 errors, 3 pre-existing warnings (`QuickActionFormDialog.svelte`, `UnifiedHeader.svelte`).
- `npm.cmd test` — 497 passed across 40 files. New: `clipViewerZoom.test.ts` (28 pure tests: clamp, fit + margin, 100 %/DPR, reset, cursor-anchored zoom, pan bounds, wheel/trackpad factors, toggle) and `clipViewerZoom.close.test.ts` (12 component tests: labels + live readout, step/fit/100 %/reset, 25–800 % clamp, announced percentage, percentage-button toggle, `+ − 0 1 Ctrl+0`, arrows-only-when-overflowing, Ctrl+wheel anchor, wheel/Shift+wheel pan, double-click, physical-pixel 100 %, re-fit on resize).
- `node tools/contrast-check.mjs` — all checks pass in both modes.
- `node tools/ownership-gate.mjs` — pass (62 owned references).

**Manual/native evidence.** `tools/repro-clip-viewer.mjs` extended with the zoom surface; **VERDICT: GREEN on all 31 checks** against a throwaway `%LOCALAPPDATA%`. `report.json` and screenshots (`viewer-main.png`, `main-row.png`, `viewer-copied.png`, `viewer-dock.png`, `dock-row.png`, `viewer-missing.png`) live in `C:\Users\admin\AppData\Local\Temp\opencode\evidence-227\`. Measured on the single 2560×1920 display at its real 150 % DPI, viewer content 640×480 CSS:

- Controls: `readout=27% live=polite`, labels `Close viewer | Zoom out | Zoom level 27 %. Toggle fit and 100 %. | Zoom in | Fit | Actual size (100 %) | Reset zoom | Copy Viewer probe to the clipboard`.
- Opens fitted: 27 % inside 640×480 CSS at dpr 1.5; `1` → 27 % → 100 %; 100 % is `1600 CSS × 1.5 = 2400 physical` for the 2400 px image.
- Mouse wheel: Ctrl+wheel at an off-centre cursor went 27 % → 120 % and held the anchored image fraction `0.440 → 0.440`; plain wheel panned vertically `top −374.3 → −494.3`; Shift+wheel panned horizontally `left −480.0 → −600.0`.
- Trackpad: a fine-grained Ctrl+wheel (deltaY −20, the pinch-sized notch) stepped 27 % → 28 % — small deltas do not jump a whole step.
- Keyboard: arrows panned `left −600.0 → −648.0`; `Ctrl+0` reset to 27 % and centred `320.0 vs 320.0`; Esc still closed the viewer.
- Drag: pointer drag moved the image `left −648.0 → −708.0`, `top −494.3 → −534.3`.
- Double-click toggled `27 % → 100 % → 27 %`; the range clamped at `25 % … 800 %`.
- Reduced motion: transition computed `1e-05s (0.010 ms)` while Zoom in still stepped `27 % → 34 %`.
- DPI matrix (physical window held constant, so CSS size and DPR change together as Windows does): 100 % → 2400.0 physical at 100 % (`2400 CSS @ 960×720`), 150 % (`1600 CSS @ 640×480`), and 200 % (`1200 CSS @ 480×360`) DPI.
- Shell behaviors intact: `topmost=false` (rect `789,557 982×733`), Copy flashes "Copied" and announces "Viewer probe copied.", Esc closes and the row menu reopens at the remembered `678×487.5` physical, the close control returns focus (`document.hasFocus()=true`, foreground `Sprout`), dock right-click retargets the one viewer, and a deleted Clip still shows the friendly empty state.

**Notes.** Two harness assertions were wrong, not the app: Chrome serializes 0.01 ms as `1e-05s`, so the reduced-motion check now parses the duration (≤ 0.001 s) instead of matching a literal; and the DPI matrix now emulates a real scaling change (constant physical size → new CSS size *and* new DPR) instead of moving only `deviceScaleFactor`, which fires no resize and proves nothing. The app-side `(resolution: Ndppx)` watch that the corrected matrix demanded is a genuine fix and stays. Not built here, as scoped: rotation, annotation, slideshow, next/previous navigation, a second window, always-on-top, and any change to the shell's open/copy behavior.

### Follow-up - 2026-10-24: narrow-window footer

Shrinking the viewer to its floor (480×360 physical = 320 CSS at 150 % scaling) clipped the toolbar: "Reset" ran past the window edge and Copy was pushed to a line of its own. The zoom group was a single non-wrapping row whose min-content width (≈ 330 CSS) exceeded the window.

First attempt — `flex-wrap: wrap` on the group — removed the clip but measured **rows=4, footer=160 px, stage=34 px**, i.e. a wall of buttons and almost no image. That is not a fix.

Final layout, structural rather than cosmetic: the group is split into two semantic rows inside the same labelled `role="group"` — `.viewer__stepper` (− / % / +) and `.viewer__presets` (Fit / 100 % / Reset). Above 460 CSS they sit on one line with Copy last, exactly as before. At ≤ 460 CSS the footer: hides the file facts (`.viewer__meta`), tightens its inline padding to `--space-2`, makes `.viewer__actions` a two-column grid (zoom group, then Copy centred beside it), stacks the zoom group into its two rows, and drops Copy's decorative icon while keeping its word.

Measured at 320 CSS: **rows=2, footer=93 px, stage=100 px**, `overflow=0`, `clipped=none`, `presets=196 px` inside a 229 px column (33 px of slack, so a font or locale difference does not tip it back to wrapping). The wide layout is unchanged (`viewer-main.png`); the folded layout is `viewer-narrow.png`.

Design rationale: 0006 pattern 2 (minimal-until-content — the secondary file facts yield before any control does) and 0005 rule 5 (same-kind controls keep one shared treatment; the presets are still the shared ghost `Button`, the stepper the shared `IconButton`). No new tokens, components, or one-off colours were introduced.

New harness check, kept as a regression: `narrow window folds the toolbar to two rows and keeps the image` — at 320 CSS it asserts nothing is clipped, the footer does not overflow, the zoom controls occupy at most two rows, and the stage keeps ≥ 80 px. Full run after the change: **VERDICT: GREEN** (32 checks).
