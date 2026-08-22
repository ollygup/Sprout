# 66 — Dock mode switch fixed → auto-hide crashes the whole app

**Parent:** 63 (dock auto-hide overlay) · **Status:** done ·
**Priority:** high — regression introduced by 63's overlay rework

> Worked with the `diagnosing-bugs` skill: repro loop built first, backtrace
> captured before any fix.

## Report (user, verbatim)

> "i noticed that if i change from docked + fixed to docked + auto-hide it
> would auto (crash?) close the app entirely."

Changing the visibility mode live (Settings or window chrome) while docked,
from **fixed** to **auto-hide**, terminated the entire process — main window
included. The reverse direction (auto-hide → fixed) shares the same code path
shape and is covered by the fix.

## Root cause (captured, not theorized)

Repro loop (built in Phase 1): `tools/repro-dock-mode-stress.ps1` +
`SPROUT_DOCK_STRESS=1` hook in `lib.rs` (`debug66_dock_mode_stress`,
debug-builds only). Rapid `set_dock_mode` toggles against a live docked
window; red = panic/abort signature or missing PASS marker.

Captured signature (deterministic at ≥8 flips, 3/4 runs @ 20×80 ms):

```
thread 'main' panicked at tao-0.35.3\src\platform_impl\windows\window.rs:296:7:
attempt to add with overflow   (set_inner_size offset math)
  via set_min_inner_size ← handle_user_message (queued WindowMessage::SetMinSize)
```

Mechanism — two concurrent geometry writers on one HWND:

- **Command thread:** `set_dock_mode`'s inline work — `reserve(sliver)` +
  `place(sliver)` (→auto-hide), `reserve(full)` + `reshape()` (→fixed;
  `reshape` calls `set_min_size/set_max_size`, which tao turns into queued
  messages whose handlers call `set_inner_size`).
- **Driver thread (~16 ms tick):** sees the flipped mode and starts its own
  placement animation.

Interleaved writers feed tao's `GetWindowRect`/`GetClientRect`-pair offset
math garbage; release profile has `panic = "abort"` → whole process exits.
Debug builds catch it in tao's event-loop guard and wedge instead.

A second, subtler instance of the same shape exists on **undock**: its
`set_min_size/set_max_size/set_size` calls are processed as queued main-thread
messages while a driver animation tick can still be in flight (rare flake,
observed ~1-in-20 at harness intensity).

## Fix (single-writer placement)

`src-tauri/src/quick_window.rs`:

- **`set_dock_mode` is a pure state flip**: rewrites `DockState.mode`, sets
  `settled = None`, applies the shell registration (`ABM_SETAUTOHIDEBAR`),
  emits, persists. Zero placement/reservation syscalls.
- **Driver owns every placement** via a one-time settle pass
  (`settle_mode`) on the tick where `settled ≠ mode`:
  - →auto-hide: shrink reservation to the sliver once (±4 px grant check,
    log-only fallback), then the existing animation slides the strip from
    wherever it is (replaces the old teleport).
  - →fixed: derive full strip — fresh derivation when arriving narrower than
    strip thickness (sliver/hidden), `desired_rect` flush-keep for an
    already-full-width bar (ticket-61 march avoidance) — reserve + atomic
    place.
- **Handshake before every driver placement** (`docked_state` re-check): an
  undock/close landing mid-slide abandons the rest instead of placing against
  a window another thread is resizing.
- **Drift guard + ABN_POSCHANGED fixed-mode re-assert skip while a
  transition is pending.**
- **Auto-hide edge switches are now driver-animated too** (`reposition`
  no longer places in auto-hide mode — same two-writer shape removed).

### Regression found during verify (also ticket 66)

First settle version left **auto-hide → fixed invisible**: the sliver is
flush with its own shrunken reservation, so `desired_rect`'s keep-rule pinned
"fixed" as a 2 px line. Fixed by deriving fresh thickness when the window is
narrower than a strip. Unit test:
`quick_window::tests::settle_fixed_expands_a_bar_arriving_from_the_sliver`.

An attempt to additionally serialize geometry behind a global mutex was
**reverted**: held-across-`SetWindowPos` locking deadlocks with Windows'
cross-thread message marshaling, and a poisoned lock silently disables all
placement. Single-writer-by-construction + handshake is the final design.

## Verify

- [x] Reproduced with stderr + backtrace captured; root cause recorded above
- [x] Single-writer placement enforced while docked+auto-hide (and transitions both ways)
- [x] fixed → auto-hide animated slide, no crash; auto-hide → fixed full strip + reservation, no crash; rapid toggle stress clean (14/14 green incl. 60 flips @ 15 ms)
- [x] Regression guards at the pure seam (4 new tests in `quick_window::tests`)
- [x] Live user pass on this build: floating → auto-hide OK, auto-hide → fixed OK (after sliver fix), no crash
- [x] `cargo test` green (292 passed); `npm run check` 0 errors
- [x] Synced to the share

VM scripted-cursor hover/reveal passes remain covered by the ticket-63 ACs;
the stress harness above doubles as the scripted VM verify for mode switches
(`tools/repro-dock-mode-stress.ps1 -Runs N [-Iters K] [-IntervalMs M]`; the
`[DEBUG-66]` hook in `lib.rs` is debug-builds + env-gated only and is kept
intentionally as that vehicle).

## Flagged

- The tao re-entrancy overflow (`set_inner_size` add-overflow at
  window.rs:296) proved triggerable through plain queued size messages once
  rect reads race *any* concurrent resize — not specific to our two-thread
  flip. Workaround stays "never place/resize from two threads"; upstream
  note belongs in `docs/research/0003` follow-ups (tao #— not filed yet):
  `set_inner_size` should use checked arithmetic or clamp shadow offsets.
