# 53 — Quick Launch dock (Win32 AppBar)

**What to build:** Docking for the Quick Launch window: it pins to the left or right screen edge as a Win32 AppBar — a slim vertical strip with the same two tabs (Quick Launch / Quick Actions) — that auto-hides to a sliver when not hovered (default) or stays fixed like a pinned taskbar. Dock/undock toggle and left↔right edge-switch arrows live in the window; mode (auto-hide/fixed) and default edge are set in the main app's Settings. Edge and mode are remembered per monitor; the AppBar is unregistered on app quit.

**Blocked by:** 52 — Quick Launch window (floating)

**Status:** ready-for-agent

- [ ] Win32 AppBar via the raw window handle (`SHAppBarMessage`: `ABM_NEW` / `ABM_SETPOS` / `ABM_AUTOHIDE` / `ABM_REMOVE`); slim strip sized from design tokens, full height of the attached monitor's work area
- [ ] Auto-hide (default): slides to a ~3px sliver when the cursor leaves the bar; slides back out on hover at the edge — taskbar-like
- [ ] Fixed mode: bar stays visible and reserves its strip; maximized windows on that edge shrink to accommodate (reserved workspace is the accepted AppBar trade-off, per ADR-0011)
- [ ] Dock/undock toggle and left↔right edge-switch arrows in the window chrome; undocking returns to the floating window at its remembered size/position
- [ ] Mode and default edge configurable in the main app's Settings; edge and mode persist per monitor; attaches to the monitor the window is on
- [ ] `ABM_REMOVE` on app quit so the edge is never left occupied
- [ ] `cargo test` green; `npm run check` 0 errors; synced to the share