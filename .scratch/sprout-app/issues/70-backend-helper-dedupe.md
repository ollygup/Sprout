# 70 — Backend helper dedupe (mechanical moves)

**What to build:** Six small backend logic blocks that exist in duplicate each
get exactly one home. All are mechanical moves with no design decisions — the
counterpart to ticket 69's seam work:

1. **PowerShell argv convention** (`-NoProfile -NonInteractive -Command` +
   quoting): four copies (launch pipeline, quick-actions runner, two inline in
   the Windows engine adapter) become one builder.
2. **Timeboxed test runners**: the Test-button plumbing duplicated between
   launch and quick-actions (shared timeout constant declared twice, parallel
   runner functions) becomes one timed-run helper; the cwd-aware variant is a
   parameter, and the Tauri command wrappers keep their per-command guards.
3. **Uninstall hives**: the identical 14-line registry-path array in walker
   and engine gets one home; share the subkey-walk skeleton, keep each
   consumer's extraction local (full entry vs display name vs install
   location).
4. **winget column locator**: the whole-word header-position scanner exists
   byte-identically twice; the winget parser module owns it, the engine
   adapter imports it.
5. **GUID-shape validation**: the 8-4-4-4-12 hex predicate exists twice; one
   copy, with the windows-side parser building its GUID on top of it.
6. **AppBar monitor info**: three MonitorFromWindow→GetMonitorInfoW boilerplate
   blocks in one file collapse into internal helpers returning different
   fields of the same probe.

**Blocked by:** None — can start immediately (independently of ticket 69; if
both run in one session, land 69 first — they touch overlapping modules).

**Status:** done

- [x] Each listed block exists exactly once; consumers import it
  - `powershell_argv` → `engine/windows.rs` (launch `command_argv`, quick-actions runner/Test, bootstrap, `powershell_output` all delegate)
  - one timed run: `launch::timed_test_result` (+ single `TEST_TIMEOUT` in launch.rs); lib.rs command wrappers keep their guards
  - `UNINSTALL_HIVES` + the subkey walk (`uninstall_subkeys`) → `walker.rs`; engine's display-name and install-location scans consume it with local extraction
  - `find_word` owned by `winget.rs`; `parse_winget_list` imports it
  - `looks_like_guid` stays in launch.rs; `parse_guid_id` builds its GUID on it
  - `appbar::monitor_info` probe behind `work_area`/`monitor_rect`/`monitor_key`
- [x] Argv output, timing behavior, hive enumeration order/results unchanged
- [x] `cargo check` clean, `cargo test` green (292 passed)
