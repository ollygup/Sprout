# 79 — Quick Clips window tab (conditional, read-only)

**What to build:** The third tab of the Quick Launch window — only once at
least one clip exists — as a read-only click-to-copy list with visible
"Copied" feedback, and the label-fitting degradation chain from research 0004.

**Blocked by:** 78 — the list/copy commands and data this surface renders

**Status:** ready-for-agent

- [ ] Window loads clips alongside entries/actions; tabs array = base two + Quick Clips **iff** ≥1 clip; the existing quick-launch-changed listener keeps presence truthful mid-session (deleting the last clip removes the tab again — accepted)
- [ ] Shared Tabs component accepts an optional per-tab tooltip/title rendered as `title` + accessible name (WAI-ARIA roving-tabindex behavior unchanged)
- [ ] Label fitting measured at runtime after mount: full labels ("Quick Launch / Quick Actions / Quick Clips") → shortened ("Launch / Actions / Clips") → icon-only with tooltips+aria-labels, degrading only on measured overflow (physical-px window means high-DPI renders narrower — never trust a 1× screenshot)
- [ ] Panel: read-only rows (name or first-line preview + one-line content excerpt); click/Enter/Space copies via the clipboard command; per-row "Copied" flash (~1.2 s) plus polite live region — silence is a bug (research rule 5)
- [ ] No editing/configuration in the window — all CRUD stays on the page (disclosure level boundary)
- [ ] Manual DPI verification at 100/125/150% recorded; chosen degradation noted if icons ship (existing icon set names verified before use)
- [ ] `npm run check` 0 errors; synced to the share
