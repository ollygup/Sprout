# 41 — Custom command entries with a Test button

**What to build:** Advanced users add launch entries that are full shell commands (PowerShell, cmd, or a direct exe) instead of picked apps — for obscure software or startup sequences nothing else can express. Before saving, a Test button runs the command and reports exit code and output so the user can verify it starts as intended. Parent spec: 37.

**Blocked by:** 38 — Launch entries: persistence, page, and reorder

**Status:** done

- [x] Add-command dialog: shell select (PowerShell / cmd / direct exe), command input, show-window toggle (hidden by default — CREATE_NO_WINDOW convention), name auto-filled from the command
- [x] `test_launch_command` command: runs the command timeboxed via the existing `run_timed_process`, returns exit code + captured output; an interactive command that outlives the box is reported honestly as not headless-verifiable (timed out), never as passed
- [x] Test runs from the dialog before save, results shown inline (exit code, output, timeout note)
- [x] Saved command entries persist in the same list with kind badge `command` and shell shown
- [x] `cargo test` green (exit-code/output/timeout behavior), `npm run check` 0 errors; synced to the share