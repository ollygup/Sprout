# 61 — Dock robustness + seamless edge switch

**What to build:** Docking follows the documented Win32 AppBar pattern so the strip reserves its space on any desktop — fixing the "space is reserved but the window overlaps other windows" symptom seen on laptop-plus-external-monitor setups — and edge switches are immediate and flicker-free, with failures surfaced instead of half-docking. Parent spec: 55.

**Blocked by:** 60 — Dock auto-hide (serialized on the same `appbar.rs` surface)

**Status:** todo

> **2026-08-20 fix note 1:** first on-device run was visibly broken — the dock
> churned every second (watchdog re-docking in a loop, no auto-hide). Root
> cause: the floating window's `max_inner_size(340, 460)` from the builder was
> never cleared — `dock()` only cleared the min — so `SetWindowPos` to the
> full-height strip was clamped by `WM_GETMINMAXINFO`, the window could never
> reach the rect `ABM_SETPOS` granted, and the new 1-second drift watchdog
> fought that divergence forever (this clamp is also the real "reserved space
> but window overlaps" root cause from spec 55 — the reserved strip was
> full-height while the placed window was 460 tall). Fixed: `reshape()` clears
> **both** min and max; the watchdog now requires two consecutive divergent
> ticks before re-docking (a transient OS auto-hide slide must not yank the
> bar) and logs each re-dock. Re-verify on-device.

> **2026-08-20 fix note 2 (the march):** user re-test found the dock
> "aggressively shifts everything to the left" — repeatedly, until the window
> ran off-screen and couldn't be closed. Diagnosed on-device (VM, 150% scale)
> with a `[DOCKPROBE]`-tagged reproduction: the march is a feedback loop in
> the AppBar re-assert. The shell recomputes `GetMonitorInfo`'s work area from
> the registered bars, so the bar's **own** placement makes `rcWork`'s edge
> sit exactly at the bar's inner edge (`rcWork.right == bar.left` for a right
> dock). Every `ABN_POSCHANGED` (which our own placement triggers) then
> rebuilt the desired rect from that self-shrunk work area — and the shell
> grants those rects verbatim — so each re-assert granted a rect one bar-width
> into the screen: 2220 → 1540 → 1200 → … → −160, then churned forever. The
> same derivation existed in `quick_window::reposition` (same-edge re-dock)
> and the drift watchdog. Fixed with `appbar::desired_rect`: the docked side
> is re-derived from the work area **only** when the work area genuinely
> changed against the bar (an intruder took/freed the edge); when `rcWork`'s
> edge merely reflects the bar's own reservation, the bar's horizontal
> position is kept and only top/bottom follow the work area (taskbar moves).
> Verified in the VM: right+fixed and left+auto-hide both dock flush and stay
> put (was ~150 churn log lines, now 2 no-op ABNs), auto-hide refusal on the
> taskbar-owned edge reconciles to fixed without marching. 4 new unit tests on
> `desired_rect` (266 passed). Re-verify on-device on both setups.

- [x] `register()` and `reposition()` follow the documented pattern: after `ABM_QUERYPOS` (which adjusts the rect by subtraction and does not preserve thickness), the strip thickness is re-applied to the returned rect; the window is placed using the rect returned by `ABM_SETPOS` (currently discarded — the divergence between the reserved rect and the placed window is the overlap root cause)
- [x] `ABN_POSCHANGED` (taskbar size/position/visibility changes, other app bars) re-queries and re-sets the bar's position — without marching: the re-assert keeps the bar flush against its edge (`desired_rect`, fix note 2)
- [x] Drift detection: a periodic `GetWindowRect` check against the expected rect re-docks the bar when they diverge (Win+Shift+→ window moves, monitor reconnects)
- [x] Edge switch is atomic: position+size applied together with no hide/show, no flicker; the failed mid-state (registered but unplaced) releases the AppBar and reports instead of leaving a half-docked window
- [x] Registration failure surfaces visibly in the window (error message) and logs the actual `SHAppBarMessage` result — no silent half-dock, no pretending to be docked
- [ ] Manual verify on both physical setups: docking reserves the strip on the laptop screen and the external monitor (maximized windows shrink, the bar never overlaps); left↔right switches are clean on both
- [x] `cargo test` green (266 passed incl. 4 new `desired_rect` tests); `npm run check` 0 errors; synced to the share