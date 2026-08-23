# 74 — Self-update UI: rail pill + Settings re-check

**What to build:** The user-facing half of ADR-0012: the rail footer's plain
version text becomes an update pill while a newer release exists; clicking
confirms, installs, and relaunches; Settings gains a manual "Check for
updates" action.

**Blocked by:** 73 — the check/install commands and the `update-available`
event this UI consumes

**Status:** done - `updateState.svelte.ts` single store fed by the startup event + manual check; rail pill with confirm/install and busy wording; Settings check row with result notices; svelte-check 0 errors

- [x] Types/API additions for check result, install command, and event payload
- [x] NavRail footer: while idle, unchanged `v{version}` text; while an update exists, an accent-tinted focusable pill `v0.4.1 ↑ 0.5.0` with tooltip "Update available" — nothing changes on other pages
- [x] Clicking the pill opens the shared confirm dialog ("Install Sprout 0.5.0 now?" noting Sprout restarts); confirming invokes install; cancel leaves state intact
- [x] While downloading/applying the pill shows progress wording and disables re-entry
- [x] Settings gains a "Check for updates" row (button + result notice): "up to date", found-version (with inline install action), or quiet failure wording consistent with app copy style
- [x] Manual Settings check updates the rail pill live (same store/state source)
- [x] Keyboard + screen-reader pass: pill is a real button, dialog traps per existing component behavior, result notice announced
- [x] `npm run check` 0 errors; manual end-to-end deferred to ticket 81's audit (needs a public repo + two tags); synced to the share
