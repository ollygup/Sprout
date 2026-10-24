# 236 — VM-only docs and docs-first skill rewrite

**Parent:** [223 — Forkable ruleset (spec)](223-forkable-ruleset-spec.md)

**What to build:** The docs carry the rules, and skills are optional aids. `conventions.md` defines the deep-module vocabulary and deletion test in the repo; `ui-ux.md` states the design-system/accessibility rules without requiring global skills; `toolchain.md` explains that `C:\Sprout` is the VM path and a standalone clone substitutes its own; and the VM-only workflow docs are marked as such.

**Blocked by:** 235 — Workspace marker and VM overlay.

**Status:** ready-for-agent

- [ ] `conventions.md` replaces the `codebase-design` skill mandate with repo-owned definitions of module/interface/seam/adapter/depth and the deletion test; the existing `shared/` and Windows-owner rules are unchanged.
- [ ] `ui-ux.md` replaces the `web-design-guidelines`/`frontend-design` skill mandate with the repo-owned rules: tokens/components, no ad-hoc values, standing research, deviation review, accessibility/contrast/keyboard/focus. It states explicitly that the skills are optional aids when installed.
- [ ] `toolchain.md` notes that `C:\Sprout` is the VM path; a standalone clone substitutes its own path; the commands themselves are unchanged; the ownership gate is path-agnostic.
- [ ] `parallel-tickets.md`, `worker.md`, `releases.md`, and `docs/release/release-process.md` carry a short "VM workspace only — read only when `.sprout-workspace` is `vm`" banner; their content is otherwise unchanged.
- [ ] No `MUST USE <skill>` line remains in `conventions.md` or `ui-ux.md`; a docs grep finds none.
- [ ] No `.opencode/skills`, `.agents/skills`, or manifest is added. `.opencode` remains OpenCode tooling.
- [ ] A standalone reader can follow every universal rule without a global skill install; a VM reader still reaches the sync/no-git rules through the overlay.
- [ ] Documentation links resolve; `node tools/ownership-gate.mjs` passes; no app behavior changes.

**Claims (recheck at dispatch):** `conventions.md`; `ui-ux.md`; `toolchain.md`; the four VM-only docs. **Explicitly not built:** changing the substance of the VM sync/no-git rules; new skill folders or a manifest; app code; a new rulebook.
