# 240 — Group color anatomy and row hover

**Parent:** [221 — Collections & media (spec)](221-collections-media-spec.md)

**What to build:** Replace ticket 225's box-in-a-box anatomy with one continuous leading bar, a calm Oklab wash, and a hovered row that carries the group's hue. A colored group draws a single 3 px bar down its full height, its body carries `color-mix(in oklab, var(--group-color) 8%, var(--bg-page))` (14 % dark), the 1 px colored outline is deleted, and hovering a row inside it paints `color-mix(in oklab, var(--group-color) 14%, var(--bg-hover))`. The three main-app collections share one rule through custom properties published by `GroupAccordion`; the dock stays colorless.

**Blocked by:** None — 225 is complete. Do not run concurrently with 229/230/232/233 (they touch `GroupAccordion` and the three collection pages).

**Status:** complete — anatomy, wash, row hover, and the Oklab contrast gate landed; harness GREEN 68/68, gates recorded below.

- [x] A colored group draws one continuous 3 px bar at its leading edge, from the top of the header to the bottom of its last row (pill ends, `--group-color` in both themes). The header-only `::before` stub is gone; neutral groups draw no bar.
- [x] The group body carries `color-mix(in oklab, var(--group-color) 8%, var(--bg-page))` in light and 14 % in dark; the header stays `--bg-page`. The 1 px colored inset outline is deleted and no tinted border replaces it.
- [x] Hovering a row inside a colored group in Clips and Quick Actions paints `color-mix(in oklab, var(--group-color) 14%, var(--bg-hover))`; neutral groups and ungrouped rows paint `var(--bg-hover)`. The Launch home rows stay display-only with no hover state. Hover transitions `background-color` on `--dur-fast`/`--ease-out`.
- [x] Secondary text inside a hovered colored row uses `--text`; on the wash it keeps `--text-muted`. All ten hues clear 4.5:1 against the composited wash and hover surfaces in both themes, and the bar clears 3:1 against the page.
- [x] `GroupAccordion` publishes the derived hover surface and hover text as inherited custom properties (`--group-row-hover`, `--group-row-hover-text`); a neutral group resets both so a neutral child inside a colored parent (229/230) inherits no hue. Pages never select on the component's internal class.
- [x] `@supports not (background: color-mix(in oklab, red, blue))`: the wash is transparent (the bar still identifies the group), hover is `var(--bg-hover)`, and secondary text stays `--text-muted`.
- [x] The three `.rack__row` rules drop the undefined `background: var(--bg-card)` for an explicit `transparent`; no other `--bg-card` user changes.
- [x] Focus-visible rings, menu surfaces, badges, and the dock are unchanged. The dock's group headers still pass no color (§B.9).
- [x] `tools/contrast-check.mjs` mixes in Oklab and gates, for all ten hues in both themes: row text on the wash, muted text on the wash, row text on the hover surface, and a ≥3 Oklab-lightness hover step from the wash. It fails if any pair regresses.
- [x] Evidence: light/dark screenshots of a colored and a neutral group at rest and with a row hovered, in Clips, Quick Actions, and the Launch home (hover only where rows are interactive), with measured values (bar geometry, resolved wash/hover colors, hover text color) in `report.json`.
- [x] `cargo check`, `cargo test`, `npm run check`, `npm run test`, `node tools/contrast-check.mjs`, and `node tools/ownership-gate.mjs` pass.

**Claims (recheck at dispatch):** `GroupAccordion.svelte` (bar, wash, published properties); `src/routes/clips/+page.svelte`, `src/routes/quick-actions/+page.svelte`, `src/routes/+page.svelte` (`.rack__row` hover and the dead `--bg-card`); `tools/contrast-check.mjs`. **Explicitly not built:** group color in the dock; color in export/backup; palette or picker changes (225 owns them); gradients, patterns, alpha, or per-theme hue variants; a user-managed palette; the other `--bg-card` call sites.

## Applied research

- 0028 — geometry (full-height bar, no tinted frame), wash alpha, hover-surface derivation, `in oklab` interpolation, and the dark-mode alpha rule.
- 0028 addendum — the measured reason secondary text is promoted on the tinted hover (3.79–3.88 light / 4.23–4.31 dark for `--text-muted`; no visible tint keeps 4.5).
- 0006 pattern 4 (contextual state lives on the object) — the hover state is painted by the row the user is pointing at, not by a page-level chrome change.
- 0005 rule 5 (same-kind controls share one treatment) — one hover rule for both interactive collections instead of two ad-hoc colors.

## Evidence — 2026-10-24

### Implementation

- `src/lib/components/GroupAccordion.svelte` — `.group--colored` is now `position: relative` with `padding-left: var(--space-3)`; a single `::before` bar is `top: 0; bottom: 0; width: 3px` with `border-radius: var(--radius-pill)`, so it spans header + body instead of the old header stub. The body keeps `padding: var(--space-1) var(--space-2)` with `padding-left: calc(var(--caret-column) + var(--space-1))`, so titles and rows still share one start. `.group__rows` carries `color-mix(in oklab, var(--group-color) 8%, var(--bg-page))`, with a `:global(:root[data-theme="dark"])` rule at 14 %. The `box-shadow: inset 0 0 0 1px` outline is gone. `.group--colored` publishes `--group-row-hover: color-mix(in oklab, var(--group-color) 14%, var(--bg-hover))` and `--group-row-hover-text: var(--text)`; `.group:not(.group--colored)` resets both to `initial` so a neutral child inherits no hue. `@supports not (background: color-mix(in oklab, red, blue))` falls back to transparent wash, `var(--bg-hover)` hover, `--text-muted` hover text. Doc comment updated.
- `src/routes/clips/+page.svelte` — `.rack__row` drops the undefined `background: var(--bg-card)` for `background: transparent`, declares `--row-text: var(--text-muted)`, transitions `background-color`, and gains `.rack__row:hover { background: var(--group-row-hover, var(--bg-hover)); --row-text: var(--group-row-hover-text, var(--text-muted)); }`. `.rack__content` reads `var(--row-text)`.
- `src/routes/quick-actions/+page.svelte` — same change, keeping the existing `border-color` hover; `.rack__command` and `.rack__note` read `var(--row-text)`.
- `src/routes/+page.svelte` — `.rack__row` drops the dead token for `transparent`; no hover rule, because Launch home rows are display-only and a hover wash there would imply a click that does not exist.
- `tools/contrast-check.mjs` — adds `bg_hover` to both palettes, Oklab helpers (`rgbToOklab`, `oklabToRgb`, `mixOklab`, `lightness`), and per-hue checks for row text on the wash, muted text on the wash, row text on the hover surface, and a ≥3 Oklab-lightness hover step.

The page-local `--row-text` alias is deliberate: writing `--text-muted: var(--group-row-hover-text, var(--text-muted))` on the page would self-reference and break the cascade.

### Applied research

Research [0028](../../../docs/research/0028-collection-color-systems.md) and its 2026-10-24 addendum: full-height bar over a framed box, no tinted outline, `in oklab` interpolation, the higher dark alpha, and the measured reason secondary text is promoted on the tinted hover.

### Harness — `tools/repro-group-color-hover.mjs`

Seeds a coloured group (`#417ed4` Launch, `#1c9139` Quick Actions, `#e03a8d` Clips) plus a neutral group in all three collections against a throwaway `%LOCALAPPDATA`, then measures the rendered result through CDP in both themes. Boot 23.1 s. **VERDICT: GREEN — 68/68 checks.** Evidence: `%TEMP%\opencode\evidence-240` (15 PNGs + `report.json`), log `%TEMP%\opencode\repro-240.log`.

Measured, both themes, all three collections:

- Bar: `width: 3px`, `top: 0px`, `bottom: 0px`, height equal to the section (e.g. 164.667 px bar in a 165 px section), colour equal to the stored hue.
- Wash: exact Oklab match and opaque — light `rgb(239, 245, 252)` / `rgb(238, 246, 239)` / `rgb(255, 241, 245)`; dark `rgb(31, 38, 48)` / `rgb(30, 40, 31)` / `rgb(50, 32, 39)`.
- Outline: `box-shadow: none` in every group.
- Alignment: coloured title x = row x = 278, neutral title x = row x = 266 — the inset did not shift content.
- Published contract: coloured group exposes `color-mix(in oklab, #417ed4 14%, #f1f1ef)` (light) / `..., #2e2e2e` (dark); neutral group exposes `""`.
- Hover: exact expected mix — light `rgb(214, 228, 213)` (actions) and `rgb(243, 218, 225)` (clips); dark `rgb(48, 59, 49)` and `rgb(70, 51, 58)`.
- Hover text: promoted to `--text` (`rgb(55, 53, 47)` light, `rgba(255, 255, 255, 0.81)` dark) and measures **9.29–9.30:1** light / **8.32–8.35:1** dark.
- Hover step from the wash: **dL 5.95–6.10** light, **7.48–7.64** dark (needs 3).
- Neutral hover: `--bg-hover` exactly (`rgb(241, 241, 239)` / `rgb(46, 46, 46)`), text stays `--text-muted` at **4.52:1** light / **4.65:1** dark.
- Launch home: hovered row stays `rgba(0, 0, 0, 0)` — display-only rows take no hover wash.

### Gates

- `node tools/contrast-check.mjs` — all checks pass. Per-hue, both themes: row text on wash 9.10–11.19 light / 8.21–10.59 dark, muted text on wash 4.59–4.66 / 4.89–4.99, row text on hover 9.10–9.32 / 8.21–8.45, hover dL 5.75–6.38 / 7.28–7.65, bar 4.07–4.89 light / 3.59–4.32 dark against the page.
- `npm run check` — 0 errors, 3 pre-existing warnings (`QuickActionFormDialog` state-capture ×2, `UnifiedHeader` redundant role).
- `npm test` — 38 files, 457 tests passed.
- `cargo check` — clean, no warnings. `cargo test` — 722 passed, 0 failed, 3 ignored (the 84 `tempfile::TempDir::into_path` deprecations are pre-existing test-only warnings).
- `node tools/ownership-gate.mjs` — pass.


## Amendment — 2026-10-24 (the tint moves from the region to the cards)

The anatomy shipped here put the 8 %/14 % mix on the group **body** (`.group--colored .group__rows`), which tinted the region — the empty space above, below and between cards included. In use it read as a coloured panel rather than coloured items, and in the 340-420 px dock strip it filled most of the window.

The hue now sits on each row card: `.group--colored` publishes `--group-row-bg` (8 % light / 14 % dark over `--bg-page`), and each surface sets `background: var(--group-row-bg, transparent)` on its own row rule — the three main-app `.rack__row` blocks and the dock's `.dock-row`. The body stays on the page surface. The bar, the hover surface, the hover text promotion, the 8 %/14 % values and the `@supports` fallback are unchanged, so every contrast ratio recorded above still holds (same colour behind the text).

Authoritative record: [spec 221, amendment 2026-10-24 (colour sits on the items, not the region)](221-collections-media-spec.md). The remaining duplication — the row chrome repeated across three pages and the dock — is captured in [ticket 242](242-shared-collection-row-shell.md).
