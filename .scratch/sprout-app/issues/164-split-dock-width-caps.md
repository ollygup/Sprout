# 164 — Split dock-width caps by mode

**What to build:** The dock-width slider caps at 30% in fixed mode and 60% in auto-hide, with the maximum following the current mode and caps persisting per monitor — fixed stays a strip, auto-hide may overlay wide.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

## Scope

- Per-mode caps in `constants/window.rs` (still the single size source); Settings slider max follows the mode; backend clamps broken stored values exactly as today; undock-narrows-to-floor preserved.
- Record the overlay-vs-reservation width policy as new UI/UX research (citing `window.rs` cap math plus ADR-0011/0021).
- Explicitly not built: caps above 60%, per-site widths, any second size source.

## ACs

- [x] Fixed slider refuses above 30%; auto-hide slider allows to 60%; switching modes re-clamps honestly.
- [x] Corrupt stored values clamp instead of collapsing or exploding the strip; per-monitor memory intact.
- [x] `npm.cmd run check` 0 errors; dock-width contract tests green.

## Verification

- `npm.cmd run check`, `dock_width_px` tests plus new per-mode coverage; manual fixed/auto-hide/floating across monitor widths.
