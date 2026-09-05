# 131 — Quick Action creation: conditional Group picker

**What to build:** The Quick Action create dialog offers group placement only when the `action` collection already has live groups — default ungrouped, with create-and-place — and shows nothing otherwise.

**Blocked by:** none — can start immediately.

**Status:** done

## Scope

- Surface: `QuickActionFormDialog` add-flow only (edit-flow reuses if trivial, no new dialog); picker lists live groups + `New group…` fusion entry; default ungrouped; zero groups → no field, no toggle, no management chrome (0004:2, 0006:2/11, 0006:10 create-and-place; Group stays per-collection per glossary).

## ACs

- [x] 0 groups → no group field; ≥1 group → optional picker defaulting ungrouped; New-group creates + places in one step; validation/cwd/stop/note behavior unchanged.
- [x] `npm.cmd run check` 0 errors; backend group tests still green.

## Implementation notes

- `QuickActionFormDialog.svelte` gains optional `groups: Group[]` (default
  `[]`, fed by the page's `groups.groups`) plus `groupsEnabled` (default
  `false`, fed by `groups.enabled`); the Group field renders only while the
  switch is on AND live groups exist — off stays fully dormant (0006 pattern
  12: stored groups never shown or touched, so an edit while off preserves
  its membership silently), zero groups stays absent (0004:2, 0006:2/11).
  Scope is Quick Actions only: `ClipFormDialog`, `CommandFormDialog`, and the
  read-only Quick Launch window/dock tabs are untouched. The shared `Select`
  (ticket 45) offers Ungrouped (default) | groups in user order | `New group…`, which reveals an
  inline name input — create-and-place in one submit (0006:10). Submit does
  create-then-assign (`createGroup("action", …)` + `assignToGroup`) for a new
  group, create-then-assign for a picked group, and nothing for Ungrouped;
  blank new-group names refused inline, backend exclusivity rejections land in
  the dialog's error slot (ticket 106's contract). Edit reuses the same picker
  trivially (preselects the live current group; save assigns/unassigns around
  `updateQuickAction`, which never touches `group_id` per ticket 89).
- Results: `npm.cmd run check` 0 errors; `cargo test` 423 passed / 0 failed
  (2 ignored, 13 live filtered).

## Verification

- `npm.cmd run check`, `cargo test` groups/quick-actions slice
- Manual: create with 0 groups, with groups, and via New-group entry.
