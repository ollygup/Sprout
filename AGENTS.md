# AGENTS.md — working rules for Sprout

Read this first. These rules exist so every session (including fresh ones) builds without surprises and never damages the source of truth.

## CodeGraph

In repositories indexed by CodeGraph (a `.codegraph/` directory exists at the repo root), reach for it BEFORE grep/find or reading files when you need to understand or locate code:

- **MCP tool** (when available): `codegraph_explore` answers most code questions in one call — the relevant symbols' verbatim source plus the call paths between them, including dynamic-dispatch hops grep can't follow. Name a file or symbol in the query to read its current line-numbered source. If it's listed but deferred, load it by name via tool search.
- **Shell** (always works): `codegraph explore "<symbol names or question>"` prints the same output.

If there is no `.codegraph/` directory, skip CodeGraph entirely — indexing is the user's decision.

## Design rules

- Every UI change must follow the `web-design-guidelines` and `frontend-design` skills and reuse the existing design system: tokens from `src/lib/styles/tokens.css` and components from `src/lib/components/`. No ad-hoc colors, type sizes, radii, or one-off component patterns; if a shared pattern genuinely doesn't fit, capture the deviation in the ticket and get it reviewed before shipping.
- Before any UI/UX design decision, read the standing research notes under `docs/research/`: `0004-progressive-disclosure-and-clips.md`, `0005-page-chrome-consistency.md`, and `0006-notion-design-patterns.md` (Notion's factual method: visibility-on-surface vs configuration-elsewhere, minimal-until-content defaults, explicit-setup gating). Cite the rule you applied; new evidence goes in a new numbered research note.
- Reusable UI geometry constants live in `src-tauri/src/constants/window.rs` — never re-declared in another module. Scan that file first before any UI-dimension change.

## Working copy rule (important)

- The repo has two homes:
  - **Master (source of truth, fallback):** `\\vmware-host\Shared Folders\Projects\Sprout`
  - **Working copy (develop here):** `C:\Sprout`
- **Never work directly on the share.** UNC paths break `.cmd`/`.bat` (npm, cargo helpers) — builds fail with "UNC paths are not supported". Always work in `C:\Sprout`.
- **Never delete or restructure anything on the share.** The share is the fallback if the working copy messes up.
- **Sync with `tools\sync.ps1`, never raw robocopy.** The share's git working tree is owned by the other device (the only git client); a blind robocopy overwrites whatever it committed and produces merge conflicts. The script snapshots the share's content hashes at session start and refuses to overwrite any file the other device changed mid-session — divergences are reported as `SHARE-NEWER` for explicit resolution:

```powershell
# session start (refreshes C:\Sprout from the share, then snapshots it)
tools\sync.ps1 -Down
# session end (copies only what we changed, guarded by the snapshot)
tools\sync.ps1 -Up
```

```powershell
# session start (refreshes C:\Sprout from the share, then snapshots it)
tools\sync.ps1 -Down
# session end (copies only what we changed, guarded by the snapshot)
tools\sync.ps1 -Up
```

- **Sync triggers are mandatory, not background knowledge.** Run `-Down` as the
  FIRST action of any working session, before reading or editing anything. Run
  `-Up` every time a unit of work completes (published tickets/spec/docs, a
  landed code change) and at session end — do not batch everything into one
  end-of-day sync. If you join work already in progress and no fresh snapshot
  exists for this session, sync `-Up` first if a snapshot from earlier the
  same session exists; otherwise back up local edits before any `-Down`
  (it overwrites differing local files).
- PowerShell's execution policy blocks `.ps1` directly (same reason npm is
  `npm.cmd`). Invoke it as:
  `powershell.exe -NoProfile -ExecutionPolicy Bypass -File "tools\sync.ps1" -Up`
- Verify the sync by running `-Up` again — expect `0 copied` when in sync. Never run `-Up` without a snapshot; the script refuses.
- The snapshot lives in `C:\Sprout\.sync-state.json` (excluded from the sync itself). If you must fall back to raw robocopy, add `/XF .sync-state.json` to the command below — and know that it silently clobbers newer share content:

```powershell
robocopy "C:\Sprout" "\\vmware-host\Shared Folders\Projects\Sprout" /E /R:1 /W:1 /NFL /NDL /NJH /NP /XD node_modules target build .svelte-kit .vscode .codegraph /XF .sync-state.json
```

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

## Releases

Releases are GitHub Releases built by CI — never hand-built installers on
this device. The whole flow (pre-flight gates, version bump in Cargo.toml,
sync, tag push, passive self-update) lives in
`docs/release/release-process.md`; follow it instead of building locally.
`npm.cmd run tauri build` stays available for the rare local artifact, with
the same pre-flight gates.

## Cleanup (device storage)

Run whenever told to do a cleanup, and automatically after any local
`tauri build`. Device-only — the share never holds these dirs; nothing
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
- `docs/` — CONTEXT.md (glossary), specs/, adr/, research/, release/ (parity gate record + archived legacy log). `tools/` — parity compare, parity preset, sync.ps1 (guarded share sync). `.scratch/sprout-app/issues/` — ticket tracker (mark ACs done as you go).

## Module design
When extracting, refactoring interfaces, or deciding module boundaries,
use codebase-design skill vocabulary (module/interface/seam/adapter/depth)
and apply its deletion test before extracting to shared/.

## Conventions (Rust + Tauri + Svelte)
- Constants: domain-split under constants/ (theme, window, app) — not one file.
- App version: Cargo.toml only, tauri.conf.json omits it (auto-inherits).
  Svelte reads via getVersion(), never a duplicated constant.
- Window sizing: `src-tauri/src/constants/window.rs` is the single size
  source — tauri.conf.json declares no windows (ADR-0013 boot-to-tray);
  runtime/docked dimensions Svelte needs come from a Tauri command, not a JS
  constant.
- shared/: only for modules that hide real complexity or have a genuine
  second adapter. Thin pass-throughs get inlined.

## Comments
- No WHAT comments — if the code needs one to be understood, fix the naming/interface instead.
- WHY comments only, self-contained (readable without opening a tracker).
- Point to an ADR by name for durable rationale, never a ticket number (tickets close/renumber; ADRs don't).
- External constraints (upstream bugs, library quirks) may cite the issue link directly.
- No history trails in comments (ticket 1 → 5 → 10) — that's git log/blame's job, not the code's.
