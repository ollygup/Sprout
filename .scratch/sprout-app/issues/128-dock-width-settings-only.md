# 128 — Dock width setting, Settings-only + switch-audit step 0

**What to build:** Settings gains a dock-width % slider (docked only, both Fixed and auto-hide, per-monitor memory, never draggable); first step of the ticket converts any instant-effect checkboxes on touched surfaces to switches per the standing rule.

**Blocked by:** none — can start immediately.

**Status:** done

## Scope

- Step 0 (merged ticket 6): inventory checkboxes on touched Settings/surface UI; convert instant-apply ones to `role="switch"` + `aria-checked` + On/Off word; leave Save-deferred (dialogs, export scope) as checkboxes (0008:2, 0007). No-op if none qualify.
- Width: % of monitor width, effective width clamped between today's width (min) and a researched cap ≤60% (in-ticket research must justify the cap — 60% of an ultrawide as a fixed AppBar is extreme); `constants/window.rs` stays the single size source; floating window stays fixed; gate stays floating-vs-docked (0012:35).

## ACs

- [x] Slider changes docked width live (Fixed + auto-hide), persists per monitor, survives restart/mode flip; floating unaffected; no drag handle exists.
- [x] Cap justification recorded in-ticket (why the chosen max, with ultrawide math).
- [x] Switch audit recorded (converted list or explicit none); converted switches apply immediately with visible state.
- [x] `npm.cmd run check` 0 errors; `cargo check` clean.

## Verification

- `npm.cmd run check`, `cargo test` dock/settings slice
- Manual: set narrow/wide per monitor, flip Fixed↔auto-hide, restart, confirm recall + reveal still clean.

## Switch audit (step 0) — explicit none

Touched surface is the Settings page (`src/routes/settings/+page.svelte`). Inventory:

- Export backup dialog scope checklist (`include[key]`, one `<input type="checkbox">`): a per-use scope choice on a moment-of-use dialog — applies on Export confirm, not instantly. Stays a checkbox per 0008:2 (switches = instant only) and 0007 (export checklist lives in the dialog, not as switches).
- Theme segmented control (`role="radio"` + `aria-checked`): already the correct instant-apply pattern, not a checkbox.
- Autostart `Select`: instant-apply via `updateAutostart` but a select, out of checkbox scope.
- Dock width slider (`type="range"`): deferred to Save like the other dock knobs — not a switch candidate.

Not touched and therefore out of scope (all Save-deferred dialog/selection flags that stay checkboxes per the 0008 applied-history convention): Quick Action `Show Stop button`, command-entry `Show a window`, preset dependency chips, plan pick/in-run checkboxes. Converted: none.

## Cap justification — max 30%

Chosen range 10–30%, default 18% (≈346px on a 1920 reference monitor — the closest whole % to today's 340). Effective px = `% of the monitor's full width (rcMonitor, never the shrinking work area), floored at 340, capped at 30% of that monitor` (`constants/window.rs::dock_width_px`, the single size source; floating stays 340).

Why 30 and not 60: a fixed AppBar permanently reserves workspace, so the cap must keep the dock a strip, not half the screen. 60% of a 3440px ultrawide is 2064px fixed — wider than a whole 1080p monitor, leaving only 1376px for apps. At 30% the reservation is at most a third of any screen while still meaningfully wider than today for long names: 1920→576 (+69%), 2560→768, 3440→1032, 5120→1536. Auto-hide shares the same width (gate stays floating-vs-docked per 0012:35) so mode flips never reset layout; the 180ms slide distance stays short.

## Implementation notes

- Placement follows 0008:1 (app-global durable preference → Settings, not the surface features menu that density ticket 129 uses per 0006:8) and 0006:4 proximity (width sits with dock mode/edge; per-monitor rows reuse flattened `.knob` per 0005:5). Tokens/`Select`/`Button`/`Disclosure` only; slider uses `accent-color: var(--accent)` + `:focus-visible` ring, no ad-hoc sizes/colors.
- Backend: `Settings.dock_width_pct` + `quicklaunch.dock.width_pct.<monitor>` memory mirroring edge/mode discipline (identity-first, ticket 110); every placement (`dock`, `reposition`, `drift_check`, `on_appbar_pos_changed`, driver `settle_mode` + per-tick width pass) derives from `dock_width_px`. Fixed re-reserves+places synchronously; auto-hide only retargets `last_rect` and the driver slides to it (single-writer, ticket 66). Backup excludes both (machine-local, existing contract).
- Results: `npm.cmd run check` 0 errors; `cargo check` clean (7 pre-existing warnings); `cargo test` 432 passed / 0 failed; `npm.cmd test` 89 passed.
