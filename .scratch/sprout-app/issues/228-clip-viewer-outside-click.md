# 228 — Clip viewer outside-click hook

**Parent:** [221 — Collections & media (spec)](221-collections-media-spec.md)

**What to build:** While the clip viewer is open, a click outside it closes it. The viewer installs a low-level mouse hook only for its own lifetime, ignores clicks inside its window/WebView, waits a 200 ms grace after opening, and unhooks on close/destroy. The outside click is not swallowed, so the dock or window underneath still receives it. If the hook cannot be installed or UIPI makes it unreliable, the viewer falls back to closing on focus loss after the same grace.

**Blocked by:** 226 — Clip viewer shell.

**Status:** ready-for-agent

- [ ] The hook is installed when the viewer opens and removed on close/destroy; no hook exists while the viewer is closed.
- [ ] A button-down outside the viewer's HWND/WebView after the 200 ms grace closes the viewer; clicks inside the viewer, its chrome, and its child WebView do not.
- [ ] The outside click is not swallowed; the dock/underlying window still receives it. Opening another image from the dock reuses the viewer rather than fighting the close.
- [ ] Esc and the close control keep working. The focus-loss fallback ignores focus loss to the viewer's own child WebView/dialogs and does not fire during the grace.
- [ ] The behavior works when the viewer is opened from the main app, the floating dock, auto-hide, and bezel; closing returns focus to the source window.
- [ ] The hook owner is recorded in the ADR-0029 inventory as the viewer module; the dock/appbar owner is not extended.
- [ ] Manual evidence covers normal and elevated foreground windows (UIPI), multi-monitor, and all dock modes. `cargo test`, `cargo check`, `npm run check`, and `node tools/ownership-gate.mjs` pass.

**Claims (recheck at dispatch):** the viewer's Windows module; hook lifecycle; ADR-0029 amendment/inventory. **Explicitly not built:** a global hotkey, a hook outside the viewer's lifetime, changes to the dock's own click/bezel handling, or updater behavior.
