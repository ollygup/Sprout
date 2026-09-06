# 139 — Settings groups + multi-keyword local search on one page

**What to build:** Settings stays one page and becomes scannable: four `Disclosure` groups (General / Dock / Companion / Backup & housekeeping) plus a filter box that matches labels, synonyms, values and descriptions.

**Blocked by:** 138 (builds over the new `{url, name}` site shape for the Companion group and the search index).

**Status:** ready-for-agent

## Scope

- One `/settings` route, `PageHeader` unchanged; groups: General (theme/timeout/retention/install dir/autostart/concurrency), Dock (mode/edge/state/width/density/reveal), Companion (sites + names + active + height), Backup & housekeeping. Shared `Disclosure` primitive with the caret-in-margin anatomy; collapsed headers show state summaries (e.g. Dock — auto-hide, left, 18%).
- Filter below the header over the shared search-input grammar: matches knob label + synonyms + current values + one-line description, so `theme`/`light`/`dark`, `width`, `mute`, `backup` all land correctly; narrows groups to matches with an honest no-match state; fully keyboard/screen-reader operable.
- All-open by default on first visit with remembered open state; validation errors auto-expand their group and focus the field; max two disclosure levels, never nested disclosures.
- Explicitly not built: nested app-rail children, top tabs, side-rail sub-pages, per-group deep routes (refused under research 0014 until ≥7 sections / deep-link demand / own-surface growth).

## ACs

- [x] Four groups render through the shared accordion with counts while collapsed; open state persists across visits. (Collapsed state summaries were cut on review — search covers findability, so headers stay caret + name + count.)
- [x] Typing `light`, `dark` or `theme` all surface the theme knob; `width` surfaces dock width; a nonsense query shows the designed empty state, never a blank page.
- [x] A failing save expands the owning group and focuses the field; dirty-guard and Save/Discard behavior unchanged.
- [x] No second rail, no tabs, no new routes; `npm.cmd run check` 0 errors.

## Implementation notes

- Rename the generic `Advanced` disclosure to `Housekeeping` for scent when touched; keep per-monitor knobs as flat reused rows (no nested cards).
- Search index entries carry `{label, synonyms[], values[], description}` so future knobs join by data, not by special-casing.

## Verification

- `npm.cmd run check`, settings frontend tests; manual: collapsed scan → filter `dark` → filter nonsense → failing-save focus → keyboard-only run-through → 900px-min window check (no nested-rail overflow).
