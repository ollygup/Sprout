# 223 — Forkable ruleset: workspace marker, VM overlay, docs-first rules (spec)

**Status:** planning package only — no application behavior changed here. Tickets 235–237 and 239 are `ready-for-agent` (created next in this unit).

**Parents / related (read before implementing):**

- [AGENTS.md](../../../AGENTS.md) — the session entry point and topic index this round restructures.
- [docs/agents/working-copy.md](../../../docs/agents/working-copy.md) — the existing VM/sync module this round reuses rather than replacing.
- [docs/agents/toolchain.md](../../../docs/agents/toolchain.md), [conventions.md](../../../docs/agents/conventions.md), [ui-ux.md](../../../docs/agents/ui-ux.md) — command, module-boundary, and UI rules affected by the docs-first rewrite.
- [docs/agents/planning.md](../../../docs/agents/planning.md) — spec/ticket slicing and the "distinguish current/accepted/proposed" rule.
- [ADR-0012](../../../docs/adr/0012-github-release-self-update.md) — the updater behavior a fork must redirect.
- [ADR-0029](../../../docs/adr/0029-one-source-of-truth-per-windows-command.md) — the ownership gate remains the pre-publish check in both modes.
- Related: [spec 221](221-collections-media-spec.md) and [spec 222](222-storage-spec.md) — this round's code work must follow whichever workspace mode the clone selects.
- Research: this round's agent-instruction and fork-release review (Further Notes).

## Problem Statement

From the user's side:

- The repository's agent rules assume exactly one machine: a Windows VM with the repo at `C:\Sprout`, a VMware shared folder as the source of truth, no git commands in the working copy, and a sync script. A developer who forks or clones the repo anywhere else cannot follow `AGENTS.md` without breaking it or editing it.
- The VM rules cannot be bypassed cleanly. "Run `-Down` first" fails on a normal clone; "never run git" blocks normal development; the share paths do not exist.
- Fork users are also developers. "Fork" is not a workspace mode, but the docs currently have no way to say "this clone is standalone" versus "this clone is the VM working copy".
- The docs contain "MUST USE `<skill>`" mandates. Skills are optional, globally installed aids; a fresh clone may not have them, so the rules are unenforceable and the docs are not the source of truth.
- A fork can be silently overwritten by upstream releases: the updater polls the upstream repository and the owner identity is embedded. Fork maintainers need a human checklist for the updater, signing, and identity.

## Solution

Introduce one untracked workspace marker, `.sprout-workspace`:

- **Absent** → standalone. Normal clone, normal git, no share, no `-Down`/`-Up`, no no-git rule. Universal quality rules still apply.
- **Contains `vm`** → VM workspace. The existing hardcoded `C:\Sprout` + share + `sync.ps1` + no-git + ownership-gate rules apply exactly as today, with the marker gating them.

Restructure `AGENTS.md` so universal rules are unconditional and the VM overlay lives in `docs/agents/working-copy.md`, read only when the marker says `vm`. Replace the skill mandates with repo-owned rules; skills become optional aids. Add a human fork-release checklist. No manifest, no `.opencode/skills`, no `.agents/skills`, no second rulebook.

## User Stories

1. As a developer who forks Sprout, I want to clone and work on a normal machine, so I can build without a VMware share.
2. As a developer who forks Sprout, I want the universal code, UI, docs, and testing rules to still apply, so a fork does not start from a lower quality bar.
3. As a VM user, I want the existing `C:\Sprout`/share/sync/no-git workflow unchanged, so my daily routine does not break.
4. As a VM user, I want setup to verify the hardcoded paths before enabling VM mode, so I never get a half-working repository.
5. As a VM user, I want the marker written only after every check passes, so a failed setup leaves a standalone clone rather than a broken VM one.
6. As a VM user, I want the marker excluded from sync and git, so it never reaches the share or upstream.
7. As a standalone user, I want no `-Down`/`-Up`/no-git rules, so normal git commands work.
8. As a standalone user, I want the ownership gate before commit/PR, so the same ownership constraint applies without the VM.
9. As a VM user, I want `-Down` first and `-Up` per unit, so sync remains snapshot-guarded.
10. As an agent, I want one marker check at session start, so I know which rule set applies without guessing.
11. As an agent, I want the VM-only docs clearly marked, so I do not apply share/no-git rules in a standalone clone.
12. As an agent, I want the universal rules to be complete on their own, so I do not need a global skill install.
13. As a docs reader, I want the deep-module vocabulary and UI rules written in the repo, so they are reviewable and stable.
14. As a docs reader, I want skills to be optional aids, so I can use them when installed but never depend on them.
15. As a fork maintainer, I want to redirect the updater to my fork, so upstream releases cannot overwrite my build.
16. As a fork maintainer, I want signing/identity guidance, so my installer and app data do not collide with upstream.
17. As a fork maintainer, I want a release checklist, so I can verify the updater, tag, and installer before publishing.
18. As a maintainer, I want no manifest or duplicate skill folders, so there is one source of truth.
19. As a maintainer, I want a verification pass for both modes, so the marker gate cannot rot.
20. As a user, I want no application behavior change from this round, so the rules work does not destabilize the app.

## Current vs accepted vs proposed

**Current (verified in source):**

- `AGENTS.md` puts the VM topology in the every-session core: work in `C:\Sprout`, never work on the share, never run any git command, run `-Down` first, run `-Up` per unit, run the ownership gate before `-Up`, invoke `sync.ps1` with the exact PowerShell command.
- `docs/agents/working-copy.md` owns the two homes, the snapshot, divergence handling, and the robocopy fallback. Its wording is authoritative and is duplicated in `AGENTS.md`.
- `docs/agents/toolchain.md` commands start "from `C:\Sprout`".
- `docs/agents/parallel-tickets.md`, `worker.md`, `releases.md`, and `docs/release/release-process.md` assume the VM/share/git split.
- `conventions.md` says "MUST USE the `codebase-design` skill vocabulary"; `ui-ux.md` says "MUST USE the `web-design-guidelines` and `frontend-design` skills".
- No `.sprout-workspace` marker exists. `sync.ps1` excludes build/cache files and `.sync-state.json`; `.gitignore` does not mention the marker.
- No project skill folder exists (`.opencode` is OpenCode tooling, not a project skill home; `.agents` does not exist).
- The updater polls `api.github.com/repos/ollygup/Sprout/releases/latest`; the owner public key is embedded in `update.rs`; an upstream release can overwrite a fork's installer. There is no fork-release checklist.

**Accepted-but-unimplemented assumed by this round:**

- The docs-first rule (docs are authoritative, skills optional) is this round's decision; no earlier ticket implements it.
- The marker does not exist yet; all current clones are implicitly VM.

**Proposed here:**

- The marker and its gate; the AGENTS/working-copy split; the VM-only doc banners; the skill-mandate rewrite; the fork checklist; the sync/gitignore exclusions.
- No app behavior change, no manifest, no project skill folders, no parameterization of the hardcoded VM paths.

## Implementation Decisions

### A. Marker semantics

1. **File.** `.sprout-workspace` at the repository root. Untracked and ignored by git. Content is `vm` (trimmed, case-insensitive) to select the VM overlay. Absent = standalone. Any other content = invalid; treat the clone as standalone and have the setup report/repair it.
2. **Why a marker, not a manifest.** A manifest would duplicate the docs and drift. The marker carries one fact — which rule set applies — and the docs carry the rules. No JSON, no schema, no generated file.
3. **Exclusions.** Add `.sprout-workspace` to `sync.ps1`'s `$excludeFiles` and to `.gitignore`. `-Down` must not bring a share copy into the working copy; `-Up` must not send the marker to the share. No other sync behavior changes.
4. **VM compatibility.** Do not parameterize `C:\Sprout` or `\\vmware-host\Shared Folders\Projects\Sprout` in `sync.ps1`. The setup verifies the current hardcoded assumptions instead. This keeps the script's snapshot/divergence logic unchanged and avoids a second path configuration.
5. **Forking is not a mode.** A fork can be standalone (normal clone) or VM (only if it actually lives at `C:\Sprout` with the share). The marker describes the workspace, not the repository's origin. A fork user who is also a developer follows the same universal rules.

### B. `AGENTS.md` structure

1. **Session start.** Read the marker first (one file). If `vm`, run `-Down` as the first substantive action, then read `docs/adr/README.md`. If absent, skip sync and read the ADR index before planning. The marker read is the only action allowed before `-Down`.
2. **Universal core.** Work in the clone; treat unfamiliar files as user work; docs are authoritative; read the ADR index and relevant ADRs; append dated amendments rather than re-litigating; follow code/UI/comments/toolchain/planning rules; run the ownership gate before commit/PR; never publish a failing tree. These rules do not mention `C:\Sprout`, the share, sync, or no-git.
3. **VM overlay pointer.** When the marker is `vm`, read `docs/agents/working-copy.md` and follow it. The overlay owns `C:\Sprout`, the share, the no-git rule, `-Down`/`-Up`, the snapshot, divergence handling, and the gate-before-`-Up` ordering. The no-git wording moves verbatim; it is not softened.
4. **Topic index.** Add a "Workspace mode" row: when setting up or using a VM workspace, read `working-copy.md`. Mark the VM-only rows (`parallel-tickets.md`, `worker.md`, `releases.md`, `release-process.md`) with a "VM only" qualifier. Universal rows stay unconditional.
5. **Standalone publishing.** Standalone uses normal git branches/commits/PRs. The ownership gate runs before commit/PR. There is no `-Up` and no share. The docs do not prescribe a particular remote or hosting provider.

### C. `working-copy.md` VM module

1. Keep the existing two-homes, no-git, sync, snapshot, divergence, and robocopy sections; their wording remains authoritative.
2. Add a **VM setup** section with this order:
   - Verify the repository root resolves to `C:\Sprout`. If not, refuse VM mode and explain that standalone is the supported path for other locations.
   - Verify `\\vmware-host\Shared Folders\Projects\Sprout` is reachable.
   - Verify `tools\sync.ps1` and `tools\ownership-gate.mjs` exist.
   - Verify `node`, `npm.cmd`, and `cargo` are on PATH (the VM toolchain).
   - Verify the marker is absent or already `vm`.
   - Write `vm` to `.sprout-workspace` **last**, after every check passes.
   - Run `sync.ps1 -Down` (the normal session-start refresh/snapshot). If it fails, delete the marker, report the failure, and leave the clone standalone.
   - Verify the marker was not copied to the share and that a second `-Up` reports `0 copied`.
3. **Idempotence.** Re-running setup on a valid VM workspace verifies and reports; it does not rewrite the marker or run a destructive sync. A marker with invalid content is reported; the setup does not silently overwrite it without the user's confirmation.
4. **Rollback.** Removing the marker returns the clone to standalone; it does not delete the database, logs, or working tree. The share is untouched.
5. **Standalone note.** Standalone does not inherit the no-git rule, the share rule, `-Down`/`-Up`, or the VM snapshot. It does inherit the ownership gate (before commit/PR), the ADR process, and every universal quality rule.

### D. Docs-first rules and skill removal

1. **`conventions.md`.** Replace the `codebase-design` skill mandate with the repo-owned vocabulary and deletion test:
   - module = a unit that hides a design decision behind an interface; interface = everything a caller must know; seam = a place where behavior can be substituted/tested; adapter = an implementation behind a seam; depth = leverage per unit of interface.
   - Deletion test: imagine deleting the module; if the complexity reappears in many callers, it earns its keep; if not, inline it. One adapter means no seam; thin pass-throughs are inlined.
   - Keep the existing `shared/` rule and the Windows-owner rule unchanged.
2. **`ui-ux.md`.** Replace the `web-design-guidelines`/`frontend-design` skill mandate with the repo-owned rules already in the file: reuse tokens/components, no ad-hoc values, cite the standing research, record deviations for review, and check accessibility/contrast/keyboard/focus. State explicitly that the skills are optional aids when installed; the docs and research notes are authoritative.
3. **No project skills.** Do not add `.opencode/skills`, `.agents/skills`, or a manifest. `.opencode` remains OpenCode tooling. A skill, if a user installs one globally, is an optional aid; no repo rule depends on it.
4. **VM-only banners.** `parallel-tickets.md`, `worker.md`, `releases.md`, and `docs/release/release-process.md` get a short "VM workspace only — read only when `.sprout-workspace` is `vm`" banner. Their content stays otherwise unchanged. Standalone developers use normal git branches/PRs and the universal gate.
5. **`toolchain.md`.** Note that `C:\Sprout` is the VM path; a standalone clone substitutes its own path. The commands themselves (`npm.cmd`, `cargo test`, etc.) are unchanged. The ownership gate is path-agnostic (`process.argv[2] ?? repo root`), so it runs in both modes.

### E. Fork release checklist

1. Add `docs/release/forking.md` as a human checklist, not a workspace mode and not an agent rulebook.
2. Cover:
   - Fork the repository; clone anywhere for standalone, or use `C:\Sprout` + the share for VM mode if the hardcoded paths actually match.
   - Redirect the updater's repository slug and any embedded owner identity to the fork; verify no upstream `ollygup/Sprout` URL or upstream key remains.
   - Decide signing/identity. Upstream ships unsigned; a fork should sign or clearly document that it does not. Avoid installer/app-identity collisions if upstream and the fork are installed side by side.
   - Tag and publish a release; verify the fork's updater sees it and that an upstream release cannot overwrite the fork.
   - Keep the version/tag discipline (`Cargo.toml` is the single version source).
   - Never commit `.sprout-workspace`, local app data, logs, or the database.
3. The checklist is guidance for a human maintainer; it does not change `update.rs` in this round.

## Seams

- **Mode gate:** `AGENTS.md` reads one marker and points to one VM module. No second mode switch, no manifest, no per-tool override.
- **VM rules:** `working-copy.md` remains the single owner; `AGENTS.md` does not duplicate its sync/no-git wording. The existing text moves, it is not rewritten.
- **Exclusions:** `sync.ps1` and `.gitignore` are the only code/config changes. No new setup script, service, or dependency.
- **Universal rules:** `conventions.md`, `ui-ux.md`, `toolchain.md`, and `planning.md` own their topics; the skill mandates are replaced in place, not copied into a new rulebook.
- **Fork guidance:** `docs/release/forking.md` is a checklist beside the existing release docs; it does not create a fork mode.

## Design rules every implementer applies

- **Docs are authoritative.** The rules must be complete without any globally installed skill. Skills are optional aids; no `MUST USE <skill>` line remains.
- **One source of truth.** VM sync/no-git wording lives in `working-copy.md`; `AGENTS.md` points to it. Do not duplicate the text and then let it drift.
- **No manifest, no project skill folder.** The marker is the only mode switch.
- **Preserve VM behavior.** The no-git rule, hardcoded paths, snapshot guard, `SHARE-NEWER` handling, and gate-before-`-Up` order stay exactly as they are. This round adds a gate, not a new sync engine.
- **Standalone is normal development.** No `-Down`/`-Up`, no share, normal git, same quality gates.
- **Forking is not a mode.** A fork is a repository origin; a workspace is standalone or VM.
- **Planning:** distinguish current/accepted/proposed; no ADR rewrite — this round adds no ADR, because it changes how rules are selected, not the decisions those rules encode. If a boundary is later found to change, append a dated amendment instead.

## Testing Decisions

Docs/config work is verified by a repeatable checklist, not by unit tests alone. Good verification proves both modes and the exclusions.

- **Standalone dry run:** clone/copy outside `C:\Sprout` with no marker; follow `AGENTS.md`; confirm no `-Down`/`-Up`/share/no-git instruction applies; run `node tools/ownership-gate.mjs`; build/check/test commands use the clone path.
- **VM dry run:** at `C:\Sprout` with the share reachable, run the setup checks; confirm the marker is written last; run `-Down`; confirm the marker is not on the share; run `-Up` twice and expect `0 copied`; confirm the no-git rule and snapshot behavior are unchanged.
- **Rollback:** make `-Down` fail (temporarily unreachable share or invalid path in a safe test copy); confirm the marker is removed and the clone is standalone.
- **Exclusion:** create the marker, run `-Up`, confirm it is not copied; confirm `.gitignore` ignores it (`git check-ignore` is a git command — run it only in a standalone scratch clone, never in the VM working copy, per the no-git rule).
- **Docs grep:** no `MUST USE` + skill name remains in `conventions.md`/`ui-ux.md`; no `.opencode/skills`, `.agents/skills`, or manifest exists.
- **Fork checklist review:** a maintainer can follow `docs/release/forking.md` to redirect the updater and publish a test release without touching upstream.
- **Prior art:** the existing `sync.ps1` snapshot tests/manual checks, `ownership-gate.mjs` path-agnostic behavior, and the release-process checklist.

## Out of Scope

- Changing `update.rs` or implementing fork-aware updater code in this round.
- Parameterizing `sync.ps1` paths or adding a second share configuration.
- Adding a manifest, `.opencode/skills`, `.agents/skills`, or any project skill folder.
- Changing the app's runtime behavior, data location, or build output.
- Changing the VM's no-git rule, snapshot guard, divergence handling, or share topology.
- CI changes or a hosted release service.
- Making forking a workspace mode.

## Further Notes

**Why reuse `working-copy.md`.** It already owns the VM-specific facts: the two homes, the no-git rule, the sync triggers, the snapshot, and the fallback. Adding a separate `vm-workspace.md` would duplicate that text or split it across two files. The marker gate turns the existing file into the VM module; `AGENTS.md` keeps only the universal core and a pointer.

**Why not a skill.** OpenCode discovers project skills under `.opencode/skills`, `.agents/skills`, `.claude/skills`, and global equivalents. `docs/agents/` is not a discovery path. The repository's current practice is `AGENTS.md` topic index -> `docs/agents/*.md`; a skill would be a new, optional surface for no gain. The setup rule belongs in the docs, where every agent reads it.

**Why not a manifest.** A manifest would restate the mode and the rule list, then drift from `AGENTS.md`. The marker is one fact; the docs are the rules. The ownership gate already verifies structure, so a second metadata file adds maintenance without enforcement.

**Why the VM setup refuses non-`C:\Sprout` paths.** `sync.ps1` hardcodes both the working copy and the share, and its snapshot/divergence logic depends on that topology. Parameterizing it would be a larger change with new failure modes. A standalone clone is the supported path for every other location; VM mode is opt-in only where the existing assumptions hold.

**Fork updater risk.** The updater polls the upstream Releases API and embeds the owner key. A fork that does not redirect both can download and run an upstream installer over its own build. The checklist makes this a deliberate, verifiable step; it does not change the updater's trust model in this round.

## Ticket map and integration ownership

| Ticket | Behavioral prerequisites | Likely paths / owner symbols | Shared contract and integration edits | Candidate wave |
| --- | --- | --- | --- | --- |
| [235 Workspace marker + VM overlay](235-workspace-marker-vm-overlay.md) | None | `AGENTS.md`, `docs/agents/working-copy.md`, `tools/sync.ps1` (`$excludeFiles`), `.gitignore` | Marker contract; AGENTS universal/VM split; setup checks/rollback | Docs 1 |
| [236 VM-only docs + docs-first skill rewrite](236-vm-only-docs-and-skill-rewrite.md) | 235 (needs the marker gate) | `conventions.md`, `ui-ux.md`, `toolchain.md`, `parallel-tickets.md`, `worker.md`, `releases.md`, `docs/release/release-process.md` | Repo-owned deep-module/UI rules; VM-only banners; no skill mandates | Docs 1/2 |
| [237 Fork release checklist](237-fork-release-checklist.md) | 235 (needs standalone/VM distinction) | New `docs/release/forking.md` | Updater redirect, signing/identity, release verification | Docs 2 |
| [239 Forkable ruleset verification](239-forkable-ruleset-verification.md) | 235–237 | Docs + sync + gate | Standalone/VM dry runs, exclusion, rollback, docs grep | Final |

**Concurrency note.** 235 and 236 share `AGENTS.md` only through the topic index; 236 starts after 235 so the gate exists. 237 is docs-only and can run beside 236 after 235. 239 is the combined verification. Claims above are estimates to recheck at dispatch.

## Acceptance and verification

- [x] User confirmed: docs are the priority source; skills are optional and user-triggered; `.opencode` is not the skill home; VM rules must be bypassable for non-VM forks; forking is not a workspace mode; no manifest; reuse `working-copy.md`.
- [ ] 235 verifies the marker gate, setup checks/rollback, and sync/gitignore exclusions.
- [ ] 236 verifies no `MUST USE <skill>` line remains and the VM-only docs are marked.
- [ ] 237 verifies the fork checklist covers updater redirect, signing/identity, and release verification.
- [ ] 239 runs the standalone and VM dry runs, including rollback and `0 copied`.
- [ ] No app behavior, dependency, or manifest change ships.
- [ ] No application behavior changed in this planning session.

## Amendment 2026-10-25 — marker values, setup tiers, and setup-script scope

Dispatch review of 235 changed five things. The original text above is preserved; this section is the current contract.

1. **Marker values are explicit; absent is *unconfigured*, not standalone.** The marker's entire content is `vm` or `standalone` (trimmed, case-insensitive). Absent, empty, or invalid content selects standalone *rules* for the session but MUST be reported, and `AGENTS.md` MUST ask the user once which mode the clone is, then write the answer. Rationale: absence was indistinguishable from "deliberately standalone", so a lost or deleted marker was a silent downgrade — on the VM it would quietly stop the sync discipline and the no-git rule. Supersedes the Solution bullet "Absent → standalone" and §A.1. The marker still holds one fact; no paths, no manifest.
2. **Session start is ask-once, never inference.** `AGENTS.md` MUST NOT infer the mode from the path, a mounted share, or git metadata. An unattended session (worker, coordinator batch, user unavailable) proceeds as standalone and MUST NOT write `vm`.
3. **Non-`C:\Sprout` paths are handled in three tiers, none of them automatic.** (a) Missing `C:\Sprout`: `-Down` already bootstraps it (it creates each copied file's parent directory), so no new code is needed — document it, and assert anchor files in 239. (b) Repo elsewhere and the user wants to keep it: the check prints a `New-Item -ItemType Junction` remedy for the human to run. (c) `C:\Sprout` already exists: refuse, and require hand inspection before any change — `Remove-Item -Recurse` on a junction can delete the tree it points at, so removal is `cmd /c rmdir`. The existing-path check MUST run before `-Down`, which would otherwise write through a stale junction.
4. **Setup gains one read-only script, and no apply mode.** `tools/vm-setup.ps1` verifies the root, share reachability, both tools, the toolchain, and the marker state; it never creates, moves, or reparents anything. This narrows "no new setup script" in Out of Scope to "no mutating setup script"; `sync.ps1` and `.gitignore` remain the only config changes. No `-Apply`, and no parameterization ticket: `C:\Sprout` and the share stay hardcoded. Revisit parameterization only when the share itself must move, or a second VM-hosted clone appears.
5. **236 also owns `docs/agents/cleanup.md`.** It is not VM-only, and its hardcoded `C:\Sprout\src-tauri\target` path is the last non-VM file that assumes the VM root.

Verification shift: 235 proves the check, the marker gate, the exclusions, and a `0 copied` `-Up`; 239 owns the standalone dry run, the rollback (delete the marker, confirm standalone), and the bootstrap anchor assertion.

## Amendment 2026-10-26 — 237 deferred, 238 closed, sync guard hardened

- **237 deferred by user decision**, not built: no fork release is planned. 239's dependency and fork-checklist AC were dropped in the same unit so the tracker carries no requirement nobody intends to meet. The upstream-updater risk recorded in Further Notes stands, now unmitigated by documentation.
- **238 closed** 2026-10-26 on the user's manual validation; the manual passes recorded against 230–233 are marked validated there. 242 is unblocked.
- **`sync.ps1` now refuses to run when the share is unreachable.** 239's rollback drill found that `-Down` against a missing share reported `Down: 0 copied`, exited 0, and wrote an empty snapshot — silently disarming the `SHARE-NEWER` guard, after which every local file looks new and `-Up` aborts at the copy ceiling. The guard runs before the state file is touched. `working-copy.md` also gained the rule that unpublished local edits require `-Up` before any `-Down` (a mid-session `-Down` reverted unpublished work during this ticket's own verification).
- The dock outside-click close for bezel mode is a new application-behavior ticket, not part of this round (223's Out of Scope excludes app behavior).
