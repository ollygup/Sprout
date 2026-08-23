# 80 — Whole-app backup: Settings Export/Import

**What to build:** One JSON backup containing all content data — products,
presets (with their requirements), launch entries, quick actions, clips —
exported from Settings and restored by a merging import that skips existing
ids and reports counts. Machine-scoped state stays local by design.

**Blocked by:** 78 — clips are part of the document, so the collection must
exist first

**Status:** done (2026-08-23)

- [x] Backup document shape: versioned + kind-tagged (`sprout-backup`), `exported_at`, one array per content collection; serde types reusing existing domain records so preset requirement snapshots survive intact
- [x] Export command: writes the file to a user-picked path via the save dialog; returns per-collection counts for the success notice
- [x] Import command: parse → validate kind/version (honest rejection copy for wrong files, mirroring preset-import behavior) → transactional merge skipping ids that already exist → summary {inserted, skipped} per collection
- [x] Exclusions enforced and stated in UI microcopy: runs history, logs, settings knobs, dock memory never travel
- [x] Settings gains a "Backup" section: Export… and Restore… buttons consistent with existing rows; restore confirms with parsed counts before writing; success/error notices follow app copy style
- [x] Round-trip tests against tempdir DBs: export → wipe → import → equality per collection; import of a foreign/legacy file rejected cleanly; partial duplicates merge without duplication
- [x] `cargo test` green; `npm run check` 0 errors; synced to the share
