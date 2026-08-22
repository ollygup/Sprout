# 64 — Quick Action run logs (per-run folders, output captured)

**What to build:** Every Quick Action run gets its own log folder under
`logs\quick-actions\` — `qa-<millis>-<action-id>\output.log` — capturing the
command's live stdout/stderr plus Sprout's own tracking lines (start, stop
requested + which path, exit code). The Logs screen lists the folders like it
lists Run folders; the existing retention knob prunes them. Follow-up to 62
(tracking exists; this makes it visible and useful on failure).

**Blocked by:** 62 — Quick Action run tracking + Stop button (the registry,
reaper, and stop paths this instruments)

**Status:** done

- [x] Per-run folder `logs\quick-actions\qa-<millis>-<action-id>\output.log` per run (every run — tracking is unconditional); name embeds creation millis for chronological sort + pruning age, same trick as `run-<millis>`
- [x] Output capture: the spawned command inherits the open log file as stdout/stderr (`Stdio::from(File)` — no reader threads); header lines (start with pid, command, cwd) written before output flows; `exited code=N` appended by the reaper; spawn failures get a folder with a `start failed` line
- [x] Stop logging: `stop requested — stop command: <cmd>` or `stop requested — tree kill (taskkill /T /F)` appended by `stop_quick_action`; the stop command's own output appends to the same file
- [x] All log writes best-effort — a logging failure never fails the run or the stop
- [x] Retention: `prune_run_logs_at` also removes expired `qa-` folders (same `log_retention_days` knob, same call sites); age from the embedded millis, mtime fallback
- [x] Logs screen: `LogLocations.quick_action_runs` (newest first, sizes) counted in `total_logs_bytes`; page gains a "Quick Action runs" section mirroring "Run folders"
- [x] `cargo test` green (stamp format, folder+header creation, prune of expired qa- folders, LogLocations listing/order); `npm run check` 0 errors; synced to the share

**Verification notes (2026-08-20):** `spawn_quick_action` now takes an optional open log `File` and inherits it as the child's stdout/stderr via `Stdio::from(try_clone)` — no reader threads; the registry's `RunningQuickAction` carries `log_path` so Stop can append its line (`stop command: X` vs `tree kill (taskkill /T /F)`) and point the stop command's own stdio at the same file. Header = start line (name/id/pid) + indented command/cwd; reaper appends `exited code=N` from the waited status; spawn failures land a `start failed` line before the error returns. Timestamps are local wall clock via Win32 `GetLocalTime` (windows-sys `Win32_System_SystemInformation` feature; `SYSTEMTIME` lives in `Foundation` in 0.61) — no chrono, per the size budget. `logs.rs` generalizes folder age to both `run-<millis>` and `qa-<millis>-<id>` prefixes (split on `-`, mtime fallback), lists/prunes both roots, and `LogLocations.quick_action_runs` feeds a new "Quick Action runs" section on the Logs screen (bytes counted in the total). Gates: `cargo test` 277 passed / 0 failed (6 new: stamp padding, folder+header+append-only, stop-path wording, exit-code-unavailable wording, captured-output round trip, qa listing+pruning), `npm run check` 0 errors 0 warnings.
