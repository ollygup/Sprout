# 92 — Honest stop lifecycle: watchdog, Stopping state, Run/Stop color mapping (window)

**What to build:** The Quick Launch window's action controls gain an unambiguous three-state language. Clicking Stop flips the control to a disabled "Stopping…" spinner until the process's exit event lands; a configured stop command gets a ten-second watchdog after which the process tree is force-killed so a hung stop can never wedge the control; if the process exits first, Stopping ends immediately. Run renders accent-filled (the row's primary verb), Stop danger-filled, Stopping in the disabled muted treatment — all from existing theme token families.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [ ] State transitions run Run → Running → Stopping → Run driven solely by run-state events
- [ ] Hung stop command is killed at the ten-second timeout and the control recovers
- [ ] Early process exit ends Stopping without waiting out the watchdog
- [ ] Colors derive from token families in both themes; exactly one primary verb per surface preserved
- [ ] Backend tests cover both watchdog paths; manual pass in light and dark
