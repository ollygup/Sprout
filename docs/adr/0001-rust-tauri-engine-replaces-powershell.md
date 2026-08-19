# Rust/Tauri engine replaces the PowerShell package

The legacy setup package (`Setup.bat`, PowerShell scripts, YAML catalog) is the battle-tested reference for install behavior, but it is not self-contained: it depends on PowerShell 5.1, execution-policy bypasses, and a script-level contract. We ported the engine to Rust (Tauri 2 backend) as a trait-based platform strategy so the app is a single self-contained exe with no scripting runtime dependency. The legacy package stayed in `legacy/` as a runnable parity reference until v1 release; the parity gate passed 2026-08-15 (ticket 10, record in `docs/release/parity-checklist.md`) and the package was deleted.

Status: accepted

## Considered options

- **Keep PowerShell as an embedded engine** (Tauri UI wrapping the scripts). Faster to ship, but contradicts the stated goal of a self-contained, portable app and preserves the fragile script↔app contract.
- **Port to Rust**. Real porting effort (exit-code whitelist, timeboxing, registry heuristics, env wiring are all translated, not rediscovered), bought the self-containment goal.
