# Boot to tray with docked Quick Launch restore

> Status: amended 2026-09-05 — original decision text preserved; see the executable-source audit amendment for current behavior and implementation gaps.

Auto-start puts Sprout in the Windows boot path (HKCU Run via the autostart plugin, on by default, toggled in Settings): at login only the resident backend and tray icon start — the main window never appears — and the Quick Launch dock materializes when its remembered state says docked. **Fixed** reappears as the reserved strip; **auto-hide** as the hover sliver; both on the remembered monitor edge/mode. Floating state — and a fresh install with no remembered state — stays tray-only until the user clicks. The same restore rule runs whenever the main app opens, so every entry point converges: *docked preferences materialize on any app start; floating waits for an explicit click.*

## Why

ADR-0010 made the tray the permanent resident so Quick Launch stays available without a webview. Auto-start extends that to machine boot: the morning routine should not require opening anything. But booting straight into a screen-edge bar would surprise a fresh install, so the dock only ever restores what the user actually left behind.

## Decisions

- **Remembered last state, not a separate default**: docking persists a docked-or-floating bit next to the existing per-monitor edge/mode records; boot and app-open reproduce exactly what was left.
- **Floating means hidden**: with floating remembered, login shows only the tray icon — the Quick Launch window waits for its explicit click, at boot and after Open Sprout alike.
- **Fresh installs float**: no remembered state ⇒ tray-only; Settings defaults still govern the first explicit dock.
- **Production-only registration**: debug builds never touch the Run key, so `tauri dev` sessions don't pollute the boot path.
- **Uninstall cleanliness**: the vendored NSIS template already deletes the product-named HKCU Run value on uninstall — no new cleanup path.

## Consequences

- A monitor missing at boot falls back to the current primary monitor with the remembered edge/mode rather than staying hidden.
- Single-instance forwarding already covers the auto-start race (a user double-launch at login lands in the resident instance).

## Amendment — 2026-09-05 (codebase accuracy pass)

"Primary monitor fallback" and "hover sliver" wording corrected: the code resolves the current monitor's memory and has no primary-monitor branch; auto-hide hides fully off-screen (no handle). Restore rules (docked materializes on any app start, floating waits for a click; fresh installs stay tray-only) verified unchanged.

## Amendment — 2026-09-05 (executable-source audit)

Dock restore is invoked at process setup (`src-tauri/src/lib.rs`) and by `open_sprout` in `src-tauri/src/tray.rs`. The single-instance callback and `open_main_window_cmd` route through `request_open_main_window` without also restoring the dock. Thus “every entry point converges” remains intended behavior rather than a complete implementation.

Restoration creates a centered Quick Launch window and resolves that window’s current-monitor preferences (`src-tauri/src/quick_window.rs`, `open` and `dock`); it does not select a previously remembered monitor. Auto-start suppression of the main window, production-only registration (`src-tauri/src/autostart.rs`), and uninstall removal of the Run-key entry (`src-tauri/nsis/installer.nsi`) are implemented. This records the restore gaps without changing the boot-to-tray decision.
