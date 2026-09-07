# 157 — Shared Dialog Enter/Ctrl+Enter submit repair

**What to build:** Own Enter-to-submit explicitly in the shared `Dialog` primitive so every create/edit form submits on Enter from single-line inputs and on Ctrl/Cmd+Enter from textareas — the repair for the reported defect where Enter dismissed every popup with nothing saved. Landed 2026-09-07; supersedes the submit half of 114.

**Blocked by:** None.

**Status:** landed 2026-09-07.

## Scope

- New pure decision helper `src/lib/dialogSubmit.ts` plus its unit tests `src/lib/dialogSubmit.test.ts`; single wiring in `src/lib/components/Dialog.svelte`'s keydown path (`requestSubmit` on the enclosed form, `defaultPrevented` precedence, `preventDefault` pre-empting the native chain for exactly-once submit).
- Explicitly not built: hint lines (carried by 158), any per-form change, any validation change.

## ACs

- [x] Enter in single-line dialog inputs submits (Companion add/edit plus all form dialogs) with identical validation/errors as button submits.
- [x] Plain Enter in textareas inserts a newline; Ctrl/Cmd+Enter submits.
- [x] Fields already consuming Enter (Product search) are unaffected via `defaultPrevented` precedence.
- [x] Escape-cancel, focus trap, and focus return are unchanged.
- [x] `npm.cmd run check` 0 errors and 0 warnings; full `vitest` green (11 files, 143 tests, 6 new); ownership gate pass.

## Implementation notes

- Native implicit submission proved unreliable inside the modal WebView2 dialogs (clicks worked everywhere, Enter never reached submit, no Rust shortcut involved); the explicit `requestSubmit` bypasses it regardless of cause.
- `requestSubmit(submitter)` preserves submitter semantics where a submit button exists; falls back to bare `requestSubmit()` otherwise.

## Verification

- `npm.cmd run test`: 11 files / 143 tests pass; `npm.cmd run check`: 0 errors, 0 warnings; `node tools/ownership-gate.mjs`: pass; sync `-Up` verified at 0 copied.
- On-device manual Enter / Ctrl+Enter pass left to the reporter (no GUI harness in this environment).
