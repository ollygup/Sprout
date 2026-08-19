# 47 — Desktop assignments actually move windows

**What to build:** A Quick Launch entry assigned to a virtual desktop reliably opens on that desktop, and the run summary says so when it can't. Root cause (found live: adding File Explorer from the installed-apps search, assigning it to Desktop 2): launching a Start Menu shortcut hands the launch to the already-running shell process, so `ShellExecuteEx` succeeds with no process handle — the entry is counted "failed" even though the window opens, and the queue waits forever for a window owned by a pid that never existed, then silently drops the desktop move. The same exact-pid assumption breaks on any wrapper/relaunch app (Discord, browsers via installer shims, Steam).

**Blocked by:** None — can start immediately

**Status:** done — verified live: entry "File Explorer (47)" assigned to Desktop 2, Start → "Quick Launch done — started 2, skipped 1, failed 0", no notes; `GetWindowDesktopId` on the new `CabinetWClass` window returns `1303c932-27bc-42b9-8e54-cb4ef2aa440a` (= Desktop 2). Pre-existing Control Panel window untouched on Desktop 1.

- [x] "Launched but no pid" is no longer a failure: a spawn that Windows reports successful without a process id counts as started
- [x] Window resolution falls back in order — exact pid, then any visible top-level window whose process image matches the entry's target exe (fixes Explorer: the shell window is explorer.exe), then windows of the spawned process's direct children (fixes wrapper launchers) — and the queue waits on and moves the window that resolution finds
- [x] The desktop move retries a few times over ~1.5 s (the shell's view-registration race) before giving up
- [x] A failed move is never silent: the run summary notes "X opened on the current desktop — could not move it" (page event + system notification), and the note also fires when the desktop no longer exists (existing fallback path)
- [x] Existing rules preserved: skip-already-running, queue cap, 15 s window timeout, "desktop no longer exists → current desktop with note", never switch the user's current desktop; tray Start all / desktop groups / single entries all share the fix
- [x] `cargo test` green with new fake-engine cases (pid 0 → started; child-window fallback; move failure → note); `npm run check` unaffected; manual verification: assign File Explorer to Desktop 2 → Start → opens on Desktop 2 with a "started" summary; synced to the share
