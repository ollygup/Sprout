# Parallel tickets — agent reference

> Read this file when: you run 2+ tickets concurrently in one session as a
> batch — i.e. you are the coordinator. Otherwise skip it: single-ticket
> sessions use AGENTS.md Core directly, and ticket workers read
> `docs/agents/worker.md`, not this file.

Why this exists: `tools\sync.ps1` keeps a single-writer snapshot
(`.sync-state.json`) and uses absolute paths — a different working directory
does NOT isolate callers. Two agents running `-Down`/`-Up` clobber each
other's snapshot and `-Up` guard. So a batch has exactly one agent that
syncs — the coordinator — and ticket workers that never sync. Worker rules
live in `worker.md`; this file MUST NOT restate them (single source of
truth, or the two drift).

## Coordinator protocol (ordered, blocking)

1. `-Down` first, then `docs/adr/README.md` + relevant ADRs in full (Core,
   unchanged). The `-Down` snapshot is the batch baseline.
2. Decide your own ticket versus fanned-out tickets. WHEN you also implement
   one ticket yourself → finish your own code BEFORE fanning out (workers
   branch from a baseline that includes it) or AFTER applying all worker
   returns — never interleaved with the apply step.
3. Pre-flight overlap check over writer tickets: derive each ticket's planned
   file set + owner symbols (`codegraph_explore`) and require strict disjoint
   (definition below). Read-only jobs (research, review, single grill rounds)
   are exempt from disjoint — readers cannot clobber — but note the ordering
   hazard when they share ground with a writer: a landing patch can invalidate
   their conclusions, so run the reader first or pin it to the frozen
   baseline. Overlap → re-scope the tickets, serialize them into separate
   batches, or carve the shared file out as a small coordinator-owned merge
   task. MUST NOT launch overlapping writers in parallel.
4. Create one workspace per writer ticket: a temp copy under a local (never
   UNC) temp dir, e.g.
   `C:\Users\admin\AppData\Local\Temp\opencode\batch-<date>\ticket-<id>\`,
   copying source only and excluding `node_modules target build .svelte-kit
   .vscode .codegraph .git`; give each copy its own `CARGO_TARGET_DIR` so
   concurrent `cargo` runs never share a target lock. The principle is
   single-writer to the snapshot and the share; temp copies are the default
   mechanism. Shared-`C:\Sprout` with enforced disjoint claims is allowed but
   fragile — an allow-list violation there corrupts siblings silently.
5. Fan out. The prompt carries exactly five things, then stops — the worker
   reads `C:\Sprout\AGENTS.md` from there and self-directs (own topic reads,
   own skills, own checking):
   1. role designation ("you are a ticket worker under a coordinator —
      `parallel-tickets.md` worker rules apply; Core sync duties are mine,
      not yours"),
   2. the job + ticket (paste the ticket text inline; the copy may be stale),
   3. the workspace path,
   4. the file/owner allow-list,
   5. which return artifact you expect (patch, findings text, grill
      questions — the job's nature decides).
   The designation line is blocking: MUST NOT fan out without it. An
   undesignated worker defaults to a full session whose FIRST action is
   `-Down`, which clobbers your snapshot.
6. Wait for every worker's return before staging anything. Workers report to
   you only; they MUST NOT talk to each other or to the share.
7. Post-flight disjoint check on the actual touched sets. Any intersection →
   MUST NOT stage both sides; keep the non-conflicting tickets and return the
   conflicting ticket to a later batch rebased on a fresh `-Down`. Never
   last-writer-wins.
8. Merge into a STAGING copy (fresh baseline copy + all writer patches in
   ticket order) — never directly into `C:\Sprout`. Git is forbidden here so
   there is no revert: `-Down` is add/update-only and never removes
   worker-added files, so a failed direct apply leaves unrecoverable residue.
   `C:\Sprout` stays clean until the batch is proven. Run full validation on
   staging: toolchain checks (`toolchain.md`) plus
   `node tools/ownership-gate.mjs`, which MUST pass. Gate failure → fix the
   placement or drop the offending ticket; MUST NOT publish a violating tree.
9. Union-copy green staging onto `C:\Sprout`. Mark ACs done as applied, each
   ticket only in its own issue file.
10. `-Up` once, then `-Up` again expecting `0 copied`. `SHARE-NEWER` →
    resolve explicitly per `working-copy.md`; never raw robocopy.

## Overlap — strict disjoint (v1)

- **Disjoint** = empty intersection of touched relative paths AND no shared
  ADR-0029 owner module (`winget/`, `windows_execution/`,
  `engine/windows/inspection.rs`, `appbar/display.rs`) AND no shared
  single-source file (`constants/window.rs`, the `Cargo.toml`-only version,
  the `lib/api.ts` + `lib/types.ts` seam, design tokens, the
  `tools/ownership-gate.mjs` `OWNERS` table plus the operation inventory —
  two tickets adding a genuinely new Windows operation collide there).
  Same file, different lines still counts as overlap.
- Pre-flight overlap → refuse parallel launch (re-scope / serialize /
  coordinator-owned merge). Post-flight overlap → exclude the conflicting
  ticket for a later batch. Line-level merging is out of scope for v1.

## Failure handling

- Worker failure or timeout → exclude that ticket's files, continue the rest.
- Coordinator without a snapshot → MUST refuse `-Up` and start with `-Down`.
- Tip, not a rule: interactive, user-facing interview jobs (`grilling`, live
  `grill-with-docs` rounds) default to the coordinator — Task relay is lossy
  for back-and-forth. Backgroundable jobs (implement, research, review,
  diagnosis write-ups, synthesis) fan out.
