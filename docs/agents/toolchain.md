# Toolchain and commands — agent reference

> Read this file when: you run, build, check, or test anything (dev server,
> svelte-check, vite build, cargo test/check). Otherwise skip it.

- Rust 1.97.1 stable, MSVC host (`x86_64-pc-windows-msvc`) via rustup — `%USERPROFILE%\.cargo\bin` is on the user PATH.
- Visual Studio Build Tools 2022 (MSVC 14.44) — required by Tauri/link.exe.
- Node.js v24.19.0, npm 11 — invoke as `npm.cmd` (PowerShell execution policy blocks `npm.ps1`).
- WebView2 runtime present.
- App data is created lazily on first launch under `%LOCALAPPDATA%\Sprout` (sprout.db + logs\) — never ship or commit it.
- `C:\Sprout` in the commands below is the VM workspace path; a standalone clone substitutes its own root. The commands themselves are unchanged, and `node tools/ownership-gate.mjs` is path-agnostic (root from `process.argv[2]`, else the script's own location).

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
cargo check              # compile check — run after test, not before
```

## Cargo gate ordering (measured)

- `cargo test` already compiles the crate; `cargo check` on its own produces
  metadata only, so it does **not** warm the artifacts `cargo test` needs.
  Running check first and test second pays the full dependency codegen twice.
  When a ticket requires both, run `cargo test` **first**, then `cargo check`
  (it reuses the test artifacts and is quick).
- Never run the two concurrently: they serialize on the same package-cache and
  target locks (`Blocking waiting for file lock on package cache`), so the
  second one just idles. Same for any other cargo invocation.
- A background rust-analyzer `cargo run --no-default-features` holds those
  locks too. If a gate sits at ~0 CPU for minutes with no `rustc` children,
  that process is the blocker — pause/close it, do not kill the gate.
- First full `debug`-profile build of `src-tauri` takes ~10–20 min on this
  machine (≈600 crates: tauri, wry, windows-sys, ring, image; Defender scanning
  the target dir dominates). Incremental reruns are seconds. Budget for it
  instead of assuming a hang; check progress with
  `Get-CimInstance Win32_Process -Filter "Name='rustc.exe'"` (new PIDs = work
  happening).
- Frontend gates are cheap by comparison: targeted `npm.cmd test -- --run <files>`
  ≈20 s, full `npm.cmd run check` ≈1–2 min. Run them while cargo works; they do
  not share the cargo lock.
