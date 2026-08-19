# 11 — App shell and design foundation

**What to build:** The dual-mode design system every other ticket builds on. Replace the current palette (the "AI-default" cream + serif look) with a token system sourced from Notion's shipped in-app palette — light (`#FFFFFF` page, `#F7F6F3` surface, `#37352F` text, alpha borders) and dark (`#191919`, `#252525`, alpha-white text/borders) — following the system theme with no manual toggle. Accent is Notion green (`#0F7B6C` / `#4DAB9A`), warmth from Notion brown/orange, statuses from Notion's semantic block set. Migrate all shared components off hardcoded `rgba()` literals, 10px font sizes, and ad-hoc radii; rename the nav's Library entry to Products; add an error page and a skip link. The app renders correctly in both themes with proven contrast.

**Blocked by:** None — can start immediately

**Status:** done

- [x] Notion-derived token set exists for light and dark; app follows the system theme via `prefers-color-scheme`, with `color-scheme` and `theme-color` set per mode; no manual toggle
- [x] All shared components (buttons, icons, dialogs, inputs, empty state, nav rail, packets) use tokens only — no hardcoded colors, 10px font sizes, or ad-hoc radii remain in the migrated set; duplicated badge/notice CSS consolidated into shared styles
- [x] Contrast verified: primary text AA/AAA in both modes; muted tiers bumped where Notion's values fail AA; focus-visible outlines present and tokenized
- [x] Nav rail: Products · Presets · Plan · History · Logs · Settings; dead "soon" branch removed; footer wordmark kept
- [x] `+error.svelte` renders a styled error page; skip link to main content in the layout
- [x] `prefers-reduced-motion` respected; motion only via transform/opacity; theme-color matches page background per mode
- [x] `npm run check` 0 errors, `cargo test` green, `cargo check` clean, `npm run build` ok