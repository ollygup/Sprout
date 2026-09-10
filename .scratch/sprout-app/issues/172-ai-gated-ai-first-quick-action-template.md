# 172 — AI-gated, AI-first Quick Action authoring template

**What to build:** The Quick Action Add/Edit dialog shows its AI drafting block only when AI assistance is actually ready, and when ready the AI block leads the dialog instead of hiding mid-form below already-filled manual fields. AI-off authoring is a pure manual form with zero AI chrome.

**Blocked by:** None — frontend-only slice. Assumes tickets 146–148 behavior (existing-local draft → explicit save → user-run) unchanged.

**Status:** implemented — awaiting validation/publish

**Parent:** [145 — AI-assisted Quick Action authoring](145-ai-assisted-quick-action-authoring-spec.md). Follow-on to 148 (which shipped the always-visible mid-form AI Disclosure this corrects).

## Scope

- `QuickActionFormDialog.svelte` (the single Add/Edit template) + its caller `routes/quick-actions/+page.svelte` (supplies readiness). No backend, no other dialog, no Details peek, no Settings changes.
- Readiness is derived from persisted Settings (`ai_provider`/`ai_model`), fail-closed. The backend remains the generation gate; the dialog gate is disclosure only.
- AI still fills shell + command only (unchanged); Name stays manual. Explicit `Use this draft` apply + normal validation/save + `aiCheckCandidate` recheck unchanged (ADR-0030).
- Explicitly not built: ticket 153 revision flow, managed/cloud readiness, 166/167 field-cleanup copy pass, any new component/token, any dialog dimension change.

## ACs

- [x] AI block is entirely absent (no Disclosure, no trigger, no placeholder copy) whenever AI is not ready: provider off/managed/cloud, blank model, or settings still loading/failed.
- [x] When ready, the AI block leads the dialog above Name/Shell/Command: Add opens it expanded, Edit opens it collapsed; manual Name/Shell/Command stay visible below for mandatory validation after apply.
- [x] Rare fields (working directory, Group, Notes, Stop, Run at start, Show in dock) sit behind one collapsed `Details` Disclosure instead of two Disclosures plus flat flags; essential fields and Test/Save behavior unchanged.
- [x] Apply → edit → save still rechecks on AI authority and blocks with the manual-save escape hatch; cancellation/late/outcome handling unchanged; no new execution path (generation never runs/tests/stops).
- [x] `npm.cmd run check` 0 errors; `node tools/ownership-gate.mjs` passes; guidelines review clean on touched files.

## Implementation notes

- Readiness convention: `ai_provider === "existing-local" && ai_model.trim() !== ""`, computed in the page from `getSettings()` alongside the existing Groups setting; dialog takes a plain boolean (fail-closed default false). No settings-shape change.
- Disclosure/research rules applied: 0004 rule 2 (frequency split — AI-first when ready, absent when not), 0004 rule 3 (two levels max — hero + one Details), 0006 pattern 1 (setup stays in Settings; dialog only drafts/reviews), 0006 pattern 3 (explicit-setup gating), 0006 pattern 7 (collapsible sections over splits), 0008 rule 1 (readiness is a global Settings concern, not a per-dialog knob). External entry-point evidence (Gmail Help-me-write inline Insert, Figma Make 0→1 vs polish split, NN/g narrow-scope guidance, Microsoft HAX invocation/dismissal/correction, Google PAIR staged onboarding) is recorded in research 0018; the Notion gating application extends 0006.
- No deviation from the shared design system: existing `Dialog`/`Disclosure`/`Button`/`Notice`/`InfoTip`/`Select`/`TestResult` and tokens only.

## Verification

- `npm.cmd run check`, `node tools/ownership-gate.mjs`; manual: AI off → Add/Edit show no AI text; AI ready → Add shows expanded hero on top, apply prefills shell+command, edited apply rechecks, Details collapses rares; Edit opens AI collapsed.
