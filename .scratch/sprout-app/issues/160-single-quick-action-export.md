# 160 — Single Quick Action export from the row menu

**What to build:** Any Quick Action exports from its row `⋯` menu to a file that imports through the existing Settings → Backup → Restore flow with honest inserted/skipped counts.

**Blocked by:** None — can start immediately (row-menu overlap with 159 needs coordination, not sequencing).

**Status:** ready-for-agent

## Scope

- New backend command constructing the same versioned `sprout-backup` envelope server-side with a one-element `quick_actions` array (ADR-0014 one-format rule — no new format); `⋯` Export item reusing the preset `saveDialog` pattern; confirm copy states the payload-identity consequence (same command+cwd skips under any name).
- Explicitly not built: any new file format, any per-item import UI (Restore already merges), other collections.

## ACs

- [x] Exported file restores through the ordinary Restore flow with true inserted/skipped counts.
- [x] Same command+cwd under a different name skips (and the copy says so); same name with different target lands.
- [x] Whole-collection export and all other collections are untouched.
- [x] `npm.cmd run check` 0 errors; related backend/frontend tests green.

## Implementation notes

- Compliant with ADR-0014/0026 by construction: a one-element array with four empty siblings is indistinguishable from a selective export that happened to contain one action.

## Verification

- `npm.cmd run check`, backup/merge tests plus new single-export coverage; manual: export → edit → restore → counts.
