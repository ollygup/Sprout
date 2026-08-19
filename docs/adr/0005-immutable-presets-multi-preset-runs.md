# Presets are immutable; composition happens at run time

Imported presets are stored exactly as authored and nothing merges into stored records; editing always forks. To combine presets (A+B from one, C+D from another), the run screen accepts multiple presets as a union with per-Requirement toggles and explicit conflict resolution for overlapping products with different policies — never silent. A composed run can be saved as a new Preset and exported. Auto-merge semantics would be surprising and effectively irreversible, so they are avoided.
