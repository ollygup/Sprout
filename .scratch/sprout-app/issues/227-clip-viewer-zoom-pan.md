# 227 — Clip viewer zoom and pan

**Parent:** [221 — Collections & media (spec)](221-collections-media-spec.md)

**What to build:** The clip viewer becomes a real image inspector: 25–800 % zoom, fit/100 %/reset shortcuts, Ctrl+wheel zoom at the cursor, +/− controls, drag-pan, double-click fit ↔ 100 %, wheel/Shift+wheel panning, and arrow-key panning. 100 % means one image pixel per physical display pixel. The zoom readout and controls are keyboard- and screen-reader-accessible.

**Blocked by:** 226 — Clip viewer shell.

**Status:** ready-for-agent

- [ ] Zoom is clamped to 25–800 %. Fit is contain-with-margin; 100 % is one image pixel per physical display pixel (device-pixel-ratio aware); reset returns to fit and centers.
- [ ] Ctrl+wheel zooms at the cursor; `+`/`−` zoom at the viewport center; `0` fits; `1` goes to 100 %; Ctrl+0 resets; double-click toggles fit ↔ 100 % at the cursor.
- [ ] Drag-pan uses pointer capture when the image is larger than the viewport; plain wheel pans vertically; Shift+wheel pans horizontally; arrow keys pan when zoomed.
- [ ] The zoom percentage is announced; −/percentage/+/Fit/100 %/Reset are labeled, focusable buttons with visible focus.
- [ ] Reduced-motion mode keeps the controls functional without animated zoom transitions.
- [ ] Pure tests cover clamp, fit, 100 %/DPR, reset, cursor-anchored zoom, and pan bounds. Manual evidence covers mouse wheel, trackpad, keyboard, and 100/150/200 % DPI.
- [ ] The viewer still closes on Esc, copies correctly, and remains not always-on-top.
- [ ] `cargo test`, `cargo check`, `npm run check`, `node tools/contrast-check.mjs`, and `node tools/ownership-gate.mjs` pass.

**Claims (recheck at dispatch):** viewer route; a new pure zoom/pan module; i18n. **Explicitly not built:** rotation, annotation, slideshow, next/previous navigation, a second window, always-on-top, or changing the shell's open/copy behavior.
