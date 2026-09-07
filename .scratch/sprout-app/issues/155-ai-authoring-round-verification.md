# 155 — Verify AI authoring across modes, shells, and app updates

**What to build:** Verify the complete user-controlled authoring feature across supported providers, both shells, new/existing installs, and release-owned resources; close only proven acceptance criteria.

**Blocked by:** 149 — discovery; 150 — cloud; 152 — stronger tier/updates; 153 — revisions; 154 — cleanup.

**Status:** ready-for-agent

**Parent:** [145 — AI-assisted Quick Action authoring](145-ai-assisted-quick-action-authoring-spec.md).

## ACs

- [ ] Verify the plain destructive boundary for generation and diagnosis of both manual and AI-authored scripts, including single-file permanent deletion, destructive overwrite, composed/encoded effects, stop commands and uncertain scope. No manual-editor bypass exposes rejected code.
- [ ] Verify packaged Sprout shared rules and both task skills load for the correct workflows without a source checkout, copied Microsoft command catalog or runtime documentation fetch. Check Windows PowerShell 5.1/CMD compatibility and honest prerequisite disclosure.
- [ ] Verify both compatible managed tiers reuse the supported integration with correct per-model settings. Reject unsupported architecture/protocol/version metadata. Enabling AI alone must not download runtime or weights.
- [ ] Exercise disabled/default Sprout with no inference installation/startup or catalog refresh, while all existing manual Quick Action workflows remain usable.
- [ ] Demonstrate managed lightweight and stronger tiers, an existing local service, and a qualified cloud provider through draft → review → explicit save → deliberate user-run. Verify original actions survive rejected revisions, disconnect, model removal, and provider failures.
- [ ] Run the qualified adversarial request/output corpus through generation, path binding, and diagnosis. Destructive/refused output never becomes usable through streaming, Copy, Save, or revised stop commands; record residual limitations and unresolved failures honestly.
- [ ] Prove generation/diagnosis never invokes generated code, including through existing Test commands, WhatIf evaluation, provider tool requests, or automatic retry. Managed inference startup and fixed discovery do not become general execution capabilities.
- [ ] Inspect outbound requests and destinations across consent cancellation, endpoint changes/redirects, discovery grants, locally bound paths, revised scripts, and error attachments. Verify no secret/context leakage or silent fallback.
- [ ] Verify old PowerShell data, both-shell identities, version-1 import, version-2 round-trip and old-reader rejection, failed merge rollback, and all unaffected backup collections.
- [ ] Verify managed process ownership, active/idle lifecycle, actual quit versus close-to-tray, cancellation/crash/recovery, offline selected-model use, and recommendation changes across app releases without automatic replacement.
- [ ] Verify packaged model metadata and attributed skill resources without a source checkout. Confirm exact artifact checks, release-only resource changes, and that the default installer contains no model weights.
- [ ] Complete applicable frontend check/build, backend check/tests, ownership gate, and keyboard/screen-reader/Settings dirty-guard checks. Test main-app authoring and compact-window manual access without adding dock configuration or regressing Companion behavior.
- [ ] Publish a concise verification report linked here with tested models/runtime/provider versions, hardware, scope, failures, and each AC's evidence. Do not mark untested model/hardware combinations as supported or close parent/spec work merely because documentation exists.

## Verification

Prefer workflow assertions and controlled failure fixtures over exact prompt snapshots or source-text matching. Real model behavior needs separately recorded qualification evidence; deterministic tests do not prove universal LLM safety. Use existing tests where they already own the behavior.

## Implementation notes

Respect ADRs 0030–0032 and the amended shell/backup decisions. This is the final integration audit, not a license to relax blockers, add autonomous capabilities, publish an app release, or change model defaults without evidence.
