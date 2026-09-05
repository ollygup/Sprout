# All data lives under %LOCALAPPDATA%\Sprout, created lazily

> Status: amended 2026-09-05 — original decision text preserved; see the executable-source audit amendment for current behavior and implementation gaps.

The SQLite database (`sprout.db`) and log files are created on first run, never shipped by the installer; the installer only places the exe (and registers the `.sprout.json` file association). Data is per-user and separated from the program; nothing stale ships in the install; uninstalling leaves the user's data intact by default. Portable/USB data mode is out of scope for v1.

## v1 installer layout (addendum)

The NSIS installer installs the exe to `%LOCALAPPDATA%\Programs\Sprout` — the standard per-user program location — **not** into the data directory. Tauri's default `currentUser` template would install to `%LOCALAPPDATA%\Sprout`, i.e. inside the data dir (ticket 10 found the collision during verification), so the installer uses a vendored copy of the tauri-bundler 2.9.4 `installer.nsi` (`src-tauri/nsis/installer.nsi`) with two local changes: install dir → `$LOCALAPPDATA\Programs\${PRODUCTNAME}`, and the uninstaller's "delete app data" checkbox → `%LOCALAPPDATA%\Sprout` (the real data dir; Tauri's default points at the bundle id folder, which never existed). Uninstalling without the checkbox leaves `%LOCALAPPDATA%\Sprout` (and its DB/logs) in place. **The vendored template must be re-diffed against upstream on every Tauri upgrade.**

## Amendment — 2026-09-05 (codebase accuracy pass)

Layout, lazy creation, per-user separation, and portable-out-of-scope are all still accurate (`db.rs` `init_at`, no `--data-dir` flag anywhere). The re-diff rule is now due, not hypothetical: the lockfile carries Tauri 2.11.5 while the vendored template still baselines 2.9.4, with no re-diff on record. Next Tauri upgrade must re-diff `src-tauri/nsis/installer.nsi` and update the baseline version named here.

## Amendment — 2026-09-05 (executable-source audit)

Lazy application data and per-user program/data separation remain implemented (`src-tauri/src/db.rs`, `data_dir` and `init_at`; the current-user branch in `src-tauri/nsis/installer.nsi`). “Only places the exe” describes exclusion of shipped user data, not the installer’s complete effects: it also manages the uninstaller, shortcuts, registration, and missing WebView2 according to `src-tauri/tauri.conf.json`.

The optional data-removal checkbox removes both local and roaming Sprout directories and is ignored during updates. Ordinary uninstall without that selection preserves user data. The lockfile’s Tauri version is 2.11.5; whether the vendored template was re-diffed is a provenance question that current source cannot establish. The standing re-diff obligation remains, but this audit does not assert an independently verified history of missing reviews.
