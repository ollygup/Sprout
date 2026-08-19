# 52 — Quick Launch window (floating)

**What to build:** A miniature Quick Launch window opened by tray left-click (or raised if already open): frameless and themed, with two tabs — Quick Launch (a single Start button that starts the whole Quick Launch list, with the entry count) and Quick Actions (each action as NAME + Run button, firing the hidden runner). The window hides to the tray on blur and remembers its size and position across restarts. The window is read-only — no configuration surface.

**Blocked by:** 50 — Quick Actions: storage and runner (the Quick Actions tab renders and runs that list)

**Status:** done

- [x] Second webview window created on demand (mirroring the existing recreated-window pattern) and destroyed on hide, keeping the backend lean; window capability added to the capability scope
- [x] Frameless with a draggable header, close button (hides to tray), themed from the existing theme store and tokens; built from shared components
- [x] Tray left-click opens the window or raises it when already open; blur (focus loss) hides it to the tray
- [x] Two tabs via a new minimal accessible tab strip (ARIA tablist) added to the component foundation per the AGENTS.md design rule
- [x] Quick Launch tab: one Start button that starts the whole Quick Launch list through the existing capped runner (`launch_entries`), with the entry count shown; summary notification behavior unchanged
- [x] Quick Actions tab: lists every action as NAME + Run button; Run fires the ticket-50 hidden runner for that action
- [x] Floating size and position persisted across restarts
- [x] `cargo test` green; `npm run check` 0 errors; synced to the share