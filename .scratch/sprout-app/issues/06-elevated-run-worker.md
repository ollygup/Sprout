# 06 — Elevated run worker with live progress and cancel

**What to build:** The real run path: the app self-relaunches as `--worker` under a single UAC prompt, the worker executes the Plan appending JSON-lines progress to a per-run status file the main process tails, the UI shows live per-Requirement progress, and the user can cancel. This ticket makes "one prompt, then it installs everything with live progress" work end to end.

**Blocked by:** 05 — Run execution in-process with winget steps and results

**Status:** done — 79 backend tests green, svelte-check 0 errors, vite build ok; worker handshake smoke-tested (dispatch, status.jsonl, done.json); UAC relaunch verified manually in-app (one prompt, worker installs, live progress, cancel, summary)

- [x] Run phase self-relaunches the exe with `--worker` under a single UAC prompt; the main process stays non-elevated throughout
- [x] The worker executes the Plan and appends JSON-lines progress to the per-run status file; the UI tails it live
- [x] Cancel aborts the running Plan safely
- [x] Results persisted by the worker appear in the summary screen
- [x] The execution pipeline from ticket 05 is reused unchanged (no fork of logic in worker mode)
