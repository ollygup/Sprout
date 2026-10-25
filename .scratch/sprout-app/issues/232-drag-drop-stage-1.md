# 232 — Drag & drop stage 1: cards and top-level groups

**Parent:** [221 — Collections & media (spec)](221-collections-media-spec.md)

**What to build:** A Notion-style six-dot grip appears on hover/focus on cards and top-level group headers in all three main-app collections. Dragging a card reorders it within or moves it between groups/ungrouped; dragging a top-level group reorders it. The drop is saved atomically, Esc cancels, and the ⋯ menu remains the complete keyboard path. No dock drag, no drag while filtering, and no child-group drag in this stage.

**Blocked by:** 230 — Nested groups main-app UI.

**Status:** done — amended 2026-10-24 (grip grab area); manual pointer/DPI pass still pending a human

- [x] A six-dot grip appears on hover/focus on cards and top-level group headers; only the grip starts a drag, and the rest of the card/header stays clickable.
- [x] The grip is a focusable button with an accessible name/instructions; the ⋯ menu remains the complete keyboard path (Move up/down, Move to group, Move into…).
- [x] A shared pointer-events drag controller serves all three collections: 4 px start threshold, ghost/insertion line, edge auto-scroll, Esc cancel, cleanup on drop/cancel, reduced-motion support, 100–200 % DPI and pen pointers.
- [x] Cards drag within and between sections; the drop resolves a target group (including ungrouped) and a global list position, then one transactional `place_item(collection, item_id, group_id, to_position)` assigns and repositions.
- [x] Card order within a group follows the global item position; Start-all continues to follow the global list order.
- [x] Top-level groups reorder by dragging their grip through the existing group move contract; child groups are not draggable in this stage.
- [x] Cross-collection drops, drags while a filter is active, and dock drags are refused. Invalid targets show a not-allowed cursor and a short reason.
- [x] A failed `place_item` reloads the page and surfaces the existing error line; no half-applied move is left behind.
- [x] Pure tests cover drop-target resolution (within/between/ungrouped/reorder/invalid); backend tests cover `place_item` atomicity and rollback. Manual evidence covers all three collections, grouped/ungrouped, and keyboard parity. *(Pure and backend tests green; manual pointer pass not yet performed.)*
- [x] `cargo test`, `cargo check`, `npm run check`, `npm run test`, and `node tools/ownership-gate.mjs` pass.

**Claims (recheck at dispatch):** a new shared `collectionDrag` controller; the three main pages; card/group headers; `ordered_list.rs` transaction-aware `place_item`; API. **Explicitly not built:** nested reparent/unnest (233); dock drag; a keyboard "drag mode"; touch-specific gestures beyond pointer events; reordering while filtering.

**Verification notes (2026-10-24):** `src/lib/collectionDrag.svelte.ts` is the one pointer-events controller (4 px start threshold, 18 px max ghost step, 40 px edge auto-scroll, Esc/pointercancel cleanup, `prefers-reduced-motion` handled by the shared animation store); `src/lib/dropTargets.ts` resolves the target purely (within/between/ungrouped/reorder/refusals), pinned by `dropTargets.test.ts` and `dragDrop.close.test.ts`. One transactional `place_item(collection, item_id, group_id, to_position)` in `src-tauri/src/ordered_list.rs` performs assign + reposition; `to_position` is the index in the full list after removing the item, and any failure reloads the page and surfaces the existing error line. Start-all still walks the global list order, not the rendered section order. Cross-collection, filtered, and dock drags are refused with a not-allowed cursor and a short reason. Gates: `cargo test` 735 passed, `cargo check` clean (no Rust change after that run), `npm.cmd run check` 0 errors / 3 pre-existing warnings, `npm.cmd run test` 561 passed (44 files), `node tools/ownership-gate.mjs` pass. **Amendment (user-reported, 2026-10-24):** the grip was hard to grab and vanished as the pointer travelled from the card to the handle. `DragGrip.svelte` is now a full-height 24 px strip at `left: -24px`, always hit-testable (no `pointer-events` gating), with `cursor: grab` and no separate hover background; because it stays a descendant, the host's `:hover` stays lit and the card highlight carries the affordance from card to handle. The three pages' `pointer-events` rules were removed accordingly, and the group header grip was pulled into the group's own gutter (`--group-inset` plus negative-margin vertical padding) so it no longer overlaps the colour bar. **Not yet verified by hand:** the pointer pass across all three collections, grouped/ungrouped, pen input, 100–200 % DPI, and keyboard parity.
