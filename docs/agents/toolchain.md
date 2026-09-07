# Toolchain and commands — agent reference

> Read this file when: you run, build, check, or test anything (dev server,
> svelte-check, vite build, cargo test/check). Otherwise skip it.

- Rust 1.97.1 stable, MSVC host (`x86_64-pc-windows-msvc`) via rustup — `%USERPROFILE%\.cargo\bin` is on the user PATH.
- Visual Studio Build Tools 2022 (MSVC 14.44) — required by Tauri/link.exe.
- Node.js v24.19.0, npm 11 — invoke as `npm.cmd` (PowerShell execution policy blocks `npm.ps1`).
- WebView2 runtime present.
- App data is created lazily on first launch under `%LOCALAPPDATA%\Sprout` (sprout.db + logs\) — never ship or commit it.

## Parallel batches

Follow `parallel-tickets.md` for coordinator scheduling and `worker.md` for
worker checks. Heavy checks use one batch-wide slot in `C:\Sprout` and its
existing dependency/build caches; isolated source copies do not each need an
installation, build, or running app. Workers own relevant tests and repairs;
the coordinator runs queued heavy checks and final combined validation.
Required checks remain required; a pending check must not be reported as passed.

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
