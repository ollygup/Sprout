# 229 — Nested groups backend

**Parent:** [221 — Collections & media (spec)](221-collections-media-spec.md)

**What to build:** Groups gain two levels in the data layer. A top-level group may hold items and child groups; a child holds only items. Names are unique among siblings, positions are sibling-scoped, moves reparent/reorder in one transaction, empty groups dissolve bottom-up, and deleting a parent ungroups its whole subtree without deleting an item.

**Blocked by:** None — can start immediately. Schedule after 225 (shared `groups.rs`/`db.rs` schema file); do not run concurrently with 225.

**Status:** ready-for-agent

- [ ] A migration adds `groups.parent_id` (nullable self-reference). Existing groups become top-level; no data is lost.
- [ ] `Group` carries `parent_id: number | null`; `list_groups` returns the flat tree in deterministic sibling order. Positions are scoped to `(collection, parent_id)`.
- [ ] Create defaults to top level. Rename and create validate names against the destination sibling set, trimmed and case-insensitive. Two different parents may use the same child name.
- [ ] `move_group(id, parent_id, to_position)` validates: parent exists, same collection, is top-level; the moved group is not itself; the moved group has no children if it becomes a child; no depth-3/cycle. A destination name collision is refused with a readable message.
- [ ] A move renumbers the old and new sibling sets in one transaction; moving to the same parent is a reorder; moving to `null` unnests.
- [ ] A group exists while it has a direct item or a child group. `sweep_empty_groups` deletes child groups with no items first, then parents with no items and no remaining children, in one transaction; natural emptying cascades upward.
- [ ] Explicit delete of a child ungroups its items and deletes it. Explicit delete of a parent ungroups every item in its subtree (direct and descendant), deletes its child groups, then deletes the parent; no item is deleted. Sibling positions compact at both levels.
- [ ] Cross-collection assignment/parenting stays impossible at the data layer; Groups off remains fully dormant; existing flat groups and their order are unchanged.
- [ ] Backend tests cover create/rename collisions, two parents sharing a name, reparent/unnest, depth-3/cycle/cross-collection refusal, bottom-up dissolve, parent/child delete, and position compaction.
- [ ] `cargo test`, `cargo check`, and `node tools/ownership-gate.mjs` pass.

**Claims (recheck at dispatch):** `groups.rs`; `db.rs` migration; group API commands; `Group` type. **Explicitly not built:** main-app or dock rendering (230/231); drag (232/233); adding groups to the backup document; color changes; a third level.
