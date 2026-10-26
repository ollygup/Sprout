# AGENTS.md — working rules for Sprout

Read this first. These rules exist so every session (including fresh ones) builds without surprises and never damages the source of truth.

All `MUST` / `MUST NOT` below are BLOCKING. No implicit bypass — when a `WHEN` condition matches, the `MUST` applies every time. Do not skip.

## Workspace mode (before anything else)

- This clone declares its mode in one untracked file at the repo root, `.sprout-workspace`. It is machine-local: `tools/sync.ps1` excludes it from the share sync and `.gitignore` excludes it from git. It carries exactly one fact — the mode. MUST NOT put paths, settings, or any manifest in it.
- WHEN a session starts → MUST read `.sprout-workspace` first and take the matching branch before reading or editing anything else:
  - `vm`, trimmed, case-insensitive → **VM workspace**. MUST run `sync.ps1 -Down` as the first substantive action, then read `docs/adr/README.md`. Everything in `docs/agents/working-copy.md` applies: `C:\Sprout`, the share, the sync triggers, the snapshot, and the no-git rule.
  - `standalone`, trimmed, case-insensitive → **standalone**. No `-Down`/`-Up`, no share, no snapshot; normal git applies. Read `docs/adr/README.md` before planning.
  - absent, empty, or any other content → **unconfigured**. Apply the standalone rules for this session, MUST report the state, and MUST ask the user once which mode this clone is, then write that single word into `.sprout-workspace`. MUST NOT infer the mode from the path, a mounted share, or git metadata.
- WHEN the user answers `vm` → MUST follow "VM setup" in `docs/agents/working-copy.md`: run `tools/vm-setup.ps1` (read-only), show the human any remedy it prints, and write `vm` only after every check passes; then run `-Down`. MUST NOT create, move, or reparent `C:\Sprout` yourself, and MUST NOT write `vm` on a machine the check rejects.
- WHEN the user answers `standalone` → MUST write `standalone`; nothing syncs and git is normal from then on.
- WHEN the session is unattended (a spawned worker, a coordinator batch, or the user cannot answer) → proceed as standalone, say so in the report, and MUST NOT write `vm`.
- The marker describes the machine, not the repository origin: a fork is standalone or VM depending on where it runs.

## Core rules (every session — no extra read)

- WHEN working on the repo → MUST work inside this clone's own root. MUST NOT work from a UNC or network path — plain `.cmd`/`.bat` helpers (npm, cargo) fail with "UNC paths are not supported".
- MUST treat unfamiliar files or changes as potential user work: investigate before deleting, overwriting, or restructuring anything. MUST NOT restructure the repository without explicit user instruction.
- WHEN you touch an area with a recorded decision → MUST read that ADR in full. MUST NOT re-litigate a recorded decision without appending a dated `## Amendment` stating what changed and why.
- WHEN any unit of work completes AND WHEN the session ends → MUST run `node tools/ownership-gate.mjs` (ADR-0029 ownership gate) and it MUST pass before anything is published — a commit, a pull request, or a sync in a VM workspace. WHEN it fails → MUST fix the placement (extend the owner) or table the new owner. MUST NOT publish a violating tree.
- Publishing: a standalone clone uses normal git branches, commits, and pull requests. A VM workspace publishes only by syncing and MUST NOT run any git command — `docs/agents/working-copy.md` owns that rule.

## CodeGraph

In repositories indexed by CodeGraph (a `.codegraph/` directory exists at the repo root), reach for it BEFORE grep/find or reading files when you need to understand or locate code:

- **MCP tool** (when available): `codegraph_explore` answers most code questions in one call — the relevant symbols' verbatim source plus the call paths between them, including dynamic-dispatch hops grep can't follow. Name a file or symbol in the query to read its current line-numbered source. If it's listed but deferred, load it by name via tool search.
- **Shell** (always works): `codegraph explore "<symbol names or question>"` prints the same output.

If there is no `.codegraph/` directory, skip CodeGraph entirely — indexing is the user's decision.

This file is the seam: the topic index plus the every-session core above. Each file under `docs/agents/` is a module behind it — read each file whose WHEN matches your task (often more than one); skip the rest entirely.

- WHEN you read any file under `docs/agents/` → MUST announce it loudly in visible response text at the time (tool-call arguments alone do not count), e.g. `Reading docs/agents/conventions.md — Windows-ownership rules apply here`.
- WHEN you finish the task → MUST end your final response with the full list of `docs/agents/` files read this session, e.g. `Agent files read: docs/agents/conventions.md, docs/agents/structure.md` (or `none`, when the task touched no topic).

## Topic index (conditional — read the file only when its WHEN matches)

| Topic | WHEN it applies | Read |
| --- | --- | --- |
| Workspace mode | WHEN the marker is absent or invalid (unconfigured), or you set up, repair, or clear VM mode | `docs/agents/working-copy.md` |
| UI/UX | WHEN you change any UI, make any UI/UX design decision, or change any UI dimension | `docs/agents/ui-ux.md` |
| Working copy details — VM only | WHEN the marker is `vm` and Core sync isn't enough: you hit a `SHARE-NEWER` divergence, join work in progress, or need the snapshot location / robocopy-fallback detail (full section, authoritative wording) | `docs/agents/working-copy.md` |
| Build, check, test | WHEN you run, build, check, or test anything | `docs/agents/toolchain.md` |
| Releases — VM only | WHEN you cut a release, bump the version, or build a local installer | `docs/agents/releases.md` |
| Cleanup | WHEN told to do a cleanup, or automatically after any local `tauri build` | `docs/agents/cleanup.md` |
| DB verification | WHEN you inspect the Library DB or reason about first-run state | `docs/agents/verification.md` |
| File/module layout | WHEN you touch files and need owners, or work tickets (AC tracking rule lives here) | `docs/agents/structure.md` |
| Rust + Tauri + Svelte conventions | WHEN you touch Rust/Tauri/Svelte, constants, version, window sizing, module boundaries, `shared/`, or any Windows invocation | `docs/agents/conventions.md` |
| Code comments | WHEN you write or edit code comments | `docs/agents/comments.md` |
| Parallel tickets — VM only | WHEN you run 2+ tickets concurrently in one session as a batch | `docs/agents/parallel-tickets.md` |
| Planning and ticket slicing | WHEN you grill with docs, create/revise a spec, or split work into tickets | `docs/agents/planning.md` |
| Worker session — VM only | WHEN you were spawned as a ticket worker under a coordinator in a parallel batch | `docs/agents/worker.md` |
