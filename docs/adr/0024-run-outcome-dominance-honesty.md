# Run verdicts dominate Failed over With-notes over Ok; cancel lands between Requirements

A Run's overall Outcome derives from its per-Requirement results by dominance: any `Failed` or timed-out step → `Failed`; else any unmanaged skip → `With notes`; else `Applied` (everything installed, upgraded, or already satisfied — nothing needed attention). `Cancelled` happens only between Requirements — the user touching `cancel` sets a marker the worker checks after the in-flight step, so the running installer always completes (a hung one is still killed by its own timebox) and the verdict records where the run stopped. Env wiring and verify commands run only after a successful step, never after a failure and never for already-satisfied rows. When an install directory was requested and the product resolves elsewhere, the detail appends the post-install honesty note ("installer ignored the requested directory"). Every failed Requirement's log ends with a verdict trailer after a `--- sprout ---` separator; already-satisfied rows write no log at all.

## Consequences

- A run is only ever "clean" when nothing needed attention — `With notes` is a completed run the user must still read.
- History replays outcomes and details; the requested directory itself is not a Run column (machine-local), only the honest landing note persists.
