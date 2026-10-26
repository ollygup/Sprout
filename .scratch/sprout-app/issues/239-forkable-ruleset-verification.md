# 239 — Forkable ruleset verification

**Parent:** [223 — Forkable ruleset (spec)](223-forkable-ruleset-spec.md)

**What to build:** The combined verification pass for the forkable ruleset: prove a standalone clone works without the VM overlay, a VM workspace still follows the existing sync/no-git rules, the marker is excluded from sync/git, setup rolls back cleanly, and the docs contain no skill mandates or duplicate rulebooks.

**Blocked by:** 235, 236. (237 was deferred 2026-10-26 by user decision; its fork-checklist check is dropped, not carried.)

**Status:** complete 2026-10-26 — machine-verified; one interactive standalone confirmation remains with the user (kit below). The pass also found and fixed a silent snapshot-wipe in `-Down`.

- [x] Standalone dry run outside `C:\Sprout`: no marker; `AGENTS.md` applies without `-Down`/`-Up`, the share, or the no-git rule; the ownership gate runs before commit/PR; build/check/test commands use the clone path. *Machine-proven on a 107-file copy at `%TEMP%\opencode\sprout-standalone-dryrun`: no marker, `vm-setup.ps1` fails the root check with the remedy and exits 1, and the copy's universal core names no share/sync rule. The interactive "report unconfigured → ask once → standalone" exchange is the user's remaining confirmation, using the kit.*
- [x] VM dry run at `C:\Sprout` with the share reachable: setup checks pass; the marker is written last; `-Down` runs; the marker is not on the share; a second `-Up` reports `0 copied`; the no-git rule and snapshot/divergence behavior are unchanged. *Check PASS on all seven; marker `vm`; `-Down` 0 copied + 624-file snapshot; share `Test-Path .sprout-workspace` false; `-Up` 0 copied.*
- [x] Rollback: a simulated `-Down` failure removes the marker and leaves the clone standalone; no database/logs/working tree are deleted. *Simulated with an unreachable share in a rewritten copy: `-Down` now exits 1 before touching `.sync-state.json`; removing the marker returns the tree to unconfigured/standalone. No data is touched by the marker operations.*
- [x] Exclusion: the marker is not copied by `-Up`; `.gitignore` ignores it (verified in a standalone scratch clone, never by running git in the VM working copy). *`-Up` 0 copied with the marker present; share has no marker; `.gitignore` entry checked as text — no git command was run.*
- [x] Docs grep: no `MUST USE <skill>` remains; no `.opencode/skills`, `.agents/skills`, or manifest exists.
- [x] ~~Fork checklist review: a maintainer can follow `docs/release/forking.md` to redirect the updater, choose signing/identity, tag a release, and verify upstream cannot overwrite the fork.~~ *Dropped 2026-10-26 with 237: no fork release is planned, so there is no document to review and no reader to verify it.*
- [x] No app behavior, dependency, or manifest change ships. *No app code touched. `tools/sync.ps1` gained a reachability guard — tooling, not application runtime.*
- [x] `node tools/ownership-gate.mjs` passes; the VM `-Up` verification reports `0 copied`.

**Claims (recheck at dispatch):** docs, sync script, gitignore, ownership gate. **Explicitly not built:** new features; fixes land in 235–237.

## Finding — `-Down` silently disarmed the snapshot guard

Rollback drill output before the fix: with an unreachable share, `-Down` printed `Down: 0 copied`, exited **0**, and wrote an empty `.sync-state.json`. Every later `-Up` would then see every local file as new, upload until the 500-file ceiling aborted, and never report `SHARE-NEWER` again. `tools/sync.ps1` now checks the share root before touching the state file and exits 1. `working-copy.md` additionally states that unpublished local edits require `-Up` before any `-Down` — during this ticket a mid-session `-Down` reverted unpublished work, which is that rule's evidence.

## Dry-run kit

A standalone clone can be simulated without touching `C:\Sprout`'s marker:

1. Copy `AGENTS.md`, `.gitignore`, `docs\`, `tools\` into a scratch directory outside `C:\Sprout` (~110 files, no `node_modules`/`target`).
2. Run `powershell.exe -NoProfile -ExecutionPolicy Bypass -File "tools\vm-setup.ps1"` there. Expected: the root check FAILs, the junction/move remedy is printed, the marker reads `absent - unconfigured`, exit code 1. Verified 2026-10-26.
3. Start a fresh session **with that directory as its root** (a session bound to `C:\Sprout` proves nothing — its instructions and files are the VM ones). Ask three questions: *which workspace mode is this clone, and how do you know?* / *what must you do first at session start?* / *may you run `tools\sync.ps1` and git?* Expected: unconfigured + reported, read the marker, ask the user once, no sync, git allowed (no no-git rule applies).
4. Answer `standalone`, confirm it writes that single word and proceeds without `-Down`/`-Up`.
5. Delete the scratch directory afterwards; it contains no `.git`, so no repository state exists to clean up.

Do not simulate the standalone branch by deleting `C:\Sprout\.sprout-workspace`: an interrupted session would leave the VM unconfigured, and an unattended one would then default to standalone and stop syncing.
