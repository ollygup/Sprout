# 08 — Command steps, winget bootstrap, unmanaged installs

**What to build:** Hardening the run path beyond plain winget: the `command` Step type (executable + args + declared success codes, timeboxed — covers node-lts-style flows via nvm), automatic winget bootstrap when the machine lacks it, and clear handling of non-winget-managed installs (detected via registry, skipped with an "update manually" note). This ticket makes "every requirement in the current catalog runs, even ones winget can't install" work end to end.

**Blocked by:** 05 — Run execution in-process with winget steps and results

**Status:** done — 112 backend tests green (was 101), svelte-check 0 errors, vite build ok

- [x] `command` Step type: executable + args + declared success codes, timeboxed, runs elevated; covers node-lts-style custom steps
- [x] Missing winget is bootstrapped automatically at run start; unsupported OS builds get a clear message
- [x] Non-winget-managed installs detected via the registry are skipped with an "update manually" note in the Plan and Run
