# 142 — Settings: retire the static Save button, the dirty bar owns saving

**What to build:** Remove the static "Save settings" button at the bottom of `/settings` so the dirty bar is the single save surface — no more stranded button under collapsed groups.

**Blocked by:** 139 (builds over the grouped Settings page and its dirty bar).

**Status:** ready-for-agent

## Scope

- Delete the `form__actions` block (button + CSS) from `src/routes/settings/+page.svelte`; the `<form>` element, `onsubmit`, `save()`, dirty tracking, dirty bar, and the nav/close guard stay exactly as they are.
- Explicitly not built: a header-row Save action (that would duplicate the bar rather than replace the duplicate), auto-save, any change to Save/Discard/guard behavior.

## ACs

- [x] No static Save button renders on `/settings` at any scroll position, collapsed or expanded.
- [x] First edit still raises the dirty bar with Save/Discard, announces through the live region, and saving clears it.
- [x] Rail-navigation guard and window-close guard still offer Save/Discard/Keep editing and complete the save.
- [x] Keyboard-only edit → Tab to the bar → Save → saved announcement; `npm.cmd run check` 0 errors.

## Implementation notes

- Losing the submit button also loses implicit Enter-to-submit in fields — accepted: the bar is already on screen the moment anything is dirty, Tab-reachable and announced, so no keyboard path strands the user.
- Add a source-contract test asserting the static button is gone while the dirty bar remains (mirrors the settings contract tests in `src/lib/settingsSearch.test.ts`).

## Verification

- `npm.cmd run check`, settings frontend tests; manual: collapse-all scan (no hanging button) → edit → bar → Save → guard paths → keyboard-only run-through.
