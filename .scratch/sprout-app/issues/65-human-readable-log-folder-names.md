# 65 — Human-readable log folder names

**What to build:** Rename per-run log folders from epoch-millis names to
readable local-time names with a slug, per `docs/research/0002-log-file-organization.md`:
Quick Action runs become `qa-<YYYYMMDD>-<HHMMSS>-<action-name>` and preset
runs become `run-<YYYYMMDD>-<HHMMSS>` (+ `-N` on same-second collision).
Legacy millis folders keep listing/pruning. Follow-up to 64; parent research
0002.

**Blocked by:** 64 — Quick Action run logs (the folders being renamed)

**Status:** done

- [x] Quick Action folders: `qa-<YYYYMMDD>-<HHMMSS>-<slug>` under `logs\quick-actions\`; slug = action name sanitized for Windows (`< > : " / \ | ? *` and whitespace → `-`, collapsed, edge-trimmed, 40-char cap); empty slug → timestamp-only name; same-second repeat gets `-2`, `-3`, … via exclusive directory creation
- [x] Preset-run ids/folders: `run-<YYYYMMDD>-<HHMMSS>` (`new_run_id`), unique against existing run dirs; History/DB treat ids opaquely so old rows are unaffected
- [x] Age parsing handles BOTH shapes everywhere it matters (`logs.rs` listing/pruning, `worker.rs` newest-run ordering): legacy millis and new local-date stamps (local → epoch via current UTC offset, mtime fallback unchanged)
- [x] `cargo test` green (sanitize rules, collision suffix, dual-format age parsing incl. known-epoch date math, run-id uniqueness); `npm run check` 0 errors; synced to the share

**Verification notes (2026-08-20):** `new_run_log_path` builds `qa-<date>-<time>-<slug>` from the local clock and claims the folder with an exclusive `create_dir` loop (`-2`, `-3`, … on AlreadyExists — race-free); `sanitize_log_slug` maps Windows-forbidden chars + whitespace to `-`, collapses repeats, trims edge `-`/`.`, caps ~40 chars at a char boundary, returns None for timestamp-only names. `new_run_id_in(runs_dir)` mirrors it for preset runs (hermetic test injects a temp dir; production wrapper passes `logs\runs`). Age parsing centralized in `logs::embedded_age_secs`: 8-digit date + 6-digit time segments → epoch via Hinnant `days_from_civil` minus the live local/UTC offset (`GetLocalTime` − `GetSystemTime`; DST drift ≤1h is noise against day-scale retention), else legacy bare-millis; `folder_age` keeps the mtime fallback and `worker::run_millis` now delegates so newest-run ordering understands both shapes. Regression caught during verification: the prefix loop originally used `?` on `strip_prefix`, short-circuiting to mtime for every `qa-` name — fixed with `let … else { continue }` and covered by the dual-format tests. Gates: `cargo test` 282 passed / 0 failed (7 new: folder-name shape, slug rules, collision suffix, readable-id uniqueness/ordering, readable-name prune+listing), `npm run check` 0 errors 0 warnings.
