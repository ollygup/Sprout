# 243 — Bezel outside-click close across windows

**Parent:** [214 — Bezel-tab dock mode + copy/i18n (spec)](214-bezel-tab-dock-mode-copy-i18n-spec.md)

**What to build:** With dock mode on **bezel** and the panel open, pressing a mouse button anywhere outside the dock window closes it — on the desktop, the taskbar, another app, or the main window. Clicks inside the panel keep working. Focus loss alone still does **not** close it (ticket 217's validated rule).

**Blocked by:** 217 (interaction contract: click-outside/Esc close, no focus-loss close). Reuses the outside-click hook owner introduced for the clip viewer in spec 221 §B.6.

**Status:** implemented 2026-10-26 — Rust + frontend wiring complete, static gates green. One live on-machine pass is still owed (a real OS click outside the window; automated CDP input never leaves the webview, so it cannot exercise this path).

- [x] A button-down outside the open panel's top-level window closes the panel, and the click is never swallowed — it still reaches the window/desktop underneath (`CallNextHookEx` on every event).
- [x] Clicks inside the panel, its tab, or its grip never close it (`WindowFromPoint` + `GetAncestor(GA_ROOT)` against the dock's HWND, which also covers the WebView2 child and the Companion child).
- [x] The 200 ms post-open grace keeps the opening click, or a click immediately after the open move, from dismissing the panel it just raised.
- [x] Focus loss, alt-tab, clicking into an elevated app that UIPI blocks, and workspace switches do NOT close the panel. If the hook cannot install at all, the in-window pointer path and Esc remain.
- [x] Armed only while the panel occupies the open rect and only in `bezel` mode; disarmed on close, undock, dock-mode change, window close/destroy, app exit, and route unmount. Arming is queued with the geometry move, so a fast open→close cannot deliver a stale arm.
- [x] The hook lives in the single shared owner `src-tauri/src/outside_click_hook.rs` with `Target::{Viewer, Bezel}`; disarm is target-scoped and the displaced consumer is parked and handed the hook back, so a dock close path never removes the clip viewer's hook, a viewer close never removes the dock's, and opening a clip from the open panel then closing the viewer leaves the panel's outside-click working (ADR-0029 amendment 2026-10-26, gate row updated).
- [x] No second `SetWindowsHookExW` site, no dock-driver change, no new geometry owner: placement still belongs to the frontend (ADR-0019), and the close request is emitted as `bezel-close-requested` so the panel runs the same closer as Esc.
- [x] `cargo test`, `npm.cmd run check`, and `node tools/ownership-gate.mjs` pass.

**Live pass still owed:** open the bezel panel, click the desktop / another app / the taskbar — it must close and the click must land; alt-tab away and back — it must stay open; click inside the panel — it must stay.

**Explicitly not built:** closing on focus loss; a second hook or a hook installed while the tab is collapsed; any change to hover-peek, open/close motion, tab drag, or the clip viewer's own outside-click and focus-loss fallback.
