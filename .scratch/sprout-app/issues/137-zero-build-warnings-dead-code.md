# 137 — Zero build warnings: delete the dead, annotate the test-only

**What to build:** The build goes warning-free with no behavior change: truly-dead code is deleted, test-only helpers are honestly marked, and every remaining suppression states its reason.

**Blocked by:** none — can start immediately.

**Status:** ready-for-agent

## Scope

- Rust only (frontend `svelte-check` already reports 0 errors / 0 warnings — keep it so).
- Delete: the unused import in the Store module's test scope; the never-constructed Store enumerator seam pair (trait + struct) whose comment claims a seam nothing takes as a parameter.
- Keep honestly: the companion clamp helper (only caller is its own test; production clamps in the Settings page) and the three walker snapshot/merge helpers (only callers are walker tests; live `snapshot()` uses the three-way merge directly) — via `cfg(test)` scoping or `allow(dead_code)` with a one-line `// test seam` reason each, not silent allows.

## ACs

- [x] `cargo check` reports 0 warnings; `npm.cmd run check` still 0 errors / 0 warnings.
- [x] No behavior change: full `cargo test` slice for settings/store/walker green; app launches, Settings loads/saves, discovery snapshot still merges.
- [x] No `allow` without a reason comment naming the seam/test that needs it; no dead code kept "just in case".

## Implementation notes

- Prefer deletion over suppression everywhere the symbol has no caller and no ticket keeps it; prefer `cfg(test)` over `allow` where the helper exists only for tests.
- Keep the diff to the warning sites — no drive-by refactors.

## Verification

- `cargo check` (0 warnings), `npm.cmd run check` (0/0), targeted `cargo test` (settings, store, walker).
