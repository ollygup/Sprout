# 183 — Quick Action AI two-view dialog (tabs + single hero + auto-review)

**What to build:** The Add/Edit dialog becomes one dialog with two exclusive views — `AI draft | Manual` tabs top-right — instead of a stacked AI hero above manual fields. AI view is a single textarea; `Use this draft` lands in Manual for full review.

**Blocked by:** None — frontend-only slice on the current tree. Assumes 172 template + 175/176/179/180 dialog sections + 167 copy rules; reconcile at implementation, do not re-add removed hints.

**Status:** ready-for-agent

**Parent:** [181 — Discord + AI two-view + clarify (spec)](181-discord-presence-ai-two-view-clarify-spec.md). Amends [ADR-0028](../../../docs/adr/0028-design-system-disclosure-rules.md) application (dated Amendment in this unit).

## Scope

- Add/Edit dialog template + its caller readiness plumbing only. No backend contract change (`aiGenerateDraft` signature unchanged), no Settings change, no new component/token, no dimension change, no Pre-action/files/editor behavior change.
- Tabs: two 1-word tabs, top-right on-surface, entirely absent when `!aiReady` (fail-closed). Add+ready opens AI; Edit opens Manual. Flips are instant, preserve typed content both ways, and keep both labels visible for scent.
- AI view: one `Describe what to do` textarea + `Generate draft` + outcome region (draft / clarify-choices / refusal / failure) + `Dismiss`. No shell picker, no find/roots chrome up front. Ctrl/Cmd+Enter generates here and never submits the dialog.
- Manual view: today's fields unchanged (Name/Shell/Command + files + `Details` rares + Test/Save + recheck hatch). `Use this draft` applies shell+command, auto-flips to Manual, focuses Command, announces `Applied — review and save`.
- Clarify rendering reuses the find-pick radio pattern where path choices are involved; plain clarify keeps notice rendering (supplied by 184's template; this ticket owns the view, not the skill text).
- Explicitly not built: separate Add-with-AI dialog, durable AI Mode preference, AI in dock/window, 153 revision flow, managed/cloud readiness changes.

## ACs

- [ ] `!aiReady` (provider off / blank model / settings loading-failed): zero AI nodes/text in Add and Edit — the dialog is byte-for-byte the manual form.
- [ ] Add+ready defaults to AI view with only the single textarea + Generate visible; Edit+ready defaults to Manual with the tab strip present; tabs read `AI draft | Manual` with correct selected/announced state.
- [ ] Tab flips preserve in-progress typing both directions; no validation fires on flip; Save stays in Manual semantics (existing validation + candidate recheck + manual-save escape hatch).
- [ ] `Use this draft` applies shell+command, flips to Manual, focuses Command, announces applied status; discard/cancel/fail leaves the saved record intact; no unreviewed fill.
- [ ] Keyboard-only + screen-reader pass: tabs, Generate, outcome, clarify choices, Dismiss all reachable/named; Ctrl/Cmd+Enter vs dialog-submit ownership correct; light/dark + real dock sizes/DPI with no label reflow.
- [ ] `npm.cmd run check` 0 errors; `node tools/ownership-gate.mjs` passes; guidelines review clean (tokens/components only, applied research rules cited in-ticket).

## Implementation notes

- Disclosure/research rules applied: 0004 rule 2 (frequency split), rule 3 (two levels max — view + one disclosure), rule 4 (tab hygiene), rule 5 (applied feedback); 0006 patterns 1/2/3/8/11 (config-elsewhere, minimal-until-content, explicit-setup gating, view-scoped on-surface, content-gated activation); 0008 rules 1–3 (classify per-use view choice, immediacy, scent); 0019 Gmail/HAX/PAIR grammar (inline hero, G8 dismiss, G9 correct-by-edit, staged onboarding). Cite the applied rule per decision.
- Claims above are estimates to recheck against code at dispatch (CodeGraph first).

## Verification

- Frontend dialog tests for gating/defaults/flips/apply-announce/recheck-hatch/shortcut ownership; manual matrix (AI off Add/Edit, AI ready Add/Edit, apply→review, clarify-choices select, Dismiss); `npm.cmd run check`; ownership gate before sync.
