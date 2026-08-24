# 96 — Self-update refuses every signed installer ("the downloaded signature could not be read")

**What to build:** Fix the update-apply path so releases produced by CI verify and install. Two halves:

1. **App side:** `verify_installer_signature` (`src-tauri/src/update.rs`) fed `tauri signer sign` output straight into `minisign_verify::Signature::decode`, which only parses bare minisign text. New `decode_signature` helper accepts both shapes — Tauri's base64-wrapped sidecar format and bare minisign files — refusing everything else (fail-closed unchanged).
2. **CI side:** `release.yml` now unwraps the `.sig` to bare minisign text before upload, because every verifier generation currently deployed (0.4.7/0.4.8) can *only* parse the bare form — this is what lets those installs self-update to v0.4.9 without a manual reinstall.

**Blocked by:** 82 (introduced the verification path)

**Status:** done — 349 backend tests green incl. 3 new; real v0.4.8 artifacts proven through a pinned-version harness (wrapped→VERIFIED under fixed code, wrapped→DECODE-FAILED under deployed code, raw→VERIFIED under deployed code, tampered→refused)

- [x] Regression fixture: real v0.4.8 `.sig` asset captured verbatim (`update/fixtures/ci-signature-v048-wrapped.txt`, public data)
- [x] Failing tests written first and watched red: `the_real_ci_signature_asset_clears_the_decode_stage` reproduced the exact production message; `tauri_wrapped_signatures_verify_end_to_end` failed pre-fix
- [x] `decode_signature` accepts base64-wrapped and bare minisign text, trims whitespace, refuses garbage — error message unchanged
- [x] `release.yml` sign step unwraps the sidecar in place (fails loudly on unexpected payload)
- [x] Cargo.toml bumped to 0.4.9 (single source of truth)
- [x] Full `cargo test` green (347 passed + new), harness proof matrix against real installer bytes

## Diagnosis record (ticket 96, 2026-08-24)

### Symptom (user)

Device on 0.4.7 offered the 0.4.8 update; confirming it fails with "the downloaded signature could not be read — refusing to run the installer". The 0.4.6→0.4.7 update had worked.

### Root cause

`tauri signer sign` writes `<file>.sig` as **one base64 line wrapping the minisign text** (Tauri's updater sidecar format). Both v0.4.7 and v0.4.8 shipped that shape (verified by downloading the assets: single-line base64; inner text is well-formed minisign with key id matching the embedded `UPDATE_PUBKEY`). `update.rs` passed the payload straight to `minisign_verify::Signature::decode`, which requires the bare `untrusted comment:`/base64/trusted-comment/base64 layout → decode always failed. Never caught earlier because:

- The fixture `FIXTURE_SIG_A` was hand-assembled in the *bare* format — not what the tool emits.
- No install ever exercised the path: 0.4.6 (no verifier) performed the 0.4.6→0.4.7 update; 0.4.7's verifier saw its first live bytes on the 0.4.8 attempt.

### Why re-tagging v0.4.8 would not have helped

The blocker for deployed devices is the artifact **format**, not the version number: their embedded verifiers reject any correctly-wrapped signature regardless of tag. Shipping v0.4.9 with CI emitting the bare form makes 0.4.7/0.4.8 installs self-update directly; forward-only tags keep release history intact per `docs/release/release-process.md`.
