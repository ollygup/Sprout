# 140 — Companion launch height: saved ratio from the first paint

**What to build:** The Companion pane opens at its saved height on every cold launch — e.g. 50% means 50% with no splitter touch — on single- and multi-monitor setups.

**Blocked by:** none — can start immediately.

**Status:** ready-for-agent

## Scope

- Docked-only pane (floating still shows none); order the launch reads (dock state, global ratio, per-monitor ratio for the *actual* dock monitor) before the native child is created/sized, and re-sync once the ratio resolves so the first paint never measures a pre-layout rect at the 0.40 init.
- Per-monitor wins with global fallback; broken stored values fall back exactly like Settings load; the first-display proxy is replaced by the real dock-monitor resolution.
- Splitter drag persists to the actual monitor (global + per-monitor write) and keeps its live-resync behavior; keyboard splitter control unchanged.

## ACs

- [x] Cold launch with height at 50% renders 50% before any pointer input, single-monitor and per-monitor multi-monitor.
- [x] Moving the dock to another screen picks up that screen's remembered height, falling back to global when absent.
- [x] Drag and arrow-key resize still persist and survive restart; out-of-range stored values clamp to 25–60% (default 40%).
- [x] `npm.cmd run check` 0 errors; settings/companion test slices green.

## Implementation notes

- Root causes addressed together: `load()` never setting companion state, concurrent unordered launch reads, first-paint sizing from the init value, and the wrong-monitor lookup.
- Single size source stays the window constants module; no new sizing policy, no floating-pane change.

## Verification

- `npm.cmd run check`, settings/companion tests; manual: set 50% → quit → launch (docked, auto-start) → measure; repeat on a second monitor; drag → relaunch recalls.
