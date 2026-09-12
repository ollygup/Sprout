# 179 — Action files backend: table, staging, `<FilesDir>` expansion, export

**What to build:** The stored + executed half of attached files: table, per-run staging, placeholder expansion in the single Windows owner, zip/JSON export, backup.

**Blocked by:** None — contract owner for 180. Assumes 175 run/log chain and 147 shell semantics.

**Status:** done — ACs verified + post-close hardening (quoting, staged-dir lifetime, export picker) amended below.

**Parent:** [173](173-quick-actions-files-clips-logs-companion-spec.md). Extends [160](160-single-quick-action-export.md) (zip only when files exist).

## Scope

- Table `quick_action_files(action_id FK CASCADE, filename, bytes BLOB)`: filename basename-only (no paths/separators), trimmed, unique per action (case-insensitive on Windows); 5MB/file, 20MB/action v1 caps enforced backend-side; per-file remove deletes one row; Action delete cascades. Migration + ordered-independent (files order = name order or insertion order — settle at implementation, record it).
- Run time: stage copies to a per-run temp dir before spawn; expand `<FilesDir>` in `command` to the staged absolute path, shell-quoted by `windows_execution/` (ADR-0029 — PowerShell vs CMD quoting differs; no second expansion site); `cwd` stays independent (no magic cwd pollution); cleanup after exit (detached-command lifetime explicitly documented); no placeholder = files inert (no staging, no failure — the 180 hint covers it).
- Export/backup in the same `sprout-backup` envelope (ADR-0014 — no second format): whole-app backup gains the additive array; single-action export writes the same JSON as 160 when fileless, else a zip bundle (JSON + files/); import restores through the ordinary merge with honest inserted/skipped counts (identity = extended action identity + filename set; content bytes never part of identity); portable-form rules apply (no absolute paths leave the machine — staged paths never persist).
- Explicitly not built: any files UI/editor (180), image Clips (178), Pre-action UI (176).

## ACs

- [x] Filename/cap/uniqueness validation with plain errors; cascade + per-file remove verified; caps enforced before disk write.
- [x] `<FilesDir>` expands exactly once per run, correctly quoted per shell; multiple occurrences all expand; missing placeholder stages nothing and fails nothing.
- [x] Staging dir cleaned on foreground exit; detached behavior documented + not wedging Stop/tracking; `auto_run` stages identically.
- [x] Zip/JSON export + ordinary Restore counts verified; whole-app round-trip verified; Rust tests + `npm.cmd run check` clean; ownership gate passes.

## Validation (batch batch-174-175-177-178-179-20260911, 2026-09-11)

- Merged in staging with 175 (shared `quick_actions.rs`/`windows_execution` owner, shared `lib.rs` run chain): 179's files interface/staging/quoting/export kept verbatim; `append_action` returns the new row id (needed to hang files on merged actions); run chain = check-first (175) then stage+expand (179) then spawn — a blocked check stages nothing; reaper cleans the staged dir; `auto_run` flows through the same path. Stop/Test commands do not expand `<FilesDir>` (run command only); logs record the stored command + attached count, never staged paths.
- Coordinator repairs: fixed tail-position `?` borrow errors in the two list fns (bind `rows` first — same logic); removed two unused re-exports (`quote_files_dir`, `FILES_DIR_PLACEHOLDER`) for zero-warning check; fixed the legacy-migration test fixture to the realistic pre-179 shape (it predated even `cwd` — no migrate regression).
- Recorded decisions: files order = name order (`ORDER BY filename COLLATE NOCASE, id`); `BackupCounts` unchanged (files ride the `quick_actions` collection); zip JSON carries metadata only, raw bytes in `files/`; whole-app JSON carries base64 inline; merge identity = extended action identity + filename set (bytes never identity); staged-dir perms inherit OS temp default.
- `cargo test`: 585 passed (incl. 5 files + staging/quote/zip/merge tests). `cargo check`: 0/0. `npm.cmd run check`: 0/0. Ownership gate: pass. Manual matrix (PS+CMD quoting with spaces, multi-file, inertness, caps, cascade, export/import counts) pending a runtime pass.

## Implementation notes

- Codebase-design: seam at `quick_actions` files interface (attach/remove/list outcomes); `windows_execution` owns staging + quoting + lifetime (deletion test — removing files support must remove staging/quoting with it). No `shared/` pass-through; no winget involvement.
- Security: basenames only, no traversal; zip entries sanitized on import; staged dir permissions inherit user-only default.

## Verification

- Rust tests (validate/cascade/staging/quote/zip/merge) + `npm.cmd run check` + ownership gate; manual: PowerShell + CMD quoting matrix, multi-file, missing-placeholder inertness, caps, cascade, export/import counts.

## Amendment — 2026-09-12 (export-picker gap found after close)

- The ACs above covered the backend envelope (zip-vs-JSON by content, magic-sniff restore) but no test touched the frontend picker wiring — and the manual runtime matrix was still pending. A user report showed the gap: the individual-export picker always offered `<name>.json` with a json-only filter, so an action with files saved zip bytes under a `.json` name (restores fine, misleads), and the Settings restore picker hid `.zip` files entirely.
- Fix: the export picker follows the attached files via the existing list seam (`<name>.zip` + zip filter while any are attached, plain JSON while fileless) through a pure tested helper; the restore picker accepts `zip`; the success notice names the attached-file count. Follow-up runtime pass on real export → restore still owed by the reporter (GUI clicking is not agent-runnable).
