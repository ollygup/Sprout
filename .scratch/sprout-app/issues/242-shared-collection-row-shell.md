# 242 — Shared collection row shell

**Parent:** [221 — Collections & media (spec)](221-collections-media-spec.md)

**What to build:** One module owns the collection row card. Today the same chrome — `display: flex`, `align-items: center`, `border: 1px solid var(--border)`, `border-radius: var(--radius)`, the group tint, the hover surface, the secondary-text alias — is written out four times: `.rack__row` in `src/routes/+page.svelte` (Launch), `src/routes/quick-actions/+page.svelte`, `src/routes/clips/+page.svelte`, and `.dock-row` in `src/lib/components/QuickLaunchRow.svelte`. The dock already has the right shape (a shell component that owns the box, states and layout while the collection supplies content and its one verb). Extract the main-app equivalent and make the three pages thin adapters.

**Why this is a real module, not a pass-through (deletion test).** Delete it and the chrome reappears in three pages plus the dock, each drifting on padding, gap and transition — which is exactly what happened before ticket 240 (the dead `--bg-card` token, three different paddings). The complexity does not vanish, so the module earns its keep. It also collapses the group tint to one consumption site, which is the concrete complaint that surfaced this ticket: `background: var(--group-row-bg, transparent)` currently repeats in four rules because Svelte scopes each page's `.rack__row` and a global override of equal specificity would resolve by bundle order (recorded in the [221 amendment](221-collections-media-spec.md) of 2026-10-24).

**Interface (to settle at dispatch):** a `CollectionRow` shell in `src/lib/components/` taking the row's main verb (`mainLabel`, `onmain`), optional context-menu handler, disabled state, and content/trailing snippets — the shape `QuickLaunchRow` already proves. The three pages keep ownership of their own content: clip title/excerpt, action name/note, launch entry/badge. Differences to preserve rather than flatten: Clips' tighter padding and its `oncontextmenu` (spec 221 right-click → viewer), Quick Actions' `role="button"` + Enter/Space handling and focus ring, Launch's display-only rows (no hover state, no pointer), and the dock's compact geometry.

**Blocked by:** 230 (nested groups main-app UI rewrites the same three pages and `GroupAccordion`; landing this first would collide) and 238 (the verification baseline the refactor must not change).

**Status:** todo

- [ ] One module owns the collection row card's box, tint, hover surface and secondary-text alias; the three main-app pages and the dock consume it instead of restating the chrome.
- [ ] `background: var(--group-row-bg, transparent)` exists in exactly one place; no page carries a colour literal or a duplicate tint rule.
- [ ] Each surface keeps its current geometry and behaviour: Clips padding + right-click, Quick Actions keyboard activation + focus ring, Launch display-only rows, dock compact rows. No pixel drift at 100 %/150 % DPI.
- [ ] Neutral groups and ungrouped rows still resolve to the page surface; a neutral child inside a coloured parent still inherits no tint (spec 221 A.6).
- [ ] The refactor is behaviour-neutral: `tools/repro-group-color-hover.mjs` stays GREEN, `tools/contrast-check.mjs` unchanged, and the 238 matrix re-runs without new findings.
- [ ] Evidence: before/after DOM geometry for one row per surface (main ×3, dock), both themes, plus the harness report.

**Claims (recheck at dispatch):** `src/routes/+page.svelte`, `src/routes/quick-actions/+page.svelte`, `src/routes/clips/+page.svelte`, `src/lib/components/QuickLaunchRow.svelte`, `src/lib/components/GroupAccordion.svelte` (published properties), `tools/repro-group-color-hover.mjs`. **Explicitly not built:** a shared *content* template (each collection keeps its own row content and verb); moving group controls or colour editing anywhere; any visual change.

## Applied research

- 0005 rule 5 — one row treatment across surfaces: the main app and the dock should read as the same card, which is what a shared shell buys.
- 0028 — the anatomy the shell must reproduce unchanged: bar from the group, tint on the card, hue on hover, no frame.
- 0004 rule 3 — the row's verb and content stay with the collection; the shell owns only the box and its states.
