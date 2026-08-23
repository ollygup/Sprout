# Self-update from GitHub Releases

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
