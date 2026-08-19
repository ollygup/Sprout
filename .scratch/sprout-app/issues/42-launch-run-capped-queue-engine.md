# 42 — Launch run: capped queue engine, Start button, summary

**What to build:** One click starts the whole Quick Launch list safely: at most N entries launch at once (the Settings cap), the rest queue, an entry frees its slot when its main window appears, apps already running are skipped, a failure never aborts the rest, and a summary notification reports started / skipped / failed at the end. A second click during a run is ignored, not stacked. Parent spec: 37.

**Blocked by:** 38 — Launch entries: persistence, page, and reorder

**Status:** done

- [x] `LauncherEngine` trait (sibling of `PlatformEngine`): `spawn(entry)`, `wait_for_window(pid, 15s)`, `move_window_to_desktop(pid, guid)`, `is_running(exe_path)`, `desktops()`, `create_desktop()` — desktop methods declared but returning empty/None until ticket 44
- [x] Windows impl: ShellExecuteExW for app entries (the .lnk/.exe as-is), CREATE_NO_WINDOW/visible per `show_window` for commands; EnumWindows-by-PID for the window wait; Toolhelp32 snapshot + `QueryFullProcessImageNameW` full-path match for skip-already-running (unreadable process path → treated as running, the safe direction)
- [x] Orchestrator (pure logic, fake-driven tests): cap+queue honored with >cap entries; windowless/command entries free their slot at spawn; 15 s window timeout counts as started, queue never stalls; skip-already-running reported; failures never abort; exact summary (started / skipped / failed + names)
- [x] `start_quick_launch` command runs the orchestrator on a background thread; concurrent-run guard returns "launch already in progress" instead of stacking
- [x] End-of-run summary notification via the notification plugin (started N, skipped M, failed K, with names of the failed)
- [x] Start button on the Quick Launch page triggers the same path as the tray will; cap hint reflects the Settings value
- [x] `cargo test` green (orchestrator against the fake: cap, queue drain, skip, tolerance, summary, guard), `npm run check` 0 errors; synced to the share