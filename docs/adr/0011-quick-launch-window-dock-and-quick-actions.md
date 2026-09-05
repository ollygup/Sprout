# Quick Launch window — AppBar dock and Quick Actions (tickets 49–54)

> Status: amended 2026-09-05 — original decision text preserved; see the executable-source audit amendment for current behavior and implementation gaps.

The tray's launch surface moves into a miniature Quick Launch window (opened by tray left-click) with two tabs — Quick Launch (a single Start button that starts the whole Quick Launch list) and Quick Actions (fire-and-forget user commands) — which can float or dock as a Win32 AppBar on the left/right screen edge, auto-hiding like the taskbar or staying fixed. The tray menu is slimmed to Open Sprout + Quit.

## Why

The tray menu is a cramped, read-only surface: every Launch entry and desktop group is a native menu item with no room to grow or give feedback, and it only starts apps. Recurring commands ("restart the docker stack", "start my dev services") have no home at all. The user wants a taskbar-like dock — a window that reserves a slim strip on a screen edge and auto-hides like the taskbar — plus a place for arbitrary commands that don't fit the app-launch model.

## Decisions

- **The Quick Launch window is the resident quick-access surface** (tray left-click opens/raises it). Its Quick Launch tab is a single Start button that starts the whole Quick Launch list; its Quick Actions tab lists each action as NAME + Run button. The window is read-only — no configuration surface.
- **Docking is a Win32 AppBar** (taskbar-like), not a custom edge-attach or a floating always-on-top overlay. The user explicitly asked for "OS docking like a taskbar". An AppBar coordinates with the shell; in `fixed` mode it reserves its screen edge — a docked bar takes space from maximized windows by design. *(Amended 2026-08-21, ticket 63: `auto-hide` mode does overlay content and reserves nothing — see below; the "never overlays" clause now applies to `fixed` only.)*
- **Two dock visibility modes, configured in the app**: auto-hide as default, and fixed (always visible, strip permanently reserved, like a pinned taskbar).
- **Live dock controls live in the window**: a dock/undock toggle and left↔right edge-switch arrows — no main-app round trip for the everyday gesture. Persistent behavior (mode, default edge) is set in the main app's Settings.
- **Quick Action is a separate machine-local concept**, not a kind of Launch entry: name + PowerShell command + optional working directory, run fire-and-forget hidden as the current user with no elevation and no status UI. Launch entries keep their app-launch + desktop-group semantics; the two lists are independent.
- **Per-monitor dock state**: edge and visibility mode persist per monitor; the floating window remembers its size and position. The AppBar is unregistered (`ABM_REMOVE`) on app quit so the edge is never left occupied.
- **The window reuses the existing design system** (tokens.css, shared components, both themes) — a new view, not a new visual language. Any genuinely new shared component (a minimal accessible tab strip) is added to the component foundation and reviewed per the AGENTS.md design rule.

## Consequences

- The tray is no longer the only resident surface; the dock can be the visible resident form.
- Maximized windows on the docked edge shrink by the strip width while the dock is fixed; in auto-hide they never resize — the strip overlays them.

## Amendment — 2026-08-21 (ticket 63)

Research (`docs/research/0003-appbar-autohide-os-contract.md`) established that
no OS mechanism ever moves or hides an appbar — motion is always the app's
own — and the user redefined auto-hide's contract: *"the main application
should stay as full width … on hover it would overlay on top of the app so
the main application doesn't need to shrink/resize"*. Auto-hide therefore
registers with the shell for coordination only (exclusivity, notifications,
z-order courtesy) and never calls `ABM_SETPOS`: hidden or revealed, other
windows keep their full size while Sprout's driver slides the strip over them
(~180 ms ease-out; hidden is fully off-screen with no handle — the 2 px sliver survives only as trigger-band math). `fixed` mode keeps the original reserving
dock unchanged. The two modes remain independent of the taskbar's own
auto-hide setting ("not tied to each other, never").
- Two triggers (window Start button, page Start button) now converge on the same runner (`launch_entries`), keeping one pipeline for launch semantics.

## Amendment — 2026-09-05 (codebase accuracy pass)

Three corrections to the window description above, all verified against `quick_window.rs` and the Quick Launch window page: the window has three tabs, not two — **Quick Clips** joins Quick Launch and Quick Actions once at least one Clip exists (and leaves when the last Clip is deleted). Quick Action rows are not NAME + Run — they are a details control plus a three-state Run/Stop/Stopping control with note glyph, backed by the tracked-run registry (see the Quick Action execution-model ADR). And the floating window does not remember size or position — it is always the fixed `340×460` palette, centered (`constants/window.rs` is the single size source). The dock, AppBar, per-monitor memory, and Settings-vs-window control split described above are unchanged. Deeper evolutions (dock-width %, density, companion pane, Groups in the window, clickable entries) are recorded in their own ADRs, not here.

## Amendment — 2026-09-05 (executable-source audit)

The absolute “never calls ABM_SETPOS” statement in the earlier amendment is inaccurate. `settle_mode` in `src-tauri/src/quick_window.rs` calls `appbar::reserve` with a zero-width reservation when entering auto-hide, and `reserve` in `src-tauri/src/appbar.rs` invokes `SHAppBarMessage(ABM_SETPOS)`. The intended distinction is zero workspace reservation for auto-hide versus a positive-width reservation for fixed mode; the mere presence of this Windows call does not imply a reserving auto-hide strip.

Motion remains app-owned. The subsequent three-tab, tracked-action, and fixed centered floating-palette corrections remain current. The driver and reservation-failure limitations are recorded in ADR-0019.
