# 67 — Version single-source + constants domain split

**What to build:** The app version is stated in exactly one file and never
duplicated again; UI-geometry constants live in the domain-split `constants/`
layout the conventions prescribe.

Today the version string appears three times: `Cargo.toml` (whose comment
invertedly names tauri.conf.json as the source of truth), `tauri.conf.json`
(which should omit it entirely — Tauri v2 auto-inherits the package version),
and `package.json` (npm metadata nobody reads). Separately,
`window_constants.rs` sits at the src root instead of under a `constants/`
module directory.

Structural only — zero behavior change.

**Blocked by:** None — can start immediately.

**Status:** done — tauri.conf.json version key removed (config validated by the
tauri-build compile), Cargo.toml comment now names itself as the source,
package.json + lockfile root de-versioned, `window_constants.rs` moved to
`src-tauri/src/constants/window.rs` with a domain-split `mod.rs` (imports
updated in lib.rs/quick_window.rs/appbar.rs, no content change), AGENTS.md
rule path updated; 292 backend tests green, svelte-check 0 errors

- [x] `tauri.conf.json` contains no `version` key; config still valid
- [x] `Cargo.toml` keeps the version; its comment names Cargo.toml itself as
      the single source of truth
- [x] `package.json` no longer carries an app version (lockfile root entry
      refreshes on next install — acceptable churn)
- [x] Repo-wide grep finds no hardcoded version string outside Cargo.toml
      (build outputs excluded)
- [x] Window geometry constants moved under `constants/` with imports updated;
      no content change beyond location
- [x] AGENTS.md's design-rule path reference updated to the new location
- [x] `cargo check` clean, `cargo test` green, `npm.cmd run check` 0 errors
