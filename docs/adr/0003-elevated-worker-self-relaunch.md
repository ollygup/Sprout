# Elevated work runs via self-relaunch with file-based progress

Installs require elevation, but the app should run normally for browsing and planning. Instead of a separate helper exe or cross-elevation IPC, the same exe relaunches itself with a `--worker` flag under a UAC prompt; the worker executes the Plan, appends progress as JSON-lines to a per-run status file that the main process tails, and persists results to SQLite. This avoids all IPC plumbing between processes of different integrity levels; the only shared state is the per-run working directory on disk.
