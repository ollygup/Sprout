# Step Types and Engine Extensibility — Research Notes

Findings gathered during the grilling session (Q13) that shaped ADR-0004 (step-executor registry) and the engine design. High-trust primary sources only.

## Winget manifest capabilities (why `winget` covers most installer shapes)

Winget manifests encode the install surface that a generic "download-and-run" step would otherwise have to reimplement:

- **Installer types**: MSIX, MSI, EXE, with recognized wrapper formats (Inno, Nullsoft, WiX, Burn) and their standard silent switches; **portable packages since winget 1.3, zip-archive packages since winget 1.5** (single nested installer or portable packages, with `PortableCommandAlias`).
- **Silent behavior**: `InstallModes` (silent, silent-with-progress, interactive) and `InstallerSwitches` (Silent / SilentWithProgress / InstallLocation / Log). Custom silent switches are passed by `winget install --silent`.
- **Success and expected return codes**: `success_codes` and `expected_return_codes` (e.g. reboot-required families) live in the manifest — matching our ported exit-code whitelist table.
- **Scope**: manifests declare supported `user` / `machine` scopes; per-user installs can run without elevation for many products.

Consequence: a future `download-run` step type duplicates machinery winget already owns for the common cases. The executor registry keeps that escape hatch cheap without building it now.

Sources: microsoft/winget-cli manifest docs; winget-pkgs manifest schema 1.5.0 (installer.md); docs.rs `winget_types` crate; Microsoft Learn "Use WinGet to install and manage applications".

## Rust strategy pattern (the Java-interface question)

Rust has the strategy pattern natively, in three encodings:

- **`trait` is the interface**; the closest match to Java's interface + class-per-strategy is **`Box<dyn Trait>`** — a fat pointer (data + vtable) giving runtime dynamic dispatch, injectable via constructors or Tauri's `app.manage(...)` (the DI container; commands receive state via `State<'_, _>`).
- **Generics** (`<S: Strategy>`) give zero-cost static dispatch when the strategy is fixed at compile time; **closures** (`impl Fn`) are the idiomatic default when the strategy is a single function — do not write a trait you do not need.
- **Object safety**: trait objects can't have generic methods or `Self`-returning methods; splitting traits keeps them boxable.

Consequence: `trait PlatformEngine` + `WindowsWingetEngine` impl swapped in at startup; macOS later is a new `impl`, and the rest of the code is untouched.

Sources: The Rust Book (ch. 18.2, trait objects); rustfaq.org dependency-injection guide; rs4ts.dev strategy-pattern-in-rust; Tauri v2 state-management docs.

## Tauri plugin/state patterns

- **Managed state**: `app.manage(T)` + `State<'_, T>` in commands; wrap in `Mutex` for mutability.
- **Plugin architecture**: a plugin is a `Builder` that registers commands and manages its own state — the model our step-executor registry mirrors in-process.
- **Events**: `emit_all` / `listen` for live progress between backend and frontend.

Sources: v2.tauri.app/develop/state-management; v2.tauri.app/develop/plugins; deepwiki.com/tauri-apps/tauri plugin-system page.
