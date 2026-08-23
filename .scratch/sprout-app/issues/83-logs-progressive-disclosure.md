# 83 — Logs screen progressive disclosure (collapsible sections, capped previews, section rhythm)

**What to build:** The Logs screen's three run-folder families (Run folders,
Quick Action runs, Quick Launch runs — tickets 09/64/77) stack unbounded
lists: a hundred Quick Action folders push Quick Launch far below the fold,
and consecutive sections have no vertical rhythm (a row's bottom edge meets
the next header directly). Apply the progressive-disclosure rules from
`docs/research/0004`: all three section headers stay visible; each section
collapses behind a chevron and shows a 3-row preview with a "Show all N"
expander when open. Frontend-only.

**Blocked by:** 77 — the third family this redesign covers

**Status:** done

- [x] Section headers: each family gets a collapsible header row like the launch page's desktop groups (`Disclosure` chevron + mono uppercase label + right-aligned `N folder(s) · size` meta), always visible per research 0004 rule 1
- [x] Expanded sections show the first 3 entries, then a quiet "Show all N" / "Show fewer" text button (hidden at ≤3) — overview first, detail on demand
- [x] Session-only state: one `expanded` boolean per family (default true), one `showAll` boolean per family (default false); nothing persisted, no settings surface
- [x] Spacing rhythm: every section after the first gets `--space-6` top margin + hairline divider + `--space-5` padding, tokens only
- [x] The three copy-pasted list blocks collapse into one local Svelte 5 `{#snippet}` parameterized by label, entries, empty copy, and state
- [x] Accessibility holds: `aria-expanded`/`aria-controls` via `Disclosure`, real buttons with visible focus, no color-only signals, reduced-motion respected (global rule)
- [x] No backend/API/type changes; `npm.cmd run check` 0 errors; synced to the share

**Verification notes (2026-08-23):** Frontend-only rewrite of
`src/routes/logs/+page.svelte`. The three families render through one local
`{#snippet logSection(key, label, entries, emptyCopy)}` (the plan-page
snippet idiom); each section is `<section aria-labelledby>` with a header
row of icon-only `Disclosure` (chevron rotates via its own transform rule,
`aria-expanded`/`aria-controls` included) + mono-uppercase label +
right-aligned count·bytes meta so readers know what's behind a collapsed
family before expanding it. Expanded bodies cap at `PREVIEW_ROWS = 3` rows
and add a token-styled mono text button ("Show all N" ↔ "Show fewer",
hidden at ≤3) — a hundred-folder family now occupies ~a third of a viewport,
so Quick Launch's header is always reachable. Session-only `$state`
(`expanded`/`showAll` records keyed `"runs"|"actions"|"launch"`; renamed away
from `open` — it collided with the page's `open(path)` Explorer action).
Section rhythm: `.family { margin-top: --space-6; padding-top: --space-5;
border-top: var(--border) }` applies uniformly, including between the roots
cards and the first family. All values from tokens.css; no new components,
no API changes (`LogLocations` already carried everything). Gates:
`npm.cmd run check` 0 errors 0 warnings; no Rust touched, so cargo untouched.

