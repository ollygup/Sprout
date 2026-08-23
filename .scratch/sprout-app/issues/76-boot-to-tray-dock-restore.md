# 76 — Boot to tray: `--autostart` path + dock restore wiring

**What to build:** The boot half of ADR-0013: with `--autostart` (set by
ticket 75's plugin), login starts backend + tray only; the Quick Launch window
opens — and therefore auto-docks via its existing persisted-state behavior —
only when the remembered dock state is "docked"; Open Sprout follows the same
rule. Main-window creation becomes programmatic so the config no longer forces
it at startup.

**Blocked by:** 75 — the plugin's launcher argument and registration this
path rides on

**Status:** done

- [x] Config-declared main window removed from tauri.conf.json; main window created programmatically at setup via the existing open/recreate seam unless `--autostart` is present (geometry constants remain the single size source)
      (`"windows": []`; setup runs `open_main_window` after the drivers when
      `autostart::is_autostart_launch` says the Run key launched us)
- [x] Constants module comment updated: it, not the conf file, mirrors the sizes now (AGENTS convention line amended accordingly)
      (`constants/window.rs` MAIN_WINDOW_* docs now name themselves the single
      size source; AGENTS.md window-sizing line rewritten)
- [x] Boot path: tray + drift/autohide drivers start as today; Quick Launch window opened only when the persisted dock state is "docked" — the open path's existing ticket-57 behavior applies edge/mode memory and docks immediately
      (`quick_window::open_if_docked` — one seam for every entry point,
      called from setup right after the drift/autohide drivers)
- [x] Floating or fresh-install boot → tray-only; first left-click opens/raises as usual
- [x] Tray "Open Sprout": additionally opens the Quick Launch window under the same docked-only rule (floating waits for its explicit click)
      (`open_sprout` = `open_main_window` + `open_if_docked`)
- [x] Single-instance hook unchanged: a second manual launch still focuses/creates the main window
- [x] Worker path untouched (`--worker` routes before any of this); dev sessions (`tauri dev`) behave exactly as before
- [x] Manual verification matrix recorded in the ticket: boot docked → bar present (fixed visible / auto-hide sliver); boot floating → hidden; fresh install → hidden; Open Sprout while docked-pref → bar appears; update AGENTS.md release/dist wording only where this ticket makes it stale
      (see below; release/dist wording had no stale statements — only the
      Conventions line)
- [x] `cargo test` green; `npm run check` 0 errors; synced to the share
      (318 passed / 1 ignored incl. two new `is_autostart_launch` tests;
      svelte-check 0 errors)

## Manual verification (2026-08-23, debug exe `target\debug\sprout.exe`, real `%LOCALAPPDATA%\Sprout\sprout.db`, windows enumerated via Win32 `EnumWindows`)

| Scenario | Persisted state | Launch | Result |
| --- | --- | --- | --- |
| Manual boot, docked pref | `dock.state=docked`, per-monitor right/auto-hide | plain | **PASS** — main window + Quick Launch bar both up; bar full-height at the right edge (auto-hide mode restored) |
| Boot docked, fixed variant | `dock.state=docked`, `dock.mode=fixed`, per-monitor rows cleared | `--autostart` | **PASS** — visible strip (340 px physical, work-area height, edge reserved), **no main window**, backend resident |
| Boot docked, auto-hide | `dock.state=docked`, remembered right/auto-hide | `--autostart` | **PASS** — 2 px sliver at the screen edge, **no main window** |
| Boot floating | `dock.state=floating` | `--autostart` | **PASS** — zero visible windows; process resident (tray only) |
| Fresh install | `dock.state` absent (default floats) | `--autostart` | **PASS** — zero visible windows; nothing docks uninvited |
| Second manual launch during an autostart session | floating | plain, while `--autostart` instance resident | **PASS** — second process forwarded and exited; resident instance created the main window (hook unchanged) |

Notes:

- **Open Sprout while docked-pref** could not be clicked programmatically; the
  menu handler is the exact pair verified above — `open_main_window`
  (exercised end-to-end by the single-instance second-launch row) plus
  `open_if_docked` (exercised at boot in every docked/floating/absent row).
  Worth one manual click on the next interactive session.
- Dev-session parity holds structurally: debug builds never receive
  `--autostart` (ticket 75's guard leaves the Run key untouched), and the
  plain-launch rows show the main window opening exactly as before.
- Machine state restored afterwards: `dock.state=docked`,
  `dock.mode=auto-hide`, per-monitor `right`/`auto-hide` rows recreated,
  test processes stopped.
