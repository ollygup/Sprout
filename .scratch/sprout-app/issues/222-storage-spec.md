# 222 — Storage: keep SQLite, fix concurrent writes with WAL (spec)

**Status:** planning package only — no application behavior changed here. Ticket 234 is `ready-for-agent` (created next in this unit).

**Parents / related (read before implementing):**

- [ADR-0006](../../../docs/adr/0006-lazy-data-in-localappdata.md) — all app data is lazy-created under `%LOCALAPPDATA%\Sprout`.
- [ADR-0026](../../../docs/adr/0026-machine-local-boundary-backup-identities.md) — the database is machine-local; exports/backups are separate documents.
- [ADR-0003](../../../docs/adr/0003-elevated-worker-self-relaunch.md) — the elevated worker is a separate process with its own database connection.
- [ADR-0029](../../../docs/adr/0029-one-source-of-truth-per-windows-command.md) — one owner per Windows/native operation; the database initializer is the single connection owner.
- Related: [spec 221](221-collections-media-spec.md) adds group migrations; [spec 223](223-forkable-ruleset-spec.md) makes the storage path portable for standalone clones.
- Research: this round's embedded-database comparison (Further Notes).

## Problem Statement

From the user's side:

- Saves occasionally fail with a raw "database is locked" error when a Quick Action run is finishing at the same time the app writes a setting, group, clip, or backup state.
- The UI can stall or fail a read while the elevated worker records a run, because the main app and the worker each hold their own SQLite connection and the default journal mode blocks one another.
- The failure is timing-dependent and hard to reproduce, so it erodes trust: the user cannot tell whether the click was saved.
- The round also asked whether SurrealDB should replace SQLite. It should not: embedded SurrealDB would introduce a second datastore lock/process model for no benefit to this relational, single-user data.

## Solution

Keep SQLite. Make the existing single connection initializer set `journal_mode=WAL` and `busy_timeout=5000` on every connection — the main app and the elevated worker alike — so readers and the writer stop blocking each other and a brief overlap waits instead of failing. Add concurrency tests that prove a second connection waits and succeeds. Do not change the schema, the database location, the backup format, or the engine.

## User Stories

1. As a Sprout user, I want a save to wait briefly instead of failing when a run is writing, so my click is not lost.
2. As a Sprout user, I want the UI to keep reading while the worker writes, so the app stays responsive during a run.
3. As a Sprout user, I want a crash or power loss to leave the database consistent, so I do not lose the whole library.
4. As a Sprout user, I want the same database file and location as today, so my existing data opens without migration drama.
5. As a Sprout user, I want no new service, port, account, or background daemon, so the app stays a single desktop install.
6. As a Sprout user, I want a clear message if the database is genuinely unavailable, so I know whether to retry or check disk space.
7. As a Sprout user, I want the worker's run records to use the same rules as the app, so concurrent runs do not corrupt or drop history.
8. As a Sprout user, I want backup/export reads to see a consistent snapshot, so a backup taken during a run is coherent.
9. As a Sprout user, I want WAL sidecar files kept beside the database and out of exports, so my backup folder does not collect engine files.
10. As a fork maintainer, I want the storage layer to remain one portable SQLite file, so I can back up or move a machine without a server.
11. As a developer, I want one initializer that both processes call, so the pragmas cannot drift.
12. As a developer, I want a test that holds a write transaction on one connection and proves the other waits and succeeds, so this regression cannot return.
13. As a developer, I want a test that the worker path uses the same initializer, so a future worker change cannot silently reintroduce the default journal.
14. As a developer, I want the database error path to translate `SQLITE_BUSY`/`SQLITE_LOCKED` into a user-readable retry message, so raw engine text does not leak.
15. As a developer, I want no new dependency or engine, so the binary size and license surface stay unchanged.
16. As a maintainer, I want the SurrealDB rejection recorded with reasons, so the question is not re-litigated without new evidence.

## Current vs accepted vs proposed

**Current (verified in source):**

- `db::init_at` opens `sprout.db`, sets `foreign_keys=ON`, and runs migrations. It does not set `journal_mode` or `busy_timeout`; SQLite's default rollback journal applies.
- The main app holds one connection in `AppState`. The elevated worker opens its own connection through the same initializer when it records a run.
- Migrations are idempotent and run on every open. The database lives under `%LOCALAPPDATA%\Sprout`.
- Writes from the two connections can collide; with no busy timeout, SQLite returns `SQLITE_BUSY` immediately rather than waiting.
- The backup document is a logical export (Products, Presets, Launch entries, Quick Actions, Clips), not a raw file copy, so WAL sidecars never enter it.

**Accepted-but-unimplemented assumed by this round:**

- ADR-0026's machine-local boundary already covers the database; no change.
- ADR-0029's single-owner rule already points at one initializer; this round makes the pragmas part of that owner.

**Proposed here:**

- WAL + `busy_timeout=5000` in the one initializer; both processes inherit it.
- A readable busy/locked error translation.
- Concurrency and pragma tests.
- A recorded rejection of SurrealDB.

## Implementation Decisions

1. **Keep SQLite; reject SurrealDB.** The data is relational (products, presets, requirements, runs, groups, ordered lists), the app is single-user, and the existing backup/export is logical. Embedded SurrealDB holds an exclusive datastore lock per process; the elevated worker is a separate process, so it would contend with the app's datastore. Running SurrealDB in server mode would add a service, port, auth, and lifecycle surface for no user-visible gain. The engine also adds binary size/licensing surface to a size-budgeted app. SQLite already provides the needed transactions, foreign keys, and a single portable file.
2. **One initializer.** `db::init_at` remains the only place that opens a connection and sets pragmas. `db::init` and the worker call it; no second `Connection::open` path may set its own pragmas. Order: open → `journal_mode=WAL` → `busy_timeout=5000` → `foreign_keys=ON` → migrations.
3. **Pragmas.** `journal_mode=WAL` is persistent in the database file; setting it on every connection is idempotent and converts existing databases on next open. `busy_timeout=5000` is per connection, so it must be set on both. Leave `synchronous` at SQLite's default (FULL) this round; the problem is lock contention, not write throughput, and durability is not traded away without evidence.
4. **Failure behavior.** If the busy timeout expires, the existing command error path surfaces a readable message ("Sprout is busy saving — try again in a moment") instead of raw `database is locked`. The translation is a small helper beside the initializer, not a new error type per command. Disk-full/corrupt errors keep their existing handling.
5. **WAL files.** `sprout.db-wal` and `sprout.db-shm` live beside `sprout.db` under `%LOCALAPPDATA%\Sprout`. They are engine files, never exported, synced, or committed. Clean shutdown may leave them; SQLite checkpoints automatically. No manual checkpoint command is added.
6. **Migrations.** WAL conversion happens before migrations, so an existing database converts once and then migrates normally. If conversion fails (read-only folder, disk full), startup reports a clear error and does not continue with a half-open database.
7. **Backup/export.** No change: the logical export reads through the same connection and sees a consistent snapshot under WAL. A raw file-copy backup is not introduced.
8. **Tests.** Use a temp database and two connections:
   - Both report `journal_mode=wal` and `busy_timeout=5000`.
   - Connection A begins a write transaction; connection B's write waits and succeeds after A commits (the test must fail if the pragmas are removed).
   - Migrations still run, foreign keys still reject an invalid reference, and the worker path calls the same initializer.
   - The busy/locked translation returns the readable message.
9. **No new dependency, no new process, no schema change.** The ticket is a small, high-confidence change with a strong regression test.

## Seams

- **Connection owner:** `db::init_at` is the single seam. Extend it; do not add pragma calls in `lib.rs`, `worker.rs`, or command handlers.
- **Error translation:** keep it beside the initializer so every command that maps a database error gets the same message; do not duplicate string matching in each command.
- **Tests:** the existing `db.rs` test module is the prior art for temp-database and migration tests. No new test harness.
- **Worker:** the worker already calls `db::init`; verify it, do not re-implement it.

## Design rules every implementer applies

- **Codebase design:** one owner for connection setup; deletion test — removing the helper must make the two connections drift, which is exactly the bug.
- **Data safety:** no schema change, no file move, no destructive migration; WAL conversion must be reversible by opening the same file with the previous build (WAL is a standard SQLite mode).
- **Planning:** distinguish current/accepted/proposed; no ADR rewrite — this is an implementation decision inside ADR-0006/0026/0029, not a new boundary.
- **Toolchain:** run `cargo test`, `cargo check`, `npm run check`, and the ownership gate per `docs/agents/toolchain.md`; do not report a pending check as passed.

## Testing Decisions

Good tests assert behavior at the database seam, not implementation details. They must fail if WAL or the busy timeout is removed.

- **`db.rs` unit tests:** temp-file database; pragma assertions; two-connection contention; migration/foreign-key regression; busy-message translation.
- **`worker.rs` test or assertion:** the worker's run-record path opens through `db::init` (the same initializer), not a local `Connection::open`.
- **Manual verification:** run a Quick Action that writes a run record while repeatedly saving a setting/group/clip; no "database is locked" appears; the app remains responsive. Repeat with the app's main window and the dock open. Check `%LOCALAPPDATA%\Sprout` for `sprout.db-wal`/`-shm` and confirm exports do not include them.
- **Prior art:** `db.rs` migration tests, `backup.rs` round-trip tests, `worker.rs` run-record tests.

## Out of Scope

- Replacing SQLite with SurrealDB or any other engine.
- Encryption at rest, cloud sync, multi-user access, or a database server.
- Changing the database location or file name.
- Changing `synchronous`, page size, cache size, or other tuning beyond WAL + busy timeout.
- A database compaction/vacuum UI.
- Changing the backup document format or adding groups to it.
- Retry loops above the busy timeout; after five seconds the user gets the readable error.

## Further Notes

**SurrealDB evaluation.** The round asked whether SurrealDB fits. It does not, for this app's shape:

- **Process model.** Embedded SurrealDB owns an exclusive datastore lock. Sprout's elevated worker is a separate process that must write run records; it would contend with the app's datastore. Server mode would add a service/port/auth lifecycle the app does not otherwise need.
- **Data shape.** The data is relational and transactional (ordered lists, foreign keys, merge-by-identity backups). SQLite's transactions and constraints already express it; a document/graph engine does not remove a layer.
- **Size/license.** The app budgets a small installer; a second engine adds binary size and license review for no user-visible feature.
- **Migration risk.** Moving the library would require a full data migration and a new backup story, while the actual bug (concurrent writes) is fixable in the existing engine.

**Why WAL.** WAL lets readers proceed while a writer holds the write lock, and it lets a second writer wait on the busy handler instead of failing immediately. It is a standard SQLite mode, persists in the file, and needs no schema or API change. The 5 s timeout is long enough to cover a run-record write or a settings save, short enough that a genuinely stuck database still surfaces an error.

## Ticket map and integration ownership

| Ticket | Behavioral prerequisites | Likely paths / owner symbols | Shared contract and integration edits | Candidate wave |
| --- | --- | --- | --- | --- |
| [234 SQLite WAL + busy timeout](234-sqlite-wal-busy-timeout.md) | None — schedule after 225/229 if their migrations are in flight (shared `db.rs`); no behavioral blocker | `db.rs` (`init_at`, error helper, tests), `worker.rs` verification, toolchain checks | Connection pragmas; readable busy message; no schema/format change | 4 |

**Concurrency note.** 234 touches `db.rs`, which 225 (group color migration) and 229 (nested-group migration) also touch. The work is small and should land after those migrations or under one integration owner for `db.rs`. It does not depend on their behavior. Claims above are estimates to recheck at dispatch.

## Acceptance and verification

- [x] User confirmed keeping SQLite and fixing concurrent writes; SurrealDB is skipped as unsuitable.
- [ ] 234 verifies WAL and `busy_timeout=5000` on both the app and worker connections.
- [ ] 234's contention test fails without the pragmas and passes with them.
- [ ] No "database is locked" appears in the manual run-record + save matrix.
- [ ] No schema, location, backup-format, or dependency change ships.
- [ ] `cargo test`, `cargo check`, `npm run check`, and the ownership gate pass.
- [ ] No application behavior changed in this planning session.
