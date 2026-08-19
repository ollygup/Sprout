# 37 — Quick Launch: tray one-click app launcher

**What to build:** A "Quick Launch" tab where the user composes a list of Launch entries — discovered apps (Start Menu shortcuts + installed-program registry) or custom commands (PowerShell/cmd/direct exe) with a Test button — optionally assigned to virtual desktops. A resident, lean tray icon (Rust backend only when the window is closed) starts the whole list on left-click; right-click offers start-all, per-desktop-group and per-app entries, Open Sprout, and Quit. Launches run through a configurable concurrency cap with a queue — at most N apps in flight, a slot frees when the app's main window appears. Virtual desktop grouping exists only on Windows 11 24H2+, hidden entirely below it. The list is machine-local, never part of Presets or exports.

**Blocked by:** none

**Status:** ready-for-agent

## Problem Statement

Setting up a dev environment means opening the same handful of apps (VS Code, IntelliJ, Postman, DBeaver) one by one every morning. The user wants one click — from the Windows tray "show hidden icons" area — that starts their configured set, without bloat: the app must not hold a UI resident while doing it, and a large set must never lag or crash the machine.

## Solution

A Quick Launch tab in Sprout where the user composes a list of Launch entries — discovered apps (Start Menu shortcuts + installed-program registry) or custom commands (PowerShell/cmd/direct exe) with a Test button — optionally assigned to virtual desktops. A resident, lean tray icon (Rust backend only when the window is closed) starts the whole list on left-click; right-click offers start-all, per-desktop-group and per-app entries, Open Sprout, and Quit. Launches run through a configurable concurrency cap with a queue — at most N apps in flight, a slot frees when the app's main window appears — so a 30-app group still fully launches without a spike. Virtual desktop grouping (assign entries to desktops, "New desktop…" created on the user's behalf) exists only on Windows 11 24H2+, hidden entirely below it. The list is machine-local, never part of Presets or exports.

## User Stories

1. As a user, I want a new "Quick Launch" tab in the navigation rail, so that I can configure which apps one click in the tray starts.
2. As a user, I want to search my installed applications (Start Menu shortcuts for the current user and all users, plus registry uninstall entries), so that I can pick real installed apps instead of typing paths.
3. As a user, I want search results to show name, publisher, and icon, so that I can recognize the app I mean.
4. As a user, I want the app list to be fresh every time I open the tab (no stale cache), so that newly installed apps appear without a resync step.
5. As a user, I want a "browse for exe" fallback picker, so that I can add an app the search missed.
6. As a user, I want to add a custom command entry with a shell choice (PowerShell, cmd, or direct exe), so that I can start anything, including obscure apps or multi-step startup sequences.
7. As a user, I want a Test button on command entries that runs the command and reports exit code and output, so that I can verify it works before saving; interactive commands are honestly reported as not headless-verifiable.
8. As a user, I want command entries to run hidden by default with a per-entry "show window" toggle, so that startup stays clean but debugging a command is possible.
9. As a user, I want my Launch entries to persist across app restarts, so that I configure once.
10. As a user, I want to reorder entries and remove them, so that the launch sequence matches my workflow.
11. As a user, I want an empty state on the tab explaining how to add the first entry, so that the feature is discoverable.
12. As a user, I want a Sprout tray icon under "show hidden icons", so that Quick Launch is one click away.
13. As a user, I want left-clicking the tray icon to start all configured apps, so that one gesture does the morning setup.
14. As a user, I want the tray right-click menu to offer Start all, per-virtual-desktop-group submenus, per-app entries, Open Sprout, and Quit, so that I can start subsets or open the app without the window.
15. As a user, I want closing the Sprout window to keep only the lean Rust backend alive in the tray, so that Quick Launch stays available without a 100–300 MB UI resident; reopening recreates the window.
16. As a user, I want Quit to be available only from the tray menu, so that closing the window never kills the launcher by accident.
17. As a user, I want launches to run under a configurable concurrency cap (default 8, range 1–50, in Settings), so that the machine never gets slammed.
18. As a user, I want entries beyond the cap to queue and start as slots free (an entry frees its slot when its main window appears), so that the whole list still launches without lag.
19. As a user, I want apps already running (matched by full exe path) to be skipped and reported, so that I never get duplicate instances; unreadable process paths are treated as running (safe direction).
20. As a user, I want a failed launch (spawn error or no window within 15 s) to never abort the rest, so that one broken app doesn't block the morning setup.
21. As a user, I want a single end-of-run notification summarizing started / skipped / failed with app names, so that I know what happened without opening anything.
22. As a user, I want a second tray click during an active launch run to be ignored with a "launch already in progress" notification, so that I can't stack runs.
23. As a user, I want to assign entries to virtual desktops via a per-entry menu (Current desktop / Desktop 2… / New desktop…), so that my environment lands arranged across desktops.
24. As a user, I want "New desktop…" to create the virtual desktop for me on the spot, so that I never leave Sprout to arrange my setup.
25. As a user, I want desktops shown with positional labels (Desktop 1, Desktop 2, …), so that assignments are readable.
26. As a user, I want the desktop assignment surface to exist only on Windows 11 24H2+ and be fully hidden below it, so that unsupported OSes never see a broken feature.
27. As a user, I want an entry assigned to a desktop that no longer exists to launch on the current desktop with a note in the summary, so that a deleted desktop never blocks the run.
28. As a user, I want a Start button on the Quick Launch page itself, so that I can trigger the same run without the tray.
29. As a user, I want the page to show "N apps — up to K launch at a time", so that the cap is visible where I configure.

## Implementation Decisions

- **Domain model**: Launch entry (one app or command in the list; distinct from "Application", the composer UI synonym for Requirement), Launch run (one execution of the list through the capped, queued pipeline), Desktop assignment (entry's target virtual desktop; NULL = current desktop). Machine-local config; never part of Presets, Plan, Run, or exports (spirit of ADR-0009, mirroring `install_dir`).
- **Seam**: new `LauncherEngine` trait at the engine strategy layer, sibling of `PlatformEngine` (which stays install-only). Surface: `spawn(entry)` (ShellExecuteExW for apps — launch the .lnk/.exe as-is; command entries through the existing `CREATE_NO_WINDOW` convention or visible per `show_window`), `wait_for_window(pid, 15s)`, `move_window_to_desktop(pid, guid)`, `is_running(exe_path)` (Toolhelp32 snapshot + `QueryFullProcessImageName` full-path match; query failure → treat as running), `desktops()`, `create_desktop()`. Windows impl + fake in tests (FakeEngine pattern, run.rs).
- **Orchestrator** `launch_run(entries, cap, engine)`: pure logic, no seam of its own. Cap+queue — at most N in flight; windowless and command entries free their slot at spawn; 15 s window timeout counts as *started* (never stalls the queue); skip-already-running transparent; unknown/deleted desktop GUID → current desktop + note; failures never abort; returns summary (started/skipped/failed + names). Concurrent-run guard: second trigger → "already in progress" notification, no stacking. Runs on a background thread; completion emitted as a notification.
- **Candidate walker**: pure function with injected roots (Start Menu dirs: per-user + ProgramData; registry uninstall reader: HKLM 32/64 + HKCU), deduped by exe path. Fresh walk on every tab open (<100 ms); no list cache, no icon cache, no resync button. Icons via `SHGetFileInfoW`, extracted lazily for visible rows only (in-memory, dies with the window). Store/AppX apps excluded from search.
- **Storage** (db.rs migration pattern + tests): `launch_entries` (id, name, kind `app|command`, target, shell `powershell|cmd|none`, show_window, desktop_id GUID or NULL, position). `meta` key `launch.concurrency` (1–50, default 8) added to the `Settings` struct; no new settings commands. Desktop stored as **GUID** (stable across Task View reorder); labels are positional (Desktop 1/2/…); real Windows desktop names deferred (undocumented COM the winvd crate does not yet expose).
- **Virtual desktops**: `winvd` crate, Win11 24H2+ (26100.2605+) only, gated at runtime like `windows_build_number()`; below the gate `desktops()` is empty and the frontend hides the whole assignment surface. "New desktop…" calls `create_desktop()` and assigns the entry. The feature never switches the user's current desktop; windows are moved after appearing.
- **Tray + residency**: `TrayIconBuilder` with the existing brand icon; left-click → start run; right-click menu (Start all (N) / per-desktop-group submenus / per-app / Open Sprout / Quit) — all items render from the same entries list. Close (× or Alt+F4) destroys the window and webview; exit-suppression wiring keeps the backend alive with zero windows; single-instance focus hook recreates the window when missing. No close-behavior setting; Quit is tray-only.
- **Tauri commands** (`lib.rs`): `list/create/update/delete/move_launch_entry`, `list_launch_candidates`, `test_launch_command` (timeboxed, exit code + output via existing `run_timed_process`), `start_quick_launch`, `list_virtual_desktops`, `create_virtual_desktop`.
- **New dependencies**: `winvd` (only new crate) + `windows-sys` features `Win32_System_Threading` (process snapshot) and `Win32_System_ProcessInformation` (QueryFullProcessImageName). `ShellExecuteExW`/`SHGetFileInfoW` already covered by `Win32_UI_Shell`. Size-budget NFR 43: winvd is small COM wrappers; LTO/size profile already in place.

## Testing Decisions

- **What makes a good test**: external behavior through the `LauncherEngine` fake — the orchestrator must prove the *rules* (cap honored, queue drains, skips, ordering, tolerance, summary), not the Windows APIs. Windows-specific calls are thin delegation behind the seam and are verified manually + via existing patterns.
- **Orchestrator** (prior art: FakeEngine-driven pipeline tests in run.rs): cap respected with >cap entries; slot freed on window appearance; windowless/command entries free at spawn; 15 s timeout → started, queue continues; skip-already-running path-matched and reported; unknown desktop → current + note; one failure never aborts the rest; summary contents exact; concurrent-run guard.
- **db.rs**: migration on a pre-existing database, CRUD roundtrip across reopen, validation (blank name/target, illegal kind/shell combos, position, concurrency 1–50) — prior art: existing db.rs suites.
- **Candidate walker**: merge/dedupe and metadata selection against injected fixtures (temp Start Menu dirs + scripted registry reader) — no real registry access in tests.
- **Command test**: exit code and output capture, timeout behavior — prior art: `command_step`/`run_timed_process` engine tests.
- **Frontend**: no test framework exists in the repo; gate is `svelte-check` 0 errors (repo convention).
- **Gates**: `cargo test` green, `npm run check` 0 errors, release build + parity checklist flow unchanged.

## Out of Scope

Autostart-at-login; launch history/logging; per-entry enable toggle (removal covers it); Store/AppX apps in search; exporting or sharing the list; renaming virtual desktops from Sprout; elevation/UAC; stagger-delay setting (cap+queue replaces it); icon disk cache / resync button; multiple Quick Launch groups; close-behavior setting; reading real Windows desktop names.

## Further Notes

- Tray left-click = start all immediately (no confirmation) — safety comes from the cap, queue, and skip-running.
- ADR-0010 records: tray-resident lean backend (destroy window, backend-only), move-after-launch virtual desktop strategy (documented API limitation), Win11 24H2+ gate, cap+queue model.
- Glossary additions: Launch entry, Launch run, Desktop assignment (CONTEXT.md — no implementation detail).
- Rollout order: DB → engine + orchestrator → walker → commands → tray/residency → frontend → docs → release gates; robocopy sync to the share after each milestone per AGENTS.md.

## Acceptance checklist

- [ ] `launch_entries` table + `launch.concurrency` setting with migration on existing databases; validation rejects blank names/targets, illegal kind/shell combos, concurrency outside 1–50
- [ ] `LauncherEngine` trait + Windows impl + fake; orchestrator honors the cap, drains the queue, frees slots on window appearance, skips path-matched running apps, falls back to current desktop for unknown GUIDs, never aborts on failure, returns an exact summary
- [ ] Candidate walker returns fresh merged/deduped list from Start Menu + registry (fixture-tested); icons lazy per visible row; no cache, no resync
- [ ] Custom command entries with shell choice, show-window toggle, and a Test button reporting exit code + output; interactive commands reported as not headless-verifiable
- [ ] Tray icon (brand icon): left-click starts all; right-click menu = Start all / per-desktop-group submenus / per-app / Open Sprout / Quit; closing the window destroys it and keeps the backend resident; Quit tray-only; second run guarded with "already in progress"
- [ ] Virtual desktop assignment per entry with "New desktop…" creation on behalf; surface fully hidden below Windows 11 24H2+; positional labels; never switches the user's desktop
- [ ] Quick Launch tab in NavRail with search-to-add, browse-for-exe fallback, command dialog, reorder/remove, cap hint, empty state; Settings holds the concurrency cap
- [ ] `cargo test` green, `npm run check` 0 errors; synced to the share
