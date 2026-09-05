# Machine-local stays local; backups merge by identity; lists stay ordered

> Status: amended 2026-09-05 — original decision text preserved; see the executable-source audit amendment for current behavior and implementation gaps.

One portability rule generalizes ADR-0009 and ADR-0014: Launch entries, Quick Actions, Clips, Groups, Companion state, dock memory, Settings, and install directories are machine-local — never part of a Preset, a Plan payload, a Run record, or a `.sprout.json` export. Whole-app backup is the only thing that carries them, as one versioned `sprout-backup` document where a selective export is the same shape with empty arrays (no second format, ever). Restore is a merging import in one transaction: records whose identity already exists are skipped and counted, never overwritten; a halfway failure leaves nothing behind. Identities are per-collection and stable across machines: Products and Presets by id, Launch entries by kind+target, Quick Actions by command+cwd, Clips by trimmed text. Install directories are stripped on the way out and on the way in. Ordered lists (entries, actions, clips, groups) share one discipline: append at `MAX+1`, update in place, delete-then-compact, positions gapless and internal-only — reorders go through move operations, never through payload edits.

## Consequences

- A file that leaves the machine (preset or backup) can never smuggle another machine's paths, window state, or settings.
- Dedup and merge never need fuzzy matching: identity is declared per collection, case rules included (case-insensitive where the OS is, byte-exact where the user typed).
- Future collections extend the backup document additively (one array + one checkbox), never as a new format.

## Amendment — 2026-09-05 (executable-source audit)

`BackupDocument` in `src-tauri/src/backup.rs` contains only Products, Presets, Launch entries, Quick Actions, and Clips. Groups/memberships, Settings, Companion state, dock memory, and Run history are excluded; install-directory overrides are stripped on export and import. Backup does preserve launch targets and desktop assignments plus Quick Action commands and working directories, so it is not a blanket filter for machine paths or desktop references. The original claim that whole-app backup carries all listed machine-local state is inaccurate.

The local Plan-to-worker request may carry per-Product install overrides inside Requirements (`src-tauri/src/lib.rs`, `launch_run`; `src-tauri/src/run.rs`, override selection). The install-directory portability rule is enforced at exported-document normalization, not by absence from local execution payloads. It does not sanitize arbitrary paths inside authored commands, targets, or working directories.

`merge` restores in one non-overwriting transaction and skips duplicates already present or already encountered in the file. Its identities are: Products by id **or trimmed case-insensitive name**; Presets by id; Launch entries by kind plus trimmed case-folded target; Quick Actions by trimmed case-folded command plus normalized case-folded working directory; Clips by trimmed byte-exact text. In particular, Quick Action command identity is not byte-exact.

The three item lists use `OrderedList` to maintain gapless positions. Groups use collection-scoped ordering: empty-group sweeps in `src-tauri/src/groups.rs` can leave gaps, whereas explicit moves renumber. The blanket gapless claim for every group position is not implemented. This records the difference without approving a change to the ordering goal.
