# 176 — Pre-action frontend: Advanced section + warn dialog

**What to build:** The visible half of Pre-action: Advanced-collapsed section in the Quick Action Add/Edit dialog + check-first Run UX (warn dialog with fix/run-anyway/cancel) + [Check] affordance.

**Blocked by:** 175 stored shape + outcome contract (do not assume it exists — carve UI-only copy work forward only if explicitly split at dispatch).

**Status:** done — automated green + reporter-run matrix 2026-09-12 (see amendment; [Run anyway]/[Check]-probe/keyboard-SR still owed a human).

**Parent:** [173](173-quick-actions-files-clips-logs-companion-spec.md). Reconcile with [172](172-ai-gated-ai-first-quick-action-template.md) dialog template + [167](167-selective-field-guidance-cleanup.md) copy rules at implementation.

## Scope

- `QuickActionFormDialog.svelte` (+ caller `routes/quick-actions/+page.svelte`, `api.ts`/`types.ts` seam only): one collapsed **Pre-action** section in Advanced/Details carrying **Pre-action check** (multiline command) + **Fix (optional)** (multiline command), with hints from 173; section absent-empty persists nothing. [Check] runs the check timeboxed and renders output like Test (no execution beyond the check).
- Run path: on check-fail payload show a moment-of-use warn dialog (research 0007) with the check output + `[Run fix]` (only when fix present) / `[Run anyway]` / `[Cancel]`; `[Run fix]` runs fix once and returns to the dialog (no auto-continue); `[Run anyway]` runs main; `[Cancel]` does nothing. State-changing runs give feedback (research 0004 rule 5 — never silent).
- Design system only: tokens + `Dialog`/`Disclosure`/`Button`/`Notice`/`InfoTip`/`TestResult` (research 0005 rules 1/5/6); exactly one primary per dialog; no new component/token/dimension without a reviewed deviation. **UI-heavy rule (binding): first apply docs/agents/ui-ux.md + `docs/research/0004,0005,0006,0007,0008` and cite the applied rules in the done notes; if none fits, do own primary-source research and record it as a new/ extended note under `docs/research/`.**
- Explicitly not built: backend chain (175), files/editor (179/180), image Clips (178), Companion (177).

## ACs

- [x] Pre-action section collapsed by default in Advanced/Details; empty = no payload change; check-only and check+fix persist per 175 validation (fix-without-check blocked with plain error). — implemented, queued for coordinator validation (check + manual Add/Edit matrix).
- [x] [Check] timeboxes, shows output, never runs main or fix. — implemented via the timeboxed Test path over the check text only; queued for coordinator validation.
- [x] Run with failing check opens the warn dialog with full check output; button set matches fix presence; each button does exactly its label; focus/keyboard/screen-reader operation holds; no silent path. — implemented; [Run anyway] = fresh Run (chain re-checks; see deviation note below); focus/keyboard/SR via shared Dialog; queued for coordinator validation.
- [x] Copy follows 167 (constraints/defaults/errors kept, tutorials trimmed); no ad-hoc styling; `npm.cmd run check` 0 errors; guidelines review clean. — validated 2026-09-11: check 0/0 on the combined tree, ownership gate pass, touched files reuse tokens/components only with one primary per dialog; manual Add/Edit + Run-matrix + keyboard/SR/light-dark pass still pending a runtime run.

## Implementation notes

- Frequency split (0004 rule 2): Pre-action hidden until needed (Advanced-collapsed); warn dialog at moment of use (0007); two disclosure levels max (0004 rule 3); visibility-on-surface vs authoring-elsewhere (0006 pattern 1) — dialog authors, page lists.
- Claims (dialog/component names, 172 template interplay) to recheck via CodeGraph at dispatch.

## Verification

- `npm.cmd run check` + ownership gate; manual: Add/Edit check-only + check+fix + empty; Run pass/fail/fix/anyway/cancel matrix; keyboard-only + light/dark; dialog copy review against 167.

## Done notes (worker-176 implementation, awaiting coordinator validation)

Applied research rules: 0004 rule 2 (Pre-action behind Details-collapsed; empty section persists nothing), 0004 rule 3 (two disclosure levels max — Details then Pre-action), 0004 rule 5 (Started flash, inline fix verdict, error lines — no silent path), 0005 rules 1/5/6 (Dialog/Disclosure/Button/Notice/InfoTip only, same-kind controls, component-owned rhythm) plus one primary per dialog (Run anyway; 0005 rule 2 / 0006 pattern 6 one accent), 0006 pattern 1 (Check probe + result inline where authored; warn dialog at the Run site, authoring stays in the dialog), 0006 pattern 7 (collapsible sections), 0007 (moment-of-use warn dialog for the per-run check decision), 0008 rule 1 (pre-action fields classified as per-action authoring data — inline fields, not a feature switch). No existing rule conflicted, so no new research note was created.

What changed: `src/lib/types.ts` (+`pre_check`/`pre_fix` on `QuickActionInput`, +`PreCheckReport`/`PreFixResult`/`QuickActionRunOutcome`); `src/lib/api.ts` (`runQuickAction` now resolves the outcome union, +`runQuickActionFix`); `QuickActionFormDialog.svelte` (nested collapsed Pre-action section with check/fix fields, [Check] probe over the Test path, fix-without-check refusal in the backend's wording, trimmed-or-null payload on Add/Edit); `routes/quick-actions/+page.svelte` (Run handles Started with a notice vs CheckBlocked with the warn dialog; Run fix once with inline verdict and no auto-continue; Run anyway as a fresh Run; Cancel/Escape/X close with no effect). New coverage: `src/lib/preActionDialog.close.test.ts` (5 cases). No new component/token/dimension — new CSS classes reuse existing token values only.

Deviation / contract gap (needs coordinator ruling, no backend touched): the 175 contract offers no bypass seam — `run_quick_action` always check-first — so [Run anyway] issues a fresh Run (the chain re-checks; a still-failing check reopens the warn dialog with fresh output). A true "run main despite a failing check" needs a backend force path (e.g. a force flag on `run_quick_action`); the frontend call site is isolated to `runAnyway` in `+page.svelte` if that lands. Related: the Quick Launch window caller ignores the new outcome payload (silent no-op on a blocked check there) — untouched per scope, flagged as follow-up.

## Amendment — 2026-09-12 (reporter-run matrix, verified against run logs)

- Combined "FilesDir demo" action (check `exit 1` → fix → check `exit 0`): three blocked runs show pre-check `exit 1` with no main section; the fix ran once under its own header (`exit 0`) with main held; the pass run shows pre-check `exit 0`, main ran once (`exit 0`), `files: 1 attached, staged for this run`, and the staged bytes round-tripped exactly into the probe file.
- Still owed a human: [Run anyway] fresh-run behavior, the [Check] probe button, keyboard-only + screen-reader + light/dark passes, Add/Edit matrix.
