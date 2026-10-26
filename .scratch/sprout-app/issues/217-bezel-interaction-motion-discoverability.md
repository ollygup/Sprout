# 217 — Bezel peek/open/close interaction + motion + discoverability

**Parent:** [214 — Bezel-tab dock mode + copy/i18n (spec)](214-bezel-tab-dock-mode-copy-i18n-spec.md)

**What to build:** The bezel tab's feel: hover only peeks it wider (never opens), click toggles the full panel, click-outside and Esc close it, focus loss alone never does — with motion-token animation and a first-run hint so nobody thinks the app vanished.

**Blocked by:** [215 Bezel mode backend](215-bezel-mode-backend.md) (geometry + open/close hooks). Copy keys (`dock.bezel.tooltip`, pulse text) coordinated with ticket 219's dictionary — key names pinned in spec 214; consume the loader when 219 lands, English literals until then.

**Status:** applied + validated 2026-09-17 (`npm run check` 0 errors; bezel interaction + bezelY + copy tests pass; contrast-check pass both themes; ownership gate pass)

- [x] Hover on the collapsed tab widens to peek only — verified hover can never open the panel (strict separation from hover mode)
- [x] Click anywhere on the tab/peek toggles open/closed; open animates via ADR-0034 tokens (`--dur-slow:280ms`, `--ease-spring`); dock driver motion boundary respected (no driver fork)
- [x] Close paths: click-outside + Esc + tab toggle all close; focus loss / alt-tab / copy-paste out of the dock does NOT close (research-backed: no aggressive auto-close)
- [x] Discoverability: hover tooltip on the tab + one-time pulse/glow on first bezel-mode entry (dismisses permanently once seen)
- [x] Keyboard + screen-reader operable (focusable tab, named toggle action, announced expanded/collapsed); contrast-check pairs pass both themes
- [x] Interaction tests (hover-never-opens, toggle, outside/Esc close, focus-loss stays open) + `npm run check` 0 errors
- [x] `node tools/ownership-gate.mjs` passes; no ADR text changes (interaction lives inside the accepted motion/disclosure rules)

**Explicitly not built:**

- Backend geometry/caps (ticket 215); Y persistence (ticket 216); Settings knobs (ticket 218); dictionary/translation (tickets 219–220)

**Follow-up 2026-10-26:** this ticket's click-outside path is the in-window pointer handler — a click that lands in another application never reaches the webview, so the panel stayed open. [243 — Bezel outside-click close across windows](243-bezel-outside-click-close.md) completes the same rule with the shared native hook; focus loss still never closes.
