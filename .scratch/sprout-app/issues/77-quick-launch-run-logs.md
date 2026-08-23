# 77 — Quick Launch run logs

**What to build:** Every Quick Launch list-run writes a per-run log folder
under the logs root — same shape and guarantees as Quick Action run logs
(ticket 64) — capturing started/skipped/failed entries with reasons plus the
desktop-assignment notes, listed on the Logs screen and pruned by retention.
Zero behavior change to launching itself.

**Blocked by:** 72 — the spec pinning reuse of the ticket-64 log-helper seam
and the "logging failure never fails a run" rule

**Status:** ready-for-agent

- [ ] Per-run folder family `ql-<millis>` created where the LaunchReport is assembled (background run thread), reusing the shared append/stamp helpers rather than duplicating them
- [ ] Log content mirrors the notification summary exactly: header line, one line per started entry, skipped entries with reasons, failed entries with reasons, desktop notes, final summary footer (`--- sprout ---` style verdict like install-run logs)
- [ ] Best-effort writes throughout: folder/log failure never fails the run or the notification
- [ ] Logs screen gains a "Quick Launch runs" section (newest first, sizes) counted in total bytes, mirroring the existing sections
- [ ] Retention pruning covers the new prefix via the generalized age logic (same `log_retention_days` knob, same call sites), with tests
- [ ] Tests mirror ticket 64's set: folder+header creation, append ordering, listing order, expired-folder prune
- [ ] The Quick Launch window gains nothing (no config surface); `cargo test` green; `npm run check` 0 errors; synced to the share
