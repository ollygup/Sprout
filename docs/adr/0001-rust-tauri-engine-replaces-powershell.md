# Rust/Tauri engine replaces the PowerShell package

> Status: amended 2026-09-05 — original decision text preserved; see the executable-source audit amendment for current behavior and implementation gaps.

The legacy setup package (`Setup.bat`, PowerShell scripts, YAML catalog) is the battle-tested reference for install behavior, but it is not self-contained: it depends on PowerShell 5.1, execution-policy bypasses, and a script-level contract. We ported the engine to Rust (Tauri 2 backend) as a trait-based platform strategy so the app is a single self-contained exe with no scripting runtime dependency. The legacy package stayed in `legacy/` as a runnable parity reference until v1 release; the parity gate passed 2026-08-15 (ticket 10, record in `docs/release/parity-checklist.md`) and the package was deleted.

Status: accepted

## Considered options

- **Keep PowerShell as an embedded engine** (Tauri UI wrapping the scripts). Faster to ship, but contradicts the stated goal of a self-contained, portable app and preserves the fragile script↔app contract.
- **Port to Rust**. Real porting effort (exit-code whitelist, timeboxing, registry heuristics, env wiring are all translated, not rediscovered), bought the self-containment goal.

## Amendment — 2026-09-05 (codebase accuracy pass)

One precision fix: "no scripting runtime dependency" means no bundled runtime and no 5.1/execution-policy/script-contract — not "never invokes PowerShell". The code still shells to the inbox `powershell.exe` where Windows offers no better API (winget bootstrap/release fetch, app discovery, Quick Action runs: `engine/windows.rs` `powershell_argv`, `launch.rs`, `quick_actions.rs`, `store.rs`). Nothing about the decision changes; the wording now can't be misread as a never-spawns-powershell claim.

## Amendment — 2026-09-05 (executable-source audit)

The install engine is implemented in Rust behind `PlatformEngine`, but operations using `powershell_argv` and `powershell_output` in `src-tauri/src/engine/windows.rs` still require an available Windows PowerShell executable. “Self-contained” means the former packaged PowerShell engine/script contract is not the execution architecture; it does not mean these features work without the OS scripting runtime.

This audit verifies the current execution path, not historical parity results, removed legacy contents, or claims that no better native operation exists. Those historical/rationale statements are preserved above and are not established by present source inspection.
