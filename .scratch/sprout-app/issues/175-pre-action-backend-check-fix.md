# 175 — Pre-action backend: check + fix storage, chain, logs, backup

**What to build:** The stored + executed half of the Pre-action section: per-Action optional `pre_check` + optional `pre_fix`, check-first run chain reusing the timeboxed Test path, same-log headers, backup/merge coverage.

**Blocked by:** None — contract owner for 176. Assumes ticket 147 shell semantics (selected shell + cwd normalization) unchanged in behavior.

**Status:** done — contract verified in code 2026-09-12 (stored shape, check-first chain, CheckBlocked payload, backup fields); batch validation recorded 585 tests incl. 12 pre-action + checks/gate.

**Parent:** [173](173-quick-actions-files-clips-logs-companion-spec.md)

## Scope

- `src-tauri/src/quick_actions.rs` + `db.rs` migrate + `lib.rs` commands + `windows_execution/` (extend the single invocation owner per ADR-0029 — no second spawn/quote site): stored shape `pre_check: Option<String>` (trimmed, empty→None) + `pre_fix: Option<String>` (trimmed, empty→None, only meaningful with a check); validation (check required once section used; fix without check refused); collision/identity unchanged except backup-identity extension below.
- Run chain: Run-click runs `pre_check` first under the Action's shell+cwd with the existing `test_quick_action` timebox (10s); exit 0 → continue to main; non-zero/timeout → stop before main and return structured warn payload (check output + has_fix). `pre_fix` runs only via explicit `[Run fix]` command, same shell/cwd/timebox, appended to the same `logs/quick-actions/<run>/output.log` with distinct headers (`pre-check`, `pre-fix`, `action`) + exit lines (ADR-0017 logging). No elevation change, no detached-tracking change, `auto_run` fires the same chain as Run.
- `backup.rs`: additive backup coverage for the two fields in the same `sprout-backup` document (ADR-0014 — no second format); merge identity extends to shell+command+cwd per the accepted 0026 amendment; portable-form stripping unchanged.
- Explicitly not built: any dialog/UI (176), files/image-Clips (178/179), Companion (177), editor highlighting (180).

## ACs

- [x] Empty section persists no trace (both None); check-only persists check; check+fix persists both; fix-without-check refused with a plain error.
- [x] Check pass → main runs once; check fail/timeout → main never spawns, warn payload carries trimmed output + has_fix + durations.
- [x] `[Run fix]` with no fix configured refused; with fix runs once, logged under its own header; main still requires a fresh Run (no silent auto-continue).
- [x] Same shell+cwd honored for check/fix/main; unknown shell fails honestly; log created best-effort per 0017 audit (no guaranteed-log claim).
- [x] Backup export/import round-trips both fields; merge skips on extended identity with honest counts; selective export shape unchanged (empty arrays, never absent keys).
- [x] Rust tests for validate/chain/merge + `npm.cmd run check` clean where touched; `node tools/ownership-gate.mjs` passes.

## Validation (batch batch-174-175-177-178-179-20260911, 2026-09-11)

- Merged in staging with 179 (shared `quick_actions.rs` owner): 175's stored shape/chain/headers kept verbatim; `append_action` returns the new row id (179's contract) with 175's 12-column params; `windows_execution/` untouched (full reuse, ADR-0029). Coordinator repairs: `groups.rs` + `ai_assist.rs` test literals gained `pre_check: None, pre_fix: None` (mechanical, recorded in batch ledger).
- `cargo test`: 585 passed, 0 failed, 3 ignored (full suite, incl. 12 pre-action tests). `cargo check`: 0 errors, 0 warnings. `npm.cmd run check`: 0/0. Ownership gate: pass. Manual Tauri-command matrix (check-only fail blocks main; check+fix fix-on-click; pass runs main once; backup round-trip) pending a runtime pass.

## Implementation notes

- Codebase-design: keep the seam at `quick_actions` (interface = stored shape + check/fix/run outcomes); `windows_execution` stays the only Windows-invocation owner; deletion test — removing this module must remove the chain, not scatter it across callers. Reuse `test_quick_action_with_timeout`, `normalized_cwd`, `spawn_quick_action` paths.
- Research overturns architecture only with evidence; record any in the ticket.

## Verification

- `cargo test` (quick_actions/backup scopes) + `npm.cmd run check` + ownership gate; manual via Tauri commands: check-only fail blocks main with output; check+fix offers and runs fix on click; pass runs main once; backup round-trip preserves fields.
