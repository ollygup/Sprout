# Releases — agent reference

> Read this file when: you cut a release, bump the app version, or build a
> local installer artifact. Otherwise skip it.

- MUST NOT hand-build installers on this device. Releases are GitHub Releases built by CI — the whole flow (pre-flight gates, version bump in Cargo.toml, sync, tag push, passive self-update) lives in `docs/release/release-process.md`; MUST follow it instead of building locally. `npm.cmd run tauri build` stays available for the rare local artifact, with the same pre-flight gates.
