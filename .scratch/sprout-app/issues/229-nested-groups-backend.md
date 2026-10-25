# 229 — Nested groups backend

**Parent:** [221 — Collections & media (spec)](221-collections-media-spec.md)

**What to build:** Groups gain two levels in the data layer. A top-level group may hold items and child groups; a child holds only items. Names are unique among siblings, positions are sibling-scoped, moves reparent/reorder in one transaction, empty groups dissolve bottom-up, and deleting a parent ungroups its whole subtree without deleting an item.

**Blocked by:** None — can start immediately. Schedule after 225 (shared `groups.rs`/`db.rs` schema file); do not run concurrently with 225.

**Status:** complete — two-level groups, sibling-scoped names/positions, one move contract, bottom-up dissolve, and subtree delete landed; gates recorded below.

- [x] A migration adds `groups.parent_id` (nullable self-reference). Existing groups become top-level; no data is lost.
- [x] `Group` carries `parent_id: number | null`; `list_groups` returns the flat tree in deterministic sibling order. Positions are scoped to `(collection, parent_id)`.
- [x] Create defaults to top level. Rename and create validate names against the destination sibling set, trimmed and case-insensitive. Two different parents may use the same child name.
- [x] `move_group(id, parent_id, to_position)` validates: parent exists, same collection, is top-level; the moved group is not itself; the moved group has no children if it becomes a child; no depth-3/cycle. A destination name collision is refused with a readable message.
- [x] A move renumbers the old and new sibling sets in one transaction; moving to the same parent is a reorder; moving to `null` unnests.
- [x] A group exists while it has a direct item or a child group. `sweep_empty_groups` deletes child groups with no items first, then parents with no items and no remaining children, in one transaction; natural emptying cascades upward.
- [x] Explicit delete of a child ungroups its items and deletes it. Explicit delete of a parent ungroups every item in its subtree (direct and descendant), deletes its child groups, then deletes the parent; no item is deleted. Sibling positions compact at both levels.
- [x] Cross-collection assignment/parenting stays impossible at the data layer; Groups off remains fully dormant; existing flat groups and their order are unchanged.
- [x] Backend tests cover create/rename collisions, two parents sharing a name, reparent/unnest, depth-3/cycle/cross-collection refusal, bottom-up dissolve, parent/child delete, and position compaction.
- [x] `cargo test`, `cargo check`, and `node tools/ownership-gate.mjs` pass.

**Claims (recheck at dispatch):** `groups.rs`; `db.rs` migration; group API commands; `Group` type. **Explicitly not built:** main-app or dock rendering (230/231); drag (232/233); adding groups to the backup document; color changes; a third level.

## Evidence - 2026-10-24

**Migration and payload.** Fresh databases create `groups.parent_id INTEGER REFERENCES groups(id) ON DELETE CASCADE`; `db::ensure_group_parent` adds it to older databases with `ALTER TABLE … ADD COLUMN` (the only self-reference SQLite allows on an added column, and it defaults every existing row to `NULL`, i.e. top-level). `Group` carries `Option<i64> parent_id` (`src/lib/types.ts` mirrors it as `number | null`). `ensure_group_parent` is idempotent; `migrates_databases_created_before_group_nesting` proves existing groups stay top level, a child can then be parented, and a re-run changes nothing. No item table is touched by the migration.

**Flat tree.** `list_groups` returns each top-level group immediately followed by its own children, ordered by `COALESCE(parent.position, g.position), (g.parent_id IS NOT NULL), g.position, g.id`; the frontend rebuilds the two levels from `parent_id` (ticket 230 owns the tree UI). `list_groups_returns_the_flat_tree_in_sibling_order` asserts `First, Child B, Child A, Second` after moving both children, with the correct `parent_id` on every row. Positions are sibling-scoped everywhere (`collection = ? AND parent_id IS ?`), so each level numbers from 0 independently.

**Names.** `create_group` is top-level-only and checks the top-level sibling set; `rename_group` checks the group's own sibling set; both trim and lowercase. `group_names_are_exclusive_within_their_collection_case_insensitively` keeps the flat guarantees (padding/case refused, other collection allowed, self excluded) and `child_names_are_sibling_scoped_and_two_parents_may_reuse_a_name` proves two parents may each have a "Work", a third under the same parent collides with a message naming `'Work'`, and a child rename collides only with its own siblings.

**One move contract.** `move_group(id, parent_id, to_position)` (the same signature in the Tauri command and `api.ts`) validates in this order: the group exists; the parent exists; parent and group share a collection; the parent is top level; the group has no children when it becomes a child; the moved group is not its own parent/child; and the destination sibling set has no name collision. The old and new sibling sets are renumbered in one unchecked transaction, with `to_position` clamped; `None` unnests, and a same-parent move is a plain reorder. `move_reparents_unnests_and_renumbers_both_levels` covers reparent, reorder, and unnest with compacted positions; `move_refuses_depth_three_cycles_cross_collection_and_missing_parents` covers every refusal.

**Dissolve and delete.** `sweep_empty_groups` is bottom-up in one transaction: child groups with no items first, then parents with neither a direct item nor a remaining child. Two passes cover the two-level limit, so an emptied child naturally takes its now-childless parent with it; survivor positions are untouched (gaps are harmless because every read orders by position). `delete_group` collects the children, nulls `group_id` for every item in the subtree (explicitly, so the rule holds even where the foreign-keys pragma is off), deletes children then the parent, and compacts only the deleted group's own level. Tests `sweep_dissolves_empty_subgroups_bottom_up`, `deleting_a_parent_ungroups_its_whole_subtree_without_deleting_items`, and `deleting_a_child_ungroups_only_its_items_and_keeps_the_parent` assert item survival, both levels' compaction, and that a parent keeps its own items when only a child goes.

**Boundaries.** `assign_item` still refuses a Group from another collection (unchanged test), `normalize`/`Collection` keeps the three namespaces apart, and no Groups-off read path changed: `update_groups_enabled` and the list/order queries only gain the `parent_id` column, so a flat table (all `NULL`) behaves exactly as before. Backup serialization, colors, dock/main rendering, and drag were not touched, per scope.

**Frontend contract (no UI built here).** `src/lib/types.ts` adds `Group.parent_id`; `src/lib/api.ts` exposes `moveGroup(id, parentId, toPosition)`; `collectionGroups.svelte.ts` passes `group.parent_id` through its reorder call, so an existing flat reorder keeps its old behavior and a nested one has the right parent when 230 lands.

**Automated gates (all in `C:\Sprout`).**

- `cargo test --lib groups` — 24 passed, 0 failed.
- `cargo test` — 732 passed, 3 ignored, 0 failed.
- `cargo check` — pass (pre-existing warnings only).
- `npm.cmd run check` — 0 errors, 3 pre-existing warnings.
- `npm.cmd test` — 497 passed across 40 files.
- `node tools/ownership-gate.mjs` — pass (77 owned references).

**Notes.** The flat payload keeps sibling order deterministic without a recursive query; the UI can group children by `parent_id` without relying on adjacency, but adjacency makes a single pass sufficient. A third level is refused at the API and has no schema support beyond the self-reference, matching "explicitly not built".
