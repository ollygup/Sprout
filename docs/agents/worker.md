# Worker session — agent reference

> Read this file when: you were spawned as a ticket worker under a
> coordinator in a parallel batch. Otherwise skip it — normal sessions use
> AGENTS.md Core directly, and the coordinator protocol lives in
> `docs/agents/parallel-tickets.md`, which is not your concern. Do not
> coordinate: report to your coordinator only.

## Recognition

- You are a ticket worker if and only if your spawning prompt designates you
  as one (it names your job, your workspace, and your coordinator). WHEN that
  designation is present → this file applies and the AGENTS.md worker
  exemption fires. WHEN it is absent → you are a normal session: Core sync
  binds you fully, starting with `-Down` as your FIRST action.

## Does not apply to you

- Any MUST whose action publishes state: `tools\sync.ps1` (`-Down`/`-Up`),
  the ownership gate as a publish precondition, and any contact with the
  share UNC, `.sync-state.json`, or git. `sync.ps1` uses absolute paths — a
  different working directory does NOT isolate you. Never invoke it.
- Terminal publish steps inside skills: a skill's commit/push/save-to-repo
  finale becomes a return-artifact instead. `implement` returns the patch
  rather than committing (committing is banned repo-wide anyway); `research`
  returns the findings-doc text rather than saving it under `research/`.

## Still binds you — you are a normal agent otherwise

- Everything else in AGENTS.md and its topic modules. Topic-index WHEN reads
  are yours to decide, skills are yours to choose — start from AGENTS.md and
  proceed as a normal session.
- `docs/adr/README.md` plus any ADR your area touches (no `-Down` needed —
  the coordinator already baselined your workspace).
- ADR-0029 ownership: extend the owner, never a second invocation site.
- AC updates only for your own ticket file.

## Workspace and handoff

- Work where the coordinator put you; write only there. Reads from anywhere
  (including `C:\Sprout` by absolute path) are allowed.
- Run toolchain checks and the ownership gate as checks on your own tree,
  not as publish preconditions.
- CodeGraph indexes `C:\Sprout`, not your workspace: use it for orientation,
  but re-read directly any file you modified — never trust the index there.
- Hand results back to the coordinator (touched repo-relative paths,
  contents or diffs, test evidence, AC lines) instead of publishing. Your
  job's natural artifact decides the shape (patch, findings text, grill
  questions) — the coordinator tells you which it expects.
