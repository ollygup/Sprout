# 241 — Dock group colour (read-only)

**Parent:** [221 — Collections & media (spec)](221-collections-media-spec.md)

**What to build:** A group's colour must be visible in the Quick Launch strip / dock, not only in the main app. The three dock tabs pass the stored colour to their `GroupAccordion`, which renders ticket 240's anatomy in its flush variant: one full-height 3 px bar, the 8 %/14 % per-card tint, and the hue-carrying row hover. Every group control — menu, colour picker, rename, move, delete — stays in the main app; the dock only displays the colour.

**Blocked by:** 240 (the anatomy and the published hover properties).

**Status:** complete

- [x] All three dock tabs (Launch, Quick Actions, Clips) pass the group's stored colour to `GroupAccordion`; a coloured group renders the bar and the per-card tint, a neutral one renders neither.
- [x] The flush variant keeps the strip's geometry: the coloured section takes a `--space-2` leading inset for the bar, the section body adds no padding, and the group's title and rows share one start. Row width is not reduced beyond that inset.
- [x] Each row card inside a coloured dock group carries `color-mix(in oklab, var(--group-color) 8%, var(--bg-page))` in light and 14 % in dark; the section body stays on the page surface, and no coloured outline or tinted border appears.
- [x] Hovering a dock row inside a coloured group paints `var(--group-row-hover)` over the strip's existing accent border, and the row's secondary text uses `var(--group-row-hover-text)`. Neutral and ungrouped dock rows keep the clear card and `--text-muted`.
- [x] Collapsing a coloured dock group leaves the bar spanning the header only, with no floating stripe.
- [x] The dock still exposes no group menu, no colour picker, and no group editing of any kind (§B.9). No write path is added.
- [x] No new colours or tokens: the existing contrast gate covers every hue on the card tint and the hover surface in both themes.
- [x] Evidence: light/dark dock screenshots with a coloured and a neutral group, at rest and with a row hovered, with measured bar geometry, resolved tint/hover colours, and hover text in `report.json`.
- [x] `cargo check`, `cargo test`, `npm run check`, `npm test`, `node tools/contrast-check.mjs`, and `node tools/ownership-gate.mjs` pass.

**Claims (recheck at dispatch):** `src/routes/quick-launch-window/+page.svelte` (three `GroupAccordion` call sites, row text rules), `src/lib/components/GroupAccordion.svelte` (flush coloured anatomy), `src/lib/components/QuickLaunchRow.svelte` (hover surface and text alias), the 240 harness. **Explicitly not built:** group menus or colour editing in the dock; a second dock palette; per-theme hue variants; colour in the floating-window chrome outside the group regions.

## Applied research

- 0028 — the same anatomy in a narrower container: one bar, a per-card tint, no tinted frame; the bar is the identifier and the items carry the hue (221 amendment, 2026-10-24).
- 0028 addendum — the hover surface needs the text promotion in the dock too, because the composited 14 % mix drops `--text-muted` below AA there as well.
- 0005 rule 5 — one row treatment for the strip and the page: the dock reuses the published properties instead of inventing a second tint.
- 0004 rule 3 — colour is display-only in the dock because every write affordance lives one surface up, in the main app.

## Evidence — 2026-10-24

**Harness.** `node tools/repro-group-color-hover.mjs` → **GREEN, 154 checks, 0 red**. Boot 145 s (first build after the 1.2.1 bump); dock attached at `http://localhost:1420/quick-launch-window`. Screenshots in `%TEMP%\opencode\evidence-240\` (`dock-{launch,actions,clips}-{light,dark}-{rest,hover,neutral-hover}.png` plus the main-app set); log `%TEMP%\opencode\repro-241c.log`.

**Dock measurements** (three tabs × two themes, 306 px panel content):

- Bar: 3 px wide, `top`/`bottom` 0, height 115.667 px against a 116 px section — it spans the whole section in every tab and theme.
- Section body transparent (`rgba(0, 0, 0, 0)`) for coloured and neutral groups; the hue is on the cards only.
- Card tint exact against the computed Oklab mix: Work `rgb(239, 245, 252)` light / `rgb(31, 38, 48)` dark; Deploy `rgb(238, 246, 239)` / `rgb(30, 40, 31)`; Snippets `rgb(255, 241, 245)` / `rgb(50, 32, 39)`. Neutral rows resolve transparent.
- Only coloured groups publish `--group-row-bg`; neutral groups publish nothing.
- Flush inset 8 px with equal right edges: Launch coloured 25–291 vs neutral 17–291; Actions/Clips 21–295 vs 13–295. The header-to-row offset is identical in both (18 px), so the inset moves the section without breaking its internal alignment.
- Hover: exact 14 % mix per hue and theme (e.g. Work light `rgb(216, 225, 237)`); the Clips excerpt is promoted to `--text` (9.30:1 light / 8.35:1 dark); neutral rows keep the clear card and `--text-muted` (5.11:1 / 5.23:1).
- No group menu, colour picker or editing affordance in any dock group.

**Main app (regression check from the same run).** Body transparent, card tint exact, neutral card transparent, hover exact, hovered text 9.29–9.30:1 light / 8.32–8.35:1 dark, hover dL 5.95–7.64, no coloured outline, display-only Launch rows keep their tint.

**Gates.** `node tools/contrast-check.mjs` — all pass (ten hues × two themes on the card tint and the hover surface). `npm run check` — 0 errors, 3 pre-existing warnings. `npm test` — 457/457 (a first run reported 456/457 with one unrelated flake; the immediate re-run was fully green). `cargo test` — 722 passed, 0 failed, 3 ignored. `node tools/ownership-gate.mjs` — pass (62 owned references). App version bumped to 1.2.1 in `src-tauri/Cargo.toml` (the single source).
