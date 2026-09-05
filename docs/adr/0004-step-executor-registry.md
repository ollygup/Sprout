# Step types dispatch through an executor registry

> Status: amended 2026-09-05 — the registry described below never existed in code; the built decision is in the Amendment section. Original text preserved.

Steps are data and execution dispatches through `HashMap<StepType, Box<dyn StepExecutor>>` — the plugin-registry pattern Tauri itself uses — so the run loop never hardcodes step logic and new step kinds are additive registrations, not invasive changes. v1 ships `winget` and `command` step types. Winget's manifest system already covers exe/msi/msix/zip-portable installs, silent switches, and success codes, so most installer shapes are winget cases; a future download-and-run type is one new executor struct plus one registry entry.

## Amendment — 2026-09-05 (codebase accuracy pass)

Steps are data (`Step::Winget` / `Step::Command` in `domain.rs`); execution crosses one seam — the `PlatformEngine` interface in `engine/mod.rs` (`prepare`, `detect`, `install`, `upgrade`, `verify`, `apply_env_wiring`, `actual_install_location`). The run loop (`run.rs`) hands each Requirement's step across that seam and never implements step logic itself; all winget/command knowledge lives behind it in the `WindowsWingetEngine` adapter. The seam is real, not hypothetical: production code crosses it from `AppState.engine`, tests cross it from fakes (`run.rs` `FakeEngine`), so the interface earns its keep on both sides.

## Considered options

- **`HashMap<StepType, Box<dyn StepExecutor>>` plugin registry** (the pattern this ADR originally described). Rejected: with two step kinds and one production adapter, a registry is a hypothetical seam — a shallow module whose interface would be nearly as complex as the dispatch it hides. The deletion test fails: deleting it removes no complexity from callers, it just moves the `match` into a map insert.
- **Hardcoding step logic in the run loop**. Rejected: it would spread winget/command knowledge across detection, install, verify, and env wiring, failing the locality the seam buys — fix once behind the seam, fixed everywhere.

## Consequences

- v1 ships `winget` and `command` step kinds. Winget's manifest system already covers exe/msi/msix/zip-portable installs, silent switches, and success codes, so most installer shapes are winget cases.
- A future download-and-run kind is a new `Step` variant plus its validation and seam arms (`domain.rs`, `engine/mod.rs`, `engine/windows.rs`, `plan.rs`, `run.rs`) — deliberately more than one registry entry, because each of those places owns a distinct part of the interface (shape, validation, execution, composition).
- The Quick Launch side has its own seam (`LauncherEngine`) — covered by the launch-pipeline ADR, not this one.

The registry paragraph above is preserved history. It never existed in code: no `StepExecutor`/`StepType` type has ever lived in `src-tauri/src` (verified by search). The decision is restated in this section in `codebase-design` vocabulary (module/interface/seam/adapter) so the ADR no longer lies about the architecture.
