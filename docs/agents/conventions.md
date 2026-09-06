# Conventions (Rust + Tauri + Svelte) — agent reference

> Read this file when: you touch Rust, Tauri config, Svelte, constants,
> the app version, window sizing, module boundaries, `shared/`, or any
> Windows command/API invocation. Otherwise skip it.

- Constants: MUST domain-split under `constants/` (theme, window, app) — MUST NOT use one file.
- App version: MUST keep version in `Cargo.toml` only, `tauri.conf.json` MUST omit it (auto-inherits). Svelte MUST read via `getVersion()`, MUST NOT use a duplicated constant.
- Window sizing: `src-tauri/src/constants/window.rs` is the single size source — `tauri.conf.json` MUST declare no windows (ADR-0013 boot-to-tray); runtime/docked dimensions Svelte needs MUST come from a Tauri command, MUST NOT come from a JS constant.
- WHEN you extract code, refactor interfaces, or decide module boundaries → MUST USE `codebase-design` skill vocabulary (module/interface/seam/adapter/depth) and MUST apply its deletion test. WHEN you consider `shared/` → MUST put a module there ONLY when it hides real complexity or has a genuine second adapter — thin pass-throughs MUST be inlined.
- Windows operations: each Sprout-owned Windows command/API has exactly one owning module (ADR-0029) — winget verbs → `winget/`, process/shell invocation → `windows_execution/`, Quick Launch window/process inspection → `engine/windows/inspection.rs`, display-config probing → `appbar/display.rs` (operation→owner map in ticket 135). WHEN you touch a Windows invocation → MUST extend the owner, MUST NOT add a second invocation site. WHEN the owner genuinely cannot cover the case → MUST record the new owner in the inventory rather than duplicating silently.
