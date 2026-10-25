# Groups are isolated namespaces that dissolve and sleep

> Status: amended 2026-10-24 — original decision text preserved; see the executable-source audit amendment for current behavior and gaps, and the 2026-10-24 amendment for the two-level tree and per-group colour.

The three machine-local lists (Launch entries, Quick Actions, Clips) share one `groups` table, but namespaces are isolated at the data layer: a group belongs to exactly one collection (`launch` / `action` / `clip`), items hold at most one `group_id`, and cross-collection membership is impossible, not just hidden. A group exists only while at least one member belongs to it — created by assigning an item ("Move to group → New group…"), dissolved automatically when its last member leaves, with explicit delete returning members to ungrouped instead. Each collection's Groups feature is opt-in (default off); off is fully dormant — lists render flat and no group affordance appears anywhere — while stored groups and memberships survive untouched, so re-enabling restores them.

## Considered options

- **Tags or multi-membership.** Rejected: groups are structure for organizing lists (one item, one place, user order), not labels for filtering. Multi-membership would complicate ordering, counts, and the window's grouped rendering for no asked use case.
- **Shared cross-collection groups.** Rejected: the three lists have different shapes, pages, and export paths; a shared namespace would let a Clip land in a Launch group and break every per-collection invariant.
- **Keeping empty groups.** Rejected: an empty group is a name with no members — it clutters pickers and invites stale-name conflicts. Dissolve-on-empty keeps the name pool live (deleting or dissolving frees the name for immediate reuse) and matches the "section appears once you use it" model from ADR-0015.

## Consequences

- Names are exclusive within their collection (trimmed, case-insensitive); ungrouped items render first, then groups in user order.
- Deleting a group never deletes items; deleting an item may dissolve its group — both in the same transaction, with empty-group sweeps on every assign/unassign/delete.
- Groups are machine-local structure, never part of Presets, Plan, Run, or exports — same boundary as desktop assignments, with the opposite activation rule (explicit opt-in switch vs activate-by-use).

## Amendment — 2026-09-05 (executable-source audit)

Namespace isolation, at most one membership per item, and dissolve-on-leave/delete are enforced by the data-layer commands in `src-tauri/src/groups.rs`. “A group exists only while it has a member” remains the intended authoring rule, not an atomic storage guarantee: `create_group` commits before the separate assignment call made by `src/lib/collectionGroups.svelte.ts`. A failed or interrupted assignment can leave an empty group until a later sweep. This is an implementation gap, not acceptance of persistent empty groups as the desired model.

Turning Groups off hides grouping and preserves existing memberships at that moment; it does not suspend reads or lifecycle maintenance. `OrderedList::delete` in `src-tauri/src/ordered_list.rs` still removes items and sweeps empty groups while the feature is off. Deleting items can therefore dissolve their groups. Feature dormancy describes the grouping presentation and controls, not frozen storage. Ordering qualifications are recorded in ADR-0026.

## Amendment — 2026-10-24 (two-level tree, per-group colour; tickets 229–233)

Groups gain exactly one level of containment inside their own collection: a group may have a parent, a parent may not itself be a child, and a group with children can never become a child. The tree is therefore at most two levels, and "one item, one place" still holds — nesting changes where a group sits, not how many memberships an item has. The move contract is transactional (`move_group`: parent plus sibling position, validated before commit); depth 3, cycles and self-moves, a group-with-children as a child, cross-collection moves, and sibling name collisions in the destination are refused with a reason rather than partially applied. Deleting a parent dissolves its children with it and returns every item in the subtree to ungrouped; deleting a child returns only its own items. The dissolve-on-empty rule is unchanged and applies per group.

Colour stays a property of the group that owns it, never an inherited wash. A child keeps its own hue under a coloured parent; where a surface has to flatten the tree into one list (the dock below its content breakpoint, ADR-0021), each card is painted with the colour of the group it actually belongs to while the single section beam keeps the parent's colour, and a neutral group clears any tint it would otherwise inherit. The leading gutter that holds the beam and the drag grip is reserved by **every** group, never by colour, so turning a colour on or off — or nesting a group under a coloured versus a neutral parent — never moves its grip, chevron, title or cards. The tint formula itself lives once as the global `.group-tint` utility in `src/lib/styles/tokens.css`, shared by the accordion and the card shell.
