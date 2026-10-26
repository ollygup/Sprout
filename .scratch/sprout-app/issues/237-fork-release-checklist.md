# 237 — Fork release checklist

**Parent:** [223 — Forkable ruleset (spec)](223-forkable-ruleset-spec.md)

**What to build:** A human checklist for publishing a Sprout fork: redirect the updater to the fork, decide signing/identity, avoid installer/app collisions with upstream, tag and verify a release, and confirm an upstream release cannot overwrite the fork. This is guidance, not a workspace mode and not an updater code change.

**Blocked by:** 235 — Workspace marker and VM overlay.

**Status:** deferred 2026-10-26 — skipped by user decision; not built. 239 no longer depends on this ticket, and its fork-checklist check was dropped rather than left waiting on a document that will not exist. The gap stays deliberate: a fork that publishes releases unmodified can still be overwritten by upstream (223's Further Notes, "Fork updater risk").

- [ ] `docs/release/forking.md` explains that a fork is a repository origin, not a workspace mode; a fork can be standalone or VM (only if the hardcoded VM paths actually match).
- [ ] The checklist covers redirecting the updater's repository slug and any embedded owner identity to the fork, and verifying no upstream `ollygup/Sprout` URL or upstream key remains.
- [ ] The checklist covers signing/identity: sign the fork's installer or clearly document that it is unsigned; avoid installer/app-identity collisions if upstream and the fork are installed side by side.
- [ ] The checklist covers tag/release verification: `Cargo.toml` remains the single version source; the fork's updater sees the fork release; an upstream release cannot overwrite the fork.
- [ ] The checklist says never to commit `.sprout-workspace`, local app data, logs, or the database.
- [ ] The doc is linked from the release docs (or the release index) and does not change `update.rs`, the updater trust model, or any runtime behavior.
- [ ] A maintainer can follow the checklist end to end for a test release without touching upstream.

**Claims (recheck at dispatch):** new `docs/release/forking.md`; a link from the existing release docs. **Explicitly not built:** fork-aware updater code; release automation; a fork workspace mode; signing infrastructure.
