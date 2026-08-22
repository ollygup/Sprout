# Log File Organization — Per-Run vs Daily Files — Research Notes

Findings gathered while reviewing ticket 64's per-run Quick Action log folders
(`qa-<millis>-<id>`) against the critique that millisecond folder names don't
help a human find "the failed docker-start from yesterday". High-trust primary
sources only.

## The two models, and who uses which

Every system examined picks one of two shapes:

- **Per-invocation artifacts** — one file (or folder) per run/command/session,
  chosen when each invocation is an isolated incident worth inspecting or
  attaching alone. Used by winget, VS Code, GitHub Actions/GitLab CI, npm/pip
  debug logs.
- **Rotating streams** — one continuous log chopped by size or time, chosen
  when the content is a homogeneous stream where incidents are located by
  querying inside the stream. Used by JetBrains IDEs, logrotate-managed
  services, the Windows Event Log (.evtx per channel), and the systemd
  journal ("query, don't browse filenames": `journalctl -u unit --since`).

A "one daily file per type" scheme is the rolling-stream model with a time
chop. No desktop tool examined uses it for per-invocation work: the moment two
runs overlap, their outputs interleave in one file, and isolating one failed
run means grepping a shared file instead of opening a folder.

Consequence: Sprout's runs are discrete user-triggered incidents (install this
preset, run that action) — the per-invocation model fits; the daily-file model
fits services, not command runs.

Sources: learn.microsoft.com winget troubleshooting/settings (per-process log
files); code.visualstudio.com / vscode issue #39572 (one logging session per
process launch); jetbrains.com "Locating IDE log files" (rolling idea.log);
man7.org logrotate(8); freedesktop.org journalctl man page; 12factor.net/logs
(logs are event streams; the app should not manage its own aggregation);
docs.github.com Actions logs (run → job → step hierarchy).

## Names carry human-readable LOCAL time, never raw epoch millis

The strongest convergence across every per-invocation system:

- **winget**: one `.log` per process invocation named with the local date and
  time (`WinGet-<yyyy>-<MM>-<dd>-<HH>-<mm>-<ss>.log` shape) in
  `DiagOutputDir`; cleanup deletes files older than `ageLimitInDays` (default
  7) and over a total-size cap. Installer logs add a `fileNameStrategy`
  setting (manifest name / timestamp / guid) — an explicit acknowledgment
  that the NAME is how users pick the right file.
- **VS Code**: one folder per session named `YYYYMMDDTHHMMSS` (local time,
  ISO-basic, lexically sortable), containing per-category files
  (`main.log`, `window1\exthost.log`, …); keeps at most ~10 sessions and
  prunes older ones at startup.
- **logrotate `dateext`**: rotated files get a `-YYYYMMDD` extension, and the
  man page requires the format be **lexically sortable** ("first the year,
  then the month then the day") because sorting by filename is how age is
  determined.
- **tauri-plugin-log** (Sprout's own framework): `RotationStrategy::KeepAll`
  "renames [rotated logs] to include the date" — same convention in the
  ecosystem Sprout ships in.

Epoch millis sort correctly but are unreadable; `YYYYMMDD-HHMMSS` sorts
identically AND reads as a date. There is no trade-off — millis buy nothing
once a fixed-width local timestamp is used.

Consequence: rename the per-run folders to embed local `YYYYMMDD-HHMMSS`
(plus, for Quick Actions, the action's name) instead of epoch millis. Sorting,
pruning-by-name, and human scanning all keep working; searching becomes
possible ("docker-start", "0820").

Sources: learn.microsoft.com winget troubleshooting + github.com/microsoft/winget-cli doc/Settings.md (fileNameStrategy, ageLimitInDays);
github.com/microsoft/vscode issue #49302 + codegenes VS Code logs overview
(session folder `20180506T185427` shape); man7.org logrotate(8) dateext/dateformat;
docs.rs tauri-plugin-log RotationStrategy.

## Retention converges on age-based pruning at process start

winget (7 days default, checked at the start of each process), VS Code (~10
sessions, pruned 10 s after startup), CI platforms (retention days per
project). Size caps are secondary. Sprout's existing `log_retention_days`
pruning at app start matches the pattern exactly; only the naming needs to
change.

Consequence: keep the retention design; nothing about readable names weakens
the millis-in-name age check, since the embedded timestamp remains parseable
(fixed-width, sortable).

Sources: learn.microsoft.com winget settings (file.ageLimitInDays, cleanup at
process start); vscode issue #39572 (session cap); docs.github.com Actions
log retention.

## Recommendation for Sprout

Keep **per-run folders** for both families (incident isolation, attach-to-bug-report,
same-platform precedent in winget and VS Code); reject one-daily-file-per-type
(interleaved runs destroy isolation, and no examined desktop tool organizes
per-invocation work that way). Fix the actual discoverability gap by renaming:

- Quick Actions: `qa-<YYYYMMDD>-<HHMMSS>-<action-name>` (e.g.
  `qa-20260820-140311-docker-start`) — action name sanitized for the
  filesystem (`\ / : * ? " < > |` stripped, length-capped), collision suffix
  when the same second repeats.
- Preset runs: `run-<YYYYMMDD>-<HHMMSS>` (optionally + preset name), same rules.
- Keep the embedded timestamp as the pruning age marker (fixed-width, lexical
  order = chronological order, satisfying logrotate's stated requirement);
  mtime stays the fallback.
- The Logs screen lists newest-first today, so readable names + that list
  deliver "find yesterday's failed docker-start" without any new UI.

If a future need for date-scoped bulk browsing appears, day-level FOLDERS
(`quick-actions\2026-08-20\…`) remain additive — but the evidence says
readable per-run names solve the stated problem first.
