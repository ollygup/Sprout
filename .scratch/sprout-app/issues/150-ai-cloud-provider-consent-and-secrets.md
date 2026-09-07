# 150 — Generate through a cloud provider with explicit disclosure

**What to build:** Let a user configure a qualified cloud provider and API key, understand exactly what is sent, approve the context, generate a draft, and save it through the same checked workflow.

**Blocked by:** 149 — scoped discovery and context grants.

**Status:** ready-for-agent

**Parent:** [145 — AI-assisted Quick Action authoring](145-ai-assisted-quick-action-authoring-spec.md).

## ACs

- [ ] Implement and verify each supported API contract selected in 146. Compatible models share the adapter and explicit model selection; unsupported capabilities/response shapes fail honestly. Do not promise one adapter fits every cloud API.
- [ ] Before cloud enablement or any inference content leaves the machine, explain that the configured third party receives prompts and included paths, filenames, scripts, errors, or system information under its policies. Show the actual recipient; cancellation sends no inference request.
- [ ] Store the API key through protected OS-backed storage accessed by the backend. Keys do not enter ordinary settings JSON, browser storage, prompts, standard logs, error bodies, or backup exports. Inventory any new Windows credential operation owner.
- [ ] Display selected provider/model and context preview. Previously approved discovery alone cannot authorize disclosure of raw results or file contents. Additional raw material requires explicit approval; reference-only app lookup should not upload actual paths.
- [ ] Consent is destination/context scoped. Provider changes, off-device endpoints, redirects, and locally bound scripts submitted for repair cannot reuse a broader nonexistent grant. No request is sent to an unapproved destination or as a hidden 'connection test' containing user context.
- [ ] Reuse refusal/output validation and explicit save from 148 across cloud responses. Provider-supplied tool requests do not grant Run/Test/Stop, arbitrary file access, network fetch, or other unavailable authority.
- [ ] Handle missing/invalid key, unauthorized response, rate limit, timeout, unsupported model/response, cancellation, and partial output without losing saved actions or falling back to another provider.
- [ ] Wire the cloud fields into existing Settings validation/search/dirty-state behavior with visible accessible warnings and errors. Do not add a separate app-rail AI page or nested third-level setup flow.
- [ ] Assert outbound payloads and destinations in tests: no pre-consent call, no unauthorized raw path/file body, no key in diagnostics, no unapproved redirect, and no silent cloud fallback from local. Demonstrate a successful benign cloud draft with explicit user setup.

## Verification

Use a controlled transport adapter to inspect request payloads, redirects, auth errors, cancellation, and disclosure changes. Real-provider checks require the implementer's explicitly supplied test credentials; do not obtain or print the user's secrets.

## Implementation notes

The cloud provider list/protocol contract is bounded by 146. Off-device custom endpoints must use truthful external disclosure or be refused, even if a provider markets itself as 'local'.
