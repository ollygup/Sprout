# 185 — Round verification + integration coordinator (spec-181)

**What to build:** Combined verification for the spec-181 round, plus sole ownership of shared glossary/ADR/research reconciliation, parent status, and cross-ticket integration.

**Blocked by:** 182 (presence contract), 183 (dialog contract), 184 (skill contract). Coordinator-only — no new behavior of its own.

**Status:** ready-for-agent

**Parent:** [181 — Discord + AI two-view + clarify (spec)](181-discord-presence-ai-two-view-clarify-spec.md).

## Scope

- Owns: 183↔184 dialog/skill handoff, backup/size overlap, 172/167 dialog-copy reconciliation, CONTEXT/ADR/research updates, parent status, overlapping-file integration, final combined manual + automated verification. Workers update their own ACs/results; nothing here rewrites their tickets.
- Publishes each completed unit through `node tools/ownership-gate.mjs` (must pass) + verified `tools\sync.ps1 -Up` (twice, expect 0 copied). If run as a concurrent batch, follow `docs/agents/parallel-tickets.md` with this ticket as the single coordinator.
- Explicitly not built: any presence/dialog/skill behavior beyond integration fixes; any new model/runtime qualification (146/150–152/155 stay open).

## ACs

- [ ] 182–184 contracts verified together: static presence + silent-absent + exit-clear; two-view gating/defaults/flips/auto-review; vague→clarify→regenerate with no disclosure widening. Cross-ticket regressions (dialog + skill + presence co-installed) exercised.
- [ ] Full relevant suites green: backend checks/tests as affected, frontend check/build as affected, deterministic fixture battery, packaging check (pinned skills + recommendations served without checkout), size-budget note for presence.
- [ ] Docs reconciled in this unit: CONTEXT planned qualifiers removed where delivered (`AI Mode`, `Clarification`, `Presence activity` — or record what remains planned with spec link); ADR-0028 + ADR-0032 Amendments appended (no original-text rewrite); research 0006 + 0019 extended with dated decision updates (no rule rewrite; new note only for a genuinely new topic); spec-181 acceptance boxes checked as delivered.
- [ ] Manual matrices recorded: presence (Discord open/closed, tray boot, exit), dialog (AI off/on × Add/Edit, apply→review, clarify pick, Dismiss, keyboard-only, light/dark, real DPI), clarify (unknown-folder / ambiguous-app / unknown-prereq answers).
- [ ] Ownership gate passes; each completed unit synced via verified `-Up` (twice, 0 copied); parent spec status updated to implemented with per-ticket evidence links.

## Implementation notes

- Planning discipline: distinguish current / accepted-but-unimplemented / this-round delivery in every status line; record assumed-pending decisions (146/150/152/155 qualification) rather than claiming them. Claims above are estimates to recheck against code at dispatch (CodeGraph first).

## Verification

- This ticket is the verification: gates above + publish receipts. No application behavior changed in this planning session beyond what 182–184 deliver.
