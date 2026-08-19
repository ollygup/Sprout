# 09 — History, Logs, and Settings screens

**What to build:** The app's memory and knobs: a Runs list (timestamp, Preset(s), overall outcome) with reopenable per-Requirement detail, a Logs tab showing where log files live on disk with sizes and an open-folder action (not a live viewer), and a Settings screen for defaults (timeout, log retention) that runs honor. This ticket makes "see what my machine received, where the logs are, and tune defaults" work end to end.

**Blocked by:** 05 — Run execution in-process with winget steps and results

**Status:** done

- [x] Runs list shows timestamp, Preset(s), and overall outcome; any past Run reopens to its per-Requirement results
- [x] Logs tab shows on-disk log locations, file sizes, and an open-folder action; no in-app log viewer
- [x] Settings persist default timeout and log-retention values and runs honor them
