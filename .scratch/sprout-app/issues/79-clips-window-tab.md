# 79 — Quick Clips window tab (conditional, read-only)

**What to build:** The third tab of the Quick Launch window — only once at
least one clip exists — as a read-only click-to-copy list with visible
"Copied" feedback, and the label-fitting degradation chain from research 0004.

**Blocked by:** 78 — the list/copy commands and data this surface renders

**Status:** done (2026-08-23)

- [x] Window loads clips alongside entries/actions; tabs array = base two + Quick Clips **iff** ≥1 clip; the existing quick-launch-changed listener keeps presence truthful mid-session (deleting the last clip removes the tab again — accepted)
- [x] Shared Tabs component accepts an optional per-tab tooltip/title rendered as `title` + accessible name (WAI-ARIA roving-tabindex behavior unchanged)
- [x] Label fitting measured at runtime: full labels ("Quick Launch / Quick Actions / Quick Clips") → shortened ("Launch / Actions / Clips") → icon-only with tooltips+aria-labels, chosen in one pass by canvas-measured text widths vs the container box (physical-px window means high-DPI renders narrower — never trust a 1× screenshot); re-fit on tab-set change, parent-box resize, and fonts.ready. **v1 regression found & fixed the same day**: rendering each candidate level and watching the tablist for overflow let the ResizeObserver see the strip's own degradation resize it — reset-to-full looped until Svelte's `effect_update_depth_exceeded` aborted the flush, freezing the whole window at first paint (eternal "Loading…", dead tab clicks, stale floating chrome while docked). Verified via CDP against the running dev app: invokes healthy (~13 ms), console clean, docked header correct (`data-tauri-drag-region="false"`), panel clicks render. Rule for the future: never put the thing you resize inside your own feedback path
- [x] Panel: read-only rows (name or first-line preview + one-line content excerpt); click/Enter/Space copies via the clipboard command; per-row "Copied" flash (~1.2 s) plus polite live region — silence is a bug (research rule 5)
- [x] No editing/configuration in the window — all CRUD stays on the page (disclosure level boundary)
- [x] Window can no longer present a failure as eternal "Loading…": startup loads carry a 10 s budget that surfaces the error line with Try again (the freeze presented exactly that way and was invisible without DevTools)
- [x] Manual DPI verification at 100/125/150% recorded; chosen degradation noted if icons ship (existing icon set names verified before use — rocket/terminal/copy all exist in Icon.svelte). Validated by the user at real display scaling (2026-08-23): labels fit and degrade as designed
- [x] `npm run check` 0 errors; synced to the share
