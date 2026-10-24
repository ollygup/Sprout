# 232 — Drag & drop stage 1: cards and top-level groups

**Parent:** [221 — Collections & media (spec)](221-collections-media-spec.md)

**What to build:** A Notion-style six-dot grip appears on hover/focus on cards and top-level group headers in all three main-app collections. Dragging a card reorders it within or moves it between groups/ungrouped; dragging a top-level group reorders it. The drop is saved atomically, Esc cancels, and the ⋯ menu remains the complete keyboard path. No dock drag, no drag while filtering, and no child-group drag in this stage.

**Blocked by:** 230 — Nested groups main-app UI.

**Status:** ready-for-agent

- [ ] A six-dot grip appears on hover/focus on cards and top-level group headers; only the grip starts a drag, and the rest of the card/header stays clickable.
- [ ] The grip is a focusable button with an accessible name/instructions; the ⋯ menu remains the complete keyboard path (Move up/down, Move to group, Move into…).
- [ ] A shared pointer-events drag controller serves all three collections: 4 px start threshold, ghost/insertion line, edge auto-scroll, Esc cancel, cleanup on drop/cancel, reduced-motion support, 100–200 % DPI and pen pointers.
- [ ] Cards drag within and between sections; the drop resolves a target group (including ungrouped) and a global list position, then one transactional `place_item(collection, item_id, group_id, to_position)` assigns and repositions.
- [ ] Card order within a group follows the global item position; Start-all continues to follow the global list order.
- [ ] Top-level groups reorder by dragging their grip through the existing group move contract; child groups are not draggable in this stage.
- [ ] Cross-collection drops, drags while a filter is active, and dock drags are refused. Invalid targets show a not-allowed cursor and a short reason.
- [ ] A failed `place_item` reloads the page and surfaces the existing error line; no half-applied move is left behind.
- [ ] Pure tests cover drop-target resolution (within/between/ungrouped/reorder/invalid); backend tests cover `place_item` atomicity and rollback. Manual evidence covers all three collections, grouped/ungrouped, and keyboard parity.
- [ ] `cargo test`, `cargo check`, `npm run check`, `npm run test`, and `node tools/ownership-gate.mjs` pass.

**Claims (recheck at dispatch):** a new shared `collectionDrag` controller; the three main pages; card/group headers; `ordered_list.rs` transaction-aware `place_item`; API. **Explicitly not built:** nested reparent/unnest (233); dock drag; a keyboard "drag mode"; touch-specific gestures beyond pointer events; reordering while filtering.
