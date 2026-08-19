# 05 — Run execution in-process with winget steps and results

**What to build:** Executing a Plan: dependency-first ordering, per-Requirement timebox, winget install/upgrade with the ported exit-code whitelist (including reboot-required detection), per-Requirement outcomes persisted with the Run, and a results summary screen. Runs non-elevated for now (dev mode); the loop is built so the elevated worker ticket reuses it unchanged. This ticket makes "run my setup and see what happened" work end to end.

**Blocked by:** 04 — Plan preview with machine detection and multi-preset composition

**Status:** done — 68 backend tests green, svelte-check 0 errors, vite build ok

- [x] Running a Plan executes Requirements in dependency order, each under its timebox (killed and recorded as timed-out if exceeded)
- [x] winget install/upgrade steps with the ported exit-code whitelist; genuine failures reported per Requirement
- [x] Run and per-Requirement results persisted; summary screen shows installed / upgraded / already OK / failed / reboot-required
- [x] Re-running skips already-satisfied Requirements
