# 234 — SQLite WAL and busy timeout

**Parent:** [222 — Storage (spec)](222-storage-spec.md)

**What to build:** The one database initializer enables WAL and a 5 s busy timeout on every connection, so the main app and the elevated worker no longer fail with "database is locked" when their writes overlap. A brief overlap waits and succeeds; a genuinely stuck database returns a readable retry message. No schema, location, backup-format, dependency, or engine change.

**Blocked by:** None — can start immediately. Schedule after 225/229 if their migrations are in flight (shared `db.rs`); coordinate one integration owner for that file.

**Status:** ready-for-agent

- [ ] `db::init_at` sets `journal_mode=WAL`, `busy_timeout=5000`, and `foreign_keys=ON` before migrations; `db::init` and the worker both open through it; no second connection path sets its own pragmas.
- [ ] Existing databases convert to WAL on next open; a conversion failure (read-only folder, disk full) reports a clear startup error and does not continue half-open.
- [ ] A two-connection test holds a write transaction on one connection and proves the other waits and succeeds after commit; the test fails if WAL or the busy timeout is removed.
- [ ] Both connections report `journal_mode=wal` and `busy_timeout=5000`; migrations and foreign-key enforcement still pass.
- [ ] The worker's run-record path is verified to use the shared initializer.
- [ ] `SQLITE_BUSY`/`SQLITE_LOCKED` maps to a readable retry message; disk-full/corrupt errors keep their existing handling.
- [ ] WAL sidecar files stay beside the database and never enter an export, backup, or sync; no raw file-copy backup is introduced.
- [ ] Manual evidence: run a Quick Action that writes a run record while repeatedly saving a setting/group/clip; no "database is locked" appears and the app stays responsive.
- [ ] `cargo test`, `cargo check`, `npm run check`, and `node tools/ownership-gate.mjs` pass.

**Claims (recheck at dispatch):** `db.rs` (`init_at`, busy-error helper, tests); `worker.rs` verification. **Explicitly not built:** SurrealDB or any engine change; schema/location/backup-format changes; `synchronous` or other tuning; a retry loop above five seconds; a database compaction UI.
