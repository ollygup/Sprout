# Self-update from GitHub Releases

> Status: amended 2026-09-16 — original decision text preserved; see the executable-source audit amendment for current behavior and implementation gaps.

Sprout ships as an NSIS installer published to GitHub Releases per version tag, and updates itself: at startup (and on demand from Settings) the backend asks the repo's Releases API for the latest tag, compares it against the Cargo.toml version, and — when newer and the user confirms — downloads the setup exe to %TEMP% with `ureq` and runs it passively (`/UPDATE /P /R`), exiting so the installer replaces it in place.

## Why

The spec's v1 Out-of-Scope listed self-update; distributing via GitHub Releases reverses that. The obvious machinery is tauri-plugin-updater, but it mandates signed update manifests — signing-key infrastructure for an app whose own binaries are unsigned, in a size-budgeted binary. Until code signing exists, signature verification of updates protects nothing the installer itself doesn't already lack.

## Decisions

- **Hand-rolled check, standard install**: all networking lives in Rust (`ureq`, blocking, rustls) so the CSP stays untouched; applying an update reuses the vendored NSIS template's existing `/UPDATE` passive path unchanged.
- **Startup-only, silent failure**: offline, private-repo 404/403, and any error count as "up to date" — startup checks never nag. A manual re-check lives in Settings.
- **One affordance**: the rail footer's version text becomes an update pill when a newer release exists; clicking confirms, downloads, and relaunches.
- **CI enforces the version contract**: a release build fails unless the pushed `vX.Y.Z` tag equals Cargo.toml's version — the single source of truth that makes the comparison sound.
- **TLS-only integrity** until code signing arrives (ADR-level acceptance).

## Consequences

- Release assets must stay machine-matchable: `Sprout_<version>_x64-setup.exe`.
- Adding signature verification later is additive — swap the downloader, keep the flow.
- The check is inert while the repo is private; going public activates it, no auth machinery needed either way.

## Amendment — ed25519-signed updates (scheme B)

The "TLS-only integrity" acceptance above is superseded while still deferring
paid code signing. Release CI signs every setup exe with a minisign/ed25519
key (`tauri signer sign`) held as Actions secrets, uploads the `.sig` beside
the asset, and the app verifies the download against a public key embedded at
build time (`UPDATE_PUBKEY` in `update.rs`, checked by the tiny
`minisign-verify` crate) before the installer ever spawns. Fail-closed: an
empty embedded key, a missing signature, or any failed check refuses the
install with an honest error in the confirm dialog — a build carrying this
code accepts only genuinely signed newer releases.

### Why

Authenticode/CA signing was rejected on cost and identity paperwork, and its
absence had left two holes: a MITM who installs a local root CA could swap the
download despite TLS, and anyone able to replace a published release asset
could do the same after the fact. Self-managed minisign signatures close both
with no third party: a valid installer now needs the private key, so repo or
account compromise alone is no longer enough to ship a malicious update — the
attacker must also steal the Actions secret. tauri-plugin-updater's manifest
machinery stays rejected; the hand-rolled flow keeps its shape and gains one
verification step between download and spawn.

### Custody rules

- The private key exists only inside the user's own generation environment
  (outside any synced folder) and as the repository Actions secrets
  `SPROUT_SIGNING_KEY` / `SPROUT_SIGNING_PASSWORD`. Never committed, never
  pasted into chat, logs, or tickets.
- The public-key body is embedded in `update.rs` and may be committed freely;
  the signing workflow fails hard when either secret is absent, so unsigned
  releases cannot ship.
- The verifying app build must reach users before or together with the first
  signed release — older builds refuse everything newer anyway once they carry
  this code.

### Residual risks

- Secret theft: whoever extracts the Actions secret can sign convincing
  updates. Mitigations are procedural (third-party actions pinned by commit
  SHA, no `pull_request_target` workflows anywhere, secrets never printed).
- No revocation: distrusting a leaked key means shipping a new public key in
  an app update.
- Scope: verification covers self-updates only — first installations still
  rest on TLS plus trust in the GitHub release page.

## Amendment — 2026-09-05 (executable-source audit)

The signed-update architecture and fail-closed verification remain implemented in `src-tauri/src/update.rs`. “One affordance” is outdated: the rail pill and Settings install action both use `src/lib/updateState.svelte.ts` and confirmation before installation. The verifier accepts bare minisign and Tauri’s base64-wrapped signature sidecars; `.github/workflows/release.yml` unwraps sidecars before publication for older verifiers.

The release workflow invokes signing and checks for a signature sidecar; it does not independently assert that both secret strings are nonempty. Runtime verification still precedes installer spawn, and the `/UPDATE /P /R` handoff remains implemented. The workflow source shows references to signing secrets and pinned actions, not proof of their actual values, custody, or the contents of published releases. Key custody and repository privacy are operational claims outside this source-only audit; the custody rules remain obligations. This amendment does not revisit the signed-update decision.

## Amendment — 2026-09-16 (dual-train releases, ticket 207)

One bare-`v*` line can no longer carry two platforms: a Mac-only fix must not
offer Windows users a phantom update, and a shared fix must ship both
installers from one commit. Releases move to namespaced trains; the
hand-rolled check, silent-failure contract, affordances, and scheme-B
minisign verification are unchanged.

### Why

Spec 206 ships a Quick-access-only Mac app from the same repo. The single
`Cargo.toml` version with its bare-`v*`-must-equal-version gate has nowhere
for a Mac version to live and no way to ship one platform alone. Splitting
the version line and namespacing tags gives each platform an independent
release cadence while keeping one repo, one workflow file, and one signing
key.

### Contract

- **Versions split (workspace).** `src-tauri/Cargo.toml` keeps the Windows
  line (currently `1.0.4`); `src-tauri/mac/Cargo.toml` owns the Mac `0.x`
  line (starting `0.1.0`). Either bumps without touching the other. Each
  train's Cargo.toml is that train's single source of truth.
- **Tags namespaced.** Windows ships `win-vA.B.C` (must equal the Windows
  package version); Mac ships `mac-vX.Y.Z` (must equal the Mac package
  version). Bare `v*` is retired: no CI trigger matches it and both
  updaters ignore it.
- **Assets per platform.** Windows publishes `Sprout_*_x64-setup.exe` +
  `.sig` (unchanged); Mac publishes `Sprout_*_aarch64.dmg` + `.sig`.
- **CI builds on tag prefix** (`release.yml`): `win-v*` runs
  `release-windows` on `windows-latest`; `mac-v*` runs `release-mac` on
  `macos-latest`. Each job fails unless its tag equals its train's version.
  Pushing both tags — even on one commit — produces two workflow runs and
  two independent Releases.
- **Updater feed filter** (`src-tauri/src/update.rs`): the check lists
  recent releases (`/releases?per_page=100`) instead of `/releases/latest`
  and offers only the running build's train — same-prefix tag, newer
  semver, usable same-train asset, highest version wins. The other train's
  tags/assets and bare `v*` never surface. Table-tested with recorded
  shapes; no network in tests.
- **Signing unchanged.** Both trains sign with the same
  `SPROUT_SIGNING_KEY` / `SPROUT_SIGNING_PASSWORD` secrets and verify
  against the same embedded `UPDATE_PUBKEY`; the `.sig` sidecar rides
  beside each asset exactly as before.

### Consequences

- `docs/release/release-process.md` is rewritten for namespaced tags; the
  old "tag `vX.Y.Z` matching Cargo.toml" gate is retired everywhere it was
  stated (workflow, ADR text above stays as history).
- The two version numbers never converge by process — independence holds
  until any future parity decision (spec 206 out of scope).
- Mac bundle wiring (dmg target, frontend Mac flag) is not this amendment:
  it lands in tickets 215/218, which own the first installable Mac
  release.
