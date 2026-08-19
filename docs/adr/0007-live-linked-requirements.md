# Requirements are live-linked to Library products

Local presets no longer embed a stale copy of a product; each Requirement is a live reference to a Library Product by id, resolved at plan and run time. Deleting a Product drops the Requirements that referenced it from local presets (with an impact count in the delete prompt), imported presets keep their authored snapshot, and a Requirement whose product has left the library is flagged "removed from library" and excluded from runs until the product is re-added.

## Why

The Library is the user's source of truth: they edit product names, winget ids, and install-location hints there, and a Product is the unit of deletion. When a preset embedded a full snapshot, edits and deletions silently diverged from every preset that referenced them — a product could be deleted and its presets kept running stale steps forever. Live linking makes the Library authoritative again, at the cost of resolving references at read time.

## Decisions

- **Stored shape**: local presets store each Requirement's product stripped to a reference — `{ "id" }` — with name and winget step resolved from the Library on every read (`list_presets`, `get_preset`). Serialization of a reference-only product is possible because `Product.name` defaults on deserialize.
- **Unresolvable references**: a Requirement whose product id has no Library match reads back flagged `unresolved` (never persisted; `skip_serializing_if` keeps files clean). It passes validation as a bare reference, shows as "product removed from library", is excluded from Plan detection and from runs, and becomes live again automatically when a product with the same id is re-added.
- **Imported presets** (ADR-0005) keep their embedded snapshot: nothing resolves against the Library and the delete never touches them. The imported flag in the database is what distinguishes the two storage paths.
- **Deletion**: `delete_product` drops the Requirement from local presets that reference the product, in the same transaction. The delete prompt reports the count of affected local presets via a read-only impact query first.
- **Export** is a point-in-time snapshot: it resolves references against the current Library, and requirements whose product left the library are left out of the file.
- **Run history** is untouched: runs persist the already-resolved requirements they executed.

## Consequences

- Editing a Product now propagates everywhere it is referenced — the composer, plan, and run always show current names and steps.
- Deleting a Product has a visible footprint: presets it was in lose the requirement (local) or silently keep a snapshot (imported).
- Stored preset payloads are smaller and reference-only, but legacy presets saved before this change (full snapshots) are handled identically: they read back resolved, and their stored payload is normalized on the next save.