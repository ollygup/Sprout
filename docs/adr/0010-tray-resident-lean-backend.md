# Tray-resident lean backend, virtual-desktop launch, cap+queue (ticket 37)

> Status: amended 2026-09-05 — original decision text preserved; see the executable-source audit amendment for current behavior and implementation gaps.

Backfilled from ticket 37's promised ADR (its "Further Notes" referenced this number, but it was never written). Sprout stays resident in the tray when the window is closed: closing destroys the window and webview, leaving only the lean Rust backend, and the tray icon is the one-click Quick Launch surface. Launches move apps to their assigned virtual desktops after their window appears, the virtual-desktop surface is gated to Windows 11 24H2+ (undocumented COM, build 26100.2605+), and launches run under a configurable concurrency cap with a queue.

## Why

Quick Launch's job is the morning routine: start the configured apps with one click, without a 100–300 MB UI resident. A resident window would defeat that, so the backend must survive with zero windows. Desktop placement and large launch sets were the two hard problems: Windows offers no public API to spawn an app onto a specific desktop (only to *move* a window after it appears), and a 30-app set must not slam the machine.

## Decisions

- **Tray-resident backend**: closing the window (× / Alt+F4) destroys it and the webview; `ExitRequested` is suppressed while the tray exists, so the process keeps only the backend. The tray menu's Quit is the only real exit; single-instance focus and the tray's Open Sprout recreate the window on demand.
- **Move-after-launch desktop placement**: apps are launched on the current desktop, then moved to their assigned virtual desktop once their main window appears (`wait_for_new_window` + `move_window_to_desktop`). This is a documented API limitation — there is no "spawn onto desktop X" — so windows may briefly flash on the current desktop.
- **Windows 11 24H2+ gate**: virtual-desktop support depends on undocumented COM wrapped by `winvd`; the whole surface is hidden below build 26100.2605 and `desktops()` is empty there. Assignments store the desktop **GUID** (stable across Task View reorder); labels are positional.
- **Cap + queue**: at most N launches in flight (default 8, 1–50, configurable); an entry frees its slot when its main window appears (windowless/command entries free at spawn); a 15 s no-window timeout counts as started and never stalls the queue; a failed launch never aborts the rest; already-running apps (full exe-path match) are skipped and reported. A second tray click during a run is ignored with a "launch already in progress" notification.

## Consequences

- The Quick Launch window (ADR-0011) inherits these rules: its Start button and the tray's Start all converge on the same capped runner.
- Windows may briefly flash on the current desktop when a launch is placed elsewhere — accepted and documented.
- Below 24H2 the desktop-group surface silently disappears; nothing else changes.

## Amendment — 2026-09-05 (codebase accuracy pass)

Three wording fixes, no behavior change: the gate is major build 26100 (the code reads `CurrentBuild` and cannot see the `.2605` UBR revision); the second-Start refusal is a command error, not a system notification; and "desktop-group surface" is retired terminology — assignment vs Groups is owned by ADR-0015. Residency, move-after-launch, GUID storage, and cap+queue numbers are verified unchanged.

## Amendment — 2026-09-05 (executable-source audit)

The tray opens the Quick Launch window; list execution starts from the window or page, not directly from the tray (`src-tauri/src/tray.rs`, `init` and `build_menu`). Existing-window matching uses case-insensitive executable basenames through `process_matches_image`/`image_basename` in `src-tauri/src/engine/windows.rs`, not full executable paths. Distinguishing two applications with the same filename in different directories is therefore not guaranteed; full-path matching remains an implementation gap against the stated policy.

Only launches with a live desktop assignment retain a queue slot after spawn (`src-tauri/src/launch.rs`, `run_launch_queue_inner`). Desktop labels prefer the Windows desktop name and otherwise use “Desktop N” (`virtual_desktops`). Main-window close destroys its webview after the unsaved-Settings gate allows the close (`src-tauri/src/lib.rs` close-event handling); it is not unconditional immediate destruction. The existing major-build gate correction and tray-residency decision remain current.
