# Release process

Division of labor between this working copy (`C:\Sprout`) and the git master
(the shared folder; remote: https://github.com/ollygup/Sprout).

## Roles

- **Working copy / agent sessions**: write code and docs, bump versions,
  sync with `tools\sync.ps1`. This device has no git installed and performs
  no git operations of any kind.
- **User / share side**: all git operations — commit, tag, push — run
  manually outside agent sessions.

## One-time setup

`.github/workflows/release.yml` was added via GitHub web UI, pulled into the
share, synced down here. On a `v*` tag push it:

1. Fails unless the tag equals `src-tauri/Cargo.toml`'s `version`.
2. Runs `npm ci` → `npm run tauri build` on `windows-latest`.
3. Signs the setup exe with the update-signing key (ADR-0012 scheme B).
4. Publishes `Sprout_<version>_x64-setup.exe` and its `.sig` to a GitHub Release.

**Update-signing secrets (before the first release that carries signature
verification):** create both as repository Actions secrets
(Settings → Secrets and variables → Actions) from your own
`tauri signer generate` keypair:

- `SPROUT_SIGNING_KEY` — the entire content of the private `.key` file.
- `SPROUT_SIGNING_PASSWORD` — its password.

The private half never enters this repo, the share, or chat; the public-key
body goes into `UPDATE_PUBKEY` in `src-tauri/src/update.rs`. From that first
verified release onward every release must be signed — older app builds
refuse newer-but-unsigned installers.

## Every release

1. Finish work in `C:\Sprout`; bump `version` in `src-tauri/Cargo.toml`
   (the single source of truth — nowhere else states it).
2. `tools\sync.ps1 -Up`, then again expecting `0 copied`.
3. User, on the share side: commit, create tag `vX.Y.Z` matching Cargo.toml,
   push with tags.
4. GitHub Actions builds and publishes the Release.
5. Installed apps check the Releases API at startup; the rail footer pill
   offers the update; confirming downloads the installer and applies it
   passively (`/UPDATE /P /R`) — see ADR-0012.

## Notes

- The update check is inert while the repo is private; it activates when the
  repo goes public. Offline or failed checks silently read as "up to date".
- Release assets must keep the exact name pattern `Sprout_*_x64-setup.exe`;
  the updater matches it and the workflow globs it (its `.sig` rides along).
- The update check is TLS-only at the metadata level but the installer itself
  is minisign-verified before it runs — see ADR-0012's amendment.
