# 161 — Companion Reload reset in the dock bar

**What to build:** A Reload button in the dock Companion bar that recreates the child WebView, so a page stuck on a bad login (or any dead-end state) recovers without leaving the dock.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

## Scope

- One `IconButton` in the bar's left cluster reusing the existing recreate logic, with loading/disabled states matching the bar idiom; bar order otherwise preserved (mute/mixer/external untouched).
- Explicitly not built: Back/Forward (no native history API in the Tauri WebView surface), Home (reselecting the site is the same recreate), omnibox or any browsing chrome (ADR-0022 scope stands).

## ACs

- [ ] A stuck login page recovers via Reload; the saved site URL is unchanged.
- [ ] Companion bar order, mute/mixer/external behavior, and floating/off teardown are unchanged.
- [ ] `npm.cmd run check` 0 errors; related frontend tests green.

## Verification

- `npm.cmd run check`, companion bar tests; manual docked-only: bad login → Reload → fresh page.
