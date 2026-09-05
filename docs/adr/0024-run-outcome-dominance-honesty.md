# Run verdicts dominate Failed over With-notes over Ok; cancel lands between Requirements

> Status: amended 2026-09-05 — original decision text preserved; see the executable-source audit amendment for current behavior and implementation gaps.

A Run's overall Outcome derives from its per-Requirement results by dominance: any `Failed` or timed-out step → `Failed`; else any unmanaged skip → `With notes`; else `Applied` (everything installed, upgraded, or already satisfied — nothing needed attention). `Cancelled` happens only between Requirements — the user touching `cancel` sets a marker the worker checks after the in-flight step, so the running installer always completes (a hung one is still killed by its own timebox) and the verdict records where the run stopped. Env wiring and verify commands run only after a successful step, never after a failure and never for already-satisfied rows. When an install directory was requested and the product resolves elsewhere, the detail appends the post-install honesty note ("installer ignored the requested directory"). Every failed Requirement's log ends with a verdict trailer after a `--- sprout ---` separator; already-satisfied rows write no log at all.

## Consequences

- A run is only ever "clean" when nothing needed attention — `With notes` is a completed run the user must still read.
- History replays outcomes and details; the requested directory itself is not a Run column (machine-local), only the honest landing note persists.

## Amendment — 2026-09-05 (executable-source audit)

For runs without observed cancellation, `execute_run_observed` in `src-tauri/src/run.rs` derives Failed, then WithNotes for unmanaged skips, then Ok. Observed cancellation currently takes precedence over all completed results, including previous failures. The marker is checked before each Requirement, not again after the final one; a cancellation arriving during the last Requirement need not become the overall verdict. Failure dominance across cancellation is therefore not implemented as the title implies.

Directory, env, and verification notes do not themselves promote Ok to WithNotes. A clean verdict consequently does not prove the absence of notes requiring attention. The stated honesty goal remains an obligation beyond the current classifier.

`finalize_step` appends the verdict trailer for failed or timed-out install/upgrade steps. `finish_step` runs verification afterward: verification failure sets Failed and replaces detail, without adding a `--- sprout ---` trailer, and may erase earlier directory/env notes. The universal failed-Requirement trailer and retained-honesty statements remain implementation gaps. Successful-step-only postprocessing and no logs for already-satisfied rows remain implemented.
