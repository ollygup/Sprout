# Parallel tickets — agent reference

> Read this file when: you coordinate 2+ concurrent tickets in one batch.
> Workers read `worker.md`; planning and ticket slicing use `planning.md`.

`tools\sync.ps1` owns one absolute-path snapshot. Exactly one coordinator
syncs and publishes. Implementation runs concurrently; integration has one
writer. Heavy validation uses the existing build environment in `C:\Sprout`.
Worker obligations live in `worker.md` rather than being duplicated here.

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
   MUST NOT delay dispatch of other ready tickets. Investigate shared owners
   and contracts once; leave ticket-specific implementation discovery to its
   owner. Prefer fewer workers for small or tightly coupled tickets; available
   agent slots are a ceiling, not a target.
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
   These are source copies: do not install dependencies or build each copy
   by default. Staging is for reconciliation; checks run under the validation
   policy below. Check editing tools actually target the assigned copy.
   If isolation is unavailable, shared `C:\Sprout` is permitted
   ONLY with strict disjoint path AND owner claims, including your own work.
   Never run overlapping writers in that shared tree.
5. Dispatch ready workers up to available concurrency BEFORE starting your
   own implementation; queue remaining ready jobs and dispatch as slots free.
   Reserve capacity for yourself when taking a ticket. Each
   prompt MUST designate "ticket worker under this coordinator; Core sync
   and publish duties belong to the coordinator", and provide the current
   ticket text, workspace, baseline ID, path/owner allow-list, discovery brief
   and validation assignment described below, plus expected return artifact.
   Point to `AGENTS.md` and `worker.md`. Workers choose their relevant reads,
   skills, and test coverage within the execution policy. An undesignated
   worker would start a destructive second sync session. While workers run,
   implement your own ticket in its
   assigned copy; do not wait for their returns first.
6. Collect completed returns and freeze each returned artifact before
   integration. You may integrate a dependency-ready return while other
   workers continue in their isolated copies. Your own ticket gets the same
   handoff/checks as every other ticket; never edit its copy while applying
   its return. In shared-tree mode, wait until all writers stop, then capture
   their results before staging. Record actual paths, baseline/result hashes,
   checks run or pending, proposed AC updates, and any unexpected overlaps in
   the batch record. A return means implemented, awaiting integration and
   validation; it is not a passing ticket. Keep workers available for repairs.
7. Reconcile in staging against each return's exact baseline. Apply only
   changed paths, not whole worker trees. For overlapping files, compare
   baseline, worker result, and current staging (three-way comparison without
   Git). Preserve both compatible changes; record ticket IDs, path/symbol,
   competing intent, chosen result, and validation in the conflict ledger.
   No silent last-writer-wins. Textually clean changes still need semantic
   review of shared contracts and owners. Unresolved conflicts block only
   affected tickets and their dependents; retain their artifacts for rework.
8. Before applying a candidate integration unit, verify each destination path
   in `C:\Sprout` still matches its expected baseline, last published/applied
   result, or captured shared-tree result, as recorded for that path. Unexpected
   local changes require reconciliation too. Preserve exact pre-apply contents
   and hashes (including absence) for every affected path outside the synced
   tree. Freeze and identify the candidate; copy only its reviewed changed
   paths from staging, explicitly accounting for additions and deletions;
   never union-copy a whole tree over unrelated work. Share deletions remain
   forbidden by Core: defer changes requiring them unless the user resolves
   that constraint. Record the candidate as `awaiting-validation`; this local
   application is NOT publication. Preserve the existing dependency/build
   directories in `C:\Sprout`; never replace them with worker/staging caches.
   Keep only one pending candidate in the local tree; repairs retain its
   original recovery snapshot until validation or restoration finishes.
   In shared-tree mode, captured writer results are unvalidated: construct
   recovery from the pre-batch/last published source, preserving unrelated
   local changes, rather than treating those writer results as safe to sync.
9. Validate the applied combination in `C:\Sprout` under the policy below.
   Run relevant `toolchain.md` and ticket-required checks. Freeze source inputs
   while checks run (all shared-tree writers must stop); record input hashes,
   commands, results, and log paths. A worker's
   passing tests do not validate the combination. Send failures with the
   tested candidate ID and diagnostics to the responsible worker for a new
   return. Reconcile repairs in staging and re-run affected checks. Failed
   tickets and dependents are repaired or excluded using retained baselines
   and returns, never by guessing an inverse edit. Before any `-Up`, including
   session end, remove unvalidated candidates from the publishable tree:
   restore their exact pre-apply contents/absence only where current hashes
   still match the applied candidate; reconcile unexpected local changes
   before restoring. Retain failed artifacts for rework. Mark ACs done only
   for applied, validated tickets, each in its own issue file, and update the
   ledger. Run `node tools/ownership-gate.mjs` in `C:\Sprout` before publication.
10. Publish each completed integration unit with Core `-Up`, then `-Up`
    again expecting `0 copied`; do not defer a completed unit to session end.
    Outstanding isolated workers keep their original baselines: publishing
    does not rebase their copies. Reconcile later returns against current
    staging. New dependent waves start from a named validated integrated
    baseline. `SHARE-NEWER` follows `working-copy.md`; never run a fresh
    `-Down` over active work or use raw robocopy.

## Discovery and return evidence

- Give each worker a compact brief tied to its baseline: exact paths and
  owner symbols, relevant findings with source references, dependencies and
  pinned contracts, acceptance criteria, existing tests/reproduction steps,
  and remaining questions. Include relevant excerpts when needed; avoid
  copying the whole conversation or making workers retrieve already-known
  facts through broad searches. Required instruction/ADR reads still apply.
- Assign permitted lightweight checks and required queued checks at dispatch.
  The ticket owner owns test coverage and failure fixes; the coordinator owns
  scheduling heavy execution and validating the combination. Do not fully
  solve every ticket before delegating its implementation.
- Require a compact return with changed paths/symbols, baseline/result hashes,
  acceptance evidence, test commands and results or explicit pending status,
  deviations from the brief, and unresolved concerns. Link full artifacts/logs
  instead of relaying them through messages. Review the actual changes and
  their evidence against acceptance criteria and shared contracts; broaden
  discovery when there is a gap or contradiction, not as a routine restart.

## Validation and device resources

- One coordinator-controlled heavy execution slot covers the whole batch,
  including the coordinator: dependency installs, compilation, project-wide
  checks, full suites, dev servers/watchers, app/browser sessions, and installer
  builds. A validation session may need an app and test runner together;
  the slot covers that whole process group. Agent concurrency does not
  authorize additional heavy sessions.
  Stop batch-owned watchers/apps before replacing candidate source; release
  the slot only after its processes stop. Existing unrelated sessions require
  coordination before replacing source or launching competing heavy work.
- Use `C:\Sprout` (or its `src-tauri` directory) and its existing caches for
  heavy checks. Do not reset caches or require cold builds per ticket by
  default; required cleanup and evidence-backed cache repairs still apply. A
  targeted Rust test can still compile extensively; classify by actual work,
  not command name. Limit runner jobs/threads where needed; such limits are
  not hard RAM caps. Required concurrency-sensitive tests still need their
  intended conditions, within the single heavy slot.
- Workers may run assigned lightweight checks in their own copies without
  installs, compilation, or app launches. Queue other checks with exact commands
  and expected outcomes. If runtime feedback is needed before a ticket is
  complete, accept an identified partial return for temporary local validation
  under steps 8–9; restore a partial candidate before any publication even if
  its checks pass. A necessary isolated
  runtime exception must be scheduled in the same slot, with separate writable
  build/test data, and recorded with its reason and cleanup responsibility.
- Validate ready integration units promptly while isolated workers continue.
  Group compatible returns when repeated expensive checks justify it; do not
  wait for the whole batch by default or run a full build automatically per
  worker. Run the required checks for the actual combined change before
  publishing. Deduplicate requests only when tested source, dependencies,
  configuration, and relevant environment still match; changed inputs require
  affected checks again. Record skipped/deferred checks honestly.
- Retain the tested candidate manifest and results; distinguish test-data state
  from reusable build caches and reset/isolate test data as required. Incremental
  builds may still rebuild substantially after dependency or configuration
  changes. Increase heavy concurrency only with measured device headroom and
  an explicit batch allocation; never infer it from free agent slots.

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
changes, validation assignment/results, and state (`running`, `returned`,
`awaiting-validation`, `validated`, `blocked`, or `published`). Record candidate
IDs, pre-apply recovery artifacts, tested input hashes, and any resource-policy
exceptions. Keep a conflict ledger beneath it; write `none`
when there are no conflicts. Link retained baseline/return artifacts and
their hashes so another session can identify exactly what remains to merge.
Preserve artifacts until publication is verified and deferred work is handed off.

- Failure/timeout: retain the artifact and exclude incomplete work plus its
  dependents; continue independent tickets. Never apply a still-changing copy.
- No snapshot: refuse `-Up`; use Core recovery rules before any `-Down`.
- Example: if 139 and 141 are dependency-ready and disjoint, dispatch worker
  141, then implement coordinator ticket 139 concurrently. Integrate either
  completed return when ready; 141 does not wait for 139's implementation.
