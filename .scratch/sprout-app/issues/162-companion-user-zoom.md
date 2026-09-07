# 162 — Companion user zoom, remembered per site

**What to build:** A zoom control in the dock Companion bar (50–200%) that remembers its setting per site and falls back to today's automatic width zoom — fully independent of the height splitter.

**Blocked by:** 161 (shares the Companion bar cluster; builds over its stabilized order).

**Status:** ready-for-agent

## Scope

- Bar `- / % / +` control applying page zoom within 50–200%, persisted per site with auto fallback; existing width-derived auto zoom stays the default; height-ratio splitter untouched.
- Explicitly not built: omnibox/tabs/history, per-load zoom editing, any change to bar buttons from 161.

## ACs

- [x] Zoom persists per site across redock/restart; unset sites follow today's auto zoom.
- [x] Zoom never moves the height splitter and vice versa.
- [x] `npm.cmd run check` 0 errors; related frontend tests green.

## Verification

- `npm.cmd run check`, companion pane tests; manual docked-only across narrow and wide dock widths.
