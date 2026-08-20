# 60 — Dock auto-hide

**What to build:** The docked bar's auto-hide mode actually works, taskbar-style: the cursor reaching the screen edge slides the bar out, and the cursor leaving the bar hides it again — on whichever edge the bar is docked. The OS-managed mechanism is fixed (correct edge, real callback message, state sync, verification) rather than replaced. Parent spec: 55.

**Blocked by:** 55 — Quick Launch window: dock, floating UX, live sync, and Quick Action control (spec); 56 — Floating window life cycle + shared UI constants (the auto-hidden-bar undock path depends on the size round-trip fix)

**Status:** todo

- [x] `set_autohide` receives the actual docked edge (currently hardcoded `ABE_LEFT` — the reason auto-hide never engaged on the right edge); the correct edge is also used on reposition
- [x] A real AppBar callback message is registered (`RegisterWindowMessage`) and set in `APPBARDATA.uCallbackMessage` before `ABM_NEW` (currently 0 — no `ABN_*` messages are ever received); `ABN_STATECHANGE` is handled and keeps the frontend's dock state honest
- [x] Auto-hide engagement is verified via `ABM_GETAUTOHIDEBAR` after `ABM_SETAUTOHIDEBAR`; when the system refuses (e.g. another auto-hide bar already owns that edge), the state is surfaced in the window instead of silently no-op'ing
- [x] Auto-hide re-applies correctly on edge switch and when the mode setting changes; fixed mode is untouched
- [ ] Manual verify on both physical setups (laptop + external monitor): cursor to the screen edge → bar slides out; cursor leaves the bar → it slides back; the floating window never auto-hides
- [x] `cargo test` green; `npm run check` 0 errors; synced to the share