# 134 — Dock rows: shared shell + uniform density-following heights

**What to build:** Extract the Quick Launch window/dock's three row geometries (Launch / Actions / Clips) into one `QuickLaunchRow` shell — box, states, tooltip, split/full layout — with the three tabs as thin adapters; drop the 44px height floors so every tab shares one content-driven height per density.

**Blocked by:** none.

**Status:** done

## Scope

- Surfaces: Quick Launch window + dock rows only, all three tabs. Main app and backend untouched.
- Shell owns geometry + interaction states only: card tokens (padding/border/radius), hover/`:focus-within` tint, `:focus-visible` ring, tip anchoring + show behavior, truncate/`min-w-0` discipline, split-vs-full layout via `trailing` presence (0005:5, 0006:13, WIG content handling). Small interface — label/callback/disabled/tip strings + `children`/`trailing` snippets — following the ticket-71 `PacketCard` precedent (shell renders structure, variants slot content).
- Collections own their content and verbs, never homogenized (0005:4): "Start X" / "Copy X…" / "About X", badges, the note glyph (stays collection content per 0006:14 — the shell never learns notes exist), tooltip text. Launch keeps no tooltip: no additions, no removals.
- Heights: drop the `.qlw__action` + compact-button 44px `min-height` floors (ticket 130's AAA floor, retired by user decision); the button keeps its 44px `min-width` and fills the row via `height: 100%`. Rows become content-driven like Launch/Clip — uniform per density; Compact clears the 24px AA floor (recorded in 0004's compact-cards case).
- No other visual or behavior change: same tokens, same tooltips, same Run→Stop→Stopping vocabulary and no-reflow guarantees, same tracking semantics (detached `-d` still reports not-running), same disabled single-flight, same lazy icons.

## ACs

- [x] All three tabs render the same card height per density (Compact/Default/Large) at 340px and at wide dock, docked + floating.
- [x] No regression: verbs, tooltips (including Launch's absence), note glyph, Run/Stop/Stopping, Starting…/Copied, disabled single-flight tint, lazy icon loading.
- [x] `npm.cmd run check` 0 errors; `npm.cmd test` green.

## Implementation notes

- New `src/lib/components/QuickLaunchRow.svelte` shell (ticket-71 `PacketCard` shape): renders the `<li>` card, the main `<button>` (`mainLabel`/`onmain`/`disabled?`), and the tip box (`tipName`/`tipBody`/`tipId?`); owns card tokens, hover/`:focus-within` tint (disabled guard preserves Launch's exact `:not(:disabled)` behavior), `:focus-visible` ring, tip anchoring, truncate/`min-w-0`. Interface is label/callback/disabled/tip strings + `children`/`trailing` snippets — `trailing` presence switches split vs. full-bleed, no boolean.
- Adapters keep everything collection-owned: verbs, badges, note glyph, tooltip text, Starting…/excerpt/Copied, muted-name-while-starting (now an explicit `--muted` class — the old `:disabled`-descendant selector can't cross the shell seam), lazy icon (re-attached to the badge span, a stable ancestor per `lazyIcon`'s contract).
- Height floors dropped (`.qlw__action` + compact-button `min-height` → button `height: 100%`); button keeps the 44px `min-width`, 15px icon/spinner box, `flex-shrink: 0` — Run→Stop→Stopping still can't reflow.
- Results: `npm.cmd run check` 0 errors **and** 0 warnings (no orphaned selectors — every remaining scoped style still binds); `npm.cmd test` 89 passed.
  Manual pass user-verified (2026-09-06): uniform card heights per density across all three tabs at 340px + wide, docked + floating; Run/Stop/Stopping no-jitter cycle; keyboard-only; reduced-motion.

## Verification

- `npm.cmd run check`, `npm.cmd test -- --run`
- Manual: before/after screenshots per tab × density; Run/Stop/Stopping cycle with no jitter at each density; keyboard-only; reduced-motion.
