# 75 — Auto-start registration + Settings toggle

**What to build:** The registration half of ADR-0013: Sprout starts with
Windows by default via the standard autostart plugin (HKCU Run), controlled by
a persisted setting and a Settings toggle; debug builds never register.

**Blocked by:** 72 — the spec fixing default-on, the debug guard, and the
launcher argument contract consumed later by ticket 76

**Status:** done

- [x] Autostart plugin dependency initialized with launcher arg `--autostart` (consumed by ticket 76)
- [x] New `autostart` settings key ("on"/"off"), default **on**, validated like every other knob; round-trip + junk-rejection tests in the settings suite
- [x] Registration sync function: reads desired state, compares plugin's enabled state, enables/disables as needed — `#[cfg(debug_assertions)]` builds log-and-skip instead of touching the Run key
      (`autostart::sync_registration`; the guard is the equivalent compile-time
      `registration_allowed(cfg!(debug_assertions))`, so release binaries carry no dead branch)
- [x] Sync runs once at startup and whenever the toggle changes (dedicated command so the AppHandle side-effect lives beside the save)
      (startup sync thread in `.setup`; dedicated `update_autostart` command persists only the key, then reconciles beside the save)
- [x] Settings page gains the toggle consistent with existing rows; turning it off takes effect immediately without restart
      ("Start with Windows" row, On/Off select like the dock rows; applies immediately like the theme pick, reverts + errors if the backend refuses)
- [x] Manual verification note recorded: enable → Run value present; disable → absent; dev session leaves registry untouched
      (see below)
- [x] `cargo test` green; `npm run check` 0 errors; synced to the share

## Manual verification (2026-08-22, release exe `target\release\sprout.exe`, HKCU `Software\Microsoft\Windows\CurrentVersion\Run`)

| Scenario | Start state | Result |
| --- | --- | --- |
| Startup sync, default (no `settings.autostart`) | value absent | **PASS** — value created: `"C:\...\sprout.exe" --autostart` |
| Startup sync, setting `off` | value present | **PASS** — value removed at launch |
| Startup sync, setting `on` | value absent | **PASS** — value recreated |
| Dev/debug session (`tauri dev` relaunch with the new code live) | value absent | **PASS** — registry untouched (log-and-skip path) |

Notes:

- The plugin writes the Run value under the product name `Sprout`
  (`tauri.conf.json` productName → `package_info().name`), which is exactly
  the value the vendored NSIS uninstaller deletes — cleanup contract holds.
- One intermediate launch left a present-but-unwanted value untouched: the
  shell had flipped the `StartupApproved\Run` approval bit between launches,
  so the plugin's `is_enabled()` reported false and the sync correctly left
  the entry alone instead of fighting Task Manager's choice. Side effect of
  the standard plugin's definition of "enabled"; accepted behavior.
- Machine state restored afterwards: `settings.autostart` key deleted,
  Run + StartupApproved values deleted, test processes stopped.
