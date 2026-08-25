# 97 — Shared GroupAccordion, real app icons on entry rows, window Groups for every tab

**What to build:** Three gaps found while exercising ticket 93's manual pass. (1) The grouped-layout orchestration (collapse state, section head markup, scoped CSS) was copy-pasted across four surfaces — extract a shared `GroupAccordion.svelte` plus a `groupCollapse` state helper and migrate every surface onto them, ticket-71 style (byte-identical moves, proven by svelte-check's unused-selector warnings). (2) Saved Launch entries never showed their apps' real icons — the `candidate_icon` pipeline only fed the Add panel — so wire the lazy icon loader into entry rows on both the main rack and the window, keeping kind glyphs for command entries and unresolvable targets. (3) The Quick Launch window's Quick Actions and Quick Clips tabs stayed flat even when their collections' Groups features were on; give them the same mirror-the-toggle treatment as the Launch list.

**Blocked by:** 93 — window upgrade (done).

**Status:** done — synced to the share; hands-on dev pass still pending a human

- [x] `GroupAccordion.svelte` owns the head row (Disclosure + count Badge + optional ⋯ actions snippet), conditional body, and section styles; all four former copies deleted
- [x] `createGroupCollapse()` owns session collapse state with SQLite id-reuse pruning; pages keep only their filter-forces-open rule
- [x] Main rack rows and window Launch rows show real app icons (lazy per-visible-row fetch, memory-only); commands/unresolvable targets keep kind glyphs
- [x] Window Actions + Clips tabs follow `action_groups` / `clip_groups` live: flat when off, ungrouped-first plus default-expanded accordions when on, sections only while they have members
- [x] Tooltips, Run/Stop/Stopping states, copy flash, and accessible names unchanged through the migration
- [ ] Hands-on pass: groups toggles from the main app reflect in an open window; icons resolve for .lnk and exe entries; light/dark

**Verification notes (2026-08-25):**

Shared components: `$lib/components/GroupAccordion.svelte` renders one group as a default-expanded accordion — labeled Disclosure, muted Badge, optional trailing `actions` snippet (the main pages' group menu), `children` for each surface's own row list, and a `flush` variant (no indent, no top margin) for the window strip. Long names ellipsize everywhere now (previously window-only) — intentional unification, the only deliberate visual delta. `$lib/groupCollapse.svelte.ts` exposes `isOpen/toggle/prune`; the filter-forces-open behavior stays page-local (`filter.trim() !== "" || collapse.isOpen(id)`), as do the deriveds, since filtering/menus legitimately differ per page. The aria-controls target moved from the page `<ul>` to the component's `.group__rows` wrapper — same computed layout, same disclosure semantics.

Icons: `$lib/lazyIcon.svelte.ts` extracts the IntersectionObserver action + `$state` cache from the main page (its Add panel consumes the module unchanged). Rows attach `use:lazyIcon={target}` to a stable ancestor (empty string skips command entries entirely); `<img>` renders once cached, else the rocket/terminal glyph. Per-webview memory only, never disk (ticket 40 contract preserved).

Window: `load()` joins `listGroups("action")` / `listGroups("clip")` into the timed parallel load and reads both feature flags alongside theme; three independent collapse helpers prune on every reload via `quick-launch-changed`, so main-app edits appear live. Action and Clip rows moved into `actionRow`/`clipRow` snippets shared by flat and grouped branches; tooltip ids, `aria-describedby`, Run/Stop/Stopping machine, and copy feedback untouched. Panels render inside `.qlw__list--padded` scroll wrappers whose last child carries the tooltip runway.

Gates: `npm.cmd run check` 0 errors / 0 warnings (zero unused-selector warnings proves each migrated style block still binds — the ticket-71 proof technique); `vitest run` 36/36; `npm.cmd run build` clean. No Rust changes this round (icons + groups domains already shipped in tickets 40 and 89), so the backend suite is untouched since its last green run.
