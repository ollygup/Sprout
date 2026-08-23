# 80 — Whole-app backup: Settings Export/Import

**What to build:** One JSON backup containing all content data — products,
presets (with their requirements), launch entries, quick actions, clips —
exported from Settings and restored by a merging import that skips existing
ids and reports counts. Machine-scoped state stays local by design.

**Blocked by:** 78 — clips are part of the document, so the collection must
exist first

**Status:** ready-for-agent

- [ ] Backup document shape: versioned + kind-tagged (`sprout-backup`), `exported_at`, one array per content collection; serde types reusing existing domain records so preset requirement snapshots survive intact
- [ ] Export command: writes the file to a user-picked path via the save dialog; returns per-collection counts for the success notice
- [ ] Import command: parse → validate kind/version (honest rejection copy for wrong files, mirroring preset-import behavior) → transactional merge skipping ids that already exist → summary {inserted, skipped} per collection
- [ ] Exclusions enforced and stated in UI microcopy: runs history, logs, settings knobs, dock memory never travel
- [ ] Settings gains a "Backup" section: Export… and Restore… buttons consistent with existing rows; restore confirms with parsed counts before writing; success/error notices follow app copy style
- [ ] Round-trip tests against tempdir DBs: export → wipe → import → equality per collection; import of a foreign/legacy file rejected cleanly; partial duplicates merge without duplication
- [ ] `cargo test` green; `npm run check` 0 errors; synced to the share
