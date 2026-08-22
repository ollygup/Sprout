# 63 — Dock auto-hide: Sprout-driven overlay sliding, independent of the taskbar

Consolidated 2026-08-21 from this ticket's original scope (refusal honesty,
shipped) and the 2026-08-20 session handoff (superseded — validated facts
folded in below). Parent spec: 55. **Overlay semantics added later on
2026-08-21** (see Decisions, second block) — they supersede every
"space reclaimed / maximized app reclaims" wording from the first block.

**User requirement (verbatim):**

> "Taskbar can be configured freely (hide or not hide) and for my docking
> (with its own configuration), they are not tied to each other, never."

**Decisions confirmed by the user (2026-08-21):**

- Taskbar feel with a smooth slide: cursor touches the screen edge → strip
  slides out (~180 ms ease-out); leaves the strip → slides back away. No
  hover delay, but the motion itself is animated, not an instant jump.
- Hide anyway: even when the shell refuses auto-hide registration, the strip
  still slides — Sprout owns the motion; the shell's opinion affects only
  coordination semantics. Banner informs either way.
- Motion must be Sprout-driven (~16 ms cursor polling): the WebView2 child
  HWND swallows mouse messages, so message-driven hover detection on the
  top-level window cannot work. Confirmed by research (see Requirements).

**Decisions confirmed by the user (2026-08-21, later — overlay semantics,
superseding the reclaim wording of the earlier round):**

- `docked + fixed` takes up fixed screen space on the declared side
  (unchanged reservation behavior; unaffected by the taskbar's own hide
  setting).
- `docked + auto-hide` overlays: the main application stays full width at
  all times — hidden *and* revealed. On hover the strip slides out **on
  top of** the app; nothing ever shrinks or resizes (taskbar parity).
  Auto-hide therefore registers with the shell for coordination only and
  never reserves workspace space (`ABM_SETPOS` is a fixed-mode call).
- The sliding animation must be visibly smooth: no Sleep-quantization
  stutter (~15.6 ms default timer resolution) while animating.
- Reveal trigger is hysteresis (refined after live testing): ONLY a touch at
  the very screen edge reveals a hidden strip — mere proximity within the
  area the strip would occupy must never pop it out, or it shadows the
  overlaid app's own chrome (close/minimize buttons). Once out, it stays out
  while the cursor is anywhere over the strip; leaving the strip hides it.

## Validated facts (VM, Windows 11 build 26200)

**Works:** AppBar registration/dock/edge-switch/undock; drift guard;
ABN_* handling; engagement verification (`ABM_GETAUTOHIDEBAR`); refusal
honesty — transient blocked state, banner with reason + switch-edge action,
requested mode never rewritten/persisted to `fixed` (shipped 2026-08-21).

**Does not work:** *nothing ever hides the strip.* The OS does not slide or
hide registered auto-hide appbars on this build; motion code is Sprout's job.
Supporting evidence:

- With taskbar auto-hide ON, `ABM_GETAUTOHIDEBAR(right)` returns 0 — the
  Win11 taskbar is not a classic queryable AppBar; it can neither conflict
  with nor influence our docking (and stock Win11 ignores registry attempts
  to move the taskbar to left/right edges).
- Registry (`StuckRects3` byte[12]) edge moves are ignored by stock Win11.
- A dummy AppBar holding the edge did not reproduce refusal either.
- DPI: 150% scaling; physical = virtualized × 1.5 for DPI-unaware harnesses.
- Tooling (dev machine `%TEMP%\opencode\`, dev-only): `cdp.mjs` CDP driver,
  `AppBarHarness.cs`/`harness.ps1` (hold/release/query edges),
  `dump-dock-meta.cjs`, `shot.ps1`.

## Implementation outline

- `window_constants.rs`: `AUTOHIDE_SLIVER_PX`, `EDGE_TRIGGER_PX`,
  `AUTOHIDE_POLL_MS`, `AUTOHIDE_ANIM_POLL_MS`, `AUTOHIDE_SLIDE_MS`.
- `appbar.rs`: registration split — `register` (`ABM_NEW`, both modes) vs
  `reserve` (`ABM_QUERYPOS`/`SETPOS`, fixed only); `monitor_rect` (full
  screen rect — the overlay strip's geometry base); pure helpers + unit
  tests: `sliver_rect`, `edge_hit`, `strip_contains`, `ease_out_cubic`,
  `slide_rect`.
- `quick_window.rs`: auto-hide driver thread beside the drift guard — polls
  the cursor every ~16 ms (8 ms while animating, `timeBeginPeriod(1)` raised
  during slides), acts only while docked && mode == "auto-hide", animates the
  window between the monitor-spanning strip and its 2 px sliver via
  `appbar::place`, runs regardless of shell engagement; immediate hide after
  dock/edge-switch/mode-set (driver reaches it within one tick); mode swaps
  manage the reservation atomically (fixed→auto-hide shrinks it to the
  sliver, auto-hide→fixed reserves the full strip); `fixed` mode inert for
  the driver; undock/close restore full rect; drift guard and `ABN_POSCHANGED`
  placement skip auto-hide entirely (no reservation to heal).

## Requirements

- [x] Refusal keeps the intended mode; blocked state transient, never persisted (shipped 2026-08-21)
- [x] Quick Launch window shows a blocked banner with reason + switch-edge action (shipped 2026-08-21)
- [x] Re-try engagement on `ABN_STATECHANGE`/`ABN_POSCHANGED` (end-to-end retest folds into the VM verify below)
- [x] **Research (docs/research):** `docs/research/0003-appbar-autohide-os-contract.md` — the appbar implements hide/reveal itself per official docs; `ABM_SETAUTOHIDEBAR` grants exclusivity/z-order/notifications only; WebView2 parent never sees WM_MOUSEMOVE (WebView2Feedback #5232)
- [x] Docs corrected everywhere: module docs no longer claim the OS owns the slide or visibility; spec 55 bullet, CONTEXT.md dock glossary entry, and an ADR-0011 amendment record the overlay contract
- [x] VM verify (2026-08-21, scripted cursor): both edges pass — proximity inside the would-be strip does NOT reveal (hysteresis fix), edge touch slides out (~227 vpx / 340 physical), stays out over the strip, re-hides on departure; hidden AND revealed leave the work area untouched in auto-hide (pure overlay); `fixed` pins the full 340×1848 strip and shrinks the workspace by exactly its width; mode restore back to auto-hide re-hides within one tick. Refused-edge repro remains unattainable on build 26200 (validated facts) — banner honesty stays covered by the shipped refusal round + unit tests. Slide *feel* (fluidity) awaits the user's judgment after the pacing fixes (1 ms timer resolution during slides, ~60 fps tick pacing, no-op move skip).
- [x] `cargo test` green (287 passed); `npm run check` 0 errors; synced to the share

## Flagged (out of scope here)

- **Regression, tracked as 66:** switching docked+fixed → docked+auto-hide
  live crashes the whole app — `set_dock_mode`'s inline sliver re-reserve +
  placement races the driver thread (two writers on one HWND). Fix there with
  the `diagnosing-bugs` skill; do not hotfix inside 63.
- `DOCK_WIDTH` (340) is applied as *physical* pixels for the docked strip
  while the floating window's 340 is *logical*: at 150% scaling the docked
  strip is physically narrower than the floating window (harness measured
  227 vs 355 virtualized). Candidate follow-up ticket.
- Multi-monitor correctness of the non-EX `ABM_*` messages (research flag):
  adopt `ABM_*EX` variants before claiming per-monitor docking beyond the
  primary monitor.
