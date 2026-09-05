# Quick Actions run hidden, unelevated, stoppable, and tracked

A Quick Action is a named PowerShell command (optional working directory) fired from the Quick Launch window. It runs hidden as the current user with no elevation and no modal status UI — fire-and-forget by default. Optionally it is **stoppable**: while its process runs, Run becomes Stop, which runs the action's own stop command when one is configured, else kills the process tree (`taskkill /T /F`). Live runs live in a per-session registry (`running_actions`, action id → child pid) fed by a reaper: a stop-command watchdog (`STOP_WATCHDOG`, 10 s) waits on an exit signal instead of polling, so an early exit stands the watchdog down and a hung stop is force-killed at the box. Tracking covers foreground commands only — detached commands (e.g. `-d`) report as not running because the process exits while the service continues. An action may be flagged **auto_run**: it runs once per Sprout start, in list order, exactly as if Run were clicked. A **note** (free-form, `note` with `notes` accepted as an alias) rides on the action as no-behavior metadata — trimmed on save, empty becomes `None`, never affecting execution.

## Considered options

- **Elevated or status-UI runs.** Rejected: actions are the user's own conveniences (restart a stack, start dev services), not installs — elevation would prompt on every click and a status UI would defeat the one-click palette.
- **Polling the process table for liveness.** Rejected: the reaper + exit-signal design gives immediate wakeups without a polling loop, and the per-session registry dies with the boot, so no stale "running" survives a restart.
- **Tracking detached services as running.** Rejected: a detached child exits immediately by design; pretending it is still running would wedge Stop forever. Honesty (not-running with the service alive) beats a comforting lie.

## Consequences

- Every tracked run writes `logs/quick-actions/<run>/output.log` with a header and exit line, so Stop and history have something to point at.
- The Quick Launch window, the Quick Actions page, and the run registry share one three-state machine (Run → Stop/Stopping) over the `quick-action-run-state-changed` event — no per-surface liveness logic.
- Notes and auto_run are machine-local like the rest of the record: carried by whole-app backup, never by Presets or preset exports.
