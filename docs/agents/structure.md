# Structure — agent reference

> Read this file when: you touch files and need to know where things live
> (frontend, backend owners, docs, tools, ticket tracker). Otherwise skip it.

Inventory below reflects the post-refactor tree (ADR-0029 ownership:
`winget/`, `windows_execution/`, `engine/windows/inspection.rs`,
`appbar/display.rs`; constants split: `constants/window.rs` with
theme/app reserved); rule wording is unchanged.

- `src/` — Svelte 5 frontend (`lib/styles/tokens.css` = design tokens; `lib/components/` = accessible component foundation; `lib/api.ts` + `lib/types.ts` = Tauri command seam; `routes/` = main-window pages `products/`, `presets/`, `plan/`, `history/`, `logs/`, `settings/`, `clips/`, `quick-actions/` plus `quick-launch-window/` (read-only mini window); `routes/+page.svelte` = launch-entries home).
- `src-tauri/src/` — Rust backend (`domain.rs` = domain model, `db.rs` = lazy SQLite (empty on first run — ADR-0008), `lib.rs` = Tauri commands + `AppState` seams, `engine/` = PlatformEngine strategy seam with `engine/windows.rs` (Windows engine: spawn/foreground/desktop-move) and `engine/windows/inspection.rs` (native window/process inspection owner), `winget/` = winget facade (`winget.rs`) + `authoring.rs` (search/show) + `mutation.rs` (install/upgrade) + `bootstrap.rs`, `windows_execution/` = process/shell invocation owner (`process.rs` timed capture/taskkill/PowerShell/action runs, `shell.rs` ShellExecuteW open/elevated), `appbar/` = dock (`appbar.rs` geometry/composition + `appbar/display.rs` QueryDisplayConfig/EDID owner), `constants/` = domain-split constants (`window.rs` single size source; theme/app reserved), plus feature modules `launch`, `quick_actions`, `quick_window`, `clips`, `groups`, `backup`, `plan`, `run`, `settings`, `tray`, `update`, `worker`).
- `docs/` — CONTEXT.md (glossary), adr/ (+ README index), research/ (numbered notes), release/ (release-process.md + parity-checklist.md + archived legacy log). `tools/` — sync.ps1 (guarded share sync), ownership-gate.mjs (ADR-0029 gate), parity-compare.mjs, parity-preset.sprout.json, contrast-check.mjs, render-icon.mjs, repro-* scripts. `.scratch/sprout-app/issues/` — ticket tracker (WHEN you complete ACs → MUST mark them done as you go). `.sync-state.json` — sync snapshot (excluded from the sync itself).
