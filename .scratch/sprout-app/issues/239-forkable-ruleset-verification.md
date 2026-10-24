# 239 — Forkable ruleset verification

**Parent:** [223 — Forkable ruleset (spec)](223-forkable-ruleset-spec.md)

**What to build:** The combined verification pass for the forkable ruleset: prove a standalone clone works without the VM overlay, a VM workspace still follows the existing sync/no-git rules, the marker is excluded from sync/git, setup rolls back cleanly, and the docs contain no skill mandates or duplicate rulebooks.

**Blocked by:** 235, 236, 237.

**Status:** ready-for-agent

- [ ] Standalone dry run outside `C:\Sprout`: no marker; `AGENTS.md` applies without `-Down`/`-Up`, the share, or the no-git rule; the ownership gate runs before commit/PR; build/check/test commands use the clone path.
- [ ] VM dry run at `C:\Sprout` with the share reachable: setup checks pass; the marker is written last; `-Down` runs; the marker is not on the share; a second `-Up` reports `0 copied`; the no-git rule and snapshot/divergence behavior are unchanged.
- [ ] Rollback: a simulated `-Down` failure removes the marker and leaves the clone standalone; no database/logs/working tree are deleted.
- [ ] Exclusion: the marker is not copied by `-Up`; `.gitignore` ignores it (verified in a standalone scratch clone, never by running git in the VM working copy).
- [ ] Docs grep: no `MUST USE <skill>` remains; no `.opencode/skills`, `.agents/skills`, or manifest exists.
- [ ] Fork checklist review: a maintainer can follow `docs/release/forking.md` to redirect the updater, choose signing/identity, tag a release, and verify upstream cannot overwrite the fork.
- [ ] No app behavior, dependency, or manifest change ships.
- [ ] `node tools/ownership-gate.mjs` passes; the VM `-Up` verification reports `0 copied`.

**Claims (recheck at dispatch):** docs, sync script, gitignore, ownership gate. **Explicitly not built:** new features; fixes land in 235–237.
