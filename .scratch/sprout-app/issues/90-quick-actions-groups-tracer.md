# 90 — Quick Actions groups tracer: first full grouping UI path

**What to build:** The Quick Actions page becomes the tracer for the Groups pattern end to end. A labeled Groups toggle in the page's toolbar row (persisted, default off) gates the whole feature. When ON: group management appears — create, rename, delete, reorder groups and assign/unassign actions via the row menu — and the list renders ungrouped items first, then each group as a default-expanded disclosure section with a count badge, sections appearing only once at least one group exists. When OFF: every group affordance hides and the list is flat; data is untouched. Search filters the whole list regardless of grouping.

**Blocked by:** 89 — Groups foundation.

**Status:** ready-for-agent

- [ ] Toggle flips visibility in both directions without any data loss
- [ ] Group sections render only once ≥1 group exists (absent-until-content)
- [ ] Ungrouped-first ordering holds; group reorder works
- [ ] Deleting a group visibly returns its members to the ungrouped list
- [ ] Search matches across all items irrespective of section
- [ ] Toolbar row still fits alongside search at real DPI (label degradation allowed)
- [ ] Type-check clean; manual dev pass over the page documented
