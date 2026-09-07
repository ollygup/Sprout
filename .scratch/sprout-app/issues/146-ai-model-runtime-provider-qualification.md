# 146 — Qualify AI models, runtime, providers, and refusal cases

**What to build:** Produce the evidence and reproducible fixtures needed to ship two honest model tiers and a bounded authoring feature. This is a qualification deliverable, not permission to install inference software on every Sprout user's machine.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

**Parent:** [145 — AI-assisted Quick Action authoring](145-ai-assisted-quick-action-authoring-spec.md).

## ACs

- [ ] Refuse permanent deletion even for one file, destructive overwrites, disk wipes/formatting, security weakening, credential extraction, and equivalent composed or disguised effects. Uncertain effects require clarification or refusal. Include repair of manually authored destructive commands and refusal-workaround requests in the blocking corpus. Never execute destructive fixtures.
- [ ] Maintain Sprout-owned shared rules plus Create Quick Action and Diagnose Quick Action task skills in the repository, bundled into the installed app. Shared rules cover draft-only behavior, destructive refusal, selected-shell compatibility, discovery/disclosure grants and explicit review/save. Creation covers authorized targets, shell-specific quoting and prerequisites; diagnosis uses selected scripts/errors and proposes a separate revision without execution. Load actual shared and relevant task content on each applicable request. Adapt useful pinned Matt Pocock principles with required notices; no unmodified developer execution/test/commit workflows or custom skills. Qualify the actual combined workload and record the selected resources and notices.
- [ ] Do not download, copy, bundle or retrieve Microsoft documentation as a runtime command catalog. Use model knowledge constrained by Sprout-authored instructions, independently authored benign examples and non-executing compatibility checks. Target Windows PowerShell 5.1 and Windows CMD explicitly; do not assume PowerShell 7 parameters or optional modules/external programs exist. Disclose or clarify unknown prerequisites. Maintainers may consult and link primary documentation for research. Command knowledge is not a safety boundary. Include PowerShell 7-only syntax, missing modules/external tools and uncertain prerequisites in compatibility cases.
- [ ] Compatible models reuse one qualified runtime/provider integration through explicit model selection and validated model-specific configuration. Record artifact identity, context limits, template requirements, memory needs and minimum runtime version. A new architecture or unsupported protocol may require a separately tested runtime/adapter update and app release; JSON alone cannot add missing support. Cloud support is a bounded tested API set, not one implementation per model or universal API compatibility.
- [ ] Record exact candidate model artifacts/revisions, quantization, download hashes/sizes, source/license notices, commercial/distribution constraints, and suitability for the intended Sprout distribution. Do not approve the proposed 3B Qwen artifact by assuming it shares the 7B license.
- [ ] Qualify a lightweight and a stronger tier with actual PowerShell/CMD authoring and diagnosis instructions. Measure Windows RAM/VRAM, CPU/GPU configuration, context budget, startup latency, generation latency, cancellation, and output usability; report missing hardware evidence honestly.
- [ ] Select a supported managed inference runtime/version and document verified download, per-user launch, health, cancellation, model release, process ownership, and exit behavior. Do not assume a model file alone is an inference service.
- [ ] Specify the minimal existing-local and cloud protocol contract: endpoint classification, model selection, authentication, response framing, context/token limits, cancellation, errors, and unsupported-capability handling. Name a bounded tested support set, not universal model/API compatibility.
- [ ] Create executable non-destructive evaluation inputs and expected outcomes for allowed, refused, and ambiguous requests, including both shells, obfuscation/composition, dangerous repair/stop-command output, prompt injection, and ordinary legitimate operations. Never execute destructive generated fixtures.
- [ ] Define blocking failure thresholds for destructive output and false positives before release. Distinguish request refusal, output rejection, and absence of execution authority; document the limits of scanning rather than promising arbitrary-script safety.
- [ ] Identify and pin the upstream authoring/diagnosis skill subset with redistribution notices. Specify adaptation of unavailable tools, automatic command execution, test loops, and commit instructions out of the runtime workflow.
- [ ] Publish a support/qualification report linked from this ticket, with remaining blockers and reproducible checks. Fill measured catalog fields only with evidence; no invented hardware requirements or fabricated passing results.

## Verification

Use primary model/runtime/provider documentation and real measurements. Evaluate generation as data; use controlled fixtures and isolated, benign manual execution checks only when the qualification task explicitly needs them. Update ACs as each qualification deliverable is completed.

## Implementation notes

Spec gate: 148 cannot start with an unknown supported transport or an undefined refusal test corpus. If a model cannot legally or technically qualify, record the failure and qualify an alternative within the agreed lightweight/stronger intent; changing agreed product scope needs an explicit decision.
