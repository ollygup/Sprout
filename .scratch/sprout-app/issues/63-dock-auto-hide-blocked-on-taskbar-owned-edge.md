# 62 — Dock auto-hide never engages on a taskbar-owned edge

**What to build:** When the docked edge is already owned by another auto-hide bar (the taskbar's own auto-hide on the same edge), Sprout's auto-hide is refused by the shell (`ABM_SETAUTOHIDEBAR`), the dock silently reconciles to fixed mode and **persists** that per-monitor, so every later dock on that monitor is permanently fixed — the dock "docks nicely but never hides". The user should be told why, be offered the free edge, and auto-hide should come back automatically when the edge frees up. Parent spec: 55.

**Status:** todo

**Repro (VM, Windows 11, taskbar on the RIGHT edge with taskbar auto-hide ON):**

1. Dock to the right edge with mode auto-hide (the default): `ABM_SETAUTOHIDEBAR` is refused — the shell returns no engagement because the taskbar's own auto-hide owns the edge ("another auto-hide bar may already own this edge").
2. `apply_dock_mode` reconciles the recorded mode to `"fixed"` and `db::save_dock_mode` **persists fixed into the monitor's dock memory** — and `dock()` writes the same on every subsequent dock resolution. The dock is now fixed forever on that monitor even though Settings still say auto-hide.
3. Result: the strip never slides away ("the window does not hide at all"), hovering the edge reveals the **taskbar**, not the dock, and the main app never reclaims the space (fixed keeps the reservation permanently — auto-hide is what releases it so maximized windows grow back, per the user's expectation).
4. Verified on-device: the left edge is free and auto-hide engages there (`ABM_GETAUTOHIDEBAR` confirms); the right edge with a taskbar auto-hide is exactly the refusal case above. Root cause is the taskbar owning the edge, not the AppBar machinery itself (ticket 61's march fix is intact — this VM build was reverted to the stable 61 version).

**Proposed fix direction (not started — deliberately deferred to keep 0.2.0 stable):**

- A refused auto-hide keeps the **requested** mode (auto-hide) and records the refusal as a transient "blocked" state — never persist `fixed` into the per-monitor memory, so a refused dock does not poison every future dock.
- Surface the blocked state in the Quick Launch window: a warning banner with the reason ("the taskbar already auto-hides this edge") and a one-click "dock to the other edge" action; the effective mode is otherwise invisible today.
- Re-try the engagement on the next shell notification (`ABN_STATECHANGE` fires when the taskbar's auto-hide setting changes; `ABN_POSCHANGED` on work-area changes): when the taskbar frees the edge, auto-hide resumes without a redock.
- Verify on the VM with a controlled cursor (`SetCursorPos`): left-edge dock hides to the sliver with the cursor centered, reveals on hover, hides again on departure; right-edge dock shows the blocked banner and the switch action. Note: a genuine auto-hide bar releases its reservation while hidden, so the main window reclaims the space automatically once the bar actually hides (this was confirmed working on the free left edge — the user's "app does not take full space" report is the fixed-reservation symptom, not a separate bug).
- `set_autohide`'s refusal error is currently surfaced only as a transient dock-error banner on the dock/switch commands; keep an error log line for diagnostics.

- [ ] Refusal keeps the intended mode; the blocked state is transient and never persisted
- [ ] Quick Launch window shows a blocked banner with the reason and a switch-edge action; effective mode is visible
- [ ] A freed edge re-engages auto-hide automatically (shell notification re-try)
- [ ] VM verify: left-edge auto-hide hides/reveals with a controlled cursor; right-edge shows the banner and switch works
- [ ] `cargo test` green; `npm run check` 0 errors; synced to the share