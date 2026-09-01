# 124 — Run/Stop same min-width (no jitter)

**What to build:** Run→Stop never changes button width; color carries the meaning.

**Blocked by:** none.

**Status:** ready-for-agent

## Scope

- Surfaces: `src/lib/components/QuickActionRunControl.svelte` (used by `quick-launch-window/+page.svelte:407` `stop(action)` and `quick-actions/+page.svelte:152` `stop(action)`), `src/lib/components/Button.svelte:36` base `btn` (`padding:8px 16px`, `gap:var(--space-2)`, `font-weight:600`) with variants `btn--primary` (`Button.svelte:59` `var(--accent)`) and `btn--danger` (`Button.svelte:79` `var(--danger-text)`).
- Measure the rendered Run control's width (longest of `Run` / `Starting…` per locale) once on mount/visible and set `--run-w` on the control root; Stop (`Stop`/`Stopping…`+spinner) applies `min-width:var(--run-w)`. Color flip `primary→danger` is the only visible change (`0006:6` one reserved accent, `0005:2` one primary per header/row).
- No new `Button` variant; reuse `tokens.css` radius/type tokens per AGENTS.md design rule.

## ACs

- [ ] Toggling Run→Stop→Stopping (`quickActionRuns.svelte` `stopping` set) keeps the button's outer width stable at 340px dock and at main-app width — no layout shift.
- [ ] Run = `primary` accent-filled, Stop = `danger` danger-filled, Stopping = disabled `0.5` opacity with spinner (existing treatment).
- [ ] `npm.cmd run check` 0 errors; `Button.svelte` snapshot unchanged except added `style:min-width` on the run-control wrapper.

## Verification

- `npm.cmd run check`
- Manual: click Quick Action Run → Stop, observe no width delta (screen record at 340px).

