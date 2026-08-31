# 117 — Note storage + API for Quick Actions

**What to build:** Quick Actions gain an optional Note — free-form formatted text whose purpose is whatever its writer wants. Stored raw in a new nullable text column, carried through create/update/list, preserved by the whole-app backup merge, and strictly machine-local: never part of Presets, Plans, Runs, or Preset exports.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [x] Idempotent migration adds the nullable notes column to Quick Actions storage
- [x] The edit payload extends with the note; create/update persist it trimmed-or-empty consistently; list returns it
- [x] Whole-app backup merge preserves notes alongside the action's other fields
- [x] Notes never appear in Preset authoring, Plan payloads, or Preset exports
- [x] Cargo tests cover CRUD round-trip, empty/null semantics, and backup passthrough (temp-database precedent)
