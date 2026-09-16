# Release process

Division of labor between this working copy (`C:\Sprout`) and the git master
(the shared folder; remote: https://github.com/ollygup/Sprout).

Dual trains (spec 206, ticket 207, ADR-0012 amendment 2026-09-16): Windows
ships `win-v*`, Mac ships `mac-v*`. Either platform can release alone, or
both together from one commit.

## Roles

- **Working copy / agent sessions**: write code and docs, bump the releasing
  train's version, sync with `tools\sync.ps1`. This device has no git
  installed and performs no git operations of any kind.
- **User / share side**: all git operations — commit, tag, push — run
  manually outside agent sessions.

## Version sources

Each train owns its version line; either bumps without touching the other:

| Train | Version source | Tag | Assets |
| --- | --- | --- | --- |
| Windows | `src-tauri/Cargo.toml` (`version`) | `win-vA.B.C` must equal it | `Sprout_*_x64-setup.exe` + `.sig` |
| Mac | `src-tauri/mac/Cargo.toml` (`version`) | `mac-vX.Y.Z` must equal it | `Sprout_*_aarch64.dmg` + `.sig` |

`tauri.conf.json` carries no version (auto-inherits the Windows package's at
build time). Bare `v*` tags are retired: CI ignores them and both updaters
ignore them.

## One-time setup

`.github/workflows/release.yml` holds two jobs behind one tag trigger
(`win-v*`, `mac-v*`):

- `release-windows` (`win-v*` → `windows-latest`): fails unless the tag
  equals `win-v` + the Windows package version; runs `npm ci` →
  `npm run tauri build`; signs the setup exe with the update-signing key
  (ADR-0012 scheme B); publishes the exe and its `.sig` to a GitHub Release.
- `release-mac` (`mac-v*` → `macos-latest`): fails unless the tag equals
  `mac-v` + the Mac package version; builds, signs the dmg, and publishes
  the dmg and its `.sig` the same way. (Mac bundle wiring lands in tickets
  215/218; the job defines the contract now.)

**Update-signing secrets (before the first release that carries signature
verification):** create both as repository Actions secrets
(Settings → Secrets and variables → Actions) from your own
`tauri signer generate` keypair:

- `SPROUT_SIGNING_KEY` — the entire content of the private `.key` file.
- `SPROUT_SIGNING_PASSWORD` — its password.

The private half never enters this repo, the share, or chat; the public-key
body goes into `UPDATE_PUBKEY` in `src-tauri/src/update.rs`. From that first
verified release onward every release must be signed — older app builds
refuse newer-but-unsigned installers. Both trains share the one keypair.

## Every release

1. Finish work in `C:\Sprout`; bump `version` in the releasing train's
   Cargo.toml (Windows: `src-tauri/Cargo.toml`; Mac:
   `src-tauri/mac/Cargo.toml`). For a shared fix, bump both in one commit.
2. `tools\sync.ps1 -Up`, then again expecting `0 copied`.
3. User, on the share side: commit, create the namespaced tag(s), push with
   tags:
   - Windows-only fix: `win-vA.B.C` matching the Windows version.
   - Mac-only fix: `mac-vX.Y.Z` matching the Mac version.
   - Shared fix: both tags on the same commit (`win-vA.B.C` +
     `mac-vX.Y.Z`); each tag triggers its own workflow run and its own
     Release.
4. GitHub Actions builds and publishes the Release(s).
5. Installed apps list recent Releases at startup; the rail footer pill
   offers the update only when the app's own train has something newer;
   confirming downloads the installer and applies it passively
   (`/UPDATE /P /R`) — see ADR-0012. Each platform ignores the other's
   tags and assets, so a Mac-only fix never nags Windows users and vice
   versa.

## Notes

- The update check is inert while the repo is private; it activates when the
  repo goes public. Offline or failed checks silently read as "up to date".
- Release assets must keep their exact per-train name patterns; each
  updater matches only its own, and each workflow job globs only its own
  (the `.sig` rides along).
- The update check is TLS-only at the metadata level but the installer itself
  is minisign-verified before it runs — see ADR-0012's amendment.
- Never push a bare `v*` tag expecting a release: nothing triggers on it.
