# AGENTS.md — working rules for Sprout

Read this first. These rules exist so every session (including fresh ones) builds without surprises and never damages the source of truth.

All `MUST` / `MUST NOT` below are BLOCKING. No implicit bypass — when a `WHEN` condition matches, the `MUST` applies every time. Do not skip.

## CodeGraph

- WHEN you need to understand or locate code and the repository is indexed by CodeGraph (a `.codegraph/` directory exists at the repo root) → MUST reach for CodeGraph BEFORE `grep`/`find` or reading files:
  - **MCP tool** (when available): `codegraph_explore` answers most code questions in one call — the relevant symbols' verbatim source plus the call paths between them, including dynamic-dispatch hops grep can't follow. Name a file or symbol in the query to read its current line-numbered source. If it's listed but deferred, load it by name via tool search.
  - **Shell** (always works): `codegraph explore "<symbol names or question>"` prints the same output.
- WHEN there is no `.codegraph/` directory → MUST skip CodeGraph entirely — indexing is the user's decision.

## UI design rules

- WHEN you change any UI (component, page, styling) → MUST USE the `web-design-guidelines` and `frontend-design` skills AND MUST REUSE the existing design system — tokens from `src/lib/styles/tokens.css`, components from `src/lib/components/`. MUST NOT introduce ad-hoc colors, type sizes, radii, or one-off component patterns. WHEN no shared pattern genuinely fits → MUST capture the deviation in the ticket and get it reviewed before shipping.
- WHEN you make any UI/UX design decision → MUST FIRST read the standing research notes under `docs/research/` — `0004-progressive-disclosure-and-clips.md`, `0005-page-chrome-consistency.md`, `0006-notion-design-patterns.md` (Notion's factual method: visibility-on-surface vs configuration-elsewhere, minimal-until-content defaults, explicit-setup gating; pattern 8 covers view-scoped switches), `0007-export-scope-selection-placement.md` (per-use scope choices → moment-of-use dialogs), and `0008-feature-menus-over-toolbar-checkboxes.md` (opt-in feature switches → the page-features menu; classify a knob before placing it) — AND MUST cite the rule you applied. WHEN you gather new evidence for an existing topic → MUST extend that topic's note (e.g. Notion findings go into 0006); WHEN the topic is genuinely NEW → MUST create a NEW numbered research note.
- WHEN you change any UI dimension → MUST FIRST scan `src-tauri/src/constants/window.rs` AND MUST NOT re-declare its values in another module (single size source; details in Conventions → Window sizing).

## Working copy rule (important)

- The repo has two homes:
  - **Master (source of truth, fallback):** `\\vmware-host\Shared Folders\Projects\Sprout`
  - **Working copy (develop here):** `C:\Sprout`
- WHEN working on the repo → MUST work in `C:\Sprout`. MUST NOT work directly on the share — UNC paths break `.cmd`/`.bat` (npm, cargo helpers) — builds fail with "UNC paths are not supported".
- MUST NOT delete or restructure anything on the share — the share is the fallback if the working copy messes up.
- Git is handled externally — MUST NOT run ANY git command here (STRICT). Git on this project belongs to the user, outside this device: MUST NOT run `git init`, `clone`, `add`, `commit`, `push`, `stash`, or ANY other git command against any path under `C:\Sprout`, ever. A `.git` directory may exist under `C:\Sprout` because it rides along with the share sync — that is fine; MUST treat it as inert data: MUST NOT create, modify, delete, or act on it. All version-control state lives elsewhere; changes made here are published by syncing (`tools\sync.ps1`), not by committing.
- WHEN syncing → MUST use `tools\sync.ps1`, MUST NOT use raw `robocopy`. The share's git working tree is owned by the other device (the only git client); a blind robocopy overwrites whatever it committed and produces merge conflicts. The script snapshots the share's content hashes at session start and refuses to overwrite any file the other device changed mid-session — divergences are reported as `SHARE-NEWER` for explicit resolution:

```powershell
# session start (refreshes C:\Sprout from the share, then snapshots it)
tools\sync.ps1 -Down
# session end (copies only what we changed, guarded by the snapshot)
tools\sync.ps1 -Up
```

- Sync triggers are blocking, not background knowledge:
  - WHEN any working session starts → MUST run `-Down` as the FIRST action, before reading or editing anything.
  - WHEN any working session starts → MUST read `docs/adr/README.md` (the one-page decision index) after `-Down`, before planning. MUST then read in full any ADR whose area the task touches. MUST NOT re-litigate a recorded decision without appending a dated `## Amendment` stating what changed and why.
  - WHEN any unit of work completes (published tickets/spec/docs, a landed code change) AND WHEN the session ends → MUST run `-Up` — MUST NOT batch everything into one end-of-day sync.
  - WHEN you join work already in progress and no fresh snapshot exists for this session → MUST first check for a snapshot from earlier the same session: WHEN such a snapshot exists → MUST sync `-Up` first; WHEN no snapshot exists → MUST back up local edits before any `-Down` (it overwrites differing local files).
- WHEN invoking the sync script → MUST invoke as `powershell.exe -NoProfile -ExecutionPolicy Bypass -File "tools\sync.ps1" -Up` (or `-Down`) — PowerShell's execution policy blocks `.ps1` directly (same reason npm is `npm.cmd`).
- WHEN you have run `-Up` → MUST verify the sync by running `-Up` again — MUST expect `0 copied` when in sync. MUST NOT run `-Up` without a snapshot; the script refuses.
- The snapshot lives in `C:\Sprout\.sync-state.json` (excluded from the sync itself). WHEN you must fall back to raw robocopy → MUST add `/XF .sync-state.json` to the command below — and know that it silently clobbers newer share content:

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

- MUST NOT hand-build installers on this device. Releases are GitHub Releases built by CI — the whole flow (pre-flight gates, version bump in Cargo.toml, sync, tag push, passive self-update) lives in `docs/release/release-process.md`; MUST follow it instead of building locally. `npm.cmd run tauri build` stays available for the rare local artifact, with the same pre-flight gates.

## Cleanup (device storage)

- WHEN told to do a cleanup, AND automatically after any local `tauri build` → MUST run cleanup. Device-only — the share never holds these dirs; nothing here is deleted from the repo or the share.
  - MUST run `Remove-Item -Recurse -Force C:\Sprout\src-tauri\target` — the big one (often several GB: debug builds + incremental caches). Recreated by the next `tauri dev` / `cargo test` / `tauri build`.
  - MUST leave `node_modules` and `%LOCALAPPDATA%\Sprout` alone (needed constantly / user data). `.svelte-kit`, `build\`, `src-tauri\gen` are tiny — optional.

## Verification helpers

- Inspect the Library DB read-only (Node has built-in sqlite):
  `node -e "const {DatabaseSync}=require('node:sqlite');const d=new DatabaseSync(process.env.LOCALAPPDATA+'\\Sprout\\sprout.db',{readOnly:true});console.log(d.prepare('SELECT COUNT(*) c FROM products').get().c)"`
- Fresh installs open to an empty Library (ADR-0008): `c` is 0 until the user adds Products from the live winget registry search — nothing is seeded.

## Structure

- `src/` — Svelte 5 frontend (`lib/styles/tokens.css` = design tokens; `lib/components/` = accessible component foundation; `routes/+page.svelte` = Library view).
- `src-tauri/src/` — Rust backend (`domain.rs` = domain model, `db.rs` = lazy SQLite (empty on first run — ADR-0008), `engine/` = PlatformEngine strategy seam, `lib.rs` = Tauri commands).
- `docs/` — CONTEXT.md (glossary), specs/, adr/, research/, release/ (parity gate record + archived legacy log). `tools/` — parity compare, parity preset, sync.ps1 (guarded share sync). `.scratch/sprout-app/issues/` — ticket tracker (WHEN you complete ACs → MUST mark them done as you go).

## Conventions (Rust + Tauri + Svelte)

- Constants: MUST domain-split under `constants/` (theme, window, app) — MUST NOT use one file.
- App version: MUST keep version in `Cargo.toml` only, `tauri.conf.json` MUST omit it (auto-inherits). Svelte MUST read via `getVersion()`, MUST NOT use a duplicated constant.
- Window sizing: `src-tauri/src/constants/window.rs` is the single size source — `tauri.conf.json` MUST declare no windows (ADR-0013 boot-to-tray); runtime/docked dimensions Svelte needs MUST come from a Tauri command, MUST NOT come from a JS constant.
- WHEN you extract code, refactor interfaces, or decide module boundaries → MUST USE `codebase-design` skill vocabulary (module/interface/seam/adapter/depth) and MUST apply its deletion test. WHEN you consider `shared/` → MUST put a module there ONLY when it hides real complexity or has a genuine second adapter — thin pass-throughs MUST be inlined.

## Comments

- MUST NOT add WHAT comments — WHEN the code needs one to be understood → MUST fix the naming/interface instead.
- MUST use WHY comments only, self-contained (readable without opening a tracker).
- MUST point to an ADR by name for durable rationale, MUST NOT point to a ticket number (tickets close/renumber; ADRs don't).
- External constraints (upstream bugs, library quirks) MAY cite the issue link directly.
- MUST NOT add history trails in comments (ticket 1 → 5 → 10) — that's `git log`/`blame`'s job, not the code's.
