# SPROUT — the code-vibed app

> **This chunk of the README is written by me, the project owner. Everything
> below it is done by the agent.**

## How this project is set up

Sprout runs inside a Virtual Machine, but its source lives in two homes:

- **Shared folder** (on my host machine) — `\\vmware-host\Shared Folders\Projects\Sprout`
  — the master/source of truth and the only home of git.
- **Working copy** (inside the VM) — `C:\Sprout` — where all development
  happens. It deliberately contains **no git**.

**Git and releases are handled by me.** Commits are made manually on the
host, and a version release happens by tagging that commit (`vX.Y.Z`,
matching the `version` in `src-tauri/Cargo.toml`) — pushing the tag triggers
CI, which builds the installer and publishes the GitHub Release; installed
apps then update themselves from it.

Whenever a task is finished in the VM, the changed files are synced up to
the shared folder with a guarded sync script (`tools/sync.ps1`), which
refuses to overwrite anything I changed on the host side meanwhile.

---

A Windows desktop app for composing, running, and sharing software-installation presets (Rust + Tauri 2 + Svelte 5). See `docs/CONTEXT.md` for the domain glossary and `docs/specs/0001-sprout-app.md` for the full spec. The legacy PowerShell package it replaces passed the release parity gate and was removed at v1 (record: `docs/release/parity-checklist.md`).

## Development

Prerequisites (documented in `AGENTS.md`): Rust (MSVC host), Visual Studio Build Tools 2022, Node.js v24, WebView2. On the VM use `npm.cmd` (the PowerShell execution policy blocks `npm.ps1`). Develop in the working copy `C:\Sprout`, never directly on the shared folder (UNC paths break npm/cargo); sync back with `tools/sync.ps1`.

```sh
npm install
npm run tauri dev     # launches the app window
npm run check         # svelte-check
npm run build         # vite build (frontend only)
cargo test            # backend tests (in src-tauri/)
```

App data (SQLite database + logs) is created lazily on first launch under `%LOCALAPPDATA%\Sprout` — fresh installs open an empty Library (ADR-0008).

## Release

Releases are GitHub Releases built by CI — never hand-built installers. The full flow lives in `docs/release/release-process.md`: bump `version` in `src-tauri/Cargo.toml` (the single source of truth), commit, tag the commit `vX.Y.Z`, and push with tags. CI refuses to publish unless the tag equals the Cargo.toml version, then builds and publishes `Sprout_<version>_x64-setup.exe`; installed apps check Releases at startup and apply updates passively (`docs/adr/0012-github-release-self-update.md`).

The installer installs per-user to `%LOCALAPPDATA%\Programs\Sprout`, registers the `.sprout.json` file association (double-click a preset to import it), and uninstalling without the "delete app data" checkbox keeps `%LOCALAPPDATA%\Sprout` intact. It uses a vendored NSIS template (`src-tauri/nsis/installer.nsi`, re-diff against upstream on Tauri upgrades — ADR-0006).

Release parity gate (ticket 10) passed — device-based same-state comparison, recorded in `docs/release/parity-checklist.md`; compare tooling remains at `tools/parity-compare.mjs`.

## Layout

- `src/` — Svelte 5 frontend: `lib/styles/tokens.css` (design tokens), `lib/components/` (accessible component foundation), routes for every page (Library, Presets, Plan, History, Logs, Settings, Quick Launch, Clips).
- `src-tauri/src/` — Rust backend: `domain.rs` (domain model), `db.rs` (lazy SQLite Library — empty on first run, ADR-0008), `engine/` (platform strategy seam), `lib.rs` (Tauri commands/state).
