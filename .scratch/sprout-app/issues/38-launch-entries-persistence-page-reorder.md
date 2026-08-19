# 38 — Launch entries: persistence, page, and reorder

**What to build:** The Quick Launch tab becomes a real surface: the user adds apps to a persistent list (browse-for-exe picker as the entry source for now), removes them, reorders them, and the list survives restarts. The Settings screen gains the concurrency knob that will gate the launch run. Parent spec: 37.

**Blocked by:** None — can start immediately

**Status:** done

- [x] `launch_entries` table (id, name, kind `app|command`, target, shell `powershell|cmd|none`, show_window, desktop_id nullable GUID, position) created by the migration; existing databases upgrade idempotently; schema test proves an old database migrates cleanly
- [x] CRUD + reorder roundtrip across reopen (list/create/update/delete/move), position honored; duplicate-name entries allowed
- [x] Validation: blank name and blank target rejected; illegal kind/shell combos rejected; position non-negative
- [x] `launch.concurrency` setting (1–50, default 8) on the `Settings` struct: load/save/validate, invalid values rejected without touching the stored ones
- [x] Dependency groundwork folded in: windows-sys features `Win32_System_Threading` + `Win32_System_Diagnostics_ToolHelp` added (parked until ticket 42); pre-added winvd/windows/png stay unused-but-present until their tickets
- [x] "Quick Launch" tab in the NavRail between Plan and History; page lists entries (name, kind badge, truncated target), add via browse-for-exe file dialog, remove, up/down reorder, cap hint "N apps — up to K launch at a time", EmptyState when the list is empty
- [x] Settings screen shows the concurrency knob alongside the other knobs and saves it with them
- [x] `cargo test` green, `npm run check` 0 errors; synced to the share