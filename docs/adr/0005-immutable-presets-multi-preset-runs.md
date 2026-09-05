# Presets are immutable; composition happens at run time

> Status: amended 2026-09-05 — original decision text preserved; see the executable-source audit amendment for current behavior and implementation gaps.

Imported presets are stored exactly as authored and nothing merges into stored records; editing always forks. To combine presets (A+B from one, C+D from another), the run screen accepts multiple presets as a union with per-Requirement toggles and explicit conflict resolution for overlapping products with different policies — never silent. A composed run can be saved as a new Preset and exported. Auto-merge semantics would be surprising and effectively irreversible, so they are avoided.

## Amendment — 2026-09-05 (codebase accuracy pass)

Two precision fixes: "editing always forks" is a UI-enforced rule, not a backend invariant (`update_preset` validates but does not refuse imported ids — a direct invoke could overwrite one; the UI is the guard). And "never silent" was overstated — the merge of byte-identical declarations is silent by design (`plan.rs` `merge_requirement`); what stays loud is every conflict. No behavior changed.

## Amendment — 2026-09-05 (executable-source audit)

Imported Presets retain portable authored snapshots, subject to removal of machine-local install directories by `import_preset_file` in `src-tauri/src/import_export.rs`; they are not retained byte-for-byte. `openEdit`, `openFork`, and `save` in `src/routes/presets/+page.svelte` require a fork for imported Presets but edit local Presets in place. The backend update path does not enforce imported immutability. Forking remains the intended imported-edit rule, not a universal requirement for every Preset edit.

Silent composition compares version policy and Step, not the complete Requirement (`src-tauri/src/plan.rs`, `same_declaration` and `merge_requirement`). It keeps the first declaration’s remaining fields, merges env entries by action and name, and unions structurally distinct verify commands. Different env values for the same action/name do not become conflicts. The earlier “byte-identical declarations” wording therefore overstates equality; ADR-0023 records the behavioral implications without silently adopting a new merge policy.
