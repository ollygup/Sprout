# 57 — Quick Launch dock settings + live sync

**What to build:** Dock/undock state becomes a persisted Settings parameter alongside the existing dock mode and edge; all three are chevron dropdowns in the main app, apply live to an open window, and the in-window dock controls write back. The Quick Launch window live-syncs with the main app via one change event — launch entries, quick actions, dock settings, and theme. Parent spec: 55.

**Blocked by:** 55 — Quick Launch window: dock, floating UX, live sync, and Quick Action control (spec)

**Status:** done — `dock.state` setting (default "floating", validation, fallback), three Settings dropdowns, `open()` docks on open, in-window controls write back, `update_settings` applies live (state/edge/mode), `quick-launch-changed` emitted from all 10 mutation commands, window listens once and reloads entries/actions/dock/theme; 254 backend tests green, svelte-check 0 errors, synced to the share 2026-08-20

- [x] New persisted setting `dock.state` ("floating" | "docked") with the existing settings pattern (default "floating", validation, migration fallback)
- [x] Settings page: three chevron dropdowns — "Quick Launch window" state (Floating/Docked), dock mode (auto-hide/fixed), dock edge (left/right) — reusing the existing Select component
- [x] `open()` honors state = docked: the window docks on open, using per-monitor edge/mode memory, falling back to settings defaults
- [x] In-window dock/undock toggle and edge-switch arrows persist their outcome (`dock.state`, `dock.edge`) so the settings and the window never diverge
- [x] `update_settings` applies dock-related changes to a live window: state change → dock/undock, edge change → reposition, mode change → re-apply auto-hide
- [x] Backend emits `quick-launch-changed` after every command that mutates what the window renders: the four Launch entry mutations, the four Quick Action mutations, `update_settings`, `update_theme`
- [x] The Quick Launch window listens once and re-runs its entry/action load, its dock-state refresh, and theme re-apply on the event — entries and actions added in the main app appear without reopening the window
- [x] `cargo test` green (settings round trip + validation for `dock.state`); `npm run check` 0 errors; synced to the share