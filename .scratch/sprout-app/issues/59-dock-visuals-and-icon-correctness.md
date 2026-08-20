# 59 — Dock visuals + icon correctness

**What to build:** The dock chrome gets meaningful, direction-aware icons and consistent spacing on both sides of the docked strip regardless of the docked edge. The dock toggle no longer reads as a pause button. Parent spec: 55.

**Blocked by:** 55 — Quick Launch window: dock, floating UX, live sync, and Quick Action control (spec)

**Status:** done

- [x] New `dock-left` and `dock-right` icons in `Icon.svelte`, drawn in the design-system stroke style (24×24 viewBox, stroke-width 1.7, `currentColor`) — the icons8 Left/Right Docking concepts are visual references only, not embedded assets
- [x] The docked hint in the window header shows the current edge's icon; the dock/undock toggle shows the target edge's icon (or the undock glyph while docked), so the chrome always tells the truth
- [x] Icon sweep of the Quick Launch window: all remaining icons (x, play, chevron-left/right, rocket, terminal, info) verified semantically correct; the unused `layout` icon is removed from `Icon.svelte`
- [x] Docked-edge CSS classes (`qlw--docked-left` / `qlw--docked-right`) mirror the header padding so both sides of the strip have the same spacing on either edge; verified by docking left and right — no gap asymmetry, no overlap with the neighboring application's space
- [x] `npm run check` 0 errors; synced to the share