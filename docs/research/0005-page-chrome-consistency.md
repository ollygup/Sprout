# Page-chrome consistency rules

Standing rules for main-app page headers — written once so every new page
applies the same test instead of hand-copying a layout (the drift this round
fixed: five near-identical header implementations had already produced one
real bug — a stretched Add button — and one missing search). For constrained
surfaces (the Quick Launch window/dock), `0004-progressive-disclosure` rules
apply instead; these two documents share rule 5's spirit.

## Sources

- AGENTS.md (repo): UI must reuse the shared component foundation; identical
  controls render identically everywhere unless a custom requirement forces a
  deviation.
- Notion product analysis (designmd.cc/benchmarks/notion, distilled from
  notion.so, verified against live extraction 2026-08): "Reserve the primary
  color for the single most important CTA on a page"; "Don't use more than one
  Primary Button per screen view."
- Jakob Nielsen, *Consistency and Standards*, NN/g usability heuristics —
  https://www.nngroup.com/articles/user-control-and-freedom/ (heuristic set;
  #4 consistency): users should not wonder whether different words, colors,
  or layouts mean the same thing.
- Jakob Nielsen, *Progressive Disclosure* (via research 0004) — split by
  frequency of use; the same frequency logic places search on every list page.

## The rules

1. **One header implementation.** Every page renders its chrome through the
   shared PageHeader component (title row, subtitle line, optional toolbar
   row). Pages never re-declare header flex/CSS — that duplication is how
   buttons started stretching to their neighbors' height.
2. **Exactly one primary (accent-filled) button per header row.** It is the
   page's main verb: create on Products/Presets/Quick Actions/Quick Clips,
   Start on Quick Launch.
3. **An add affordance is secondary when its page already has a primary
   verb.** Quick Launch's Add is outline — deliberate hierarchy, not neglect;
   everywhere else Add *is* the primary.
4. **Search lives in the toolbar row below the header** — never inline in the
   actions row — and every filterable list page carries it, through the same
   SearchInput, filtering client-side over name + the row's distinguishing
   content.
5. **Same-kind controls share one treatment**, down to icons: an add button
   uses the shared plus icon, never a bare text glyph.
6. **Vertical rhythm is owned by the component**: title block → space →
   toolbar → space → content. Changing it once changes it everywhere.

## Applied case study (2026-08, ticket 84)

Before: seven hand-copied header layouts (Products, Presets, Quick Launch,
Quick Actions, Quick Clips, History, Logs — plus Settings and Plan's
title-only variants); Quick Actions had no search at all; Quick Clips' search
sat inside the actions row and stretched its Add button taller than every
other add in the app; Products used a text "+" while every other page used
the icon. After: every main-app page renders through PageHeader; the only
per-page code left is the snippets (buttons, subtitle text, search binding).
