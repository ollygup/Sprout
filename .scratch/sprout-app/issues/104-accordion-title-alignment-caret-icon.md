# 104 — GroupAccordion alignment to title text; Disclosure caret

**What to build:** Two linked fixes in the shared section primitives. (1) `GroupAccordion.svelte`: indent `.group__rows` to the disclosure's title-text start (icon column width + gap, ~34px today) instead of `--space-5`, so body content aligns with the header label per Notion toggle anatomy; the `flush` variant stays at 0 for the window strip. (2) `Disclosure.svelte`: replace the chevron glyph with a small filled triangle caret pointing right when closed, rotating 90° to point down when open — fixing today's inverted down-closed/up-open rotation. Keep the transform-only transition and the `prefers-reduced-motion` collapse; keep button metrics/hover treatment so nothing else shifts. Both changes propagate everywhere through the shared components (pages, window tabs, composer rows, form sections) — no per-surface overrides.

**Blocked by:** none.

**Status:** done — synced to the share; visual confirmation pass still pending a human

- [x] Body left edge aligns with title-text start in labeled accordions on all surfaces
- [x] Flush variant unchanged (0 indent, window strip density preserved)
- [x] Caret: ▸ closed → ▾ open, 90° transform transition, reduced-motion safe
- [x] Icon-only mode (composer rows) and labeled mode both correct
- [x] Long-name ellipsis behavior unchanged
- [ ] Visual pass light/dark on library, quick-actions, clips pages + all three window tabs

**Verification notes (2026-08-25):**

First attempt aligned the body to Disclosure's 26px button width — wrong anchor: in labeled mode the button stretches to 100% and the chevron span shrinks to its glyph, so the label actually started at icon + gap (~17px), which is why content sat right of the title. The fix pins one `--caret-column: 14px` custom property on `.group` that both sides read: the head's chevron box gets that fixed width (glyph centered inside) and the body indents by `calc(var(--caret-column) + var(--space-1))`, so alignment holds even if the glyph size changes again. `Disclosure` now renders the filled-triangle `caret` icon (added to the shared registry with `fill="currentColor"`) at 14px, rotating 90° open; transition and reduced-motion collapse untouched. Gates: svelte-check 0/0, vitest 36/36.
