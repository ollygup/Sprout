# 145 — AI-assisted Quick Action authoring (spec)

**What to build:** Optional AI help that drafts PowerShell/CMD commands for Quick Actions without executing them. Users choose managed local inference, an existing local model service, or a cloud provider; review and save drafts; and run saved actions themselves. Recommendations and curated skills ship with normal Sprout releases.

**Blocked by:** None — planning parent; implementation proceeds through tickets 146–155, with qualification and shell compatibility first.

**Status:** ready-for-agent

## Problem Statement

Users want convenient scripts for tasks such as opening an installed application, starting a development service, or working with selected files, without having to write PowerShell/CMD from scratch. They also want help understanding errors from a script they ran. They should not have to install a model, disclose local information to a cloud service, trust an AI with command execution, or depend on a recommended model forever to use Sprout.

## Solution

AI assistance is off until deliberately configured. Settings presents optional managed local setup prominently, with existing-local-model and cloud-provider alternatives. Managed setup downloads both the chosen model and its inference runtime; the first release aims to offer lightweight and stronger tested tiers. It installs nothing by default. Enabling AI alone never downloads weights or a runtime; managed installation requires a separate explicit Install action.

In the main app's Quick Action authoring flow, the user describes a task and chooses PowerShell or CMD. Sprout loads the curated authoring skill and supplies only authorized context. Destructive assistance is refused. A completed draft shows its command, shell, assumptions, affected targets, and explanation without claiming it was executed. The user reviews and explicitly saves it; existing manual Run controls then execute the saved action. AI assistance has no Run/Test/Stop tool.

For “open X,” Sprout can perform bounded local discovery rather than guess a path. Lookup permission does not authorize cloud disclosure. For diagnosis, the user supplies selected error output; Sprout proposes a separate revision which cannot replace the saved action without acceptance and saving.

Model recommendations are bundled JSON, updated only with app releases. Installed selections never change automatically. Skills are fixed, attributed runtime resources bundled with the app, not user-editable files or a list of names the model must somehow locate.

## User Stories

1. As a Sprout user, I want AI off by default, so normal usage installs no inference software or model weights.
2. As a new AI user, I want managed local setup, so I do not have to configure an inference service.
3. As a user with limited hardware, I want a tested lightweight tier with honest download and memory information.
4. As a user with stronger hardware, I want a stronger tested model without replacing my current choice unexpectedly.
5. As an advanced user, I want to connect to my existing local model service and select a supported model it exposes.
6. As an advanced user, I want Sprout to leave my own service, configuration, and model files alone.
7. As a cloud user, I want to supply my provider and API key without installing local inference software.
8. As a privacy-conscious user, I want a warning before cloud enablement and before any inference context is sent.
9. As a user changing provider, I want destination-specific disclosure rather than inherited consent for a different server.
10. As a user drafting an action, I want PowerShell or CMD to be explicit so saved text runs under the intended shell.
11. As a user, I want a reviewable draft with an explanation and assumptions rather than an automatically executed command.
12. As a user, I want dangerous requests refused with a useful explanation and a safer alternative where possible.
13. As a user, I want cancellation or provider failure to leave my saved actions unchanged.
14. As a user asking to open an app, I want installed-app lookup instead of a fabricated executable path.
15. As a user searching folders, I want search limited to approved locations without reading file contents by default.
16. As a user with ambiguous matches, I want to select the intended target before a command is finalized.
17. As a cloud user, I want local paths resolved on my machine where possible, rather than uploaded unnecessarily.
18. As a user, I want to preview and approve additional raw context before it leaves the machine.
19. As a user diagnosing a failure, I want to select the script and error output rather than grant automatic log collection.
20. As a user revising an action, I want to compare and accept the proposed change while keeping the original until saving.
21. As a user editing concurrently, I want an older draft prevented from overwriting a newer saved action silently.
22. As a user saving an AI-assisted action, I want normal name, working-directory, duplicate, Group, note, and run-control behavior.
23. As a user, I want saved actions to remain runnable and manually editable when AI is disabled or unavailable.
24. As a managed-local user, I want inference to start when needed, release memory after inactivity, and stop when Sprout exits.
25. As a user cancelling a download, I want incomplete files never presented as an installed usable model.
26. As a user updating Sprout, I want newer recommendations without automatic downloads, switches, or deletion of my installed model.
27. As a user disconnecting AI, I want credentials and active requests handled without deleting saved actions.
28. As a user exporting a backup, I want saved action shell semantics preserved without exporting API keys, prompts, or model weights.
29. As a keyboard/screen-reader user, I want setup, disclosure, generation, review, and refusal states operable and understandable.
30. As a maintainer, I want one reviewed release path for recommendations and skills, with no additional backend server to operate.

## Implementation Decisions

### Accepted behavior

- **Authoring only (ADR-0030):** generation, diagnosis, revision, acceptance, saving, and non-executing validation never run generated code. Existing executable Test commands are not available to the AI. A user-operated Test elsewhere remains an execution action, never relabeled “AI validation.” Managed inference startup and trusted read-only OS discovery are separate capabilities with fixed app-owned behavior.
- **Policy enforcement:** apply request checks before inference and output checks before a draft becomes available for use. Refused requests and outputs expose no actionable rejected script through preview, streaming, Copy, or Save. The first implementation may buffer candidate code until checks finish; progress remains visible. Recheck edited candidates when accepted through AI assistance. A model rating its own output safe is not authorization. Manual authoring remains outside this feature's refusal promise.
- **Destructive scope:** disk wipes/formatting, permanent deletion (including a single file), destructive overwrites, credential extraction, disabling security controls, and equivalent disguised/composed operations are refused. Ordinary scoped non-destructive tasks remain supported. Qualification defines permitted/refused/needs-clarification fixtures, false-positive cases, obfuscated inputs, and handling of uncertain classification before the generation slice ships. No guaranteed-safety or sandbox claim is made.
- **Plain refusal boundary:** "Destructive commands are outside AI assistance. You can write and manage those commands in the manual editor." Refuse permanent deletion even for one file, destructive overwrites, disk wipes/formatting, security weakening, credential extraction, and equivalent composed or disguised effects. Uncertain effects require clarification or refusal. Do not supply destructive code, actionable workaround steps or a prefilled manual draft after refusal. The same boundary applies to repairing manually authored destructive scripts. Risk explanations and non-destructive alternatives remain allowed. This is a product boundary, not a safety guarantee or transfer of responsibility.
- **Command knowledge:** Do not download, copy, bundle or retrieve Microsoft documentation as a runtime command catalog. Use model knowledge constrained by Sprout-authored instructions, independently authored benign examples and non-executing compatibility checks. Target Windows PowerShell 5.1 and Windows CMD explicitly; do not assume PowerShell 7 parameters or optional modules/external programs exist. Disclose or clarify unknown prerequisites. Maintainers may consult and link primary documentation for research. Command knowledge is not a safety boundary.
- **Model compatibility:** Compatible models reuse one qualified runtime/provider integration through explicit model selection and validated model-specific configuration. Record artifact identity, context limits, template requirements, memory needs and minimum runtime version. A new architecture or unsupported protocol may require a separately tested runtime/adapter update and app release; JSON alone cannot add missing support. Cloud support is a bounded tested API set, not one implementation per model or universal API compatibility.
- **Explicit shell:** Quick Actions gain PowerShell/CMD selection, with missing legacy values interpreted as PowerShell and unknown values rejected. Run, user-operated Test, and stop commands respect the selected shell. Quick Launch's existing shell support does not imply Quick Actions already have it. Extend the current Windows execution owner rather than duplicating invocation knowledge.
- **Shell-aware identity and backups:** add shell to existing normalized command/working-directory identity consistently across create/update collisions and backup merge. Preserve existing equality rules within each shell. Evolve the same backup envelope to version 2 for new exports; accept legacy version 1 as PowerShell. Legacy readers must reject version 2 instead of treating CMD as PowerShell. Version-2 Quick Actions require an explicit valid shell; inconsistent version-1 records declaring CMD are rejected. Preserve selective export, atomic non-overwriting merge, and existing collections.
- **Explicit acceptance:** new drafts are transient until saving. Revisions retain a baseline identity/revision and cannot overwrite intervening edits. Regenerate, discard, cancel, or fail leaves saved content intact. Show changes to shell, command, working directory, and any proposed note; do not silently overwrite a user's note or flags. Generated actions default to auto-run off. AI never enables startup execution. Existing user-chosen auto-run is preserved and made visible during revision acceptance.
- **Providers and context (ADR-0031):** one selected provider route; no silent model/provider/cloud fallback. Existing-local means an on-device user-managed service, not arbitrary GGUF import. Unsupported protocols/models fail honestly. LAN or otherwise off-device endpoints cannot be labeled private on-device inference; a non-loopback destination needs the external disclosure path or is refused in v1. Destination changes and redirects cannot bypass consent.
- **Discovery:** an explicit find request permits installed-app metadata and approved-folder name/path search. No background whole-disk crawl or default content reading. Expanding roots or reading contents requires user permission. Enforce root canonicalization, reparse-point containment, bounded results/depth/time, cancellation, and ambiguity selection locally. Never execute a found file to identify it.
- **Cloud disclosure:** warn before enabling and before the first inference request. Display the recipient and included context. Local discovery grants do not authorize uploading its output. Prefer opaque target references and local trusted path binding; references are not model-provided paths or general shell interpolation. Reject unknown, cross-request, stale, or modified references; bind and quote for the selected shell before final validation/review. Any extra raw paths, filenames, content, or system details require a preview and explicit disclosure grant. Never resend the locally resolved final script during later cloud diagnosis without checking its disclosure scope.
- **Managed setup and lifetime:** explicitly requested runtime and weight downloads are staged and verified before activation; missing or corrupt artifacts are unusable. Per-user installation and an app-owned inference process must not replace a user's own installation or require administrative elevation. Start on the first generation request, do not spawn duplicate runtimes for overlapping requests, do not unload during an active request, unload after inactivity, and stop the owned runtime on actual Sprout exit. Main-window close-to-tray is not exit. Existing-local services are never stopped or unloaded by Sprout.
- **Release-owned resources (ADR-0032):** recommendations are bundled JSON with stable IDs, exact artifact revisions, hashes/sizes, license and runtime compatibility, plus measured hardware information. Model weights are not bundled into the default installer. Recommendations and curated skill contents change through normal app version bumps/tags; no recommendation polling or independent catalog backend/signing flow. Keep installed inventory separate from recommendations so removed/changed entries cannot silently switch or erase installed selections.
- **Skills:** Maintain Sprout-owned shared rules plus Create Quick Action and Diagnose Quick Action task skills in the repository, bundled into the installed app. Shared rules cover draft-only behavior, destructive refusal, selected-shell compatibility, discovery/disclosure grants and explicit review/save. Creation covers authorized targets, shell-specific quoting and prerequisites; diagnosis uses selected scripts/errors and proposes a separate revision without execution. Load actual shared and relevant task content on each applicable request. Adapt useful pinned Matt Pocock principles with required notices; no unmodified developer execution/test/commit workflows or custom skills. Skills never grant permissions.
- **Data minimization:** provider settings, discovery grants, target maps, credentials, and managed-model inventory are machine-local. API keys use protected OS-backed storage; never ordinary settings JSON, browser storage, prompt text, or routine logs. Default drafts/conversation/error context are session-transient with no persistent chat history feature. Operational diagnostics redact secrets/context. Backups carry saved action content and shell only, not AI settings, credentials, grants, prompts, weights, or skills. Saved scripts may contain paths as they already can.

### Modules and test seams

Use one deep AI assistance module as the principal caller/test seam: request a draft or diagnosis from an explicit provider and authorized context; return a candidate, refusal, cancellation, or actionable failure. It hides prompt assembly, skill selection, provider response handling, context checks, and draft validation. Real provider adapters and an exercised deterministic test adapter justify variation; do not add a universal tool dispatcher or a pass-through “AI shared” module.

Existing Quick Action create/update validation and persistence remain the saving interface; existing manual execution remains the running interface. Reuse installed-app discovery behind bounded operations. Windows process/shell mechanics stay with their current owner, including managed-runtime process lifetime; model install/lifetime policy remains inside AI assistance. Protected credential access may require a newly inventoried Windows operation owner. Removing AI assistance should remove AI policy/orchestration complexity without changing manual Quick Actions; removing the Windows execution owner would redistribute Windows compatibility knowledge, so that owner remains substantive.

### UX conventions

Global AI setup belongs in the existing Settings page as a searchable Disclosure section; generation and revision belong beside the Quick Action being authored. Follow research 0004 rules 2–3, 0006 patterns 3/4/8, 0008 rules 1–2, and the existing Settings-navigation note 0014. No new app-rail destination, nested rail, or authoring UI in the compact dock/window. Setup alternatives remain discoverable; disclose their fields when selected without nesting a third disclosure level. Preserve the Settings Save/Discard/dirty-guard behavior, shared PageHeader/Dialog/Disclosure/SearchInput/button patterns, tokens, keyboard access, and visible progress/errors. Explicit download/connect commands are distinct from Save-deferred configuration fields.

## Testing Decisions

- Test observable workflow behavior at the AI assistance interface and existing save/run interfaces, with deterministic provider/discovery/transport responses. Prefer a small set of real seams over prompt-string snapshots or source-text assertions.
- Prove zero calls to script Run/Test/Stop and general command execution throughout generation, diagnosis, validation, cancellation, and save; distinguish authorized managed-runtime startup and fixed read-only discovery from executing generated text.
- Exercise both shells, malformed/truncated output, unsupported models, cancellation/late responses, provider timeout, uncertain/refused output, and local/cloud failures. Require refusal before candidate display/copy/save; do not execute destructive fixtures.
- Check outbound payloads: no request before consent, no unauthorized raw discovery data, no credential leakage, no destination/redirect bypass, and no unapproved paths reintroduced by a locally bound draft or repair. Test local inference with network observations scoped to AI requests.
- Test path binding with spaces, quotes, shell metacharacters, Unicode, duplicate app names, missing targets, stale references, and reparse-point escape attempts. Found content remains untrusted input.
- Exercise legacy database migration, both-shell duplicate semantics, version-1 import, version-2 round-trip, unknown/missing-shell rejection, legacy-reader version rejection, transactional rollback, and selection-preserving export.
- Managed-runtime tests use controlled process/download/clock behavior: cancellation, hash mismatch, insufficient disk/memory, crash/retry, overlapping requests, idle expiry, actual app exit, and foreign-runtime ownership. Hardware qualification measures the actual skill/context workload; download size is never a RAM requirement.
- Revision tests cover reject/accept/save, newer local edits, source deletion, failed persistence, unrelated metadata preservation, auto-run visibility, and no automatic execution.
- Prior art: Quick Action migration/validation/duplicate/cwd/run-stop tests, existing Windows execution transcript tests, backup merge tests, Settings state/dirty-guard tests, and shared dialog accessibility tests. Existing executable command tests are prior art for the manual runner, not an AI tool.
- Each implementation slice runs relevant existing tests, frontend check/build as affected, backend checks/tests as affected, and the ownership gate before sync. Final integration also verifies packaged resources without a repository checkout and keyboard/compact-window regressions. This documentation task does not claim those implementation checks already passed.

## Out of Scope

- Autonomous execution, testing/repair loops, AI-triggered stop/run/elevation, or an AI tool that enables auto-run.
- A new execution sandbox or a promise that arbitrary generated scripts cannot be destructive.
- Automatically installing models/runtime at Sprout installation or ordinary startup.
- Arbitrary local model-file imports, training/fine-tuning, and universal compatibility with all model servers.
- A copied/downloaded Microsoft command-reference dataset or runtime documentation-fetch facility.
- Custom/editable skills, runtime discovery of skill folders, remote skill refresh, and independent model-catalog polling/backend.
- Whole-drive indexing, automatic file-body/log upload, persistent chat-history/search, and implicit cloud fallback.
- Changing manual Quick Action privilege policy, fixing unrelated existing runner/watchdog gaps, or replacing Preset/Quick Launch execution.

## Further Notes

### Qualification gates, not silently chosen defaults

Ticket 146 resolves exact model artifacts and redistribution/use licenses, the managed runtime and version, supported existing-local/cloud protocols, realistic hardware/context/latency budgets, and the destructive-request fixture policy. The intended pair is lightweight and stronger; parameter counts alone do not qualify a recommendation. The idle timeout and request/download/search limits are documented implementation defaults with boundary tests, not invented user commitments. If a candidate or transport cannot meet the contract, report the blocker rather than silently weakening safety or changing the agreed scope.

Conversation research on 2026-09-06 found Qwen2.5-Coder-3B-Instruct Q4_K_M at about 2.1 GB and the 7B counterpart at about 4.68 GB; the 3B artifact carries a research license with separate commercial terms, while the 7B candidate has different terms. Reverify current primary sources during qualification; neither is approved for shipping by this spec. Sources: [3B artifact/license](https://huggingface.co/Qwen/Qwen2.5-Coder-3B-Instruct-GGUF), [7B artifact](https://huggingface.co/Qwen/Qwen2.5-Coder-7B-Instruct-GGUF), [Matt Pocock skills](https://github.com/mattpocock/skills/).

The reviewed guidance supports least privilege, narrow tools, and application-enforced authorization; it does not establish a universal ban on every destructive operation. Refusing destructive authoring is Sprout's accepted product policy. Local inference does not eliminate prompt injection or harmful output. Sources: [OWASP Excessive Agency](https://genai.owasp.org/llmrisk/llm062025-excessive-agency/), [OWASP Prompt Injection](https://genai.owasp.org/llmrisk/llm01-prompt-injection/), [PowerShell execution policies](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_execution_policies), [ShouldProcess/WhatIf limitations](https://learn.microsoft.com/en-us/powershell/scripting/learn/deep-dives/everything-about-shouldprocess).

### Ticket map

| Ticket | Delivers | Blocked by |
| --- | --- | --- |
| [146](146-ai-model-runtime-provider-qualification.md) | Qualified support matrix, safety fixtures, explicit ship gates | None |
| [147](147-quick-action-shells-and-backup-compatibility.md) | Manual PowerShell/CMD action with safe migration/backup semantics | None |
| [148](148-ai-existing-local-draft-to-quick-action.md) | Existing-local connection → checked draft → explicit save → user-run action | 146, 147 |
| [149](149-ai-scoped-local-target-discovery.md) | Find/select local targets within approved scope and bind paths locally | 148 |
| [150](150-ai-cloud-provider-consent-and-secrets.md) | Cloud generation with recipient disclosure, context grants, protected keys | 149 |
| [151](151-ai-managed-lightweight-local-setup.md) | Explicit verified lightweight install → managed on-demand generation | 148 |
| [152](152-ai-stronger-model-choice-and-release-recommendations.md) | Stronger tier and release-driven recommendation/selection behavior | 151 |
| [153](153-ai-diagnosis-and-reviewed-action-revisions.md) | Selected errors → proposed revision → accepted replacement | 148 |
| [154](154-ai-disconnect-cleanup-and-recovery.md) | Revoke/disconnect/remove owned AI resources without losing actions | 150, 151, 153 |
| [155](155-ai-authoring-round-verification.md) | Cross-mode, adversarial, packaged-resource and compatibility audit | 149, 150, 152, 153, 154 |

146 is a bounded qualification task with a reviewable report; 147 is an independently demoable compatibility prerequisite. The implementation tickets are complete user-facing slices rather than separate backend/UI layers. Dependencies represent behavior requirements, not merely shared-file edits; coordinate any future concurrent work under the repository's parallel-ticket rules.

### Decision links

[ADR-0030: draft-only authoring](../../../docs/adr/0030-ai-drafts-quick-actions-never-executes.md), [ADR-0031: providers and scoped context](../../../docs/adr/0031-optional-ai-providers-and-scoped-context.md), [ADR-0032: release-owned recommendations/skills](../../../docs/adr/0032-model-recommendations-and-skills-ship-with-app.md). Dated extensions to ADR-0017, ADR-0014, and ADR-0026 preserve existing execution and backup decisions while recording shell support. The existing Catalog glossary term remains reserved for available software Products; use Model recommendation here.

Numbering was reserved before another session published issue 144. This spec 145 and tickets 146–155 are authoritative; the original numeric reservations in ADRs 0030–0032 are corrected by their dated amendments.
