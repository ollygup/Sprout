# 89 — Groups foundation: per-collection groups domain

**What to build:** A user-defined Groups domain scoped per collection: group storage keyed by a collection discriminator (launch / action / clip) so namespaces are isolated at the data layer — a Quick Action group accepts only Quick Actions, and likewise for Clips and Launch entries. Items gain a nullable group reference (max one group per item). Commands cover create, rename, delete (members return to ungrouped), reorder, and per-collection assign/unassign. Three persisted settings keys (one per collection's Groups feature) default off. The domain glossary gains the Group term, distinct from virtual-desktop assignment.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [ ] Create/rename/reorder/delete/assign/unassign all work per collection
- [ ] Assigning an item to another collection's group is rejected at the data layer
- [ ] An item holds at most one group; deleting a group returns members to ungrouped without deleting them
- [ ] List ordering helpers return ungrouped items first, then groups in user order
- [ ] Settings keys persist with default-off values
- [ ] Glossary documents Group and touches up Launch entry wording
- [ ] Backend test suite covers isolation, single-membership, delete-cascade ordering
