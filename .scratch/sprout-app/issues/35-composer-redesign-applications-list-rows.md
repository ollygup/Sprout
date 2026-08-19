# 35 — Composer redesign: Applications list rows

**What to build:** The preset composer renames Requirements to Applications and lays each one out as a single thin row — product name with its winget id, version policy, an expand chevron, and a remove button. Advanced options (timeout, depends on, env wiring, verify commands) sit in an inline panel that unfolds per row. Collapsed rows that carry hidden values show count tags (e.g. `2 env · 1 verify · 1 dep`). Composer state is extracted into a plain module covered by a new Vitest seam, so add/remove/expand/policy/dependency/env/verify logic is tested without the UI.

**Blocked by:** None — can start immediately.

**Status:** implemented — agent done, human verification of the manual flow pending

- [x] Composer section title, add button, row labels, validation messages, and InfoTips use "Application"
- [x] Each application renders as one thin row: product name + winget id, version-policy select, expand chevron, remove button; dashed fold line separates rows
- [x] A blank row shows the product picker inline; choosing a product collapses the row to the name line
- [x] The chevron unfolds that row's advanced panel (timeout, depends on, env wiring, verify) — one row at a time, `aria-expanded`/`aria-controls` correct
- [x] Collapsed rows with hidden values show count tags
- [x] Composer state logic covered by the new Vitest seam; backend untouched
- [x] CONTEXT.md notes the composer synonym: Requirements are presented as "Applications"
- [ ] `npm run check` 0 errors (done); manual add-many-applications flow is comfortable (no per-row scrolling) — needs a human pass in `npm run tauri dev`