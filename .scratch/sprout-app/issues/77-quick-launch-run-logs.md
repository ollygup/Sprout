# 77 — Quick Launch run logs

**What to build:** Every Quick Launch list-run writes a per-run log folder
under the logs root — same shape and guarantees as Quick Action run logs
(ticket 64) — capturing started/skipped/failed entries with reasons plus the
desktop-assignment notes, listed on the Logs screen and pruned by retention.
Zero behavior change to launching itself.

**Blocked by:** 72 — the spec pinning reuse of the ticket-64 log-helper seam
and the "logging failure never fails a run" rule

**Status:** done

- [x] Per-run folder family `ql-<millis>` created where the LaunchReport is assembled (background run thread), reusing the shared append/stamp helpers rather than duplicating them
- [x] Log content mirrors the notification summary exactly: header line, one line per started entry, skipped entries with reasons, failed entries with reasons, desktop notes, final summary footer (`--- sprout ---` style verdict like install-run logs)
- [x] Best-effort writes throughout: folder/log failure never fails the run or the notification
- [x] Logs screen gains a "Quick Launch runs" section (newest first, sizes) counted in total bytes, mirroring the existing sections
- [x] Retention pruning covers the new prefix via the generalized age logic (same `log_retention_days` knob, same call sites), with tests
- [x] Tests mirror ticket 64's set: folder+header creation, append ordering, listing order, expired-folder prune
- [x] The Quick Launch window gains nothing (no config surface); `cargo test` green; `npm run check` 0 errors; synced to the share

**Verification notes (2026-08-23):** The ticket-64 seam stayed the one copy:
`quick_actions::create_run_log_folder(root, base)` is the extracted
exclusive-create core (suffix `-2`, `-3`, … on collision), and
`append_log_line`/`log_stamp` are used as-is — `launch.rs` adds only its own
constants (`QL_LOGS_DIR_NAME = "quick-launch"`, `QL_LOG_PREFIX = "ql-"`) and
three thin writers: `new_launch_run_log_path` (folder under
`logs\quick-launch\ql-<millis>`, bare epoch millis like `run-<millis>` so the
existing legacy age parse reads it — `embedded_age_secs` gained the `ql-`
prefix in its loop), `write_launch_run_header` (start stamp + entry count +
cap, written before the queue so a wedged run still leaves its start), and
`write_launch_run_summary` (indented started/skipped/failed/note lines in
report order, then blank line + `--- sprout ---` + the exact
`launch_summary_body` text the notification carries). Wiring sits in
lib.rs's background launch thread around `run_launch_queue`; every write is
swallowed-error best-effort and `run_launch_queue` itself is untouched.
`LogLocations.quick_launch_runs` feeds a "Quick Launch runs" Logs-screen
section (newest first, sizes, counted in the total); pruning scans the
`quick-launch` root at the same two call sites (worker + app start) under the
same `log_retention_days` knob. Gates: `cargo test` 323 passed / 0 failed /
1 ignored (Edge live probe; new tests: ql folder+age, header+story+verdict
ordering incl. append-not-truncate, clean-run minimal shape, ql listing+total,
expired ql prune), `npm.cmd run check` 0 errors 0 warnings.
