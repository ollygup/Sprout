# 167 - Remove redundant field guidance across Sprout

**Parent:** [166](166-field-cleanup-dock-filter-companion-navigation-spec.md)
**Status:** ready-for-agent
**Blocked by:** none in this round. Preserve 157's existing dialog submission behavior.

## Outcome

Forms keep the information needed to make a correct choice without repeating familiar keyboard and field mechanics. Remove the five permanent Ctrl+Enter hint lines while leaving Enter/newline, Ctrl+Enter submission, validation and visible submit buttons unchanged.

## Scope and ownership

Use [research 0017](../../../docs/research/0017-field-helper-audit.md) as the exhaustive baseline checklist: QuickActionFormDialog, CommandFormDialog, ClipFormDialog, ProductFormDialog, PresetFormDialog, GroupNameDialog; Companion manager; every Settings section and backup choices; collection/installed-app search; Plan selectors. Audit the current tree for newly added fields before claiming coverage. Third-party Companion website fields are outside Sprout's ownership.

Likely paths: `src/lib/components/*FormDialog.svelte`, `GroupNameDialog.svelte`, `src/routes/settings/+page.svelte`, `companion/+page.svelte`, `plan/+page.svelte`, and collection routes. Own helper copy and missing input names only; 168 owns visibility chrome, 169 Start scope, and 170 Companion selection. Coordinate overlapping page files by edit region through the spec-166 integration coordinator. Do not change submission, execution, storage, or numeric defaults.

Apply research 0010's accepted selective-help policy: remove label repetition and obvious tutorials; shorten useful default/format guidance; disclose long optional explanations; retain constraints, errors, status and consequential behavior. Improve labels first. Required information must remain available before submission and must not exist only in a placeholder, hover tooltip, or low-salience hint. Use existing tokens and components; apply frontend-design and web-design-guidelines during implementation.

## Acceptance criteria

- [ ] Remove all five keyboard-hint instances (Quick Action command/notes/stop command, launch command, Clip text); no replacement instruction is repeated beneath fields.
- [ ] Review every row in research 0017, recording each field's final disposition and concise rationale in this ticket or an appended audit result. Already-lean fields may stay unchanged; coverage is not measured by deletions.
- [ ] Remove redundant generic Name/Notes/Clip tutorials and duplicate search mechanics. Consolidate optional shell/format explanations while retaining direct-execution constraints, empty-value defaults, stop/tracking/startup semantics and installation consequences.
- [ ] Settings helper text accurately describes complete auto-hide with deliberate edge reveal, fixed reservation versus overlay, timeout termination, raw-log retention versus retained history, per-monitor scope and Companion Off/docked behavior.
- [ ] Product environment-action and per-monitor Settings edge/mode selectors have distinct accessible names. Review repeated environment/verification inputs for stable identification; never replace labels with placeholders.
- [ ] Constraints, disabled-choice reasons, validation errors, loading/no-results states and meaningful confirmation consequences survive cleanup.
- [ ] Inspect all changed surfaces in light/dark at supported sizes, keyboard-only and with accessible-name inspection. Capture coverage and any retained explanations in the ticket.
- [ ] `npm.cmd run check` passes; existing relevant dialog-submission tests pass. Avoid snapshot/string tests that merely assert the deleted wording. No new tests are required for copy-only edits.

## Reconciliation

158's hint-placement requirement and the corresponding 109/114 prose are superseded by 166; do not restore them. Mark this ticket's ACs as completed during implementation. The spec-166 coordinator owns final shared glossary/ADR/status updates.
