# 234 — SQLite WAL and busy timeout

**Parent:** [222 — Storage (spec)](222-storage-spec.md)

**What to build:** The one database initializer enables WAL and a 5 s busy timeout on every connection, so the main app and the elevated worker no longer fail with "database is locked" when their writes overlap. A brief overlap waits and succeeds; a genuinely stuck database returns a readable retry message. No schema, location, backup-format, dependency, or engine change.

**Blocked by:** None — can start immediately. Schedule after 225/229 if their migrations are in flight (shared `db.rs`); coordinate one integration owner for that file.

**Status:** implemented 2026-10-25 — automated validation green; AC8's manual stress pass is still owed a human.

- [x] `db::init_at` sets `journal_mode=WAL`, `busy_timeout=5000`, and `foreign_keys=ON` before migrations; `db::init` and the worker both open through it; no second connection path sets its own pragmas.
- [x] Existing databases convert to WAL on next open; a conversion failure (read-only folder, disk full) reports a clear startup error and does not continue half-open.
- [x] A two-connection test holds a write transaction on one connection and proves the other waits and succeeds after commit; the test fails if WAL or the busy timeout is removed.
- [x] Both connections report `journal_mode=wal` and `busy_timeout=5000`; migrations and foreign-key enforcement still pass.
- [x] The worker's run-record path is verified to use the shared initializer.
- [x] `SQLITE_BUSY`/`SQLITE_LOCKED` maps to a readable retry message; disk-full/corrupt errors keep their existing handling.
- [x] WAL sidecar files stay beside the database and never enter an export, backup, or sync; no raw file-copy backup is introduced.
- [ ] Manual evidence: run a Quick Action that writes a run record while repeatedly saving a setting/group/clip; no "database is locked" appears and the app stays responsive.
- [x] `cargo test`, `cargo check`, `npm run check`, and `node tools/ownership-gate.mjs` pass.

## Implementation and evidence (2026-10-25)

- `db::init_at` sets `busy_timeout` first (so a conversion blocked by the other
  writer waits instead of failing), then `PRAGMA journal_mode = WAL`, refusing to
  continue when SQLite reports any other mode, then keeps `foreign_keys=ON` —
  all before `migrate`. `db::init` and both worker call sites
  (`crate::db::init()` for the install-directory read and the run-record write)
  inherit it; the only remaining `Connection::open` calls are test fixtures.
- `db::is_contention` / `db::error_text` translate `SQLITE_BUSY` and
  `SQLITE_LOCKED` into "Sprout is busy saving something else. Try again in a
  moment." Every other failure keeps SQLite's own text (disk full, corrupt,
  read-only). The mapping is used at the command boundary (`command_error` in
  `lib.rs`, applied to the 70 command/error conversions; mutex-poison paths stay
  plain `to_string`) and in the worker's run-record failure message.
- New tests: `every_connection_opens_in_wal_with_the_busy_timeout`,
  `an_existing_database_converts_to_wal_and_keeps_its_rows`,
  `a_second_writer_waits_for_the_open_transaction` (fails with SQLITE_BUSY if the
  timeout is removed), `contention_reads_as_a_retry_but_real_failures_keep_their_message`.
- Backups export JSON/zip built from queries (`backup.rs`), never a database file
  copy; `sprout.db-wal`/`-shm` stay in `%LOCALAPPDATA%\Sprout` and
  `tools\sync.ps1` only syncs the repo — so sidecars reach neither.
- Gates: `cargo test --lib` 742 tests, 739 passed / 0 failed / 3 pre-existing
  ignored (the bin target holds no tests and was left unlinked because a dev
  instance from an earlier session still has `sprout-windows-desktop.exe` open);
  `cargo check` clean; `npm.cmd run check` 0 errors (3 pre-existing warnings);
  `node tools/ownership-gate.mjs` pass (77 owned references).
- Known limitation, unchanged by this ticket: the Logs page's database size
  reports `sprout.db` only, so it excludes the WAL sidecar's bytes.
- AC8 needs a live app pass; the automated two-connection test covers the same
  overlap deterministically.

**Claims (recheck at dispatch):** `db.rs` (`init_at`, busy-error helper, tests); `worker.rs` verification; `lib.rs` command-error seam only (the busy message must cross the Tauri boundary, so the helper is called where commands stringify their errors). **Explicitly not built:** SurrealDB or any engine change; schema/location/backup-format changes; `synchronous` or other tuning; a retry loop above five seconds; a database compaction UI.
