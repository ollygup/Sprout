# Quick Launch window — AppBar dock and Quick Actions (tickets 49–54)

The tray's launch surface moves into a miniature Quick Launch window (opened by tray left-click) with two tabs — Quick Launch (a single Start button that starts the whole Quick Launch list) and Quick Actions (fire-and-forget user commands) — which can float or dock as a Win32 AppBar on the left/right screen edge, auto-hiding like the taskbar or staying fixed. The tray menu is slimmed to Open Sprout + Quit.

## Why

The tray menu is a cramped, read-only surface: every Launch entry and desktop group is a native menu item with no room to grow or give feedback, and it only starts apps. Recurring commands ("restart the docker stack", "start my dev services") have no home at all. The user wants a taskbar-like dock — a window that reserves a slim strip on a screen edge and auto-hides like the taskbar — plus a place for arbitrary commands that don't fit the app-launch model.

## Decisions

- **The Quick Launch window is the resident quick-access surface** (tray left-click opens/raises it). Its Quick Launch tab is a single Start button that starts the whole Quick Launch list; its Quick Actions tab lists each action as NAME + Run button. The window is read-only — no configuration surface.
- **Docking is a Win32 AppBar** (taskbar-like), not a custom edge-attach or a floating always-on-top overlay. The user explicitly asked for "OS docking like a taskbar". An AppBar reserves its screen edge, which is the intended trade-off; this supersedes the earlier "must not affect other applications" constraint — a docked bar takes space from maximized windows by design. It never *overlays* content; it reserves it.
- **Two dock visibility modes, configured in the app**: auto-hide (slides to a sliver when not hovered; space reclaimed) as default, and fixed (always visible, strip permanently reserved, like a pinned taskbar).
- **Live dock controls live in the window**: a dock/undock toggle and left↔right edge-switch arrows — no main-app round trip for the everyday gesture. Persistent behavior (mode, default edge) is set in the main app's Settings.
- **Quick Action is a separate machine-local concept**, not a kind of Launch entry: name + PowerShell command + optional working directory, run fire-and-forget hidden as the current user with no elevation and no status UI. Launch entries keep their app-launch + desktop-group semantics; the two lists are independent.
- **Per-monitor dock state**: edge and visibility mode persist per monitor; the floating window remembers its size and position. The AppBar is unregistered (`ABM_REMOVE`) on app quit so the edge is never left occupied.
- **The window reuses the existing design system** (tokens.css, shared components, both themes) — a new view, not a new visual language. Any genuinely new shared component (a minimal accessible tab strip) is added to the component foundation and reviewed per the AGENTS.md design rule.

## Consequences

- The tray is no longer the only resident surface; the dock can be the visible resident form.
- Maximized windows on the docked edge shrink by the strip width while the dock is fixed; auto-hide reclaims it otherwise.
- Two triggers (window Start button, page Start button) now converge on the same runner (`launch_entries`), keeping one pipeline for launch semantics.