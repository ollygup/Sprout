# 108 — Main app shows a second (document-level) scrollbar that drags the navigation rail

**What to build:** On the main app, with enough Quick Clips to overflow the window, TWO vertical scrollbars exist: the expected `.main` one, plus an outer one at the very window edge whose scrolling moves the entire app including the NavRail — which the shell's geometry (`+layout.svelte`: `.shell { height:100vh; overflow:hidden }`, rail outside `.main`, the sole `overflow-y:auto` container) forbids. Count-dependent: absent with short lists. The Quick Launch window is unaffected. Diagnosis-first like ticket 102: identify at runtime which element owns the outer thumb and which element exceeds the viewport before fixing beyond the guard below.

**Blocked by:** none.

**Status:** done — synced to the share; verified

- [x] Preventive hardening applied first: `html/body` pinned (`height:100%; overflow:hidden; overscroll-behavior:none`) at layout scope so no content growth can ever create a document scroller — the same guarantee `.mini` already gives the mini window
- [x] Runtime identification: devtools measurement caught the escapee — `documentElement.scrollHeight` was 1826 vs 800 viewport; element sweep named **`div.sr-only` on the clips page**
- [x] Root cause: that page's copy-announcement live region is `position:absolute` with auto offsets and sits in DOM *after* the whole clip list. No positioned ancestor exists, so its containing block is the initial containing block (the document) — `.main`'s `overflow:hidden` never contained it, and it took its natural flow position ~1800px down, stretching the document by exactly that distance. Its `clip:rect(0 0 0 0)` only suppressed painting; scrollable overflow still counted it. Count-dependent by construction (deeper list ⇒ deeper region), Quick Clips-only because only that page places a live region after long content.
- [x] Code-side fix: canonical `.sr-only` utility promoted to `tokens.css` with explicit `top/left: 0` (pins to document origin — zero geometry regardless of DOM placement) plus `clip-path: inset(50%)` alongside legacy `clip`; per-page copies deleted (clips page's `.sr-only`, window's `.qlw__sr` → shared utility). Layout hardening retained as belt-and-braces.
- [x] Final confirmation: one-liner prints `{docH: 800, winH: 800, diff: 0}` on Quick Clips with the long list (2026-08-25)

**Static findings (2026-08-25, pre-runtime):** shell math rules out layout flow; grep sweep of components + routes finds no `fixed`/`sticky`/`100vh` outside ContextMenu (correct), mini/layout shells (intended); clips page has zero internal scrollers (rows live directly in `.main`); dialogs are native `<dialog>` top-layer. So the growth mechanism is not visible statically — hence runtime inventory.

**Lesson:** visually-hidden recipes without explicit offsets rely on DOM placement luck — an absolutely-positioned sr-only region inherits the document as containing block when no ancestor is positioned, converting "invisible" into "infinitely placed". The shared utility now makes correct placement unforgeable.
