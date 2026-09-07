# 153 — Diagnose selected errors and accept a revision explicitly

**What to build:** Let a user select an existing Quick Action and error output, request an explanation or revised script, inspect the changes, and replace the saved content only after accepting and saving.

**Blocked by:** 148 — checked draft and save workflow.

**Status:** ready-for-agent

**Parent:** [145 — AI-assisted Quick Action authoring](145-ai-assisted-quick-action-authoring-spec.md).

## ACs

- [ ] Support manually authored and AI-authored saved Quick Actions. Load Sprout shared rules plus Diagnose Quick Action using explicitly selected script/error context; explain and propose separately reviewed revisions without execution.
- [ ] Refuse permanent deletion even for one file, destructive overwrites, disk wipes/formatting, security weakening, credential extraction, and equivalent composed or disguised effects. Uncertain effects require clarification or refusal. Do not repair or complete destructive behavior, or offer actionable workaround steps. Present "Destructive commands are outside AI assistance. You can write and manage those commands in the manual editor." without a prefilled rejected command. Risk explanation and a non-destructive alternative remain allowed.
- [ ] Load the bundled adapted diagnosis skill when the user asks for diagnosis. Receive explicitly selected script/error context; do not automatically collect logs, inspect new folders, execute Test, rerun the action, or loop through attempted fixes.
- [ ] Keep the selected saved action and its baseline revision/identity separate from each proposed revision. Show changed command, shell, working directory, and any proposed note clearly; no AI success claim based on unexecuted output.
- [ ] Run request/output checks for diagnosis and revisions exactly as for generation. Dangerous fixes, disguised high-impact commands, and malicious instructions embedded in selected error text cannot gain new permissions.
- [ ] Reject, cancel, fail, or regenerate leaves the original intact. Explicit acceptance plus successful saving changes only reviewed fields; preserve Group/order, user notes not accepted for replacement, stoppable settings, and auto-run unless the user deliberately changes them.
- [ ] Expose an existing user-enabled auto-run flag during acceptance. The AI cannot enable it or run the new command now. New actions derived from a revision still default auto-run off.
- [ ] Detect source deletion or newer edits before saving a revision; show a conflict and retain both the current saved state and reviewable candidate rather than overwriting silently.
- [ ] When a cloud adapter is present, recheck disclosure for script paths bound locally in an earlier draft and newly selected error output. Revoking or declining the additional disclosure prevents the request.
- [ ] Verify allowed repair, explanation-only response, refusal, malformed output, cancellation, stale completion, save failure, conflicting edit, and source deletion. Assert zero script Run/Test/Stop calls throughout.

## Verification

Use deterministic diagnosis transcripts for both shells and a manually supplied benign error sample. Test rejection and acceptance through persistence, not just visual diff rendering. Final cross-mode behavior is verified in 155 once cloud/managed adapters exist.

## Implementation notes

This ticket can start after 148; do not block it on later providers merely because it must work through the common interface. No persistent chat-history feature or executable repair loop is introduced.
