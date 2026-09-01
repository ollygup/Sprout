# 123 — dock header minimal: wordmark → mark only

**What to build:** The Quick Launch window/dock header stops wasting 60px on the wordmark at 340px.

**Blocked by:** none.

**Status:** ready-for-agent

## Scope

- File: `src/routes/quick-launch-window/+page.svelte:448` `header.qlw__bar`.
- When `dock.docked == true`: remove `<h1 class="qlw__title">Quick Launch</h1>` (keep `SproutMark size=16` `+page.svelte:452` as `data-tauri-drag-region={titleBarDragRegion(dock.docked)}` `src/lib/quickLaunchTitleBar.ts`). Edge arrows + `dock-left`/`dock-right` hint + `IconButton undock` + `x` close stay. Floating window (`dock.docked==false`) keeps the text for discoverability — one-header caveat per `0005-page-chrome-consistency.md:1`.
- Reuse `src/lib/styles/tokens.css` spacing, `src/lib/components/IconButton.svelte` quiet variant; no ad-hoc colors/radii per AGENTS.md design rule.
- Cite `0006-notion-design-patterns.md:5` relocating familiar controls has cost — mark keeps brand scent vs icon-only tab.

## ACs

- [ ] Docked header at 340px physical shows mark (16px) + dock-edge arrows + undock + close with no flex wrap / no tab degradation triggered by header width alone (verify `Tabs.svelte:178` `display:flex` `hug-left`).
- [ ] Header drag still moves the window when docked/floating (drag region on header+mark).
- [ ] Floating palette still shows `Quick Launch` text.
- [ ] `npm.cmd run check` 0 errors.

## Verification

- `npm.cmd run check`
- Visual: dock on left/right, resize to 340px, screenshot before/after; tab labels stay `full` with 2 tabs.

