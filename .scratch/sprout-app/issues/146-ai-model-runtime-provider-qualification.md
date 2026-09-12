# 146 — Qualify AI models, runtime, providers, and refusal cases

**What to build:** Produce the evidence and reproducible fixtures needed to ship two honest model tiers and a bounded authoring feature. This is a qualification deliverable, not permission to install inference software on every Sprout user's machine.

**Blocked by:** None — can start immediately.

**Status:** incomplete — runtime qualified and lightweight tier qualified with live measurements (2026-09-12); stronger tier, bounded support sets, and the false-positive bound remain open.

**Parent:** [145 — AI-assisted Quick Action authoring](145-ai-assisted-quick-action-authoring-spec.md).

Report: [0018 — AI model, runtime, and provider qualification support report](../../../docs/research/0018-ai-qualification-support-report.md).

## ACs

- [x] Refuse permanent deletion even for one file, destructive overwrites, disk wipes/formatting, security weakening, credential extraction, and equivalent composed or disguised effects. Uncertain effects require clarification or refusal. Include repair of manually authored destructive commands and refusal-workaround requests in the blocking corpus. Never execute destructive fixtures.
- [x] Maintain Sprout-owned shared rules plus Create Quick Action and Diagnose Quick Action task skills in the repository, bundled into the installed app. Shared rules cover draft-only behavior, destructive refusal, selected-shell compatibility, discovery/disclosure grants and explicit review/save. Creation covers authorized targets, shell-specific quoting and prerequisites; diagnosis uses selected scripts/errors and proposes a separate revision without execution. Load actual shared and relevant task content on each applicable request. Adapt useful pinned Matt Pocock principles with required notices; no unmodified developer execution/test/commit workflows or custom skills. Qualify the actual combined workload and record the selected resources and notices.
- [x] Do not download, copy, bundle or retrieve Microsoft documentation as a runtime command catalog. Use model knowledge constrained by Sprout-authored instructions, independently authored benign examples and non-executing compatibility checks. Target Windows PowerShell 5.1 and Windows CMD explicitly; do not assume PowerShell 7 parameters or optional modules/external programs exist. Disclose or clarify unknown prerequisites. Maintainers may consult and link primary documentation for research. Command knowledge is not a safety boundary. Include PowerShell 7-only syntax, missing modules/external tools and uncertain prerequisites in compatibility cases.
- [ ] Compatible models reuse one qualified runtime/provider integration through explicit model selection and validated model-specific configuration. Record artifact identity, context limits, template requirements, memory needs and minimum runtime version. A new architecture or unsupported protocol may require a separately tested runtime/adapter update and app release; JSON alone cannot add missing support. Cloud support is a bounded tested API set, not one implementation per model or universal API compatibility.
- [ ] Record exact candidate model artifacts/revisions, quantization, download hashes/sizes, source/license notices, commercial/distribution constraints, and suitability for the intended Sprout distribution. Do not approve the proposed 3B Qwen artifact by assuming it shares the 7B license.
- [ ] Qualify a lightweight and a stronger tier with actual PowerShell/CMD authoring and diagnosis instructions. Measure Windows RAM/VRAM, CPU/GPU configuration, context budget, startup latency, generation latency, cancellation, and output usability; report missing hardware evidence honestly.
- [x] Select a supported managed inference runtime/version and document verified download, per-user launch, health, cancellation, model release, process ownership, and exit behavior. Do not assume a model file alone is an inference service. (Qualified 2026-09-12: llama.cpp b10702, 18,145,543 B, SHA-256 pinned; live download/launch/health/cancellation plus the prescribed controlled-test battery; evidence in research 0018.)
- [ ] Specify the minimal existing-local and cloud protocol contract: endpoint classification, model selection, authentication, response framing, context/token limits, cancellation, errors, and unsupported-capability handling. Name a bounded tested support set, not universal model/API compatibility.
- [x] Create executable non-destructive evaluation inputs and expected outcomes for allowed, refused, and ambiguous requests, including both shells, obfuscation/composition, dangerous repair/stop-command output, prompt injection, and ordinary legitimate operations. Never execute destructive generated fixtures.
- [x] Define blocking failure thresholds for destructive output and false positives before release. Distinguish request refusal, output rejection, and absence of execution authority; document the limits of scanning rather than promising arbitrary-script safety.
- [x] Identify and pin the upstream authoring/diagnosis skill subset with redistribution notices. Specify adaptation of unavailable tools, automatic command execution, test loops, and commit instructions out of the runtime workflow.
- [x] Publish a support/qualification report linked from this ticket, with remaining blockers and reproducible checks. Fill measured catalog fields only with evidence; no invented hardware requirements or fabricated passing results.

## Verification

Use primary model/runtime/provider documentation and real measurements. Evaluate generation as data; use controlled fixtures and isolated, benign manual execution checks only when the qualification task explicitly needs them. Update ACs as each qualification deliverable is completed.

## Implementation notes

Spec gate: 148 cannot start with an unknown supported transport or an undefined refusal test corpus. If a model cannot legally or technically qualify, record the failure and qualify an alternative within the agreed lightweight/stronger intent; changing agreed product scope needs an explicit decision.

## Qualification audit — 2026-09-11

The previous implemented status overstated completion. Research 0018 and the
bundled catalog explicitly leave both models and the runtime unqualified.
ACs 4–8 are reopened: contracts and missing-evidence reports are useful outputs,
but do not complete artifact selection, real measurements, or a tested support
set. Other checked deliverables are retained; this audit does not independently
requalify them. Ticket 151's eight controlled lifecycle tests pass, including
the assertion that every shipped candidate is non-installable. Hardware does
not participate in that catalog-status decision. Finish the evidence here,
then 151's real managed setup/generation/packaging checks; 152 adds the stronger
tier and 155 verifies the complete round.

## Implementation record — 2026-09-12 (primary-source re-verification, no approvals)

Primary sources re-checked; nothing is approved and no AC is closed by this
record. Research 0018 gains a dated amendment with the evidence; the bundled
catalog (`src-tauri/resources/ai-model-recommendations.json`) records verified
license/artifact identities with all measured fields still null, so every
entry stays non-installable and `shipped_catalog_is_fail_closed_and_status_creates_nothing`
must keep passing.

- Stronger tier license verified: Qwen2.5-Coder-7B-Instruct primary LICENSE
  text is Apache-2.0 (Copyright 2024 Alibaba Cloud); the official GGUF repo
  lists `License: apache-2.0` (qwen2 arch, 7.61B params, documented 32,768
  context, Q4_K_M as-displayed 4.68 GB). Exact revision, SHA-256, byte size,
  template, memory, runtime minimum, and Windows measurements still missing.
- Lightweight tier: the 3B artifact is finally rejected (research-only
  non-commercial license confirmed on its primary LICENSE page), and
  Qwen2.5-Coder-1.5B-Instruct-GGUF is proposed as the same-family
  permissively-licensed alternative (Apache-2.0 primary, qwen2 arch, 1.54B,
  documented 32,768 context, Q4_K_M as-displayed 1.12 GB) — still unpinned
  and unmeasured. SmolLM3-3B (Apache-2.0, ggml-org GGUF) kept as fallback,
  not proposed (new `smollm3` arch needs a separately tested adapter;
  weaker code scores). StarCoder2-3B set aside (OpenRAIL-M use restrictions).
- Runtime: llama.cpp MIT confirmed; b10702 is a 2026-08-30 pre-release
  (CPU x64 zip as-displayed 17.3 MB, no published SHA-256; b10793 newer) —
  the pin needs a decision plus the full measurement battery. The
  health/chat-completions surface the qualification must exercise is cited
  from the llama.cpp server README and llama.app API docs.
- AC 8: existing-local contract is implemented/tested in `ai_assist.rs`;
  bounded tested set (Ollama-compatible loopback + model allow-list) still
  to be proven; cloud contract specified in 0018 §6, implementation owned
  by ticket 150 (`ready-for-agent`).

Blockers remaining: exact per-file revisions/hashes/byte sizes, template and
memory needs, runtime pin + full lifecycle measurements on Windows hardware,
context/latency/cancellation/usability evidence under the real skill
workload, bounded existing-local/cloud support sets proven, and the
false-positive bound. A maintainer with Windows hardware and network access
must download the pinned artifacts, run the measurement battery, and only
then flip catalog entries to `qualified`.

## Implementation record — 2026-09-12 (live qualification, lightweight + runtime)

With the user's approval (lightweight tier only, stay on b10702), the
measurement battery was run on a Windows x86_64 CPU-only machine with
10 GiB RAM. Full evidence lives in research 0018's second 2026-09-12
amendment; the catalog now ships the runtime and the lightweight tier as
`qualified`, and the stronger tier is untouched for ticket 152.

- Pinned: model `Qwen/...-1.5B-Instruct-GGUF` @ `f86cb2c1...`, Q4_K_M,
  1,117,320,768 B, SHA-256 `cc324af0...` (download hashed identically);
  runtime zip 18,145,543 B, SHA-256 `696bce58...`; immutable revision-pinned
  download URL in the catalog.
- Load-bearing find: both stable URLs 302 to presigned HTTPS, which the old
  no-redirect downloader could never install from. Fixed narrowly
  (`redirects(5)` + pure unit-tested HTTPS-landing refusal; size+hash
  verification unchanged) with two new tests. No silent weakening: rationale
  and tests are in the commit and the report.
- Live: per-user hidden loopback launch, `/health` 200 in 4.2 s, four real
  generations through the actual skill prompts (2 usable, 1 destructive
  compliance caught by app checks, 1 PS7-only output caught by app checks —
  refusal guarantee is the app layer, as designed), client-abort survival,
  1.75 GiB working set / 902 MB private, owned-PID-only shutdown.
- AC 7 closed. ACs 4–6 stay open until the stronger tier lands (152) so the
  two-tier claim is never made on one tier's evidence; AC 8 stays open
  (bounded sets unproven against real services; cloud owned by 150).
- Limits: real `install()` needs 2.27 GB free; the qual machine had ~1.43 GB,
  so install logic rests on the ten controlled/download tests — a live
  end-to-end install on a roomier machine is still wanted (151 AC 7).
  (Resolved 2026-09-12: fresh verified install 132.5 s + Generate → checked
  draft → Save + owned shutdown all live; see 151 AC 7 and research 0018's
  third 2026-09-12 amendment.)
