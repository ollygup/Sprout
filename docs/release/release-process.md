# Release process

Division of labor between this working copy (`C:\Sprout`) and the git master
(the shared folder; remote: https://github.com/ollygup/Sprout).

## Roles

- **Working copy / agent sessions**: write code and docs, bump versions,
  sync with `tools\sync.ps1`. This device has no git installed and performs
  no git operations of any kind.
- **User / share side**: all git operations — commit, tag, push — run
  manually outside agent sessions.

## One-time setup (done)

`.github/workflows/release.yml` lives on the master (added via GitHub web UI,
pulled into the share, synced down here). On a `v*` tag push it:

1. Fails unless the tag equals `src-tauri/Cargo.toml`'s `version`.
2. Runs `npm ci` → `npm run tauri build` on `windows-latest`.
3. Publishes `Sprout_<version>_x64-setup.exe` to a GitHub Release.

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
  the updater matches it and the workflow globs it.
