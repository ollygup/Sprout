# History is forever, logs expire, and run-active is answered from disk

> Status: amended 2026-09-05 — original decision text preserved; see the executable-source audit amendment for current behavior and implementation gaps.

Run history rows live forever; raw log folders expire. `log_retention_days` (default 30, range 1 day–10 years) prunes per-run folders after every run and at startup across `runs/`, `quick-actions/`, and `quick-launch/`; only the folders age out, never the Library rows. A folder's age is its embedded timestamp when the name carries one, else file mtime with a local-vs-UTC skew allowance. The Logs screen shows locations and sizes with open-folder actions and never renders log content — logs are evidence on disk, not app data. Whether a run is "active" is answered purely from disk, so the banner survives an app restart mid-run: a fresh folder gets a boot grace (3 min, covering the UAC-prompt gap), a quiet folder past the sum of its timeboxes plus margin is a dead worker rather than a slow one, and a recently finished run hands its completion to the UI exactly once inside a short window even when nothing was watching.

## Consequences

- Killing the app mid-run loses nothing: the worker persists the Run, the next start prunes and reports from the same files.
- Log location, not log content, is the interface — support and debugging point at paths, and retention stays a storage knob rather than a data-loss story.

## Amendment — 2026-09-05 (executable-source audit)

Retention runs at application startup and after an install Run is successfully persisted (`src-tauri/src/lib.rs` and `src-tauri/src/worker.rs`, the production callers of log pruning). Quick Action and Quick Launch completions do not each trigger a retention pass. Those passes do prune all three log families, while retaining history rows.

`find_recently_finished_run_at` in `src-tauri/src/worker.rs` repeatedly offers recent completion during a sixty-second window. Notification deduplication lives in the frontend’s in-memory state in `src/lib/runAwareness.svelte.ts`; “exactly once” is scoped to that frontend lifetime, not durable across restarts. Disk liveness inspection uses the boot grace and timebox-based limit when the request is readable, with a twenty-four-hour fallback for an unreadable request.

A surviving worker can finish after the UI closes, but worker termination and persistence failure remain possible; `run_worker` explicitly handles database-write failure. “Loses nothing” is not an unconditional durability guarantee. The history/log separation and Logs screen’s path/size/open-folder interface remain implemented.
