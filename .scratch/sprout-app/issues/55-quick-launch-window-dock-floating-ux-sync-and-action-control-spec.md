# 55 — Quick Launch window: dock, floating UX, live sync, and Quick Action control (spec)

**What to build:** A consolidated fix round for the Quick Launch window and Quick Launch dock (tickets 52–53): the floating window stops auto-hiding on blur (persistent until closed, freely draggable, fixed 340×460 centered — never remembered); dock/undock state becomes a persisted Settings parameter alongside dock mode and dock edge; the window live-syncs with the main app; the dock's auto-hide actually works (OS-managed, taskbar-like slide at the edge); docking follows the documented Win32 AppBar pattern so it reserves its strip on any desktop instead of overlapping; edge spacing is consistent on both sides; dock icons are meaningful; the undock round trip restores the exact original size; Quick Actions editor copy becomes concise behind InfoTip buttons; and Quick Actions gain optional run tracking with a Stop button. Implemented via tickets 56–62.

**Blocked by:** none (feature-area spec; implemented via tickets 56–62)

**Status:** ready-for-agent

## Problem Statement

The Quick Launch window (tickets 52–53) shipped with several broken or rough edges: the floating window is destroyed on blur, which makes dragging it evaporate mid-gesture and forces the tray to be the only way back; the docked bar's auto-hide never engages (the auto-hide call always targets the left edge regardless of the docked edge, and no AppBar callback message is registered), so the bar appears to "minimize to the tray" instead of sliding at the screen edge like the taskbar; on some physical multi-monitor setups (laptop + external monitor) the dock fails to reserve its strip and overlaps other windows while the main application shrinks; edge spacing is asymmetric depending on which side is docked; the dock toggle icon reads as a pause button; undocking after docking leaves the window smaller than it started (two divergent size constants); the window shows stale data — entries, actions, and dock settings added in the main app don't appear until the window is reopened; dock/undock is not a persisted setting; the Quick Actions editor's help text is verbose; and Quick Actions run fire-and-forget with no way to see or stop a running command.

## Solution

The Quick Launch window becomes a persistent, draggable, fixed-size palette that only closes on explicit user action, with dock state (floating/docked), mode (auto-hide/fixed), and edge (left/right) all configurable in the main app's Settings and live-applied. The window subscribes to a single change event and always reflects the main app's data. The dock is fixed to the documented Win32 AppBar pattern: correct edge in every call, a real callback message, auto-hide verified against the system, the strip thickness re-applied after `ABM_QUERYPOS`, the window placed at the rect returned by `ABM_SETPOS`, and re-assertion on `ABN_POSCHANGED` and drift — with failures surfaced in the window instead of half-docking. Spacing is mirrored per docked edge; icons are redrawn meaningfully. Quick Actions gain an optional Stop button: a persisted "stoppable" flag plus a stop command, an in-memory process registry tracking the spawned `Child`, and a reaper thread that flips the Run button to Stop while the command is alive.

## User Stories

**Floating window**

1. As a user, I want the floating window to stay open when I click elsewhere, so that I can consult it while working in other apps.
2. As a user, I want to drag the floating window by its header without it vanishing, so that I can move it anywhere.
3. As a user, I want the × button (and Alt+F4) to close it to the tray, so that closing is deliberate.
4. As a user, I want the tray left-click to reopen or raise the window, so that the tray stays the entry point.
5. As a user, I want the floating window to open at its fixed 340×460 size, centered, so that it never opens huge or off-screen.

**Settings**

6. As a user, I want the dock state (Floating / Docked) as a persisted setting in the main app, so that the window reopens in the state I left it.
7. As a user, I want dock mode (auto-hide / fixed) and dock edge (left / right) as Settings dropdowns, so that persistent dock behavior is configured where I configure everything else.
8. As a user, I want the in-window dock toggle and edge arrows to write back to the settings, so that the two surfaces never diverge.
9. As a user, I want changing dock settings in the main app to apply to an open window immediately, so that I don't have to reopen it.

**Live sync**

10. As a user, I want a Quick Action or Launch entry I add in the main app to appear in the open Quick Launch window without reopening it, so that the window is never stale.
11. As a user, I want dock edge/mode changes in Settings to reflect in the open window's dock chrome immediately, so that the chrome always tells the truth.
12. As a user, I want the window's theme to follow a theme change in the main app without reopening, so that it never looks out of place.

**Dock auto-hide**

13. As a user, I want the docked bar to slide out when my cursor reaches the screen edge and hide again when the cursor leaves the bar, so that auto-hide behaves like the taskbar.
14. As a user, I want auto-hide to engage on whichever edge the bar is docked, so that the mode setting actually does something.
15. As a user, I want the window to never auto-hide while floating, so that only the docked bar can hide itself.

**Dock robustness and edge switch**

16. As a user, I want docking to reserve its strip on any desktop — including laptop-plus-external-monitor setups — so that the bar never overlaps other windows.
17. As a user, I want the bar to re-assert its position when the taskbar, another app bar, or my monitor arrangement changes, so that it stays glued to the edge.
18. As a user, I want switching edges to be immediate and flicker-free, so that relocating the bar is a clean gesture.
19. As a user, I want a failed dock attempt to tell me visibly instead of leaving a half-docked window, so that I know what went wrong.

**Spacing and icons**

20. As a user, I want consistent spacing on both sides of the docked strip regardless of which edge it is docked to, so that it reads as one clean surface.
21. As a user, I want dock icons that show the docking direction, so that the chrome is self-explanatory rather than a pause glyph.
22. As a user, I want undocking to return the window to exactly the size it had before docking, so that the round trip is lossless.

**Quick Actions editor copy**

23. As a user, I want the form help text to be short and plain, with details behind an info button, so that the form reads quickly.

**Quick Action run tracking**

24. As a user, I want a Quick Action that is running to show a Stop button in the Quick Launch window, so that I can terminate it from where I started it.
25. As a user, I want the option to provide a stop command per action (e.g. `docker compose stop`), so that services stop gracefully instead of being killed.
26. As a user, I want a "Show Stop button" option revealed progressively, so that the form stays simple unless I opt in.
27. As a user, I want the tracking caveat — foreground commands only; detached commands (e.g. `-d`) report as not running — explained where I configure it, so that the Stop button is not a surprise.

## Implementation Decisions

- **The floating window stops being destroyed on blur** (`Focused(false)` handler): it stays open until the × button / Alt+F4 (destroy → tray reopens it). Dragging the header (existing `data-tauri-drag-region`) works without a drag guard because blur no longer destroys. Geometry stays fixed 340×460, always centered on open — no position or size memory (the earlier near-full-screen-size bug stays buried).
- **One shared window-geometry constant source**: a new `window_constants.rs` module holds `WINDOW_WIDTH` (340), `WINDOW_HEIGHT` (460), `DOCK_WIDTH = WINDOW_WIDTH`, and the main window's sizes/minimums. The docked strip becomes 340 wide; the 320 `DOCK_WIDTH` in `appbar.rs` is deleted. `undock()` restores the exact 340×460 inner size using the same size-API family the builder used (inner + min + max), fixing the shrink round trip. A new AGENTS.md design rule requires reusable UI geometry constants to live in that file.
- **Dock state becomes a persisted setting** (`dock.state` = "floating" | "docked"), joining the existing `dock.mode` and `dock.edge` defaults. Settings shows three chevron dropdowns (state, mode, edge). `open()` docks when state = docked, using the per-monitor edge/mode memory, falling back to settings defaults. The in-window dock/undock toggle and edge arrows persist their outcomes to the setting. `update_settings` applies dock-related changes to a live window (state → dock/undock, edge → reposition, mode → re-apply auto-hide).
- **Live sync via one event**: the backend emits `quick-launch-changed` at the end of every command that mutates what the window renders — the four Launch entry mutations, the four Quick Action mutations, `update_settings`, `update_theme`. The window listens once and re-runs its load (`entries`, `actions`) and dock-state refresh (plus theme re-apply). ~12 lines total; no polling.
- **Auto-hide stays OS-managed** (taskbar parity), fixed properly: `set_autohide` receives the *actual* docked edge; a real `uCallbackMessage` (from `RegisterWindowMessage`) is set before `ABM_NEW`; `ABN_STATECHANGE` is handled to keep the frontend honest; engagement is verified via `ABM_GETAUTOHIDEBAR` and a failure is surfaced, not swallowed. Floating windows never auto-hide. If the OS path still misbehaves on the physical setups after this ticket, an app-managed hover hook becomes a follow-up — not part of this round.
- **Docking follows the documented AppBar pattern exactly** (root cause of the "space reserved but window overlaps" symptom): after `ABM_QUERYPOS`, the strip thickness is re-applied to the returned rect; the window is placed using the rect returned by `ABM_SETPOS` (currently discarded). `ABN_POSCHANGED` re-queries and re-sets position; a drift check compares `GetWindowRect` against the expected rect (Win+Shift+→, monitor reconnect) and re-docks. Edge switches apply position+size atomically with no hide/show. Registration failure: in-window error, no half-docked state, one log line carrying the actual `SHAppBarMessage` result.
- **Dock visuals**: new `dock-left`/`dock-right` icons drawn in the design-system stroke style (24×24, stroke 1.7, `currentColor`) — the icons8 Left/Right Docking concepts are visual references only, not embedded assets (license). The docked hint shows the current edge; the dock toggle shows the target edge. The unused `layout` icon is removed. Docked-edge CSS classes (`qlw--docked-left/right`) mirror the header padding so both sides of the strip have the same spacing on either edge.
- **Quick Actions form copy**: the three verbose hint texts move behind the existing `InfoTip` pattern (prior art: `PresetFormDialog`) with concise labels: "Shown in the Quick Actions tab." / "PowerShell script; runs with -NoProfile -NonInteractive. Multi-line is fine." / "Working directory; empty = the app's folder."
- **Quick Action run tracking**: the `quick_actions` table gains `stoppable` (0/1) and `stop_command` (nullable text). Running state lives in an in-memory registry in `AppState` (keeps the spawned `Child` instead of dropping it; liveness via `try_wait`). A reaper thread per tracked action emits `quick-action-run-state-changed` on exit so the Quick Launch window's Run button flips to Stop with no polling. Stop runs the user's stop command; with no stop command it kills the process tree (`taskkill /T /F`, prior art already in the repo). Editor: a "Show Stop button" checkbox reveals the Stop command field (progressive disclosure) with a concise note that tracking covers foreground commands only (detached commands like `docker compose up -d` report as not running — inherent PID limitation).
- **Schema changes**: `quick_actions` gains two columns via the existing idempotent migration pattern; `dock.state` is a new settings key.

## Testing Decisions

- **What makes a good test**: external behavior through existing seams — the `db.rs` suites for migrations/CRUD/settings round trips, the `run_timed_process` path for command execution, pure geometry math for the AppBar rects (prior art: `appbar.rs` unit tests, `db.rs` dock-memory suites, `settings.rs` dock validation suites). Win32 syscalls (`SHAppBarMessage`) are not unit-testable and are verified manually.
- **Modules**: `db.rs` (migration + round trips for `stoppable`/`stop_command` and `dock.state`), `quick_actions.rs` (validation of the new fields, stop-command resolution), the process registry (liveness transitions), `appbar.rs` (rect math with the re-applied thickness and the SETPOS-returned rect). Frontend gated by `svelte-check` 0 errors (repo convention; no frontend test framework exists).
- **Manual verification surface** (ticket ACs): both physical setups (laptop screen + external monitor, different laptop models) for dock reservation/overlap, auto-hide slide on both edges, edge-switch flicker, and the dock→undock→dock→undock size round trip.
- **Gates**: `cargo test` green, `npm run check` 0 errors, sync to the share after each ticket per AGENTS.md.

## Out of Scope

App-managed auto-hide animation (only as a follow-up if the fixed OS path still fails on the physical setups); floating-window position memory; main-window geometry persistence; WebView2 memory footprint (it is the rendering engine and exits when all windows close — no change); a full-app icon audit beyond the Quick Launch window (all other icons were verified correct); PID-based tracking of detached/daemonized commands (inherent limit, documented instead); elevation, per-action logs, or starting actions at login.

## Further Notes

- Glossary updates land with the implementation tickets: "Quick Launch window" loses "hides to the tray on blur" (becomes persistent until closed); "Quick Action" gains the optional Stop button and stop command (CONTEXT.md — glossary only).
- ADR-0011's per-monitor state and AppBar decision stands; the blur-hide reversal and the Quick Action tracking extension are recorded in this spec's decisions and the ticket ACs — an ADR amendment is only worth adding if the tracking shape surprises future readers (it is a plain schema + registry extension, so likely not).
- AGENTS.md gains one design rule: reusable UI geometry constants live in `src-tauri/src/window_constants.rs` — never re-declared in another module; scan that file first before any UI-dimension change.
- Rollout order: 56 (floating UX + constants) → 57 (settings + live sync) → 58 (copy) → 59 (visuals) → 60 (auto-hide) → 61 (robustness) → 62 (run tracking). Tickets 60 and 61 are serialized (same `appbar.rs` surface); 62 follows 57 (reuses the event-sync pattern).

## Acceptance checklist

- [ ] Floating window persistent (no blur-destroy), draggable, fixed 340×460 centered; × / Alt+F4 closes to the tray; tray raises it
- [ ] Single shared UI-geometry constant file (340 width everywhere); undock round trip restores exact size; AGENTS.md rule added
- [ ] `dock.state` setting + state/mode/edge dropdowns; Settings changes apply live; in-window controls write back; docked reopens docked
- [ ] `quick-launch-changed` event from all mutation commands; window reloads entries, actions, dock state, theme without reopening
- [ ] Auto-hide engages on the actual docked edge (verified via `ABM_GETAUTOHIDEBAR`), taskbar-style slide, failure surfaced
- [ ] Documented AppBar pattern (thickness re-applied, SETPOS rect used, `ABN_POSCHANGED`, drift re-dock, atomic edge switch); dock reserves its strip on both physical setups; failures visible
- [ ] `dock-left`/`dock-right` icons in design-system style; hint/toggle edge-correct; dead `layout` icon removed; mirrored per-edge spacing
- [ ] Quick Actions editor copy concise behind InfoTip
- [ ] `stoppable` + `stop_command` columns; in-memory registry + reaper; Run ↔ Stop in the window; stop command or tree kill; progressive-disclosure editor UI with foreground-only note
- [ ] `cargo test` green; `npm run check` 0 errors; synced to the share after each ticket