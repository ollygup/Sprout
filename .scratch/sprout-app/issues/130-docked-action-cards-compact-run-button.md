# 130 — Compact docked action cards: fixed icon-only Run/Stop

**What to build:** Docked + floating Quick Action rows become `[flex text | fixed full-height icon Run/Stop]` with spinner for Stopping — as compact as Launch/Clip rows, same grammar, at any dock width.

**Blocked by:** none — width-agnostic by construction (works before/after ticket 128).

**Status:** done

## Scope

- Surfaces: Quick Launch window + dock rows only; main-app `/quick-actions` keeps icon+text Run/Stop/`Stopping…` with `--run-w`.
- Fixed-width icon button (icon + existing `Button` padding, full card height, clears 44px AAA both axes); flex text side absorbs resizable width with `truncate`/`min-w-0`; states Run `primary`/Stop `danger`/Stopping disabled spinner (tokens only, no new variant); `aria-label` + tooltip mirror (`Run/Stop/Stopping <name>`), `:focus-visible` ring, no hover dependency; reduced-motion freezes spinner (0004:4/5, 0005:2, 0006:6/13/14, WIG, NN/g target size).

## ACs

- [x] Dock + floating rows match Launch/Clip compactness; text → details dialog, button → Run/Stop; detached (`-d`) still report not-running.
- [x] Run→Stop→Stopping never reflows or truncates controls at 340px and at wide dock; keyboard + touch + reduced-motion pass.
- [x] `npm.cmd run check` 0 errors.

## Implementation notes

- `QuickActionRunControl.svelte` gains a `compact` prop (window/dock only;
  main-app `/quick-actions` keeps icon+text with `--run-w` untouched):
  icon-only Run `primary` / Stop `danger` / Stopping disabled spinner via
  the shared `Button` (no new variant, tokens only), `aria-label` + `title`
  mirror (`Run/Stop/Stopping <name>`), `aria-describedby` still points at
  the row tip, `:focus-visible` ring, spinner frozen under
  `prefers-reduced-motion` (0004:4/5, 0005:2, 0006:6/13/14, WIG, NN/g
  target size). Fixed width by construction (15px icon box incl. the
  spinner + existing Button padding, 44px floors) — the compact branch
  skips the ticket-124 measurement entirely, so Run→Stop→Stopping cannot
  reflow at 340px or wide.
- `quick-launch-window/+page.svelte` action rows are now
  `[flex text | fixed full-height icon Run/Stop]`: the text side is a real
  `<button>` (`flex:1 + min-w-0`, truncate) opening a read-only
  `QuickActionDetailsDialog` (0006:13, 0004:3 — no Edit there; full
  configuration stays in the main app), the icon button alone runs/stops;
  card border/radius/padding tokens match the Launch/Clip cards and the
  row keeps `min-height: 44px`. Two sibling buttons, never nested (the
  entire-card-is-Run and hover-reveal alternatives stay rejected per
  0004's applied cases). An open dialog re-resolves by id on background
  reloads and closes when its action is deleted. Tracking semantics
  unchanged — detached (`-d`) commands still report not-running.
- `QuickActionDetailsDialog.svelte`: `onedit` optional (omitted = no Edit
  button, "No note." hint); the main-app caller still passes it.
- Results: `npm.cmd run check` 0 errors; `npm.cmd test` 89 passed.
  Manual pass user-verified (2026-09-06): Run/Stop/Stopping cycle at narrow
  + wide dock, Fixed + auto-hide + floating, keyboard-only, reduced-motion.
- Follow-up in ticket 134: the 44px height floor retires (uniform
  content-driven heights per density) and the three row geometries merge
  into one shared `QuickLaunchRow` shell.

## Verification

- `npm.cmd run check`, run-control/frontend tests
- Manual: Run/Stop/Stopping cycle at narrow + wide, fixed + auto-hide + floating; screen-record no-jitter.
