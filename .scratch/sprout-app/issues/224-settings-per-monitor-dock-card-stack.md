# 224 — Settings per-monitor dock card stack

**Parent:** [221 — Collections & media (spec)](221-collections-media-spec.md)

**What to build:** The per-monitor dock card in Settings stacks vertically instead of squeezing its label, edge/mode selects, width slider, and bezel slider into one horizontal row. At one or two displays, 100–200 % DPI, the card stays inside its column with no horizontal overflow; the existing width readout and per-mode caps are unchanged. This is the confirmed cause of the dock-size settings bug.

**Blocked by:** None — can start immediately.

**Status:** complete — stacked layout landed; matrix and gates green (see Evidence).

- [x] The per-monitor card uses the existing `knob knob--stack` variant (no new card component, no new CSS pattern, no new token).
- [x] The label, edge/mode selects, width row, and bezel row flow vertically at full width; every control remains reachable and readable.
- [x] No horizontal overflow at 340 px content width, 1 and 2 displays, 100/150/200 % DPI, light and dark.
- [x] The width readout keeps the existing `effectiveWidthPx` formula and per-mode caps; no dock-width behavior changes.
- [x] Settings search still finds the global and per-monitor dock-width controls; their labels/IDs are unchanged.
- [x] `npm run check`, the relevant settings/search tests, `node tools/contrast-check.mjs`, and `node tools/ownership-gate.mjs` pass.
- [x] Manual evidence is recorded for the matrix above.

**Claims (recheck at dispatch):** Settings per-monitor card; the existing `.knob--stack` style; settings search tests. **Explicitly not built:** a new responsive card component; any dock-width formula, cap, token, or window-size change; a redesign of the other Settings knobs.

## Evidence — 2026-10-24

**Change.** Each per-monitor `<article>` in `src/routes/settings/+page.svelte` now renders `class="knob knob--stack"` (it was plain `knob`). No markup structure, control, ID, CSS rule, token, or formula changed; the existing stack variant does the work. `settingsSearch.ts` keys (`dock-width`, `per-monitor`) are untouched, so the existing search test still covers both controls.

**Automated gates (all in `C:\Sprout`).**

- `npm run check` — 0 errors (3 pre-existing warnings in `QuickActionFormDialog.svelte` / `UnifiedHeader.svelte`, unrelated).
- `npm run test` — 36 files / 447 tests pass. New `src/lib/settingsPerMonitorStack.close.test.ts` (4 tests): stack variant on every display card, four full-width rows with two selects / two sliders, 18 % readout (`~345 px` @1920, `~460 px` @2560), and the fixed-mode re-clamp (55 → 30 %, `~576 px`).
- `node tools/contrast-check.mjs` — pass, light + dark.
- `node tools/ownership-gate.mjs` — pass (62 owned references).
- One pre-existing stale assertion repaired in `src/lib/managedSettings.close.test.ts`: it still expected "Available to install" while shipped copy (en/zh-CN) says "Available models". Test text only; no behavior change.

**Manual matrix.** Driven against the real app (`npm run tauri dev`) via the new CDP probe `tools/repro-settings-per-monitor-card.mjs`; the two-display pass uses the in-app `?preview-per-monitor=1` dev preview, and the card is forced to a 340 px column to test the narrow case directly. `scrollW/clientW` is the card's own horizontal scroll pair; `escaping` counts rows/controls past the content box.

| Displays | DPI | Theme | Card width | Result |
| --- | --- | --- | --- | --- |
| 1 | 100 / 150 / 200 % | light, dark | n/a — no per-monitor card; global card in-layout (576 px) | Green — `575/575`, nothing escaping |
| 2 | 100 / 150 / 200 % | light, dark | 340 px | Green — 2 cards, `flex-direction: column`, content 308 px, `339/339`, 0 escaping |
| 2 | 100 / 150 / 200 % | light, dark | in-layout (576 px) | Green — 2 cards, content 544 px, `575/575`, 0 escaping |

All 24 runs printed `VERDICT: GREEN`. DPI was applied with CDP device-scale emulation (1 / 1.5 / 2); layout in CSS px is identical at each step, which the unchanged measurements confirm. Light/dark were switched through the real theme picker and the stored mode was restored afterwards; collapsed Settings sections opened for the run were collapsed back.

**Probe sensitivity (proves the matrix detects the reported bug).** Re-running with `--legacy-row`, which re-injects the pre-fix row layout, reports RED in every two-display cell: card scroll `830/339` at 340 px and `830/575` at 576 px, with the width and bezel rows escaping the card (e.g. `…@781.8–1097`). With the fix the same probe is green — so the green matrix is not a vacuous pass.

**Visual check.** 200 % light/dark captures of the two-display cards at 340 px: label, edge/mode selects, width row, and bezel row read top-to-bottom, each control full width and legible; no label clipping or horizontal scrollbar.

Reproduce: `node tools/repro-settings-per-monitor-card.mjs --mode dev` (`--legacy-row` for the sensitivity check, `--shots DIR` for PNGs).
