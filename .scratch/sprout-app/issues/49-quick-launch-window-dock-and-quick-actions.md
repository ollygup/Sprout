# 49 — Quick Launch window, dock, and Quick Actions (spec)

**What to build:** A miniature Quick Launch window opened from the tray icon, with two tabs — Quick Launch (a single Start button that starts the whole Quick Launch list) and Quick Actions (NAME + Run-button list of user-authored commands) — that floats as a window (hiding to the tray on blur) or docks as a Win32 AppBar on the left/right screen edge (auto-hide or fixed, like the taskbar), while the tray menu is slimmed to Open Sprout and Quit. Configuration for both tabs lives in the main app; the window is read-only.

**Blocked by:** none (feature-area spec; implemented via tickets 50–54)

**Status:** ready-for-agent

## Problem Statement

The tray menu is the only quick-access surface, and it is cramped: every Launch entry and desktop group is a native menu item, with no room to grow, no feedback, and nothing beyond starting apps. Recurring commands — "restart the docker stack", "start my dev services" — don't fit the app-launch model at all; today they mean opening Docker/IntelliJ and clicking through play buttons. The user wants a taskbar-like dock: a window that floats for quick access, then pins to a screen edge like the taskbar (auto-hiding to a sliver, or staying fixed), and that can fire arbitrary commands — all configured in the main app, never configured in the dock itself.

## Solution

The tray icon opens a small Quick Launch window. Tab "Quick Launch" holds one big Start button that starts the entire Quick Launch list. Tab "Quick Actions" lists user-authored actions, each a name with a Run button. The window floats (vanishing to the tray on blur) or docks to the left/right screen edge as a Win32 AppBar — auto-hiding to a 3px sliver when not hovered (default) or staying fixed like a pinned taskbar — with dock controls in the window and persistent behavior in the app's Settings. The tray menu shrinks to Open Sprout + Quit. A new Quick Actions page in the main app composes actions (name + PowerShell command + optional working directory, with a Test button), reorders them, and removes them.

## User Stories

**Quick Launch window**

1. As a user, I want left-clicking the tray icon to open the Quick Launch window (or raise it if already open), so that quick access is one gesture away.
2. As a user, I want the window to have two tabs — Quick Launch and Quick Actions — so that app launching and command running are clearly separated.
3. As a user, I want the Quick Launch tab to be a single Start button that starts the whole Quick Launch list, so that the morning routine is one click.
4. As a user, I want the Quick Launch tab to show the entry count next to the button, so that I know what the Start button will launch.
5. As a user, I want the Quick Actions tab to list every action as NAME + Run button, so that I can trigger one command without opening the main app.
6. As a user, I want the window to hide to the tray when I click elsewhere (blur), so that it stays out of the way; the tray icon reopens it.
7. As a user, I want the window to follow the app's light/dark theme and existing design language, so that it feels like part of Sprout.
8. As a user, I want the window's size and position remembered across restarts, so that it reappears where I left it.

**Docking**

9. As a user, I want to dock the window to the left or right screen edge from the window itself, so that I can pin it like a taskbar without opening the main app.
10. As a user, I want to switch the dock between the left and right edges from the window, so that I can relocate it without undocking.
11. As a user, I want to undock back to a floating window, so that I can move it freely when I don't want it pinned.
12. As a user, I want the docked bar to auto-hide (slide to a sliver when not hovered) by default, so that it takes minimal space.
13. As a user, I want a fixed mode where the dock stays visible and reserves its strip, so that I can keep it pinned like a taskbar.
14. As a user, I want dock mode (auto-hide/fixed) and default edge configurable in the main app's Settings, so that persistent behavior is set where I configure things.
15. As a user, I want the dock to attach to the monitor the window is on and remember its edge per monitor, so that multi-monitor setups work naturally.
16. As a user, I want the AppBar unregistered on app quit, so that the screen edge is never left occupied.

**Quick Actions**

17. As a user, I want a Quick Actions page in the main app where I compose actions as name + PowerShell command + optional working directory, so that I can define arbitrary commands once.
18. As a user, I want multi-line commands, so that complex scripts (restart a docker stack, start dev services) fit.
19. As a user, I want a Test button that runs the command timeboxed and reports exit code and output, so that I can verify it before saving.
20. As a user, I want to reorder and remove actions, so that the list matches my workflow.
21. As a user, I want actions to run fire-and-forget, hidden, as the current user with no elevation, so that they just do whatever I wrote.
22. As a user, I want actions to persist across restarts, so that I configure once.

**Tray**

23. As a user, I want the tray right-click menu to be just Open Sprout and Quit, so that the launch list lives in the window, not a cramped menu.
24. As a user, I want left-click on the tray to show/focus the window, so that the tray still reopens quick access.

## Implementation Decisions

- **A second webview window** created on demand (mirroring the existing "main window recreated on tray open" pattern) and destroyed on hide, so the backend stays lean. Frameless (no OS titlebar) with a draggable header and a close button; themed from the existing theme store and `tokens.css`; window capability added to the existing capability scope.
- **Docking is a Win32 AppBar** via the raw window handle (`SHAppBarMessage`: `ABM_NEW` / `ABM_SETPOS` / `ABM_AUTOHIDE` / `ABM_REMOVE`), with the slim strip sized from design tokens and an auto-hide sliver (~3px). Dock controls in the window: dock/undock toggle, left↔right edge-switch arrows. Dock state (edge, auto-hide/fixed) persists per monitor; the floating window persists size/position. `ABM_REMOVE` on app exit.
- **Quick Actions are a separate machine-local concept** stored in a new `quick_actions` table (id, name, command, cwd nullable, position) with the existing migration pattern. CRUD commands mirror the Launch entry commands; the run command executes PowerShell (`-NoProfile -NonInteractive -Command`) hidden via the existing `CREATE_NO_WINDOW` spawn path, fire-and-forget on a background thread, current user, no elevation, no status UI.
- **The window's Start button and the page's Start button converge on the existing runner** (`launch_entries`), so one pipeline keeps launch semantics (cap, queue, skips, summary notification).
- **New shared component**: a minimal accessible tab strip (ARIA tablist) added to the component foundation and built from existing tokens — the repo has no tab UI today. All other visuals reuse existing components (Button, IconButton, EmptyState, TextInput, Select, ContextMenu).
- **Tray**: left-click opens/raises the window; right-click menu rebuilt to two items (Open Sprout, Quit). The menu-building path for launch items (start-all, desktop groups, per-entry) is removed.

## Testing Decisions

- **What makes a good test**: external behavior through the seams that already exist — the quick-actions CRUD and runner against the database, and the launch pipeline unchanged. Win32 AppBar behavior is not unit-testable and is verified manually; the logic around it (per-monitor state, mode defaults, window lifecycle) is kept thin and testable.
- **Modules**: `db.rs` suites for the `quick_actions` migration, CRUD roundtrip across reopen, validation (blank name/command, position), and `Settings` additions (dock mode, default edge) — prior art: existing db.rs suites. Command execution reuses the tested `run_timed_process` / spawn path. Frontend gated by `svelte-check` 0 errors (repo convention; no frontend test framework exists).
- **Gates**: `cargo test` green, `npm run check` 0 errors, release build + parity checklist flow unchanged.

## Out of Scope

Per-entry launching from the window (the window's Quick Launch tab is Start-all only); desktop-group management in the window; per-action status, logs, or stop buttons; elevation or running actions at login; window-size settings; dock theming beyond the app theme; anything that would make the window configurable.

## Further Notes

- ADR-0011 records the AppBar dock decision, the Quick Action concept, and the tray slimming; ADR-0010 backfills the tray-resident backend/cap+queue rules the window inherits.
- Glossary additions: Launch entry, Quick Launch, Quick Launch window, Quick Launch dock, Quick Action (CONTEXT.md — no implementation detail).
- Rollout order: quick-actions storage/runner (50) → editor page (51) and window (52) → dock (53) and tray slimming (54); robocopy sync to the share after each milestone per AGENTS.md.

## Acceptance checklist

- [ ] `quick_actions` table + migration on existing databases; validation rejects blank names/commands; CRUD + run command tested
- [ ] Quick Actions page in the main app: compose name + multi-line PowerShell + optional working directory, Test button (timeboxed, exit code + output), reorder, remove, empty state
- [ ] Quick Launch window: tray left-click opens/raises it; two tabs; Quick Launch tab = Start button + entry count; Quick Actions tab = NAME + Run list; blur hides to tray; themed; size/position persisted
- [ ] Dock: Win32 AppBar on left/right edge, auto-hide (default) and fixed modes, dock/undock toggle + edge switch in the window, mode/default edge in Settings, per-monitor memory, `ABM_REMOVE` on quit
- [ ] Tray: right-click = Open Sprout + Quit; launch-item menu path removed
- [ ] `cargo test` green, `npm run check` 0 errors; synced to the share