# 152 — Choose a stronger model without automatic replacement

**What to build:** Offer the qualified stronger tier alongside the lightweight model and preserve explicit installed selections across downloads, switches, failures, and later Sprout releases.

**Blocked by:** 151 — managed lightweight setup and bundled resource format.

**Status:** ready-for-agent

**Parent:** [145 — AI-assisted Quick Action authoring](145-ai-assisted-quick-action-authoring-spec.md).

## ACs

- [ ] Reuse 151’s verified installation, lifecycle and authoring integration for the stronger compatible model. Selection changes qualified model/configuration. Test per-model context/template requirements and reject compatibility that JSON alone cannot supply.
- [ ] Show both qualified tiers with model identity, source/license, download size, and measured suitability. Present the stronger model as a user choice, not an automatic consequence of detected hardware.
- [ ] Install and verify the stronger model through the managed flow. Switching is explicit; failed installation or activation retains a usable previous selection and reports the failure without cloud fallback.
- [ ] Manage installed inventory separately from the release's recommendation list. A new app release can recommend a different model without downloading it, switching selections, or deleting an installed model.
- [ ] An installed model missing from the new recommendations remains identifiable. Incompatibility is reported with deliberate recovery choices; it cannot silently map to a new model ID or corrupt existing inventory.
- [ ] Bundled JSON and fixed skills change only with normal app releases/version tags. There is no raw-repository JSON polling, separate catalog server/publication, independent catalog signing, or remote skill update.
- [ ] Catalog schema validation rejects duplicate IDs, invalid artifact metadata, unsupported runtime requirements, and executable installation instructions. Exact artifact hashes remain tied to the requested selection.
- [ ] Verify an old/new bundled-catalog fixture migration, lightweight-to-stronger switching, insufficient memory, removed recommendations, failed downloads, and offline startup without losing saved Quick Actions.
- [ ] Document the maintainer workflow for qualifying a replacement, updating bundled data/resources/notices, and publishing an app release. Record that editing repository JSON alone does not update installed users.

## Verification

Use two qualified artifacts for manual selection tests where hardware permits; use deterministic compatibility/resource fixtures for failures and older/newer app-catalog transitions. Check that default installation remains weight-free.

## Implementation notes

This ticket does not implement remote recommendation updates. Release mechanics remain owned by the established app-update process; do not introduce another signing-key lifecycle.
