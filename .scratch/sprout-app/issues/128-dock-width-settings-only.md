# 128 — Dock width setting, Settings-only + switch-audit step 0

**What to build:** Settings gains a dock-width % slider (docked only, both Fixed and auto-hide, per-monitor memory, never draggable); first step of the ticket converts any instant-effect checkboxes on touched surfaces to switches per the standing rule.

**Blocked by:** none — can start immediately.

**Status:** ready-for-agent

## Scope

- Step 0 (merged ticket 6): inventory checkboxes on touched Settings/surface UI; convert instant-apply ones to `role="switch"` + `aria-checked` + On/Off word; leave Save-deferred (dialogs, export scope) as checkboxes (0008:2, 0007). No-op if none qualify.
- Width: % of monitor width, effective width clamped between today's width (min) and a researched cap ≤60% (in-ticket research must justify the cap — 60% of an ultrawide as a fixed AppBar is extreme); `constants/window.rs` stays the single size source; floating window stays fixed; gate stays floating-vs-docked (0012:35).

## ACs

- [ ] Slider changes docked width live (Fixed + auto-hide), persists per monitor, survives restart/mode flip; floating unaffected; no drag handle exists.
- [ ] Cap justification recorded in-ticket (why the chosen max, with ultrawide math).
- [ ] Switch audit recorded (converted list or explicit none); converted switches apply immediately with visible state.
- [ ] `npm.cmd run check` 0 errors; `cargo check` clean.

## Verification

- `npm.cmd run check`, `cargo test` dock/settings slice
- Manual: set narrow/wide per monitor, flip Fixed↔auto-hide, restart, confirm recall + reveal still clean.
