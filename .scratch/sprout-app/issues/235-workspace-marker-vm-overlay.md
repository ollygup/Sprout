# 235 — Workspace marker and VM overlay

**Parent:** [223 — Forkable ruleset (spec)](223-forkable-ruleset-spec.md)

**What to build:** A clone is standalone unless an untracked `.sprout-workspace` file contains `vm`. `AGENTS.md` reads that marker at session start, keeps the universal rules unconditional, and points VM sessions to `docs/agents/working-copy.md` for the existing `C:\Sprout`/share/sync/no-git overlay. The VM setup verifies the hardcoded paths and toolchain, writes the marker last, and rolls back if the first `-Down` fails. The marker is excluded from sync and git.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [ ] Marker semantics: absent = standalone; content `vm` (trimmed, case-insensitive) = VM; any other content = standalone with a reported/repairable invalid marker.
- [ ] `AGENTS.md` reads the marker first; when `vm`, runs `-Down` as the first substantive action, then reads the ADR index; when absent, skips sync and reads the ADR index before planning.
- [ ] The universal core no longer hardcodes `C:\Sprout`, the share, no-git, or `-Up`; it covers working in the clone, ADRs/amendments, quality rules, and the ownership gate before commit/PR.
- [ ] The VM overlay pointer sends `vm` sessions to `working-copy.md`; the existing no-git, share, sync, snapshot, divergence, and robocopy wording is preserved verbatim there.
- [ ] `working-copy.md` gains the VM setup sequence: verify repo root `C:\Sprout`, share reachability, `sync.ps1`/`ownership-gate.mjs`, Node/npm/cargo, and the marker state; write `vm` last; run `-Down`; remove the marker and report if `-Down` fails; re-running setup is idempotent.
- [ ] Standalone mode explicitly has no `-Down`/`-Up`, no share, and no no-git rule; it uses normal git and runs the ownership gate before commit/PR.
- [ ] `.sprout-workspace` is added to `sync.ps1`'s exclusion list and `.gitignore`; `-Down` does not copy it, and `-Up` reports `0 copied` with it present.
- [ ] The topic index has a workspace-mode row and VM-only qualifiers; no manifest, project skill folder, or path parameterization is added.
- [ ] Verification: standalone dry run outside `C:\Sprout`; VM dry run at `C:\Sprout`; simulated `-Down` failure rolls back; marker exclusion proven; no app behavior change.
- [ ] `node tools/ownership-gate.mjs` passes; the VM `-Up` verification reports `0 copied`.

**Claims (recheck at dispatch):** `AGENTS.md`; `docs/agents/working-copy.md`; `tools/sync.ps1` exclusion list; `.gitignore`. **Explicitly not built:** parameterizing `C:\Sprout`/the share; a manifest; `.opencode/skills`/`.agents/skills`; updater code; app behavior.
