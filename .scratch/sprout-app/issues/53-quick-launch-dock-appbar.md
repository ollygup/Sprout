# 53 — Quick Launch dock (Win32 AppBar)

**What to build:** Docking for the Quick Launch window: it pins to the left or right screen edge as a Win32 AppBar — a slim vertical strip with the same two tabs (Quick Launch / Quick Actions) — that auto-hides to a sliver when not hovered (default) or stays fixed like a pinned taskbar. Dock/undock toggle and left↔right edge-switch arrows live in the window; mode (auto-hide/fixed) and default edge are set in the main app's Settings. Edge and mode are remembered per monitor; the AppBar is unregistered on app quit.

**Blocked by:** 52 — Quick Launch window (floating)

**Status:** done

- [x] Win32 AppBar via the raw window handle (`SHAppBarMessage`: `ABM_NEW` / `ABM_SETPOS` / `ABM_AUTOHIDE` / `ABM_REMOVE`); slim strip sized from design tokens, full height of the attached monitor's work area
- [x] Auto-hide (default): slides to a ~3px sliver when the cursor leaves the bar; slides back out on hover at the edge — taskbar-like (OS-managed via `ABM_SETAUTOHIDEBAR`; the system's ~2px sliver is honored)
- [x] Fixed mode: bar stays visible and reserves its strip; maximized windows on that edge shrink to accommodate (reserved workspace is the accepted AppBar trade-off, per ADR-0011)
- [x] Dock/undock toggle and left↔right edge-switch arrows in the window chrome; undocking returns to the floating window at its fixed default size, centered (per user feedback 2026-08-19: the floating window always spawns centered at a fixed size — no geometry is remembered; a remembered near-full-screen size made it open huge and impossible to move)
- [x] Mode and default edge configurable in the main app's Settings; edge and mode persist per monitor (keyed by the monitor's device name); attaches to the monitor the window is on
- [x] `ABM_REMOVE` on app quit so the edge is never left occupied
- [x] `cargo test` green; `npm run check` 0 errors; synced to the share