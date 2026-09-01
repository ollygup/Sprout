# 121 — foreground-if-running for single-tap Launch entries

**What to build:** A single Launch entry tap foregrounds its already-running window instead of spawning a second instance; batch `Start all` keeps skip and does not foreground.

**Blocked by:** none.

**Status:** ready-for-agent

## Scope

- Only `LaunchEntryKind::App` (`launch.rs:32` `.lnk`/`.exe`/future `shell:AppsFolder` UWP) — `Command` entries always spawn via `launch.rs:331` `command_argv`.
- Single path: `src/routes/quick-launch-window/+page.svelte` row `onclick={() => startEntry(entry)}` and `src/routes/quick-launch/+page.svelte` per-row run affordance → new `start_launch_entry`/`run_single_launch_entry` command that takes `isSingle=true`.
- Batch path: `startQuickLaunch` / `run_launch_queue` (`launch.rs:468`) keeps ticket 99 behavior — already-running windows on target desktop are reported as `skipped: "already on Desktop N"` and free the slot at spawn, never foreground.

## Match / foreground contract

- Snapshot pre-launch windows per desktop (`FakeLauncher.windows` `launch.rs:1104` seam reused): `EnumWindows` → `GetWindowThreadProcessId` → `QueryFullProcessImageName` basename trimmed, case-insensitive, equals entry target basename (for `.lnk` resolve target first).
- Target desktop = `entry.desktop_id` when `Some(guid)` live in engine's desktop list, otherwise current desktop per `docs/CONTEXT.md:66` Launch entry. Match only windows on that desktop (`launch.rs:498` per-window-per-desktop skip rule).
- On hit: `IsIconic→ShowWindow(SW_RESTORE)` + `SetForegroundWindow` at normal Z (no `HWND_TOPMOST`) so a Fixed dock (`appbar.rs` `ABM_SETPOS` work-area squeeze) stays as-is — overlapping single foreground appears above it per Q10. No `ShellExecute`/`ActivateApplication` on hit. Multiple hits → foreground most-recent Z.
- All other entries (including `Command`, unassigned mismatch, dead `desktop_id` fallback per ticket 99 `Some(false)` path) free at spawn with existing notes.

## ACs

- [x] Single tap on running app on same desktop foregrounds (restores if minimized) and run reports `skipped: foregrounded` or equivalent `LaunchReport` without a second pid; no duplicate process observed.
- [x] `Start all` with 1 running + 1 cold: report `1 started, 1 skipped (already on Desktop N)`, no foreground steals focus from batch.
- [x] Unassigned entry whose window is only on another desktop does not foreground — it spawns normally (per-desktop rule).
- [x] `cargo test` additions `foregounds_single_on_same_desktop`, `batch_skips_without_foreground`, `unassigned_foregounds_on_current_desktop` green; all `launch.rs:1103` `FakeLauncher` `window_delay`/`handed_off`/`move_fail` paths stay green.

## Verification

- `cargo test --manifest-path src-tauri/Cargo.toml`  (focus `launch::tests`)
- Manual: pin Notepad `.lnk`, open it, single-tap → foregrounds; `Start all` with Notepad open → skipped notice; minimized Notepad → restores.

