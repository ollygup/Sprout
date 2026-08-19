# Sprout

A Windows desktop app for composing, running, and sharing software-installation presets (Rust + Tauri 2 + Svelte 5). See `docs/CONTEXT.md` for the domain glossary and `docs/specs/0001-sprout-app.md` for the full spec. The legacy PowerShell package it replaces passed the release parity gate and was removed at v1 (record: `docs/release/parity-checklist.md`).

## Development

Prerequisites (already installed on this machine — see `AGENTS.md`): Rust 1.97.1 (MSVC host, `%USERPROFILE%\.cargo\bin` on PATH), Visual Studio Build Tools 2022, Node.js v24, WebView2. Use `npm.cmd` (the PowerShell execution policy blocks `npm.ps1`).

**Important:** develop in the local working copy `C:\Sprout`, never directly on the shared folder (UNC paths break npm/cargo). The share is the master/source of truth; sync back with add/update-only robocopy — see `AGENTS.md` for the exact commands.

```sh
npm install
npm run tauri dev     # launches the app window
npm run check         # svelte-check
npm run build         # vite build (frontend only)
cargo test            # backend tests (in src-tauri/)
```

App data (SQLite database + logs) is created lazily on first launch under `%LOCALAPPDATA%\Sprout`.

## Release

```sh
npm run tauri build     # release exe + NSIS installer (cargo on PATH)
```

Artifacts land in `src-tauri/target/release/` and are copied to `dist\` (`Sprout.exe` ≈ 4.6 MB, `Sprout_0.1.0_x64-setup.exe` ≈ 1.9 MB). The installer installs per-user to `%LOCALAPPDATA%\Programs\Sprout`, registers the `.sprout.json` file association (double-click a preset to import it), and uninstalling without the "delete app data" checkbox keeps `%LOCALAPPDATA%\Sprout` intact. The installer uses a vendored NSIS template (`src-tauri/nsis/installer.nsi`, re-diff against upstream on Tauri upgrades — ADR-0006).

Release parity gate (ticket 10) passed — device-based same-state comparison, recorded in `docs/release/parity-checklist.md`; compare tooling remains at `tools/parity-compare.mjs`.

## Layout

- `src/` — Svelte 5 frontend: `lib/styles/tokens.css` (design tokens), `lib/components/` (accessible component foundation), `routes/+page.svelte` (Library view).
- `src-tauri/src/` — Rust backend: `domain.rs` (domain model), `db.rs` + `seed.rs` (lazy SQLite Library + seed data), `engine/` (platform strategy seam), `lib.rs` (Tauri commands/state).
