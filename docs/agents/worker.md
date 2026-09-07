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
- Use the assigned immutable baseline for implementation and review claims;
  live `C:\Sprout` reads are orientation only. Do not silently import another
  ticket's in-progress changes. Stay inside the path/owner allow-list; report
  newly needed shared edits to the coordinator before writing outside it.
  In a shared working tree, stop affected edits until ownership is resolved.
- Reuse the coordinator's discovery brief against your assigned baseline.
  Inspect relevant source and verify assumptions; broaden discovery for a
  specific gap, contradiction, or implementation need. Do not repeat shared
  architecture searches or re-plan the whole batch. Required instruction and
  ADR reads still apply. Report material deviations from the brief.
- Own the ticket's relevant test coverage, reproduction steps, and failure
  fixes. Run only assigned lightweight checks locally. Do not automatically
  install dependencies, compile, run project-wide checks, start watchers/dev
  apps, or create a separate build environment. A narrow Rust test may still
  require a heavy build. Queue heavy checks with exact commands and expected
  outcomes; the coordinator runs them against an identified candidate in
  `C:\Sprout`, including the ownership gate. Needed early runtime feedback or
  an isolated runtime exception goes through the coordinator's heavy slot.
  Never write into or borrow writable caches from `C:\Sprout` yourself.
- CodeGraph indexes `C:\Sprout`, not your workspace: use it for orientation,
  but re-read directly any file you modified — never trust the index there.
- Hand results back to the coordinator (touched repo-relative paths/symbols,
  contents or diffs, acceptance evidence, checks run and explicitly pending,
  proposed AC lines) instead of publishing. Link full artifacts/logs and keep
  the message compact; do not narrate the investigation again. Your
  job's natural artifact decides the shape (patch, findings text, grill
  questions) — the coordinator tells you which it expects.
- Include baseline ID, baseline/result hashes for changed files, explicit
  additions/deletions, shared-contract changes, and unresolved dependencies.
  Freeze the returned artifact; subsequent revisions need a new identified
  return. AC changes are proposals until the coordinator validates and applies
  them. A return is implemented, awaiting validation, not a claim that checks
  passed. Remain available for diagnostics and repairs; use the coordinator's
  tested candidate ID and reproduction evidence, and return an identified
  revision without changing the frozen artifact. Report coordination needs
  to the coordinator, not sibling workers.
