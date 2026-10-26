# 235 — Workspace marker and VM overlay

**Parent:** [223 — Forkable ruleset (spec)](223-forkable-ruleset-spec.md)

**What to build:** A clone is standalone unless an untracked `.sprout-workspace` file contains `vm`. `AGENTS.md` reads that marker at session start, keeps the universal rules unconditional, and points VM sessions to `docs/agents/working-copy.md` for the existing `C:\Sprout`/share/sync/no-git overlay. The VM setup verifies the hardcoded paths and toolchain, writes the marker last, and rolls back if the first `-Down` fails. The marker is excluded from sync and git.

**Blocked by:** None — can start immediately.

**Status:** implemented 2026-10-25 — marker gate, read-only check, exclusions, and the live VM marker landed; the standalone dry run and rollback drill are handed to 239 (combined verification).

- [x] Marker semantics: ~~absent = standalone; content `vm` (trimmed, case-insensitive) = VM; any other content = standalone with a reported/repairable invalid marker.~~ *Amended 2026-10-25: content is `vm` or `standalone`; absent, empty, or invalid = **unconfigured** — standalone rules apply, the agent reports it and asks the user once, then writes the answer. Absence no longer masquerades as a deliberate standalone clone.*
- [x] `AGENTS.md` reads the marker first; when `vm`, runs `-Down` as the first substantive action, then reads the ADR index; when absent, skips sync, reports unconfigured, asks once, and reads the ADR index before planning.
- [x] The universal core no longer hardcodes `C:\Sprout`, the share, no-git, or `-Up`; it covers working in the clone, UNC safety, ADRs/amendments, unfamiliar-file ownership, and the ownership gate before commit/PR.
- [x] The VM overlay pointer sends `vm` sessions to `working-copy.md`; the existing no-git, share, sync, snapshot, divergence, and robocopy wording is preserved verbatim there, and its "keep both identical with AGENTS.md" header is gone.
- [x] `working-copy.md` gains the marker section and the VM setup sequence: verify root, share, `sync.ps1`/`ownership-gate.mjs`, toolchain, marker state; write `vm` last; run `-Down`; delete the marker and report if `-Down` fails; re-running is idempotent. *Amended 2026-10-25: setup verifies, it never configures. Missing `C:\Sprout` is bootstrapped by `-Down` itself; a repo elsewhere gets a printed junction/move remedy for the human; an existing `C:\Sprout` is refused and inspected by hand before `-Down` runs (a stale junction would otherwise be written through).*
- [x] Standalone mode explicitly has no `-Down`/`-Up`, no share, and no no-git rule; it uses normal git and runs the ownership gate before commit/PR.
- [x] `.sprout-workspace` is added to `sync.ps1`'s exclusion list and `.gitignore`; `-Up` reports `0 copied` with it present and the marker is absent from the share.
- [x] The topic index has a workspace-mode row and VM-only qualifiers (`working-copy`, `releases`, `parallel-tickets`, `worker`); no manifest, project skill folder, or path parameterization is added.
- [ ] Verification: standalone dry run outside `C:\Sprout`; VM dry run at `C:\Sprout`; simulated `-Down` failure rolls back; marker exclusion proven; no app behavior change. *Handed to 239: the standalone dry run and the rollback drill. Proven here: VM check pass, marker written last, marker exclusion, `-Up` `0 copied` twice, ownership gate.*
- [x] `node tools/ownership-gate.mjs` passes; the VM `-Up` verification reports `0 copied`.

**Amendment 2026-10-25:** adds the read-only `tools/vm-setup.ps1` (no apply mode, so 223's "no new setup script" narrows to "no mutating setup script"), explicit marker values with ask-once, and three-tier path handling. Claims now also include `tools/vm-setup.ps1`, `.gitignore`, and `docs/agents/structure.md`.

**Claims (recheck at dispatch):** `AGENTS.md`; `docs/agents/working-copy.md`; `tools/sync.ps1` exclusion list; `.gitignore`. **Explicitly not built:** parameterizing `C:\Sprout`/the share; a manifest; `.opencode/skills`/`.agents/skills`; updater code; app behavior.
