# Presets are immutable; composition happens at run time

> Status: amended 2026-09-05 — original text preserved below; the correction is in the Amendment section.

Imported presets are stored exactly as authored and nothing merges into stored records; editing always forks. To combine presets (A+B from one, C+D from another), the run screen accepts multiple presets as a union with per-Requirement toggles and explicit conflict resolution for overlapping products with different policies — never silent. A composed run can be saved as a new Preset and exported. Auto-merge semantics would be surprising and effectively irreversible, so they are avoided.

## Amendment — 2026-09-05 (codebase accuracy pass)

Two precision fixes: "editing always forks" is a UI-enforced rule, not a backend invariant (`update_preset` validates but does not refuse imported ids — a direct invoke could overwrite one; the UI is the guard). And "never silent" was overstated — the merge of byte-identical declarations is silent by design (`plan.rs` `merge_requirement`); what stays loud is every conflict. No behavior changed.
