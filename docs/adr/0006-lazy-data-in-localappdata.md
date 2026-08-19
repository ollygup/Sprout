# All data lives under %LOCALAPPDATA%\Sprout, created lazily

The SQLite database (`sprout.db`) and log files are created on first run, never shipped by the installer; the installer only places the exe (and registers the `.sprout.json` file association). Data is per-user and separated from the program; nothing stale ships in the install; uninstalling leaves the user's data intact by default. Portable/USB data mode is out of scope for v1.

## v1 installer layout (addendum)

The NSIS installer installs the exe to `%LOCALAPPDATA%\Programs\Sprout` — the standard per-user program location — **not** into the data directory. Tauri's default `currentUser` template would install to `%LOCALAPPDATA%\Sprout`, i.e. inside the data dir (ticket 10 found the collision during verification), so the installer uses a vendored copy of the tauri-bundler 2.9.4 `installer.nsi` (`src-tauri/nsis/installer.nsi`) with two local changes: install dir → `$LOCALAPPDATA\Programs\${PRODUCTNAME}`, and the uninstaller's "delete app data" checkbox → `%LOCALAPPDATA%\Sprout` (the real data dir; Tauri's default points at the bundle id folder, which never existed). Uninstalling without the checkbox leaves `%LOCALAPPDATA%\Sprout` (and its DB/logs) in place. **The vendored template must be re-diffed against upstream on every Tauri upgrade.**

