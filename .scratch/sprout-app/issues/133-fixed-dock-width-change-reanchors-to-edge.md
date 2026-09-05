# 133 — Fixed dock width change re-anchors to the edge

**What to build:** Fix the dead-strip bug on explicit width change while fixed+docked: changing Dock width (e.g. 18% → 30%) must re-anchor the strip flush to the screen edge at the new width instead of deriving from the self-shrunk work area.

**Blocked by:** none — can start immediately.

**Status:** done (automated green + user-validated on a live dock)

## Scope

- Backend only (`src-tauri/src/quick_window.rs`, `src-tauri/src/appbar.rs` + tests): no schema, no new commands, no Settings/frontend changes. The Settings knob, persistence, and fan-out from ticket 128 stay as-is.
- Root cause (diagnosed, no fix yet): `apply_width`'s fixed branch calls `desired_rect`, whose flush-keep (ticket-61 drift guard) discards an explicit resize — geometry stays at the old width while memory already holds the new %; then `settle_mode`'s narrow-bar branch re-derives via `appbar_rect(work, …)` against the self-shrunk work area, marching the strip one old-width into the screen. Fix: derive explicit-resize placements from `monitor_rect` (reservation-free), mirroring the auto-hide pattern; bypass the flush-keep when actual vs. requested widths differ.
- Extract a pure `fixed_width_desired(monitor, work, actual, width, edge)` helper so the edge-anchoring is unit-testable without HWND (the missing seam is part of the bug — `settle_fixed_desired` hardcodes `DOCK_WIDTH` and cannot express a width change).

## ACs

- [x] 18→30 and 30→18 re-anchor flush to the monitor edge at the new width, left and right edges, fixed mode; no dead strip; auto-hide→fixed flips with a simultaneous width change also re-anchor.
- [x] Auto-hide, floating, and per-monitor paths behave exactly as before (no regressions).
- [x] New unit tests pin the derivation (self-shrunk work + old-width flush actual + new width ⇒ flush-to-monitor-edge strip); `npm.cmd run check` 0 errors; `cargo test` green.

## Results

- Fix is one branch in `apply_width` (`quick_window.rs`): on explicit resize (live width ≠ requested — the early return proves it) the proposal anchors via `appbar_rect(monitor, …)` instead of `desired_rect(work, …)`. `reserve()` still reconciles other bars/taskbars; overlapping our own stale registration reads as a move. `settle_mode` deliberately untouched: traced that its narrow branch is unreachable for this state post-fix, and its work-anchored derivation is correct for genuine arrivals (sliver/mode-flip, pinned by existing tests) — changing it would risk the ticket-66/119 behavior for no reachable case.
- Tests: new `fixed_width_change_anchors_the_new_strip_at_the_monitor_edge` (widen+narrow × left+right, plus the old derivation's stale-keep documented beside it) and `settle_fixed_keeps_a_bar_converged_at_a_non_default_width`; `settle_fixed_desired` mirror generalized with a width param (was hardcoded to `DOCK_WIDTH`, the coverage hole). All 4 pre-existing `desired_rect` + 3 pre-existing `settle_fixed` tests pass unchanged.
- Suites: `npm.cmd run check` 0 errors; `cargo test` 435 passed / 0 failed; `npm.cmd test` 89 passed (rerun — frontend untouched).
- Live-dock manual pass: user-validated — width changes re-anchor flush with no dead strip. Ticket closed.

## Verification

- `npm.cmd run check`, `cargo test` (new + existing `desired_rect`/`settle_fixed` slices)
- Manual: dock fixed at 18% → Settings width to 30% → Save → strip flush at new width, both edges; back to 18%; flip fixed↔auto-hide with a width change in flight; per-monitor spot-check where available.
