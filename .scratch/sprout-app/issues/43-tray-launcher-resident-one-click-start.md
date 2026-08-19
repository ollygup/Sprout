# 43 — Tray launcher: resident, one-click start, menu

**What to build:** Sprout lives lean in the tray under "show hidden icons": closing the window destroys it and keeps only the Rust backend resident (no webview, a few MB), left-click starts the whole Quick Launch list, right-click offers Start all / per-app items / Open Sprout / Quit, and Quit is the only way to exit the app. Parent spec: 37.

**Blocked by:** 42 — Launch run: capped queue engine, Start button, summary

**Status:** done

- [x] Tray icon with the existing brand icon and tooltip; left-click = start all (same `start_quick_launch` path, same concurrent-run guard)
- [x] Right-click menu: Start all (N) / per-app items (each starts that single entry through the same runner) / Open Sprout / Quit; menu rebuilt whenever the entries change
- [x] Closing the window (× or Alt+F4) destroys it; exit-suppression keeps the backend alive with zero windows; Open Sprout recreates the window with its configured size; Quit (tray-only) exits for real
- [x] Single-instance focus hook recreates the window when it was destroyed instead of assuming it exists
- [x] Empty list behavior: left-click with zero entries → "Nothing configured in Quick Launch" notification
- [x] `cargo test` green, `npm run check` 0 errors; manual verification of the tray flow in `tauri dev`; synced to the share