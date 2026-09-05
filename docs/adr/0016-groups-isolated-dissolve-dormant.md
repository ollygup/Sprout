# Groups are isolated namespaces that dissolve and sleep

The three machine-local lists (Launch entries, Quick Actions, Clips) share one `groups` table, but namespaces are isolated at the data layer: a group belongs to exactly one collection (`launch` / `action` / `clip`), items hold at most one `group_id`, and cross-collection membership is impossible, not just hidden. A group exists only while at least one member belongs to it — created by assigning an item ("Move to group → New group…"), dissolved automatically when its last member leaves, with explicit delete returning members to ungrouped instead. Each collection's Groups feature is opt-in (default off); off is fully dormant — lists render flat and no group affordance appears anywhere — while stored groups and memberships survive untouched, so re-enabling restores them.

## Considered options

- **Tags or multi-membership.** Rejected: groups are structure for organizing lists (one item, one place, user order), not labels for filtering. Multi-membership would complicate ordering, counts, and the window's grouped rendering for no asked use case.
- **Shared cross-collection groups.** Rejected: the three lists have different shapes, pages, and export paths; a shared namespace would let a Clip land in a Launch group and break every per-collection invariant.
- **Keeping empty groups.** Rejected: an empty group is a name with no members — it clutters pickers and invites stale-name conflicts. Dissolve-on-empty keeps the name pool live (deleting or dissolving frees the name for immediate reuse) and matches the "section appears once you use it" model from ADR-0015.

## Consequences

- Names are exclusive within their collection (trimmed, case-insensitive); ungrouped items render first, then groups in user order.
- Deleting a group never deletes items; deleting an item may dissolve its group — both in the same transaction, with empty-group sweeps on every assign/unassign/delete.
- Groups are machine-local structure, never part of Presets, Plan, Run, or exports — same boundary as desktop assignments, with the opposite activation rule (explicit opt-in switch vs activate-by-use).
