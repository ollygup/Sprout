# Plan merges identical declarations, conflicts on differences, never downgrades

Composing presets is a pure function over their resolved Requirements, grouped by product. Same product with the same policy and step merges into one row (env and verify lists unioned, first declaration wins the rest) — the only silent case, safe because either declaration alone would do the same thing. Same product with a different policy or step is an explicit `conflict`: the row names its sources and candidates, offers no auto-pick, and blocks the run and the save-as-preset until the user picks one policy or excludes the row. A pinned requirement with a newer version installed is `SatisfiedByNewer` — reported, never downgraded to the pin. An installed-but-unmanaged product is an unmanaged skip, never force-adopted. A non-Windows preset file imports for reference with a warning and never plans cleanly. A Requirement whose product left the Library is `unresolved`: shown as "removed from library" with no candidates and excluded from detection and runs until the product is re-added — and it re-lights automatically when the id returns.

## Consequences

- The Plan screen's gating (`undecidedCount` blocks Start and Save) is a direct rendering of this semantics, not extra UI policy.
- Merge-then-conflict keeps the common case (two presets both wanting latest Git) to one row while keeping the dangerous case (pinned vs latest) always explicit.
