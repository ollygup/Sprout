# AGENTS.md — working rules for Sprout

Read this first. These rules exist so every session (including fresh ones) builds without surprises and never damages the source of truth.

## Default query
- Always look for codegraph folder or codegraph in the root of the repository, if found, default to use codegraph for queries, if not fallback to grep method.

## Working copy rule (important)

- The repo has two homes:
  - **Master (source of truth, fallback):** `\\vmware-host\Shared Folders\Projects\Sprout`
  - **Working copy (develop here):** `C:\Sprout`
- **Never work directly on the share.** UNC paths break `.cmd`/`.bat` (npm, cargo helpers) — builds fail with "UNC paths are not supported". Always work in `C:\Sprout`.
- **Never delete or restructure anything on the share.** The share is the fallback if the working copy messes up.
- After finishing changes in `C:\Sprout`, sync back with add/update-only robocopy (never `/MIR`, which deletes):

```powershell
robocopy "C:\Sprout" "\\vmware-host\Shared Folders\Projects\Sprout" /E /R:1 /W:1 /NFL /NDL /NJH /NP /XD node_modules target build .svelte-kit .vscode
```

- Verify the sync with the same command plus `/L` (list-only) — expect `Copied: 0` when in sync.

## Toolchain (already installed on this machine)

- Rust 1.97.1 stable, MSVC host (`x86_64-pc-windows-msvc`) via rustup — `%USERPROFILE%\.cargo\bin` is on the user PATH.
- Visual Studio Build Tools 2022 (MSVC 14.44) — required by Tauri/link.exe.
- Node.js v24.19.0, npm 11 — invoke as `npm.cmd` (PowerShell execution policy blocks `npm.ps1`).
- WebView2 runtime present.
- App data is created lazily on first launch under `%LOCALAPPDATA%\Sprout` (sprout.db + logs\) — never ship or commit it.

## Commands

```powershell
# from C:\Sprout
npm.cmd run tauri dev    # launch the app window (cargo must be on PATH — it is)
npm.cmd run check        # svelte-check (0 errors expected)
npm.cmd run build        # vite build → build/
# from C:\Sprout\src-tauri
cargo test               # backend tests (CRUD, presets, runs, validation)
cargo check              # fast compile check
```

## Release build (exe + setup.exe)

Run from `C:\Sprout`. Outputs land in `src-tauri\target\release\`, then get
copied to the distribution folder `dist\` (both locations).

1. Pre-flight: `npm.cmd run check` (svelte-check, 0 errors) and `cargo test`
   in `src-tauri\`.
2. `npm.cmd run tauri build` — vite build → cargo release → NSIS installer
   (vendored `src-tauri/nsis/installer.nsi`). Allow a long timeout; the first
   release compile of the day takes several minutes.
3. Verify artifacts:
   - `src-tauri\target\release\sprout.exe`
   - `src-tauri\target\release\bundle\nsis\Sprout_<version>_x64-setup.exe`
4. Copy (overwrite) both into `C:\Sprout\dist\` — the distribution folder.
5. Cleanup: run the "Cleanup (device storage)" step below — AFTER step 4, so
   the fresh exe is safe in `dist\`.
6. Sync to the share with the robocopy command above — note its `/XD` list
   excludes `node_modules target build .svelte-kit .vscode`, NOT `dist`, so
   the new installers reach the share. Verify with `/L` (expect `Copied: 0`).

## Cleanup (device storage)

Run whenever told to do a cleanup, and automatically as the last step of
every release build. Device-only — the share never holds these dirs; nothing
here is deleted from the repo or the share.

- `Remove-Item -Recurse -Force C:\Sprout\src-tauri\target` — the big one
  (often several GB: debug builds + incremental caches). Recreated by the
  next `tauri dev` / `cargo test` / `tauri build`.
- Leave `node_modules` and `%LOCALAPPDATA%\Sprout` alone (needed constantly
  / user data). `.svelte-kit`, `build\`, `src-tauri\gen` are tiny — optional.

## Verification helpers

- Inspect the Library DB read-only (Node has built-in sqlite):
  `node -e "const {DatabaseSync}=require('node:sqlite');const d=new DatabaseSync(process.env.LOCALAPPDATA+'\\Sprout\\sprout.db',{readOnly:true});console.log(d.prepare('SELECT COUNT(*) c FROM products').get().c)"`
- Fresh installs open to an empty Library (ADR-0008): `c` is 0 until the
  user adds Products from the live winget registry search — nothing is seeded.

## Structure

- `src/` — Svelte 5 frontend (`lib/styles/tokens.css` = design tokens; `lib/components/` = accessible component foundation; `routes/+page.svelte` = Library view).
- `src-tauri/src/` — Rust backend (`domain.rs` = domain model, `db.rs` = lazy SQLite (empty on first run — ADR-0008), `engine/` = PlatformEngine strategy seam, `lib.rs` = Tauri commands).
- `docs/` — CONTEXT.md (glossary), specs/, adr/, research/, release/ (parity gate record + archived legacy log). `tools/` — parity compare + parity preset. `.scratch/sprout-app/issues/` — ticket tracker (mark ACs done as you go).
