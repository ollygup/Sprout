# 73 — Self-update check backend (GitHub Releases)

**What to build:** The Rust half of ADR-0012: query the repo's Releases API,
decide whether a newer release exists, and apply it — download the setup exe
and hand off to the vendored NSIS passive-update path. All networking in Rust;
CSP untouched.

**Blocked by:** 72 — the spec fixing transport (ureq, no updater plugin), the
silent-failure contract, and the asset-name pattern

**Status:** done - `update.rs` (ureq/rustls, silent checks) wired through `check_for_update`/`install_update` and the startup emit thread; fixture-tested

- [x] `ureq` dependency (rustls) added; size-budget note kept honest against spec NFR 43
- [x] Update module with origin constant (`https://api.github.com/repos/ollygup/Sprout/releases/latest`) + required User-Agent header
- [x] Pure functions over data: strip-`v` semver triple comparison; release-JSON parsing into (tag, notes); asset selection matching `Sprout_*_x64-setup.exe` (first match) — tested against recorded GitHub-API fixtures, no network in tests
- [x] `check_for_update` command returning current version + newer-release info or "none"; startup background thread runs it once and emits a single `update-available` event with {version, url}
- [x] Silent-failure contract: offline, 403/404 (private repo), malformed payload → treated as up-to-date, never an error surface
- [x] `install_update(url)` command: streams the asset to `%TEMP%\<asset-name>`, spawns the installer detached with `/UPDATE /P /R`, exits the app shortly after so NSIS can replace the exe (template's running-app macro covers the race)
- [x] `cargo test` green (fixture tests for compare/parse/pick incl. prerelease-looking tags ignored); `npm run check` untouched-side clean; synced to the share
