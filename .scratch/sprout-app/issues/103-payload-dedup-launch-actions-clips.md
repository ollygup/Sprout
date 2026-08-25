# 103 — Payload-identity dedup for launch entries, quick actions, clips

**What to build:** Reject duplicates by payload, never by name. Backend validators gain a DB check (excluding self on update) returning typed errors: launch entries — same `kind` + `target` trimmed, case-insensitive; quick actions — same `command` + `cwd` trimmed, case-insensitive; clips — same `content` trimmed, case-sensitive. All three form dialogs render the error inline next to the offending field, keep the dialog open and interactive, and re-enable saving after correction (the ticket-28 stuck-Saving class must not recur). Bulk surfaces stay silent-skip: `import_backup` already skips existing identities — extend the same skip-and-count to the installed-app search add path and the file-picker add path with a one-line notice. No schema changes; existing duplicates are grandfathered untouched.

**Blocked by:** none.

**Status:** done — synced to the share; hands-on dialog passes still pending a human

- [x] Typed duplicate errors from all three validators; messages name the colliding item
- [x] Case/trim variants caught (`"vscode.exe"` vs `"VSCode.exe"`); clip content stays case-sensitive
- [x] Update path excludes self — rename-only edits always succeed
- [x] Three dialogs show inline errors; no stuck "Saving…" state anywhere (dialogs already rendered backend errors inline — zero dialog markup changes needed)
- [x] Search/file-picker adds skip existing with notice; backup import counts unchanged
- [x] cargo test covers: reject exact/variant payloads, accept distinct, self-exclusion

**Verification notes (2026-08-25):**

Collision checks live in new `colliding_entry` / `colliding_action` / `colliding_clip` functions — deliberately OUTSIDE the `validate_*` functions, because backup import validates every record through those and must keep its skip semantics; only the six create/update commands in lib.rs consult the collision checks and turn a hit into a user-facing error naming the existing item ("…is already in Quick Launch with this target." / "…already runs this exact command." / "This text is already saved as \"X\".", untitled clips get the nameless variant). Rules as specified: launch = kind + target NOCASE trimmed; quick action = command + cwd NOCASE trimmed with NULL-aware cwd matching (no-cwd never collides with some-cwd); clip = content byte-equality after trim (case-sensitive — content is data). Frontend: the three form dialogs needed nothing (their catch already renders `String(e)` inline via `role="alert"`); library picker/search adds pre-check the loaded list client-side and flash "Already in Quick Launch — nothing added." instead of an error dialog, per the bulk-surface decision. Tests: one suite per collection covering trim/case variants, kind/cwd distinctions, self-exclusion on edit. Gates: cargo test 372 passed (3 new), svelte-check 0/0, vitest 36/36.

Note: `target\debug\sprout.exe` was locked by a running Sprout instance during disk cleanup — that instance should be closed/restarted to pick up the new backend.

**Amendment (2026-08-25, same session): import identity now payload-keyed too.** The backup merge had been skipping launch entries and quick actions by NAME (`entry_names`/`action_names` sets) — inconsistent with this ticket's rule in both directions: same-name/different-target imports were wrongly skipped as "already exists", while same-payload-under-a-new-name imported as a fresh duplicate the create-path now rejects. The merge keys switched to payloads (`kind + target` / `command + cwd`, trimmed + case-folded, joined with `\u{1f}` so no value can forge a cross-part collision; clips were already content-keyed). `partial_duplicates_merge_without_duplication` and `partial_restore_into_a_populated_database_reports_true_counts` hold unchanged — their fixtures collide on payload as well as name; new `import_identity_is_payload_not_name` pins both corrected directions. cargo test 373 passed.
