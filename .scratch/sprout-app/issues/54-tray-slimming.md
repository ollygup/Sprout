# 54 — Tray slimming

**What to build:** The tray right-click menu becomes just Open Sprout and Quit. The launch-item menu path (Start all, per-desktop-group submenus, per-entry items) is removed — that functionality now lives in the Quick Launch window and the Quick Launch page. Tray left-click opens/raises the Quick Launch window instead of starting the whole list.

**Blocked by:** 52 — Quick Launch window (floating) — the window must exist to be the tray's left-click target

**Status:** ready-for-agent

- [ ] Right-click menu rebuilt to two items: Open Sprout (recreates/focuses the main window) and Quit (the only real exit)
- [ ] Left-click opens/raises the Quick Launch window; when the window is docked (ticket 53), left-click raises it into focus
- [ ] Launch-item menu building removed (~the menu-building path for start-all, desktop groups, and per-entry items); the window's Start button and the page's Start button remain the launch triggers through the shared runner
- [ ] `cargo test` green; `npm run check` 0 errors; synced to the share