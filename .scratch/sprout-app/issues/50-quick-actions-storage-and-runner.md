# 50 — Quick Actions: storage and runner

**What to build:** A new machine-local Quick Actions list the app can store, query, and run — name + PowerShell command + optional working directory — with CRUD commands, a fire-and-forget hidden runner (current user, no elevation), and a timeboxed Test command. This is the backend half of the Quick Actions tab; no UI in this ticket.

**Blocked by:** None — can start immediately.

**Status:** done

- [x] New `quick_actions` table (id, name, command, cwd nullable, position) via the existing db migration pattern; migration runs on existing databases
- [x] Validation rejects blank names and blank commands; position ordering enforced; illegal values never saved
- [x] CRUD commands mirroring the Launch entry commands: list, create, update, delete, move (reorder) — roundtrip across a reopened database
- [x] Run command: executes the action's command via PowerShell (`-NoProfile -NonInteractive -Command`) hidden (`CREATE_NO_WINDOW`), working directory honored when set, fire-and-forget on a background thread, current user, no elevation, no status UI, no notification
- [x] Test command: timeboxed run (prior art: the Launch entry Test button) reporting exit code and output; interactive commands honestly reported as not headless-verifiable
- [x] Settings additions validated (dock mode, default edge) with the existing Settings pattern
- [x] `cargo test` green; `npm run check` 0 errors; synced to the share