# 82 — ed25519-signed updates (ADR-0012 model B)

**What to build:** Close ADR-0012's documented gap ("TLS-only integrity until
code signing arrives") with self-managed minisign signatures instead of a CA:
CI signs every setup exe with an ed25519 key held as Actions secrets; the app
verifies against an embedded public key before spawning any downloaded
installer. Decided after the 74 test session exposed that the silent-failure
contract also hides unsigned-supply-chain risk.

**Blocked by:** nothing (follow-up to 73/74; independent of 75–81)

**Status:** implemented — awaiting the first signed release to go live
(secrets created by the user; next tag ships the verifying build + first
signed installer together)

**Scheme (decided):** tauri/minisign — `tauri signer generate` + `tauri signer
sign`, verified in-app by the `minisign-verify` crate (~0.2.x, tiny, no Tauri
baggage). Chosen over raw ed25519-dalek plumbing; rejected CA/Authenticode for
now (cost/identity paperwork), revisit if distribution widens.

## Work items

- [x] **Keys (user, one-time):** user generated their own keypair (`tauri signer generate`, key id `5345CD6883CC4501`) outside any synced folder; private-key file content → GitHub secret `SPROUT_SIGNING_KEY`, password → `SPROUT_SIGNING_PASSWORD` (created 2026-08-24); public-key body embedded into `UPDATE_PUBKEY` by the agent. Private half never entered the repo, share, chat, or agent context.
- [x] **`release.yml`:** signing step after `npm run tauri build` — pwsh, globs the single `Sprout_*_x64-setup.exe`, runs `npx tauri signer sign "<exe>"` with `TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.SPROUT_SIGNING_KEY }}` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.SPROUT_SIGNING_PASSWORD }}`; asserts the `.sig` was produced; gh-release `files:` uploads `*.exe.sig`.
- [x] **`release.yml` hardening:** third-party actions pinned by commit SHA (`actions/checkout@11d5960…`, `actions/setup-node@49933ea…`, `dtolnay/rust-toolchain@4360b52…` stable head, `softprops/action-gh-release@3bb1273…` v2); verified `.github/workflows/` contains only `release.yml` — no `pull_request_target` anywhere.
- [x] **`update.rs`:** `const UPDATE_PUBKEY: &str` added (currently **empty** = fail-closed until the real key body lands); `apply_update()` fetches `<url>.sig` via `download_to_string`/`download_agent` between download and spawn and calls `verify_installer_signature(UPDATE_PUBKEY, exe_bytes, sig_text)` — `PublicKey::from_base64` + `Signature::decode` + `pk.verify(..., allow_legacy=false)` (0.2.5 API; tauri emits pre-hashed `ED` signatures). Fail-closed: empty constant, unreadable key/sig, or failed check = honest refusal error surfaced through the confirm dialog; installer never spawns.
- [x] **Tests (fixture-style, no network):** two throwaway keypairs generated during implementation, fixed byte string signed once each; pubkey bodies + signature text embedded as constants (private halves deleted). Cases: genuine bytes verify; one flipped byte rejects; wrong pubkey rejects; empty key refuses closed; unreadable signature text refuses. Only public material committed.
- [x] **ADR-0012 amendment:** scheme-B section recorded — why (no CA cost/identity; kills local-root-CA MITM injection + post-publish asset substitution; account compromise now requires stealing the Actions secret too), custody rules, residual risks (secret theft; no revocation short of shipping a new pubkey; covers updates, not first installs). `docs/release/release-process.md` gained the secrets pre-flight.
- [x] **`cargo test` green; `npm run check` clean; synced to the share** — 341 tests pass (incl. `the_shipped_pubkey_is_a_valid_minisign_key` guarding the embedded constant), svelte-check 0 errors; synced up and re-verified `0 copied`.

## Validation

- **Key-match proof without exposing the private half:** the signature file
  embeds the signer's key id. Signing anything with the private key and
  comparing that id against `5345CD6883CC4501` (from the `.pub`) proves the
  pair matches what CI will use.
- **True end-to-end:** the first release built after this code lands — CI's
  sign step fails hard if either secret is missing/misspelled, and an
  installed build refuses a bad or missing `.sig`, so a successful
  tag → signed-release → in-app update cycle is the final proof.

## Rollout rule

The verifying build must ship **before or with** the first signed release:
updated builds refuse any newer-but-unsigned release. So create the two GitHub
secrets BEFORE tagging whatever version first contains this code; from then on
every release must be signed.

## Secrets-in-public-repo ground rules (verified this session)

Fork PRs never receive secrets ✓; solo-maintainer write access is the only
reroute path ✓; risks live in `pull_request_target` workflows (none exist),
unpinned third-party actions (fixed above), and printing secret values to logs.
