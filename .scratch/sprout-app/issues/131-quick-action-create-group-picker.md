# 131 — Quick Action creation: conditional Group picker

**What to build:** The Quick Action create dialog offers group placement only when the `action` collection already has live groups — default ungrouped, with create-and-place — and shows nothing otherwise.

**Blocked by:** none — can start immediately.

**Status:** ready-for-agent

## Scope

- Surface: `QuickActionFormDialog` add-flow only (edit-flow reuses if trivial, no new dialog); picker lists live groups + `New group…` fusion entry; default ungrouped; zero groups → no field, no toggle, no management chrome (0004:2, 0006:2/11, 0006:10 create-and-place; Group stays per-collection per glossary).

## ACs

- [ ] 0 groups → no group field; ≥1 group → optional picker defaulting ungrouped; New-group creates + places in one step; validation/cwd/stop/note behavior unchanged.
- [ ] `npm.cmd run check` 0 errors; backend group tests still green.

## Verification

- `npm.cmd run check`, `cargo test` groups/quick-actions slice
- Manual: create with 0 groups, with groups, and via New-group entry.
