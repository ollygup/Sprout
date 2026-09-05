# Requirements are live-linked to Library products

> Status: amended 2026-09-05 — original decision text preserved; see the executable-source audit amendment for current behavior and implementation gaps.

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

## Amendment — 2026-09-05 (executable-source audit)

Local Presets store stripped Product placeholders keyed by id, not literally a JSON object containing only `id`: default/empty Product fields also serialize (`src-tauri/src/db.rs`, `preset_to_row`; `src-tauri/src/domain.rs`, `Product`). Read resolution fills current name, winget id, install-location hint, and install directory; the Requirement’s env and verify declarations remain its own. Ordinary Product deletion transactionally removes local Requirements, so re-adding a Product revives only dangling references that still exist, not rows already pruned by deletion.

`unresolved` is derived and cleared during normal local persistence, but serde omits only false values. True values can serialize, and imported payloads bypass local normalization and Library resolution (`resolve_requirements`, `preset_to_row`, and `import_preset_file`). “Never persisted” is therefore not an enforced invariant for imported data.

`compute_plan` in `src-tauri/src/lib.rs` resolves Library Products and excludes unresolved Requirements from detection. `launch_run` instead filters submitted unresolved flags and writes the submitted Requirements without resolving current Library membership again; `run_worker` executes that snapshot. A request loaded before a Product edit/deletion can consequently become stale. Preventing that remains an enforcement gap against live-reference intent.

`RunRecord` and `RequirementOutcome` in `src-tauri/src/run.rs` preserve per-Requirement outcomes and captured Product identities/names, not complete resolved Requirements. Full Requirements are in the per-run request document, subject to log-directory retention. Export omits unresolved Requirements, but may subsequently fail validation if a retained Requirement still depends on an omitted Product (`export_preset`/`export_to_json` and `Preset::validate`). Product deletion does not rewrite those dependency references. These are limitations of the current implementation, not authorization to discard dependency intent.
