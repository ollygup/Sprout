# 44 — Virtual desktop grouping

**What to build:** Entries can be assigned to Windows virtual desktops, so one click arranges the user's environment across desktops. Assignments happen through a per-entry menu (Current desktop / Desktop 2… / New desktop… — "New desktop…" creates the virtual desktop on the user's behalf), desktops are labeled with their Windows name when available ("Desktop N" otherwise), launched windows are moved to their assigned desktop, and the whole surface is fully hidden below Windows 11 24H2. Parent spec: 37.

**Blocked by:** 42 — Launch run: capped queue engine, Start button, summary

**Status:** implemented

- [x] winvd-backed `desktops()` and `create_desktop()` in the Windows launcher (the winvd dependency is exercised here); runtime-gated on Windows 11 24H2+ like the build-number check; below the gate `desktops()` is empty
- [x] `list_virtual_desktops` and `create_virtual_desktop` commands; desktop stored as GUID (stable across Task View reorder), labels = Windows name when non-empty else "Desktop N"
- [x] Per-entry assignment menu on the Quick Launch page: Current desktop / each desktop / New desktop…; menu and all grouping UI hidden entirely below 24H2
- [x] Orchestrator moves an assigned entry's window to its desktop after it appears; a desktop GUID that no longer exists falls back to the current desktop with a note in the summary; never switches the user's current desktop
- [x] Tray menu gains per-desktop-group submenus (each starts that group's entries through the same runner); hidden below 24H2
- [x] `cargo test` green (orchestrator desktop-move ordering + fallback with the fake; gating logic), `npm run check` 0 errors; manual verification on 24H2; synced to the share