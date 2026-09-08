# 163 — Add-site Enable-now hint

**What to build:** Saving a Companion site shows "Site saved — Enable now" with a one-tap action that activates the site on-surface, so nobody hunts through Settings to make a new site appear.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

## Scope

- Extend the Companion page's existing `Notice` + `flash()` pattern with one action slot; Enable-now sets the active site and refreshes the dock; auto-clear as today.
- Explicitly not built: duplicating the Active-site picker onto the page, hyperlinking to Settings instead of acting, reusing the persistent Settings dirty bar (it guards deferred saves — different component, different purpose).

## ACs

- [x] After add, the notice offers Enable-now; tapping it shows the site in the dock with no Settings visit.
- [x] Dismissal timing and error paths match today's flash behavior.
- [x] `npm.cmd run check` 0 errors; related frontend tests green.

## Implementation notes

- Visibility on-surface with action (research 0006 pattern 1), one accent as the next step (pattern 6), transient feedback not a guard (0004 rule 5).

## Verification

- `npm.cmd run check`, companion page tests; manual: add → Enable-now → docked pane appears.
