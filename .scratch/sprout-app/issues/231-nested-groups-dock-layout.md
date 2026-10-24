# 231 — Nested groups dock layout

**Parent:** [221 — Collections & media (spec)](221-collections-media-spec.md)

**What to build:** The dock renders two-level groups correctly at its real content width. At 420 CSS px or wider, child sub-headers are indented and always expanded, and the parent chevron collapses the whole subtree. Below 420, the subtree flattens into one list with no child sub-headers. The same pass fixes the current narrow-width group overflow: labels truncate, count badges stay visible, and rows stay inside the strip.

**Blocked by:** 230 — Nested groups main-app UI.

**Status:** ready-for-agent

- [ ] The layout mode is driven by the tab panel's **content width** via a container query, not the window width or a new size constant.
- [ ] At ≥ 420 CSS px: parent header, direct items, then each child sub-header indented and always expanded. The parent chevron collapses the entire subtree; child groups have no chevron.
- [ ] Below 420: one flattened list — parent header, then all items in tree order (parent direct first, then each child's items in child order), with no child sub-headers.
- [ ] A parent with visible descendants but no direct items still renders; a parent whose whole subtree is hidden drops; a parent with no visible descendants drops.
- [ ] Labels truncate, count badges stay visible, and no row overflows at 340 px content width.
- [ ] The dock has no group menus and no color editing; the read-only surface stays read-only.
- [ ] Pure tests cover the mode decision at 419/420/421 CSS px and the flatten/nest ordering. Manual evidence covers 340/420/800 px, fixed/auto-hide/bezel, 100/150/200 % DPI, light/dark, and a parent with only child items.
- [ ] `npm run check`, `npm run test`, `node tools/contrast-check.mjs`, and `node tools/ownership-gate.mjs` pass.

**Claims (recheck at dispatch):** the dock page; the shared accordion's flush/nested presentation; a pure layout-mode function. **Explicitly not built:** a new dock window or size constant; changes to dock modes/width formulas; drag; group menus/color editing in the dock.
