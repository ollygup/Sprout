# 154 — Disable or remove owned AI resources without losing actions

**What to build:** Let users revoke discovery access, disconnect a provider, disable AI, and remove specifically selected Sprout-owned inference resources while keeping their saved Quick Actions and user-managed installations intact.

**Blocked by:** 150 — cloud credentials/consent; 151 — managed runtime; 153 — revision state.

**Status:** ready-for-agent

**Parent:** [145 — AI-assisted Quick Action authoring](145-ai-assisted-quick-action-authoring-spec.md).

## ACs

- [ ] Managed resource removal is fixed trusted Sprout behavior over explicitly selected owned artifacts, never an AI-generated cleanup script or general deletion tool exposed to inference. It creates no exception for destructive AI authoring.
- [ ] Disabling AI or disconnecting a route cancels associated active requests and prevents late results from updating drafts or saved actions. Manual editing and Run/Stop of saved actions still work without a provider.
- [ ] Disconnecting a cloud account removes its stored credential and applicable consent records through their owners. Errors are honest; secrets never appear in logs or whole-app export.
- [ ] Revoking a discovery root invalidates related pending target references and prevents subsequent access. Closing/discarding the authoring session clears transient prompts, selected error context, and candidate state; no hidden persistent conversation archive remains.
- [ ] Provide deliberate removal of chosen managed model/runtime artifacts with a preview of owned targets and released space where known. Validate actual ownership/paths, stop only the owned runtime when needed, and never recursively delete a user-managed installation.
- [ ] When applying Off to managed AI, stop the Sprout-owned inference server and release its model memory in addition to cancelling requests. Offer an explicit choice to keep installed downloads for later reuse or remove them; Off alone must not silently delete files. Keep the existing Settings Save/Discard behavior and make removal a deliberate, previewed action.
- [ ] The removal preview and operation cover the selected model weights, Sprout-owned inference server, bundled runtime libraries/dependencies, and associated app-owned download archives, partial downloads and caches. Retain artifacts required by any managed model the user keeps. Removing all managed models can remove the entire owned inference installation. Do not remove system GPU drivers, shared system prerequisites, or independently installed local services/models; preserve saved Quick Actions.
- [ ] Interrupted cleanup, runtime crash, missing/corrupt artifacts, or stale inventory produce recoverable state without deleting saved actions or treating a partial installation as ready. An ordinary app update does not perform unsolicited model removal.
- [ ] Backups contain saved Quick Action command/shell content but no credentials, provider settings, discovery grants/maps, transient prompts, model files, or skills. Normal backup/import behavior for unrelated collections is preserved.
- [ ] Verify multi-route switching/disconnect, active-request cancellation, root revocation, late completion, missing key deletion, interrupted managed removal, and foreign-resource preservation. Document operational metadata retention/redaction and bounded transient-data behavior.
- [ ] Keep existing general app-uninstall/data-retention policy; do not silently expand the uninstaller's deletion scope or alter the default preservation of user data.

## Verification

Use isolated owned-resource fixtures and protected credential-store test doubles, plus targeted benign Windows checks where needed. Assert exact ownership and surviving saved actions; never test removal against a real user-managed model directory.

## Implementation notes

This slice owns user-visible revocation and cleanup, not a general cleanup of the repository. Model privacy and ownership claims must remain true during failures, not only successful requests.

## Clarification — 2026-09-11

The user explicitly requested a removal option when managed setup is turned
off, including the model, server and its dependencies. The two added ACs make
that scope explicit within this existing ticket; no new cleanup ticket is
needed. They remain unimplemented. Verify both keep/remove paths, absence of a
running owned server after Off, complete removal of exclusively owned runtime
dependencies, and preservation of dependencies needed by retained models.

Apply research 0004 rule 2 and 0007's moment-of-use disclosure: reveal the owned
target/space preview when removal is requested, rather than permanently listing
every artifact in Settings. This clarification does not relax the existing
150/151/153 dependencies or claim the full ticket can close before those
provider and revision lifecycles exist.
