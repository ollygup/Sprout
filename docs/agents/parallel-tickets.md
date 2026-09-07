# Parallel tickets — agent reference

> Read this file when: you coordinate 2+ concurrent tickets in one batch.
> Workers read `worker.md`; planning and ticket slicing use `planning.md`.

`tools\sync.ps1` owns one absolute-path snapshot. Exactly one coordinator
syncs and publishes. Implementation runs concurrently; integration has one
writer. Worker obligations live in `worker.md` rather than being duplicated here.

## Coordinator protocol (ordered, blocking)

1. Run Core `-Down`, then read the ADR index and relevant ADRs. Preserve an
   immutable source baseline plus relative-path content hashes (including
   absence for new files) outside the synced tree. Give it a batch ID.
2. Plan the whole batch, INCLUDING your own ticket, before implementation.
   Record dependencies, planned paths/owner symbols, shared contracts,
   workspace paths, and integration order in a batch record under
   `.scratch/sprout-app/batches/<batch-id>.md`. Use CodeGraph for code
   ownership when indexed; otherwise inspect source directly. Select all
   dependency-ready tickets for the first wave. A coordinator-owned ticket
   MUST NOT delay dispatch of other ready tickets.
3. Resolve expected overlap using the rules below. Prefer disjoint work;
   file overlap alone is not a behavioral dependency. Pin any shared
   interface before parallel consumers start. Unresolved behavior or a
   prerequisite implementation remains a real blocker, not a merge task.
4. Create an isolated local source copy from the same immutable baseline
   for EACH writer, including yourself, and another for staging. Use a
   writable local temp directory outside the synced tree; this is the Core
   working-directory exception. Exclude `.git`, `.sync-state.json`,
   `.codegraph`, `node_modules`, `target`, `build`, and `.svelte-kit`;
   do not copy dependency/build caches or link writable source across copies.
   Install needed dependencies per toolchain rules and give concurrent Cargo
   jobs distinct `CARGO_TARGET_DIR` values. Check tooling actually targets the
   assigned copy. If isolation is unavailable, shared `C:\Sprout` is permitted
   ONLY with strict disjoint path AND owner claims, including your own work.
   Never run overlapping writers in that shared tree.
5. Dispatch ready workers up to available concurrency BEFORE starting your
   own implementation; queue remaining ready jobs and dispatch as slots free.
   Reserve capacity for yourself when taking a ticket. Each
   prompt MUST designate "ticket worker under this coordinator; Core sync
   and publish duties belong to the coordinator", and provide the current
   ticket text, workspace, baseline ID, path/owner allow-list, dependencies
   and pinned contracts, plus expected return artifact. Point to `AGENTS.md`
   and `worker.md`. Workers otherwise choose their own relevant reads,
   skills, and checks. An undesignated worker would start a destructive
   second sync session. While workers run, implement your own ticket in its
   assigned copy; do not wait for their returns first.
6. Collect completed returns and freeze each returned artifact before
   integration. You may integrate a dependency-ready return while other
   workers continue in their isolated copies. Your own ticket gets the same
   handoff/checks as every other ticket; never edit its copy while applying
   its return. In shared-tree mode, wait until all writers stop, then capture
   their results before staging. Record actual paths, baseline/result hashes,
   checks, proposed AC updates, and any unexpected overlaps in the batch record.
7. Reconcile in staging against each return's exact baseline. Apply only
   changed paths, not whole worker trees. For overlapping files, compare
   baseline, worker result, and current staging (three-way comparison without
   Git). Preserve both compatible changes; record ticket IDs, path/symbol,
   competing intent, chosen result, and validation in the conflict ledger.
   No silent last-writer-wins. Textually clean changes still need semantic
   review of shared contracts and owners. Unresolved conflicts block only
   affected tickets and their dependents; retain their artifacts for rework.
8. Validate the combined staging tree using relevant `toolchain.md` checks
   and the ownership gate. A worker's passing tests do not validate the
   combination. Re-run affected checks after any merge correction. Failed
   tickets and dependents are repaired or excluded by rebuilding staging
   from the baseline plus retained returns, never by guessing an inverse edit.
9. Before publishing, verify each destination path in `C:\Sprout` still
   matches its expected baseline, last published result, or captured
   shared-tree result, as recorded for that path. Unexpected
   local changes require reconciliation too. Copy only the validated changed
   paths from staging, explicitly accounting for additions and deletions;
   never union-copy a whole tree over unrelated work. Share deletions remain
   forbidden by Core: defer changes requiring them unless the user resolves
   that constraint. Mark ACs done only for applied, validated tickets, each
   in its own issue file, and update the batch ledger. Run
   `node tools/ownership-gate.mjs` in `C:\Sprout` before publication.
10. Publish each completed integration unit with Core `-Up`, then `-Up`
    again expecting `0 copied`; do not defer a completed unit to session end.
    Outstanding isolated workers keep their original baselines: publishing
    does not rebase their copies. Reconcile later returns against current
    staging. New dependent waves start from a named validated integrated
    baseline. `SHARE-NEWER` follows `working-copy.md`; never run a fresh
    `-Down` over active work or use raw robocopy.

## Overlap and integration ownership

- Prefer empty intersections of paths and ADR-0029 owner modules. Shared
  owners, API/types seams, constants, design tokens, version files, and the
  ownership inventory/gate are integration hotspots even across different
  files. Do not create duplicate owners just to make tickets look disjoint.
- For predictable small shared edits, designate one integration owner and
  let other tickets return requested additions against a pinned contract.
  For substantial compatible edits, isolated writers may touch the same file
  when the batch record names their symbols/intent, shared contract, merge
  owner, order, and combined checks BEFORE dispatch. Same file/different
  lines needs this plan too; it is not automatically safe.
- Unexpected overlap is a merge review, not automatic ticket rejection.
  Stop affected shared-tree writers immediately and preserve current contents
  and available baselines before repair. Lost overwritten work cannot be
  recovered merely by knowing the file names; use isolation for planned overlap.
- Read-only research/review/grill work may run concurrently against an
  immutable baseline. Record that baseline and recheck conclusions affected
  by implementation changes before using them. Interactive grills normally
  stay with the coordinator so user answers are not lost through relay.

## Batch record and failure handling

The batch record MUST contain a table of ticket, dependencies, baseline,
workspace, claimed paths/owners, shared contract/integration owner, actual
changes, validation, and state (`running`, `returned`, `integrated`,
`blocked`, or `published`). Keep a conflict ledger beneath it; write `none`
when there are no conflicts. Link retained baseline/return artifacts and
their hashes so another session can identify exactly what remains to merge.
Preserve artifacts until publication is verified and deferred work is handed off.

- Failure/timeout: retain the artifact and exclude incomplete work plus its
  dependents; continue independent tickets. Never apply a still-changing copy.
- No snapshot: refuse `-Up`; use Core recovery rules before any `-Down`.
- Example: if 139 and 141 are dependency-ready and disjoint, dispatch worker
  141, then implement coordinator ticket 139 concurrently. Integrate either
  completed return when ready; 141 does not wait for 139's implementation.
