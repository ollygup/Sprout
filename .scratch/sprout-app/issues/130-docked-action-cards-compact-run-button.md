# 130 — Compact docked action cards: fixed icon-only Run/Stop

**What to build:** Docked + floating Quick Action rows become `[flex text | fixed full-height icon Run/Stop]` with spinner for Stopping — as compact as Launch/Clip rows, same grammar, at any dock width.

**Blocked by:** none — width-agnostic by construction (works before/after ticket 128).

**Status:** ready-for-agent

## Scope

- Surfaces: Quick Launch window + dock rows only; main-app `/quick-actions` keeps icon+text Run/Stop/`Stopping…` with `--run-w`.
- Fixed-width icon button (icon + existing `Button` padding, full card height, clears 44px AAA both axes); flex text side absorbs resizable width with `truncate`/`min-w-0`; states Run `primary`/Stop `danger`/Stopping disabled spinner (tokens only, no new variant); `aria-label` + tooltip mirror (`Run/Stop/Stopping <name>`), `:focus-visible` ring, no hover dependency; reduced-motion freezes spinner (0004:4/5, 0005:2, 0006:6/13/14, WIG, NN/g target size).

## ACs

- [ ] Dock + floating rows match Launch/Clip compactness; text → details dialog, button → Run/Stop; detached (`-d`) still report not-running.
- [ ] Run→Stop→Stopping never reflows or truncates controls at 340px and at wide dock; keyboard + touch + reduced-motion pass.
- [ ] `npm.cmd run check` 0 errors.

## Verification

- `npm.cmd run check`, run-control/frontend tests
- Manual: Run/Stop/Stopping cycle at narrow + wide, fixed + auto-hide + floating; screen-record no-jitter.
