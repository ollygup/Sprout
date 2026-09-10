# 148 — Connect an existing local model and save a checked draft

**What to build:** Connect to a supported user-managed on-device inference service, request a PowerShell/CMD Script draft, review it, save it as a Quick Action, and run it only through a deliberate user action.

**Blocked by:** 146 — qualification; 147 — shell-aware Quick Actions.

**Status:** implemented — awaiting validation/publish

**Parent:** [145 — AI-assisted Quick Action authoring](145-ai-assisted-quick-action-authoring-spec.md).

## ACs

- [x] Load bundled Sprout shared rules plus Create Quick Action on every generation request. Test behavior with both shells and adversarial context, rather than exact prompt snapshots. No copied Microsoft command dataset or runtime documentation fetch.
- [x] Present the plain boundary: "Destructive commands are outside AI assistance. You can write and manage those commands in the manual editor." Refuse permanent deletion even for one file, destructive overwrites, disk wipes/formatting, security weakening, credential extraction, and equivalent composed or disguised effects. Uncertain effects require clarification or refusal. No rejected code or workaround is transferred to the manual editor.
- [x] Target Windows PowerShell 5.1 or Windows CMD explicitly; disclose uncertain prerequisites without pretending execution succeeded. Check effects and composition rather than treating command-name membership as proof of safety.
- [x] Add opt-in AI setup to the existing Settings disclosure/search/Save-Discard behavior. Expose the existing-local path without downloading or installing anything, plus discoverable managed/cloud alternatives that do not pretend their later capabilities already work.
- [x] Validate on-device endpoint classification and the qualified protocol/model choice. Connection/model errors are actionable; unsupported capabilities, redirects to off-device destinations, and unavailable service failures cannot trigger silent fallback.
- [x] Implement the AI assistance interface with a real supported provider adapter and an exercised deterministic test adapter. Load the bundled, pinned, attributed authoring instructions; no custom skill editor, folder scan, or remote skill refresh.
- [x] Generate for an explicit shell from the typed request and explicitly supplied context. Apply qualified request and output checks outside model self-approval. Keep rejected code out of streamed preview, Copy, and Save; present refusal reasons or needed clarification without executable rejected output.
- [x] Present a reviewable candidate with shell, command, assumptions/targets, and explanation. Mark it unexecuted. Non-executing validation must not invoke Quick Action/Quick Launch Test or general shell evaluation, even for 'dry run' or 'repair'.
- [x] Require explicit saving through normal Quick Action validation/persistence. New actions have auto-run off; AI cannot set execution flags or overwrite user-authored metadata silently. Keep manual editor/run behavior independent of provider availability.
- [x] Handle cancellation, malformed/truncated responses, timeout, double-submit, and late completion without persisting partial actions, replacing newer drafts, or falsely claiming success. Use bounded request/output limits documented from qualification.
- [x] Tests demonstrate the complete local request → checked draft → explicit save flow and zero script Run/Test/Stop invocations throughout generation and saving, including malicious model output. No credentials or prompt payloads enter routine logs.

## Verification

Use deterministic provider responses for allowed/refused/error paths and a benign real supported local model for a manual smoke check. Verify disabled AI installs nothing, keyboard/focus behavior, exact persisted command/shell, and user-triggered Run only after saving.

## Implementation notes

Keep provider orchestration behind one deep authoring interface; reuse existing save/run interfaces. The user-managed service is never launched, unloaded, reconfigured, or stopped by Sprout. Ship no placeholder model recommendations that have not passed 146.
