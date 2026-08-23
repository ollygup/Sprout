# 82 — ed25519-signed updates (ADR-0012 model B)

**What to build:** Close ADR-0012's documented gap ("TLS-only integrity until
code signing arrives") with self-managed minisign signatures instead of a CA:
CI signs every setup exe with an ed25519 key held as Actions secrets; the app
verifies against an embedded public key before spawning any downloaded
installer. Decided after the 74 test session exposed that the silent-failure
contract also hides unsigned-supply-chain risk.

**Blocked by:** nothing (follow-up to 73/74; independent of 75–81)

**Status:** ready-for-agent

**Scheme (decided):** tauri/minisign — `tauri signer generate` + `tauri signer
sign`, verified in-app by the `minisign-verify` crate (~0.2.x, tiny, no Tauri
baggage). Chosen over raw ed25519-dalek plumbing; rejected CA/Authenticode for
now (cost/identity paperwork), revisit if distribution widens.

## Work items

- [ ] **Keys (user, one-time):** `npm.cmd run tauri -- signer generate --ci -w <path> --password <pw>` outside any synced folder; private-key file content → GitHub secret `SPROUT_SIGNING_KEY`, password → `SPROUT_SIGNING_PASSWORD`; public key body (`RWR…` line) goes into the new constant below. Never commit or chat-paste the private half.
- [ ] **`release.yml`:** signing step after `npm run tauri build` — pwsh, glob the single `Sprout_*_x64-setup.exe`, run `npx tauri signer sign "<exe>"` with `TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.SPROUT_SIGNING_KEY }}` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.SPROUT_SIGNING_PASSWORD }}`; extend the gh-release `files:` glob to also upload `*.exe.sig`.
- [ ] **`release.yml` hardening:** pin third-party actions (`actions/checkout@v4`, `actions/setup-node@v4`, `dtolnay/rust-toolchain@stable`, `softprops/action-gh-release@v2`) by commit SHA; verify no workflow anywhere uses `pull_request_target`.
- [ ] **`update.rs`:** `const UPDATE_PUBKEY: &str` holding the public-key base64 body; in `apply_update()` between `download_to_file` and `spawn_installer_detached`: fetch `<url>.sig` with the existing `download_agent` into a string (small response), then verify exe bytes via `PublicKey::from_base64` + `MinisigSig::from_string` + `pk.verify`. Fail-closed: empty constant or failed check = honest refusal error surfaced in the confirm dialog, installer never runs.
- [ ] **Tests (fixture-style, no network):** generate two THROWAWAY keypairs locally during implementation; sign a fixed byte string once; embed pubkey + signature text as constants. Cases: genuine bytes verify; one flipped byte rejects; wrong pubkey rejects; empty-key refuses. Only public keys/signatures may be committed.
- [ ] **ADR-0012 amendment:** new section recording scheme B, why (no CA cost/identity, adequate threat coverage: kills local-root-CA MITM injection + post-publish asset substitution; account compromise now requires stealing the Actions secret too), custody rules, residual risks (secret theft; no revocation short of shipping a new pubkey; verification only covers updates, not first installs).
- [ ] **`cargo test` green; `npm run check` clean; synced to the share**

## Rollout rule

The verifying build must ship **before or with** the first signed release:
updated builds refuse any newer-but-unsigned release. So create the two GitHub
secrets BEFORE tagging whatever version first contains this code; from then on
every release must be signed.

## Secrets-in-public-repo ground rules (verified this session)

Fork PRs never receive secrets ✓; solo-maintainer write access is the only
reroute path ✓; risks live in `pull_request_target` workflows (none exist),
unpinned third-party actions (fixed above), and printing secret values to logs.
