# 69 — Backend DB-layer dedupe

**What to build:** SQLite persistence logic that exists twice becomes one
implementation with two thin consumers:

1. **Ordered-list CRUD seam** (the ticket's one real design decision) — Launch
   entries and Quick Actions implement the same four persistence shapes
   (create-at-end with MAX(position)+1, update preserving position, delete
   with position compaction, move via read-all → remove/reinsert → renumber)
   differing only in table/column names. Extract one parameterized
   implementation behind a small interface; both entities become adapters.
   SQL shapes stay fixed strings with bound parameters; identifiers come from
   trusted internal constants only; per-command validation stays at the
   command edge.
2. **Meta upsert** — settings' upsert duplicates the key-value ON CONFLICT
   upsert verbatim; db owns it once.
3. **Epoch-now** — three copies of the unix-seconds-now expression (db, run,
   logs pruning) become one helper.
4. **Settings load()** — six near-identical query→validate→default blocks
   collapse internally (same-file cleanup).

Positions must stay gapless through create/delete/move in both lists — the
existing CRUD tests are the guard and must pass unmodified.

**Blocked by:** None — can start immediately.

**Status:** done

- [x] No duplicate upsert / epoch-now / ordered-CRUD logic remains across
      modules
- [x] Existing CRUD/presets/runs test suites pass unmodified
- [x] Gapless positions hold after create/delete/move in both lists
- [x] `cargo check` clean, `cargo test` green
