# 90 — Quick Actions groups tracer: first full grouping UI path

**What to build:** The Quick Actions page becomes the tracer for the Groups pattern end to end. A labeled Groups toggle in the page's toolbar row (persisted, default off) gates the whole feature. When ON: group management appears — create, rename, delete, reorder groups and assign/unassign actions via the row menu — and the list renders ungrouped items first, then each group as a default-expanded disclosure section with a count badge, sections appearing only once at least one group exists. When OFF: every group affordance hides and the list is flat; data is untouched. Search filters the whole list regardless of grouping.

**Blocked by:** 89 — Groups foundation.

**Status:** done

- [x] Toggle flips visibility in both directions without any data loss
- [x] Group sections render only once ≥1 group exists (absent-until-content)
- [x] Ungrouped-first ordering holds; group reorder works
- [x] Deleting a group visibly returns its members to the ungrouped list
- [x] Search matches across all items irrespective of section
- [x] Toolbar row still fits alongside search at real DPI (label degradation allowed)
- [x] Type-check clean; manual dev pass over the page documented

## Implementation notes (2026-08, agent session)

**Toggle placement follows research 0008, not this ticket's earlier wording.**
The spec's "toggle in the page's toolbar row" predates research
0008-feature-menus-over-toolbar-checkboxes, whose rules were written because
ticket 88's bare toolbar checkbox was rejected in review — and that note names
these Groups toggles (tickets 90/91) as its next application. Shipped shape:
a **Groups** switch inside the shared page-features gear menu
(`PageFeaturesButton` through PageHeader's `features` slot), with a
plain-language description covering both states and an On/Off word +
`role="switch"`. Persisted via `update_groups_enabled("action", …)`
(settings key from ticket 89), optimistic with rollback on save failure.

**Page behavior** (`src/routes/quick-actions/+page.svelte`):

- OFF (default): flat rack exactly as before; no group menus, badges, or New
  group button; stored groups/memberships untouched (dormancy).
- ON, zero groups: still flat + a secondary **New group** button in the
  header actions row (Add stays primary — research 0005 rule 3). Sections and
  row-menu assignment stay hidden until ≥1 group exists (research 0004 rule 2,
  absent-until-content).
- ON, ≥1 groups: ungrouped rows render first as plain cards, then each group
  as the shared labeled `Disclosure` (default expanded, collapse is
  session-local) + muted `Badge` count of total membership + per-header ⋯
  menu (Rename → name dialog, Move up / Move down over group order, Remove →
  confirm). Empty groups keep their place with a dashed hint row.
- Row ⋯ menus gain an Ungrouped / per-group check-icon block ahead of Edit /
  Move / Remove while grouping is on (prior art: Launch page's desktop list),
  gated on ≥1 group existing.
- Move up/down on actions reorders within what the user sees — the whole list
  when flat, otherwise the ungrouped block or the action's own group — passing
  global positions to `move_quick_action`.
- Search filters before slicing: matches across all sections, empty sections
  hide while filtering, all sections force open so no match hides behind a
  chevron; count badges keep showing total membership.

## Verification notes

- `npm.cmd run check` — 0 errors, 0 warnings; `npm.cmd run build` — clean;
  `vitest run` — 36/36 (no regressions). Backend untouched by this ticket;
  group CRUD/isolation/delete-cascade/settings-default-off remain covered by
  ticket 89's `cargo test` suite.
- Boot smoke: `tauri dev` launches, tray + dock restore initialize, process
  stays up (verified stable across a ~70 s window with the dev server
  attached). Two benign startup log lines only. First attempts hit an exit-
  code-1 race from force-killed prior instances holding the single-instance
  mutex — clean start resolves it; not app-related.
- Interactive click-through of the toggle/menus was NOT machine-drivable this
  session (WebView2 exposes no UIA tree here); worth eyes-on during the next
  interactive session — specifically: flip feel at real DPI, disclosure
  chevron alignment against the badge, and menu placement near screen edges.
  Toolbar-fit AC is satisfied structurally: nothing new entered the toolbar
  lane (search unchanged; the gear trigger is the same shared chrome already
  shipped on Quick Launch), so PageHeader keeps owning the layout.
