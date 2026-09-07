# AGENTS.md — working rules for Sprout

Read this first. These rules exist so every session (including fresh ones) builds without surprises and never damages the source of truth.

All `MUST` / `MUST NOT` below are BLOCKING. No implicit bypass — when a `WHEN` condition matches, the `MUST` applies every time. Do not skip.

## CodeGraph

- WHEN you need to understand or locate code and the repository is indexed by CodeGraph (a `.codegraph/` directory exists at the repo root) → MUST reach for CodeGraph BEFORE `grep`/`find` or reading files:
  - **MCP tool** (when available): `codegraph_explore` answers most code questions in one call — the relevant symbols' verbatim source plus the call paths between them, including dynamic-dispatch hops grep can't follow. Name a file or symbol in the query to read its current line-numbered source. If it's listed but deferred, load it by name via tool search.
  - **Shell** (always works): `codegraph explore "<symbol names or question>"` prints the same output.
- WHEN there is no `.codegraph/` directory → MUST skip CodeGraph entirely — indexing is the user's decision.

This file is the seam: the topic index plus the every-session core below. Each file under `docs/agents/` is a module behind it — read each file whose WHEN matches your task (often more than one); skip the rest entirely.

- WHEN you read any file under `docs/agents/` → MUST announce it loudly in visible response text at the time (tool-call arguments alone do not count), e.g. `Reading docs/agents/conventions.md — Windows-ownership rules apply here`.
- WHEN you finish the task → MUST end your final response with the full list of `docs/agents/` files read this session, e.g. `Agent files read: docs/agents/conventions.md, docs/agents/structure.md` (or `none`, when the task touched no topic).

## Core rules (every session — no extra read)

- WHEN working on the repo → MUST work in `C:\Sprout`, except coordinator-assigned local isolated writer/staging copies under `docs/agents/parallel-tickets.md`. That exception applies to the coordinator's own ticket too; sync/publish commands still run only from `C:\Sprout`. MUST NOT work directly on the share — UNC paths break `.cmd`/`.bat` (npm, cargo helpers) — builds fail with "UNC paths are not supported".
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
- WHEN you run 2+ tickets concurrently in one session as a batch → MUST follow `docs/agents/parallel-tickets.md`: exactly one coordinator runs `-Down`/`-Up` and the ownership gate; ticket workers MUST NOT sync. MUST NOT run parallel tickets as independent sessions sharing one working copy.
- WHEN you were spawned as a ticket worker under a coordinator in a parallel batch (your spawning prompt designates you as one) → the Core sync and publish duties DO NOT apply to you: session-start `-Down`, per-unit/session-end `-Up` and gate-before-`-Up`, and any contact with the share UNC, `.sync-state.json`, or git. Any future MUST whose action publishes state falls inside this exemption automatically. Everything else in this file and its topic modules applies unchanged — topic-index WHEN reads are yours to decide, skills are yours to choose. MUST read `docs/agents/worker.md` and follow it.

Full working-copy detail (two homes, snapshot location, divergence handling, robocopy fallback) lives in `docs/agents/working-copy.md` — read it when Core sync isn't enough.

## Topic index (conditional — read the file only when its WHEN matches)

| Topic | WHEN it applies | Read |
| --- | --- | --- |
| UI/UX | WHEN you change any UI, make any UI/UX design decision, or change any UI dimension | `docs/agents/ui-ux.md` |
| Working copy details | WHEN Core sync isn't enough: you hit a `SHARE-NEWER` divergence, join work in progress, or need the snapshot location / robocopy-fallback detail (full section, authoritative wording) | `docs/agents/working-copy.md` |
| Build, check, test | WHEN you run, build, check, or test anything | `docs/agents/toolchain.md` |
| Releases | WHEN you cut a release, bump the version, or build a local installer | `docs/agents/releases.md` |
| Cleanup | WHEN told to do a cleanup, or automatically after any local `tauri build` | `docs/agents/cleanup.md` |
| DB verification | WHEN you inspect the Library DB or reason about first-run state | `docs/agents/verification.md` |
| File/module layout | WHEN you touch files and need owners, or work tickets (AC tracking rule lives here) | `docs/agents/structure.md` |
| Rust + Tauri + Svelte conventions | WHEN you touch Rust/Tauri/Svelte, constants, version, window sizing, module boundaries, `shared/`, or any Windows invocation | `docs/agents/conventions.md` |
| Code comments | WHEN you write or edit code comments | `docs/agents/comments.md` |
| Parallel tickets | WHEN you run 2+ tickets concurrently in one session as a batch | `docs/agents/parallel-tickets.md` |
| Planning and ticket slicing | WHEN you grill with docs, create/revise a spec, or split work into tickets | `docs/agents/planning.md` |
| Worker session | WHEN you were spawned as a ticket worker under a coordinator in a parallel batch | `docs/agents/worker.md` |
