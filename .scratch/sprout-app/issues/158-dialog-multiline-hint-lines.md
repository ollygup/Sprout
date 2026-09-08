# 158 — Ctrl+Enter hint lines under dialog multi-line fields

**What to build:** A one-line hint beneath every textarea in a Dialog-hosted form stating the submit combination, reusing the existing `field__hint` treatment. The combination itself already works via 157; this advertises it per research 0010. Completes the remainder of 114.

**Blocked by:** 157 (landed — the grammar this advertises).

**Status:** ready-for-agent

## Scope

- Audit each Dialog form's textareas (Quick Action command plus notes, Clip content, launch command, and any multi-line Product/preset/group-name fields) and add the hint line in existing hint styling; no behavior change.
- Explicitly not built: copy changes beyond the hint, any submit/validation/trap behavior (owned by 157).

## ACs

- [x] Every textarea in a Dialog form shows the submit-combination hint in existing hint styling.
- [x] Single-line fields gain no hint (native Enter needs none).
- [x] `npm.cmd run check` 0 errors; related frontend tests green.

## Amendment — 2026-09-08 (hint requirement superseded)

The user accepted removing the repeated Ctrl+Enter hint lines in spec 166 (166-field-cleanup-dock-filter-companion-navigation-spec.md). Preserve submission behavior owned by 157; the old requirement to show a hint beneath every textarea is historical and must not be reimplemented. Source audit found all five hints already present, so removing them is pending cleanup, not a claim that they never shipped. Other scope and historical checkboxes above are unchanged.
