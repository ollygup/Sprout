# Elevated work runs via self-relaunch with file-based progress

> Status: amended 2026-09-05 — original decision text preserved; see the executable-source audit amendment for current behavior and implementation gaps.

Installs require elevation, but the app should run normally for browsing and planning. Instead of a separate helper exe or cross-elevation IPC, the same exe relaunches itself with a `--worker` flag under a UAC prompt; the worker executes the Plan, appends progress as JSON-lines to a per-run status file that the main process tails, and persists results to SQLite. This avoids all IPC plumbing between processes of different integrity levels; the only shared state is the per-run working directory on disk.

## Amendment — 2026-09-05 (executable-source audit)

The per-run directory carries the cross-process request, progress, cancellation, and completion protocol, but it is not the only shared state. Both processes use the Library SQLite database: `run_worker` in `src-tauri/src/worker.rs` reads Settings and persists Run results there. `launch_run` in `src-tauri/src/lib.rs` writes the request and relaunches the current executable with `--worker --run`; `launch_elevated` uses the Windows `runas` verb.

The worker re-detects installation state against the submitted Requirement snapshot. It does not rebuild that snapshot by re-reading current Library Products. The same-executable, file-based progress decision remains implemented; the run-start freshness gap is recorded in ADR-0007.
