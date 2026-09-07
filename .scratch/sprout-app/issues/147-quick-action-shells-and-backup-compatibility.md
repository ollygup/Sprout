# 147 — PowerShell/CMD Quick Actions with compatible backup semantics

**What to build:** Let a user create, edit, save, manually test/run/stop, export, and restore a Quick Action under an explicit PowerShell or CMD shell, while existing PowerShell actions keep their behavior.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

**Parent:** [145 — AI-assisted Quick Action authoring](145-ai-assisted-quick-action-authoring-spec.md).

## ACs

- [ ] Add explicit PowerShell/CMD selection through the existing authoring, validation, persistence, and manual execution flow. Legacy database actions default to PowerShell; unknown stored/input shell values fail honestly rather than falling back.
- [ ] Run, user-operated Test, and stop commands use the action's shell and working directory through the current Windows execution owner. Preserve hidden execution, existing privilege policy, logs, run/stop events, and watchdog behavior; do not create a second command runner.
- [ ] Extend create/update collision and backup merge identity with shell while preserving existing command/cwd normalization within each shell. The same text under different shells can coexist; true duplicates retain existing messages and import skip counts.
- [ ] Advance new backup exports to envelope version 2 and require valid explicit shell values in version-2 Quick Actions. Accept version 1 as legacy PowerShell; reject inconsistent version-1 CMD declarations. Keep one evolving format, the same five collections, and selective export semantics.
- [ ] Demonstrate that the legacy version-1 reader rejects version-2 exports rather than silently interpreting CMD text as PowerShell. Unknown future versions and malformed shells fail before any merge writes; failed restore remains transactional.
- [ ] Preserve action identity/order, Group membership, note, auto-run, and stoppable behavior through migration and update. Existing user-selected startup execution remains unchanged.
- [ ] Cover both shells, cwd/quoting/multiline behavior, start/stop/test results, old-database migration, duplicate cases, legacy import, new export round-trip, and rollback with targeted tests; retain relevant existing tests.

## Verification

Demo a benign PowerShell action and a benign CMD action with identical text where meaningful; restart and backup/restore them. Verify the manual controls and shell labels using keyboard access. Run affected frontend/backend checks and the ownership gate.

## Implementation notes

This is the compatibility prerequisite, not AI generation. Do not add direct-executable/no-shell Quick Actions, elevation, or an AI execution tool. The accepted ADR-0017/0014/0026 amendments describe the reason for this change.
