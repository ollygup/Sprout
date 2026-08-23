# Boot to tray with docked Quick Launch restore

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
