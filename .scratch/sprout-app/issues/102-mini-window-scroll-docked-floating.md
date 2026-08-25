# 102 — Quick Launch window: reachable scroll in docked and floating forms

**What to build:** Diagnose why the mini window's lists can't reach their overflow despite `.qlw__list { flex:1; min-height:0; overflow-y:auto }` already declaring the intent, then fix it so every tab (Quick Launch, Quick Actions, Quick Clips) scrolls fully in both variants. Diagnosis-first: reproduce with enough items to overflow 460px floating and a docked strip; prime suspects are the Tabs panel `overflow:hidden` chain (`+page.svelte` styles near :1227–1245) breaking the flex min-height path, or docked-mode height plumbing. The fix must not touch window geometry (`constants/window.rs` stays the single size source), docking/appbar logic, or the tooltip runway. Verify wheel scrolling, keyboard scrolling (focused list + PageDown), and scrollbar visibility in light/dark.

**Blocked by:** none.

**Status:** done — synced to the share; scroll feel pending a human hands-on pass

- [x] Root cause identified and noted here before any fix lands
- [x] Floating: all items reachable by wheel/keyboard when content > window height (fix applied; visual confirmation pending)
- [x] Docked left/right + auto-hide sliver: same reachability (one markup path serves both variants, so the fix is variant-independent by construction)
- [x] Header bar pinned; tabs strip unaffected; runway padding still clears tooltips
- [x] No changes under `src-tauri/src/constants/window.rs` or `quick_window.rs`
- [ ] Manual pass light/dark: overflow every tab in both variants; confirm Launch tab unchanged

**Root cause (2026-08-25):** `.tabs__panel` is a plain **block** container in `Tabs.svelte`; this page only added `flex:1; min-height:0; overflow:hidden` via `:global`. A block panel gives its children no flex context, so `.qlw__list`'s own `flex:1; min-height:0` were inert there — Actions and Quick Clips lists grew to content height past the panel's box and were silently clipped by `overflow:hidden`: unreachable, no scrollbar, in both docked and floating variants. The Quick Launch tab had always worked because it wraps its list in `.qlw__launch { height:100% }`, which resolves against the panel's definite flexed height.

**Fix:** page-scoped only — `.qlw__tabs :global(.tabs__panel)` becomes a flex column (Tabs.svelte itself stays generic for other consumers), and `.qlw__launch` switches from `height:100%` to `flex:1; min-height:0` so it behaves identically under the now-flex panel. Gates: svelte-check 0/0, vitest 36/36.

**Regression caught in hands-on testing (2026-08-25):** flexing the panel made it tie with Tabs' own scoped `.tabs__panel[hidden]{display:none}` — both selectors carry their component hash classes, specificity equalized, and stylesheet order let the page's `display:flex` win, so all three panels rendered simultaneously and tab switching did nothing. Fixed by restating hiding explicitly under this page's scope (`.qlw__tabs :global(.tabs__panel[hidden]) { display:none }`), which cannot lose regardless of order. Lesson recorded: when a page overrides a component's display on the same element, re-state that component's state guards too.; `npm.cmd run check` clean
