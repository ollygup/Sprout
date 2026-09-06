# Working copy and sync — agent reference

> Read this file when AGENTS.md Core sync isn't enough: you hit a
> `SHARE-NEWER` divergence, join work already in progress (snapshot check),
> or need the snapshot location / raw-robocopy fallback. Routine session
> boundaries (-Down first, -Up at close, ADR read, ownership gate) live in
> AGENTS.md Core so a session can start with zero extra reads; the wording
> here is authoritative — keep both identical.

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
