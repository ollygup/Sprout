# VM workspace and sync — agent reference

> Read this file when: `.sprout-workspace` contains `vm` (it then applies to
> every session), or you are setting up, repairing, or clearing VM mode. A
> standalone clone never reads it. This file owns the VM rules — the working
> copy, the share, the sync triggers, the no-git rule, the snapshot, and the
> robocopy fallback. `AGENTS.md` only points here; do not duplicate this text
> there.

## Workspace marker

- `.sprout-workspace` at the repo root is the only mode switch. Its entire
  content is one word: `vm` or `standalone` (trimmed, case-insensitive).
  Absent or any other content means unconfigured: standalone rules apply for
  the session, the agent reports it, and the user chooses once.
- The marker is machine-local. `tools/sync.ps1` excludes it and `.gitignore`
  ignores it, so it must never reach the share or git.
- Clearing VM mode means deleting the marker; the clone returns to standalone.
  That touches neither the database, the logs, the working tree, nor the share.

## VM setup

Setup verifies this machine; it does not configure it. Nothing here creates,
moves, or reparents a directory automatically, and there is no apply mode.

1. Run the read-only check:
   `powershell.exe -NoProfile -ExecutionPolicy Bypass -File "tools\vm-setup.ps1"`
   It verifies the working copy is exactly `C:\Sprout`, the share is
   reachable, `tools\sync.ps1` and `tools\ownership-gate.mjs` exist, and
   `node`, `npm.cmd`, and `cargo` are on PATH.
2. When the repo is not at `C:\Sprout`, VM mode is unavailable there. Show the
   human the remedies the check prints and let them choose:
   - keep the checkout where it is: create `C:\Sprout` as a directory junction
     (needs no elevation; builds work through it);
   - or move the checkout to `C:\Sprout` for real.
   When `C:\Sprout` already exists, stop — it must be inspected by hand first.
   Removing a junction with `Remove-Item -Recurse` can delete the tree it
   points at; the safe removal is `cmd /c rmdir "C:\Sprout"`.
3. When `C:\Sprout` does not exist at all, `-Down` bootstraps it: it creates
   each copied file's parent directory, so a missing working copy is filled
   from the share in one pass. This is the only supported path setup, and it
   still requires the user's `vm` answer first — never run it to "try" VM mode.
4. Only after every check passes: write `vm` as the marker's entire content.
5. Run `-Down` (the normal session-start refresh; triggers below). If it
   fails, delete the marker, report the failure, and leave the clone
   standalone — a half-enabled VM workspace is worse than none.
6. Verify: the marker is not on the share, a second `-Up` reports `0 copied`,
   and the working copy still builds.

Re-running setup is idempotent — it re-checks and reports; it does not rewrite
the marker or run a destructive sync. Invalid marker content is reported, not
silently overwritten.

## Standalone (no marker, or `standalone`)

None of this file applies: no `-Down`/`-Up`, no share, no snapshot; normal git
is expected. The ownership gate still runs before a commit or pull request,
and every universal rule in `AGENTS.md` still applies.

## The VM topology

- The repo has two homes:
  - **Master (source of truth, fallback):** `\\vmware-host\Shared Folders\Projects\Sprout`
  - **Working copy (develop here):** `C:\Sprout`
- WHEN working on the repo → MUST work in `C:\Sprout`. MUST NOT work directly on the share — UNC paths break `.cmd`/`.bat` (npm, cargo helpers) — builds fail with "UNC paths are not supported".
- MUST NOT delete or restructure anything on the share — the share is the fallback if the working copy messes up.
- Git is handled externally — MUST NOT run ANY git command here (STRICT). Git on this project belongs to the user, outside this device: MUST NOT run `git init`, `clone`, `add`, `commit`, `push`, `stash`, or ANY other git command against any path under `C:\Sprout`, ever. A `.git` directory may exist under `C:\Sprout` because it rides along with the share sync — that is fine; MUST treat it as inert data: MUST NOT create, modify, delete, or act on it. All version-control state lives elsewhere; changes made here are published by syncing (`tools\sync.ps1`), not by committing.
- WHEN syncing → MUST use `tools\sync.ps1`, MUST NOT use raw `robocopy`. The share's git working tree is owned by the other device (the only git client); a blind robocopy overwrites whatever it committed and produces merge conflicts. The script snapshots the share's content hashes at session start and refuses to overwrite any file the other device changed mid-session — divergences are reported as `SHARE-NEWER` for explicit resolution:

```powershell
# session start (refreshes C:\Sprout from the share, then snapshots it)
tools\sync.ps1 -Down
# session end (copies only what we changed, guarded by the snapshot)
tools\sync.ps1 -Up
```

- Sync triggers are blocking, not background knowledge:
  - WHEN any working session starts → MUST run `-Down` as the FIRST action, before reading or editing anything.
  - WHEN local edits exist and are not yet published → MUST run `-Up` BEFORE any `-Down`. `-Down` treats the share as newer and overwrites a differing local file, so a mid-session `-Down` silently reverts unpublished work; only the session-start `-Down` (which precedes edits) is safe.
  - WHEN any working session starts → MUST read `docs/adr/README.md` (the one-page decision index) after `-Down`, before planning. MUST then read in full any ADR whose area the task touches. MUST NOT re-litigate a recorded decision without appending a dated `## Amendment` stating what changed and why.
  - WHEN any unit of work completes (published tickets/spec/docs, a landed code change) AND WHEN the session ends → MUST run `-Up` — MUST NOT batch everything into one end-of-day sync.
  - WHEN any unit of work completes AND WHEN the session ends → MUST run `node tools/ownership-gate.mjs` (ADR-0029 ownership gate) and it MUST pass BEFORE running `-Up`. WHEN it fails → MUST fix the placement (extend the owner) or table the new owner — MUST NOT sync a violating tree.
  - WHEN you join work already in progress and no fresh snapshot exists for this session → MUST first check for a snapshot from earlier the same session: WHEN such a snapshot exists → MUST sync `-Up` first; WHEN no snapshot exists → MUST back up local edits before any `-Down` (it overwrites differing local files).
- WHEN invoking the sync script → MUST invoke as `powershell.exe -NoProfile -ExecutionPolicy Bypass -File "tools\sync.ps1" -Up` (or `-Down`) — PowerShell's execution policy blocks `.ps1` directly (same reason npm is `npm.cmd`).
- WHEN you have run `-Up` → MUST verify the sync by running `-Up` again — MUST expect `0 copied` when in sync. MUST NOT run `-Up` without a snapshot; the script refuses.
- The snapshot lives in `C:\Sprout\.sync-state.json` (excluded from the sync itself). WHEN you must fall back to raw robocopy → MUST add `/XF .sync-state.json` to the command below — and know that it silently clobbers newer share content:

```powershell
robocopy "C:\Sprout" "\\vmware-host\Shared Folders\Projects\Sprout" /E /R:1 /W:1 /NFL /NDL /NJH /NP /XD node_modules target build .svelte-kit .vscode .codegraph /XF .sync-state.json
```
