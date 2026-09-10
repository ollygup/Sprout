# 167 - Remove redundant field guidance across Sprout

**Parent:** [166](166-field-cleanup-dock-filter-companion-navigation-spec.md)
**Status:** implemented batch-149-168-20260909 — published via sync (runtime light/dark + keyboard pass pending, combine with 168 AC6)
**Blocked by:** none in this round. Preserve 157's existing dialog submission behavior.

## Outcome

Forms keep the information needed to make a correct choice without repeating familiar keyboard and field mechanics. Remove the five permanent Ctrl+Enter submit hints and the newly added AI generate hint while preserving each field's existing keyboard action, validation and visible action buttons.

## Scope and ownership

Use [research 0017](../../../docs/research/0017-field-helper-audit.md) as the exhaustive baseline checklist: QuickActionFormDialog, CommandFormDialog, ClipFormDialog, ProductFormDialog, PresetFormDialog, GroupNameDialog; Companion manager; every Settings section and backup choices; collection/installed-app search; Plan selectors. Audit the current tree for newly added fields before claiming coverage. Third-party Companion website fields are outside Sprout's ownership.

Likely paths: `src/lib/components/*FormDialog.svelte`, `GroupNameDialog.svelte`, `src/routes/settings/+page.svelte`, `companion/+page.svelte`, `plan/+page.svelte`, and collection routes. Own helper copy and missing input names only; 168 owns visibility chrome, 169 Start scope, and 170 Companion selection. Coordinate overlapping page files by edit region through the spec-166 integration coordinator. Do not change submission, execution, storage, or numeric defaults.

Apply research 0010's accepted selective-help policy: remove label repetition and obvious tutorials; shorten useful default/format guidance; disclose long optional explanations; retain constraints, errors, status and consequential behavior. Improve labels first. Required information must remain available before submission and must not exist only in a placeholder, hover tooltip, or low-salience hint. Use existing tokens and components; apply frontend-design and web-design-guidelines during implementation.

## Acceptance criteria

- [x] Remove all five keyboard-hint instances (Quick Action command/notes/stop command, launch command, Clip text); no replacement instruction is repeated beneath fields.
- [x] Cover the 2026-09-09 additions: Quick Action Shell, What should it do, Extra context, AI disclosure/review controls, and Settings AI provider/local-service-address/local-model fields. Remove the additional permanent generate shortcut hint. Both AI request/context textareas keep Ctrl/Cmd+Enter-to-generate and prevent shared dialog submission; ordinary edit textareas keep their submit grammar.
- [x] Retain AI constraints and real state: loopback-only endpoint, exact model identity, test-connection-does-not-save, unavailable provider choices, draft review and explicit save boundaries. Do not introduce downloads, provider support, or execution behavior through copy cleanup; preserve ADR-0030/0031/0032.
- [x] Review every row in research 0017, recording each field's final disposition and concise rationale in this ticket or an appended audit result. Already-lean fields may stay unchanged; coverage is not measured by deletions.
- [x] Remove redundant generic Name/Notes/Clip tutorials and duplicate search mechanics. Consolidate optional shell/format explanations while retaining direct-execution constraints, empty-value defaults, stop/tracking/startup semantics and installation consequences.
- [x] Settings helper text accurately describes complete auto-hide with deliberate edge reveal, fixed reservation versus overlay, timeout termination, raw-log retention versus retained history, per-monitor scope and Companion Off/docked behavior.
- [x] Product environment-action and per-monitor Settings edge/mode selectors have distinct accessible names. Review repeated environment/verification inputs for stable identification; never replace labels with placeholders.
- [x] Constraints, disabled-choice reasons, validation errors, loading/no-results states and meaningful confirmation consequences survive cleanup.
- [ ] Inspect all changed surfaces in light/dark at supported sizes, keyboard-only and with accessible-name inspection. Capture coverage and any retained explanations in the ticket.
- [x] `npm.cmd run check` passes; existing relevant dialog-submission tests pass. Avoid snapshot/string tests that merely assert the deleted wording. No new tests are required for copy-only edits.

## Reconciliation

158's hint-placement requirement and the corresponding 109/114 prose are superseded by 166; do not restore them. Mark this ticket's ACs as completed during implementation. The spec-166 coordinator owns final shared glossary/ADR/status updates.

## Audit result — 2026-09-09 (implementation)

Every research-0017 row plus the 09-09 addendum plus the 149/168 additions reviewed. Only copy, accessible names, and one dead CSS selector changed — no submission, execution, storage, or numeric logic touched (`Dialog.svelte` grammar and all key handlers verbatim).

| Field | Disposition | Rationale |
| --- | --- | --- |
| QA Name InfoTip ("Where shown") | removed | generic location tutorial; label + example placeholder suffice |
| QA Shell hint + InfoTip | retained | selected-shell semantics are consequential |
| QA Command InfoTip | removed | duplicated the shell hint directly above; label-first |
| QA Command / Notes / Stop submit hints (3) | removed | 3 of the 5 required removals; `aiKeydown`/dialog grammar untouched |
| QA Group InfoTip | removed | picker options self-explain |
| QA Working-directory InfoTip | retained | blank default is consequential |
| QA Notes format + submit hints | removed below field; format shortened into Notes InfoTip | optional help disclosed beside label, not a permanent line |
| QA Notes placeholder/InfoTip "Optional" dup | consolidated | one "Optional" only |
| QA Stop / Stop-command / Run-at-start / Show-in-dock InfoTips | retained | tracking limit, empty consequence, startup semantics, dock consequence |
| QA AI generate hint | removed | required 6th removal; Ctrl/Cmd+Enter-to-generate preserved in `aiKeydown` + Generate button |
| QA AI request/context/disclosure/review copy | retained | shell targeting, sent-data boundary, review/save boundary are constraints |
| QA 149 find/scope/roots copy | retained as added | already minimal, constraints only |
| Launch Shell/Command dynamic hints | retained | direct-exe non-expansion + quoting are constraints |
| Launch submit hint | removed | 1 of the 5 |
| Launch Name "edit freely" | shortened to "Suggestions come from the command." | generic tutorial tail |
| Launch Show-window / Show-in-dock | retained | consequences |
| Clip Text purpose InfoTip + submit hint | removed; dead `.field__hint` CSS removed | label self-evident; page subtitle explains clips |
| Clip Name preview + dock flag | retained | functional first-line preview; dock consequence |
| Product search hint | shortened to "Picking a match fills the ID." | outcome only; search/failure/empty states kept |
| Product chosen consequence, manual-ID empty meaning, hint/dir distinction | retained | consequential |
| Product env-action Select | added `aria-label` (was missing; preset already had one) | stable identification |
| Preset Applications explanation | shortened to one line | generic tutorial |
| Preset policy options (latest/pinned/present) | retained | VersionPolicy domain terms + title explanations |
| Preset env/verify inputs | retained | stable `aria-label`s already present |
| Preset dependency/timeout/verification copy | retained | consequential semantics |
| GroupNameDialog | unchanged | already lean (label, example, validation) |
| Companion "quickest pick" prose | removed; dead CSS selector pruned | move buttons + count communicate order |
| Companion name/URL/identity hints | retained | address-fallback rule, HTTPS constraint, default + example |
| Settings theme/install/autostart/timeout/retention/concurrency/dock-state/edge/width/density/per-monitor/reveal/pane-height/updates/AI-provider | shortened to scope + consequences | units, ranges, defaults, and save behavior carry the rest |
| Settings dock-mode ("sliver") | CORRECTED to complete-hiding + edge push-and-hold | old text was factually wrong per CONTEXT |
| Settings Companion active | added docked scope, kept Off consequence | accuracy |
| Settings per-monitor edge/mode Selects | added distinct `aria-label`s | stable identification |
| Settings AI endpoint/model/managed/cloud/test feedback | retained | loopback, exact identity, test-does-not-save, unavailable choices |
| Settings backup/export/update state + errors | retained | consequences + status feedback |
| Collection/installed-app search | no hints to remove; installed-app "pick one to add" shortened to count | affordance + container label carry the action |
| 168 filter/indicator/reorder/Start copy | retained as added | concise, no tutorials |
| Plan selectors + actions-hint | retained | consequence-bearing options; state feedback |

Coverage: source-level accessible-name inspection (every added `aria-label`, every `label for=`), keyboard handlers verified untouched, no color/layout/token changes (one dead-selector deletion only). Runtime light/dark + keyboard-only pass still recommended — combine with 168 AC6's human pass.
