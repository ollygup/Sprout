# Rust/Tauri engine replaces the PowerShell package

> Status: amended 2026-09-05 — original text preserved below; the correction is in the Amendment section.

The legacy setup package (`Setup.bat`, PowerShell scripts, YAML catalog) is the battle-tested reference for install behavior, but it is not self-contained: it depends on PowerShell 5.1, execution-policy bypasses, and a script-level contract. We ported the engine to Rust (Tauri 2 backend) as a trait-based platform strategy so the app is a single self-contained exe with no scripting runtime dependency. The legacy package stayed in `legacy/` as a runnable parity reference until v1 release; the parity gate passed 2026-08-15 (ticket 10, record in `docs/release/parity-checklist.md`) and the package was deleted.

Status: accepted

## Considered options

- **Keep PowerShell as an embedded engine** (Tauri UI wrapping the scripts). Faster to ship, but contradicts the stated goal of a self-contained, portable app and preserves the fragile script↔app contract.
- **Port to Rust**. Real porting effort (exit-code whitelist, timeboxing, registry heuristics, env wiring are all translated, not rediscovered), bought the self-containment goal.

## Amendment — 2026-09-05 (codebase accuracy pass)

One precision fix: "no scripting runtime dependency" means no bundled runtime and no 5.1/execution-policy/script-contract — not "never invokes PowerShell". The code still shells to the inbox `powershell.exe` where Windows offers no better API (winget bootstrap/release fetch, app discovery, Quick Action runs: `engine/windows.rs` `powershell_argv`, `launch.rs`, `quick_actions.rs`, `store.rs`). Nothing about the decision changes; the wording now can't be misread as a never-spawns-powershell claim.
