# 112 — Layered auto-hide reveal gate

**What to build:** Revealing a hidden dock requires intent: the cursor must be at the actual screen-edge sliver (the reserved invisible zone itself, not an interior band), traveling predominantly *into* the edge, and hold there through a short dwell (~200 ms) that cancels the instant the cursor leaves the band. Grazes along the seam, overshoot-and-rebound fly-throughs, and cross-monitor seam transits all fail some layer and never reveal. Hide-side hysteresis, slide animation, and reservation behavior are untouched.

**Blocked by:** None — can start immediately.

**Status:** done

- [x] Trigger zone derives from the auto-hide sliver constant itself — the separate interior trigger band is gone (single size source)
- [x] Samples dominated by along-the-edge motion accumulate nothing toward reveal; toward-edge travel accumulates past a sensitivity threshold
- [x] Reveal fires only after the dwell elapses inside the band; any exit from the band cancels the pending reveal
- [x] Cross-monitor transit through the band never reveals the dock (cancel-if-left covers it; no per-topology special case)
- [x] Decision logic sits behind a pure-function seam with cargo tests covering: graze, fly-through, deliberate push, cross-seam transit
- [x] Poll-loop structure and cost unchanged in order of magnitude; constants single-sourced in the shared window-constants module
