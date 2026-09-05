# Machine-local stays local; backups merge by identity; lists stay ordered

One portability rule generalizes ADR-0009 and ADR-0014: Launch entries, Quick Actions, Clips, Groups, Companion state, dock memory, Settings, and install directories are machine-local — never part of a Preset, a Plan payload, a Run record, or a `.sprout.json` export. Whole-app backup is the only thing that carries them, as one versioned `sprout-backup` document where a selective export is the same shape with empty arrays (no second format, ever). Restore is a merging import in one transaction: records whose identity already exists are skipped and counted, never overwritten; a halfway failure leaves nothing behind. Identities are per-collection and stable across machines: Products and Presets by id, Launch entries by kind+target, Quick Actions by command+cwd, Clips by trimmed text. Install directories are stripped on the way out and on the way in. Ordered lists (entries, actions, clips, groups) share one discipline: append at `MAX+1`, update in place, delete-then-compact, positions gapless and internal-only — reorders go through move operations, never through payload edits.

## Consequences

- A file that leaves the machine (preset or backup) can never smuggle another machine's paths, window state, or settings.
- Dedup and merge never need fuzzy matching: identity is declared per collection, case rules included (case-insensitive where the OS is, byte-exact where the user typed).
- Future collections extend the backup document additively (one array + one checkbox), never as a new format.
