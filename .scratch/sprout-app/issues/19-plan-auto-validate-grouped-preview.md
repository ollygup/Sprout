# 19 — Plan page: auto-validate and grouped preview

**What to build:** The Plan page's first two stages rebuilt. Selection is ephemeral per visit — nothing pre-selected on arrival, no validation triggered by tab navigation. Selecting a Preset auto-validates against the machine immediately (all its requirements join the selection) and recomputes live on any change — the Build/Validate/Rebuild buttons cease to exist. Results render in grouped sections instead of a card-by-card dump: Ready to apply (n) · Already good (n) · Needs your decision (n, the conflict strip) · Needs attention (n), each with a count in its header. A quiet "Check again" covers machine-state staleness between planning and running. URL state (`?stage=…`, `?presets=…`) makes the page deep-linkable and supports open-in-plan prefill by preset names (graceful when presets are gone). "Save as new preset" preserved.

**Blocked by:** 11 — App shell and design foundation; 14 — Presets page and composer rebuild; 15 — Live-linked requirements; 16 — Honest run outcomes

**Status:** done

- [x] Three-stage model with URL state; deep-linking works; arriving with `?presets=…` prefills the selection by name (missing presets reported gracefully, not broken)
- [x] Selection ephemeral per visit; nothing pre-selected on arrival; navigating tabs never re-validates
- [x] Selecting a Preset auto-validates instantly; selection changes recompute live; no Build/Validate/Rebuild button exists anywhere
- [x] Results grouped: Ready to apply (n) · Already good (n) · Needs your decision (n, conflict strip) · Needs attention (n) — counts in headers; per-requirement toggles preserved within groups
- [x] Quiet "Check again" affordance for staleness; machine changes between planning and running surfaced, never silently assumed
- [x] "Save as new preset" works from a resolved selection
- [x] `npm run check` 0 errors, `cargo test` green, `cargo check` clean, `npm run build` ok