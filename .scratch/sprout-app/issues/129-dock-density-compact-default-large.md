# 129 — Dock density: Compact / Default / Large

**What to build:** Settings gains a List density knob beside the other dock knobs that rescales dock + floating list text via existing type tokens only.

**Blocked by:** none — can start immediately.

**Status:** done (user-verified docked/fixed; see test report)

## Scope

- Surface: Settings dock section (0008:1 app-global concern — one knob reshaping all three window tabs sits with dock mode/edge/width, deferred to Save like them); options Compact/Default/Large mapping onto `--text-*` tokens; dock + floating window lists only; main app untouched; default Default; persisted. The first cut used the Quick Launch page's features menu per 0006:8 — refused in review under 0008:1's classifier (the per-collection Groups switches are the view-scoped ones; a cross-tab knob is window-global), recorded in 0008's applied history.
- Tokens/components only; no ad-hoc px; truncation/`min-w-0` behavior preserved at each step (WIG content handling); shared chrome unchanged (0005).
- Row-height compactness for Quick Actions is out of scope here — action rows stay button-height until ticket 130 lands its icon-only Run/Stop; 130 kept a 44px floor, and ticket 134 converges all three tabs to one shared, content-driven height per density.

## ACs

- [x] Each density visibly rescales dock + floating rows, persists across restart, never clips controls at 340px or the ticket-128 wide width.
- [x] Contrast stays AA via existing token pairs (contrast-check tool if tokens touched — they aren't).
- [x] `npm.cmd run check` 0 errors.

## Implementation notes

- Placement follows 0008:1 (one knob reshaping all three window tabs is a window-global concern → Settings, beside dock mode/edge/width, deferred to Save like them — the same home ticket 128's width uses). The dock/floating window owns no configuration surface per CONTEXT, so it only re-reads through `quick-launch-changed`/`reconcile_quick_launch_settings`.
- Control is the neighbors' `Select` (`variant="small"`, Compact/Default/Large) in a standard `.knob` row (0005:5 same-kind treatment), participating in load/baseline/dirty/Discard/Save exactly like the width knob; broken stored values fall back to Default on load and before save.
- Persistence: `Settings.dock_density` (`"compact"|"default"|"large"`, default `"default"`), validated + load-fallback + full-save roundtrip coverage. Machine-local like the other dock knobs — never in Preset exports.
- Rendering: `.qlw--density-compact/large` re-point three aliases (`--qlw-name/meta/micro`) one `--text-*` step down/up; nine row-text sites (entry/action/clip names, excerpts, Starting…/Copied, tips, list count) read the aliases. Geometry, truncation, `min-w-0`, badges, buttons, group headers (shared `Disclosure`, 0005), and the main app are untouched — larger text truncates earlier, never clips; no color changes, so AA holds by construction.
- Results: `npm.cmd run check` 0 errors; `cargo test` 433 passed / 0 failed (new `invalid_stored_dock_density_falls_back_to_default` + validation/roundtrip coverage); `npm.cmd test` 89 passed. Live narrow/wide + Fixed/auto-hide/floating + keyboard-only pass recommended on the next dev run (headless session — verified by construction and tests, not by eye).

## Verification

- `npm.cmd run check`, related frontend tests
- Manual: set each density in Settings → Save at narrow + wide dock, fixed + auto-hide + floating, keyboard-only.

## Test report

- Automated: `npm.cmd run check` 0 errors; `cargo test` 433 passed / 0 failed; `npm.cmd test` 89 passed.
- Manual (user, 2026-09-05): docked + fixed mode — each density changes list fonts correctly, persists via Settings save.
- Manual (user, 2026-09-05): all remaining matrix validated — wide dock (toward 30%) at Large truncates with ellipsis, Run/Stop and clip controls never clipped at 340px or wide; auto-hide reveal clean at each density without reopening; floating window renders all three densities; all three tabs (Launch / Actions / Clips) at Compact and Large; restart recalls Compact/Large in dock (and floating if left floating); keyboard-only Tab to List density `Select`, arrows + Save, dirty bar appears/clears with visible focus ring.
