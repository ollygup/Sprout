# 84 — Page chrome consistency: shared PageHeader across list pages

**What to build:** Every main-app list page renders its chrome through one
shared PageHeader component instead of five hand-copied header layouts, so the
same kind of control looks exactly the same everywhere (AGENTS.md design rule;
Notion-derived hierarchy rules per research 0005): one primary action per
header row, every filterable list carries the identical search input in a
toolbar row below the header (never inside it), and add affordances share one
icon treatment. Fixes the concrete drifts: Quick Actions gains the missing
search; Quick Clips' search moves out of the header row and its Add button
returns to standard height (flex-stretch bug); Products' text "+" becomes the
shared plus icon.

**Blocked by:** none — can start immediately.

**Status:** done (2026-08-23)

- [x] Shared PageHeader component owns all header geometry once (title,
      actions row with align-items:center, subtitle line, optional toolbar
      row below); styling only from tokens.css / existing components
- [x] Products, Presets, Quick Launch, Quick Actions, and Quick Clips render
      through PageHeader; each page's private header markup/CSS is deleted
      (History, Logs, Settings, and Plan adopted too — same drift class)
- [x] Header hierarchy is uniform: exactly one primary button per header —
      Products/Presets/Quick Actions/Quick Clips Add (green), Quick Launch
      Start (green) with Add secondary; auxiliary buttons secondary
- [x] Search placement uniform: Products, Quick Launch, Quick Actions, and
      Quick Clips filter via the shared SearchInput in the PageHeader toolbar
      slot; Quick Actions filters name+command client-side with a
      "Nothing matches" empty state
- [x] Products Add uses the shared plus Icon (no bare-text "+")
- [x] Accessibility unchanged or better: h1 ids still wired to
      aria-labelledby, search keeps its aria-label, focus ring untouched
- [x] research 0005 records the standing chrome rules and sources
- [x] `npm run check` 0 errors/0 warnings; vitest green; synced to the share
