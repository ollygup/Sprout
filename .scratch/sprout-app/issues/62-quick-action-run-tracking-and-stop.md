# 62 — Quick Action run tracking + Stop button

**What to build:** Quick Actions can be tracked while running: the Quick Launch window's Run button flips to Stop for the action's lifetime, and Stop either runs the action's own stop command (e.g. `docker compose stop`) or kills the process tree. The editor gains a "Show Stop button" checkbox that progressively reveals a Stop command field. Parent spec: 55.

**Blocked by:** 55 — Quick Launch window: dock, floating UX, live sync, and Quick Action control (spec); 57 — Quick Launch dock settings + live sync (reuses the event-sync pattern for run-state changes)

**Status:** todo

- [ ] `quick_actions` table gains `stoppable` (0/1, default 0) and `stop_command` (nullable text) via the existing idempotent migration pattern; CRUD validation (stoppable implies stop_command may be empty — empty means tree kill); round-trip tests across reopen
- [ ] In-memory process registry in `AppState` (e.g. `HashMap<action_id, Child>`): `run_quick_action` keeps the spawned `Child` instead of dropping it, so liveness is `try_wait`; registry is per-session only (PIDs die with the boot anyway)
- [ ] A reaper thread per tracked action waits on the `Child` and emits `quick-action-run-state-changed` on exit; starting an action emits the running state — the Quick Launch window's Quick Actions tab flips Run ↔ Stop with no polling
- [ ] Stop runs the action's `stop_command` when set (same hidden PowerShell spawn path); otherwise kills the process tree (`taskkill /T /F`, prior art: `launch.rs` / `engine/windows.rs`); the registry entry is removed when the process exits or is stopped
- [ ] Editor UI: "Show Stop button" checkbox (concise label) reveals the Stop command field with an InfoTip — "Runs when Stop is clicked. Empty = kills the process tree." — plus the tracking caveat: foreground commands only; detached commands (e.g. `docker compose up -d`) report as not running because the process exits while the service continues
- [ ] `cargo test` green (migration, CRUD validation, registry liveness transitions, stop-command resolution); `npm run check` 0 errors; synced to the share