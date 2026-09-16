//! Sprout Mac desktop train (spec 206, ticket 207).
//!
//! Version owner for the `mac-v*` release train. The Mac app content itself
//! (PlatformEngine Mac adapter, shells, frontend flag) lands in tickets
//! 208–215; this crate exists now so the two version lines split in 207:
//! `src-tauri/Cargo.toml` keeps the Windows `win-v*` line, this package owns
//! the Mac `mac-v*` `0.x` line. Either bumps without touching the other.
//!
//! The release contract (tags, assets, updater feed rule) lives in ADR-0012's
//! 2026-09-16 amendment and `docs/release/release-process.md`.

/// The Mac train's tag prefix: release tags are `mac-vX.Y.Z`.
pub const TAG_PREFIX: &str = "mac-v";

/// The Mac train's installer asset suffix (Tauri `dmg` bundle).
pub const ASSET_SUFFIX: &str = ".dmg";

/// The Mac train's version — `src-tauri/mac/Cargo.toml` is the single source
/// of truth for this train, mirroring how the Windows package owns its line.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn train_contract_constants_hold() {
        assert_eq!(TAG_PREFIX, "mac-v");
        assert_eq!(ASSET_SUFFIX, ".dmg");
        // 0.x line per spec 206; three-part semver for the tag gate.
        let parts: Vec<&str> = version().split('.').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "0");
    }
}
