# 129 — Dock density: Compact / Default / Large

**What to build:** The Quick Launch surface features menu gains a density switch that rescales dock + floating list text via existing type tokens only.

**Blocked by:** none — can start immediately.

**Status:** ready-for-agent

## Scope

- Surface: on-surface features menu (0006:8 view-scoped switch), options Compact/Default/Large mapping onto `--text-*` tokens; dock + floating window lists only; main app untouched; default Default; persisted.
- Tokens/components only; no ad-hoc px; truncation/`min-w-0` behavior preserved at each step (WIG content handling); shared chrome unchanged (0005).

## ACs

- [ ] Each density visibly rescales dock + floating rows, persists across restart, never clips controls at 340px or the ticket-128 wide width.
- [ ] Contrast stays AA via existing token pairs (contrast-check tool if tokens touched — they aren't).
- [ ] `npm.cmd run check` 0 errors.

## Verification

- `npm.cmd run check`, related frontend tests
- Manual: cycle densities at narrow + wide dock, fixed + auto-hide + floating, keyboard-only.
