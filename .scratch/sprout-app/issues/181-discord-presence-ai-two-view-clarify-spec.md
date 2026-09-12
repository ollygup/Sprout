# 181 — Discord Rich Presence (offline static) + AI two-view dialog + scoped clarification (spec)

**Status:** ready-for-agent. Planning package only — no application behavior changed here.

**Parents / related (read before implementing):**
- [145 AI-assisted Quick Action authoring](145-ai-assisted-quick-action-authoring-spec.md) + [172 AI-gated template](172-ai-gated-ai-first-quick-action-template.md) — own the single Add/Edit dialog template, `aiReady` disclosure gate, shell field, Test/Save grammar. This round replaces 172's stacked hero with two exclusive views; reconcile copy/Disclosure with 167 at implementation.
- [50 storage/runner](50-quick-actions-storage-and-runner.md) / [51 editor page](51-quick-actions-editor-page.md) / [62 run tracking](62-quick-action-run-tracking-and-stop.md) / [64 run logs](64-quick-action-run-logs.md) — Quick Action execution model (hidden, unelevated, no status UI). Unchanged here.
- [166 field cleanup / dock filter](166-field-cleanup-dock-filter-companion-navigation-spec.md) + [167 selective guidance](167-selective-field-guidance-cleanup.md) — helper-text policy, AI request/context Ctrl+Enter ownership, Details disclosure. This round's single-textarea hero extends 167's coverage; do not re-add removed hints.
- [173 files/pre-action round](173-quick-actions-files-clips-logs-companion-spec.md) + tickets 175/176/179/180 — Pre-action section, `<FilesDir>` files contract, editor coloring/autocomplete inside the same dialog. This round adds no new dialog section; reconcile tab placement with Pre-action + files sections.
- ADRs: 0017 (execution model), 0028 (design system + disclosure), 0029 (one owner per Windows command), 0030 (draft-only), 0031 (providers + scoped context), 0032 (bundled recommendations + skills). Where this round changes an accepted decision, a dated Amendment ships in the same unit of work — no silent overwrite. New Discord decision ships as 0033.

## Problem Statement

Discord users want Sprout to show a lightweight "Playing …" presence while the app runs, with no account access, no OAuth, and no user-data leakage — fully consistent with Sprout's offline posture (AI cloud mode excepted). Separately, the current Quick Action AI block stacks an AI hero above the full manual form plus `Details`, with request + context + find + roots + generate all visible at once: more than two disclosure levels, no progressive disclosure, and too much chrome for a task the user believes needs one textarea. Vague requests ("do something to my Downloads") currently guess instead of asking back like a grill.

## Solution

Ship static offline Rich Presence via the handwired `discord-rich-presence` IPC crate (local pipe only, text-only, silent when Discord is closed; Application ID requested from the user, no Settings switch in v1). Rework the Quick Action Add/Edit dialog into one dialog with two exclusive views — `AI draft` / `Manual` tabs top-right, content-gated on `aiReady`, AI-first on Add, manual-first on Edit, single-textarea hero, `Use this draft` auto-flips to Manual for full review. Extend the pinned `create-quick-action` skill with a scoped-down grill-style clarification template (same tone/rules/structure, command-generation scope only): vague → `Clarify` with pickable choices → regenerate; never guess, never auto-disclose.

## User Stories

1. As a Discord user, I want Sprout to show a static presence while it runs, so friends see what I am using without my linking any account.
2. As a privacy-conscious user, I want presence to send no user info, names, paths, or counts, so offline stays offline.
3. As a user without Discord running, I want Sprout to start and run normally with no error, so presence never blocks the app.
4. As the app owner, I want one stated Application ID to enable presence, so there is no per-user configuration or secret to manage.
5. As a Sprout user with AI configured, I want Add to open directly in a single AI textarea, so I am not confronted with two stacked forms.
6. As a Sprout user without AI configured, I want Add to look exactly like the old manual form with zero AI chrome, so unconfigured capability never advertises itself.
7. As an AI-view user, I want one obvious way back to manual fields, so toggling off restores the familiar form.
8. As a keyboard/screen-reader user, I want both views reachable, named, and announced, so the view switch is operable and understandable.
9. As a drafting user, I want `Use this draft` to land me in the full manual form with values applied, so review covers name, shell, command, and consequential flags before saving.
10. As a revising user, I want an applied-then-edited draft rechecked before saving with a manual-save escape hatch, so AI authority never silently passes.
11. As a vague requester ("clean my Downloads"), I want the model to ask which folder / which target with choices instead of inventing a path, so the draft aligns with my intent.
12. As a cloud-mode user, I want clarification answers and discovery picks to require fresh disclosure approval before upload, so answering a question never widens consent.
13. As a manual author, I want AI-disabled authoring fully preserved (shells, Test, Run/Stop, files, pre-action), so AI is appointment-only (0006 pattern 3).
14. As a maintainer, I want presence + dialog + skill changes through normal app releases with no new backend, polling, or custom-skill facility, so distribution stays simple.

## Implementation Decisions

### Current vs accepted vs proposed

- Current (verified via CodeGraph/source, not prose): dialog is a single Add/Edit template with an AI-first inline hero when `aiReady` + full manual fields below + one `Details` disclosure; `aiReady` derives from persisted Settings (`existing-local` + named model, or managed-installed) fail-closed; `aiGenerateDraft(request, shell, context, requestId)` returns draft/refused/clarify/failed; clarify renders as a plain notice; discovery is a separate find/bind flow with approved roots; no Discord code exists; size budget NFR-43 holds.
- Accepted-but-unimplemented assumed: shell-aware backup v2 + identity (147); 166/167 copy rules including AI generate-hint ownership; 175/176 pre-action + 179/180 files/editor contracts inside the same dialog.
- Proposed here: (a) static presence via new single-owner IPC module, always-attempt + silent-fail, placeholder App ID; (b) two-view dialog replacing the stacked hero; (c) skill clarification template with no skill-structure change. No elevation/tracking/backup-format change, no second Windows-invocation site, no custom skills, no remote catalog.

### Discord presence (new ADR-0033)

- Handwire the `discord-rich-presence` crate directly (no Tauri wrapper plugin), per Discord Rich Presence overview + crate docs: `DiscordIpcClient::new(APPLICATION_ID)` → `connect()` → `set_activity(Activity::new().details().state())` → `close()` over the local IPC pipe. No OAuth scopes, no token, no user-ID read.
- Static v1 text only: `details: "Using Sprout"`, `state: "Composing presets"`. No names, paths, counts, run states. Any dynamic per-screen text is future work with its own disclosure review.
- Lifecycle: background thread started with the app; connect → set → reconnect-with-backoff loop; `clear/close` on actual exit. Never blocks startup or main-window open; tray-only boot included. Discord closed/absent = silent no-op + local debug log. No toast, no error dialog, no queue.
- Identity: one hardcoded Application ID constant (placeholder until the user supplies the real numeric ID in ticket 182). Not Settings-editable, not backed up, not exported. No art assets in v1.
- Ownership (ADR-0029): the new presence module is the sole Discord IPC owner. No second invocation site. Conventions hold: version stays in `Cargo.toml`, no window-size change, `codebase-design` seam language for the module boundary.

### Dialog two-view (amends ADR-0028 + 0006/0019 application)

- One dialog, two exclusive views behind `AI draft | Manual` tabs (1-word labels, `0004` rule 4 tab hygiene). Tabs sit top-right of the dialog surface (`0006` pattern 8 view-scoped on-surface). Entire tab strip absent when `!aiReady` (`0004` rule 2 frequency split + `0006` pattern 11 content-gated activation — no master switch, no disabled teaser).
- Add + `aiReady` opens AI view; Edit opens Manual (172's Add-AI-first / Edit-manual-first ordering retained, stacking removed). Either tab flips instantly with no Save step (`0008` rule 2 immediacy); both labels stay visible for scent (`0008` rule 3).
- AI view contains exactly: one `Describe what to do` textarea + `Generate draft` + outcome region (draft card / clarify choices / refusal / failure) + quiet `Dismiss`. No shell picker, no `Search in`/`Find`/roots chrome up front (`0006` pattern 2 minimal-until-content, pattern 1 config-lives-with-what-it-governs). `Extra context` merges into the one prompt. Ctrl/Cmd+Enter generates here and never submits the dialog (167 ownership preserved).
- Discovery/disclosure surfaces (find/bind, approved folders, cloud recipient preview) appear only when a draft/clarify needs them, behind the existing disclosure rhythm — never all visible at once. Restores `0004` rule 3 two-level maximum (L1 view, L2 one disclosure).
- `Use this draft` applies `shell` + `command`, auto-flips to Manual for full review (name/shell/command/cwd/flags/files/pre-action), focuses Command, announces `Applied — review and save` (`0004` rule 5 feedback; HAX G9 + accordion-editing per 0019). Reject/discard/cancel/fail leaves the saved record intact; applied-then-edited rechecks via existing candidate check with manual-save escape (ADR-0030 unchanged).
- Tokens/components only (`tokens.css`, `Dialog`/`Button`/`Disclosure`/`Notice`/`InfoTip`/`Select`); no ad-hoc colors/type/radii; no dimension change (single size source untouched). Deviation, if any, goes in the ticket for review before shipping.

### Scoped clarification (extends ADR-0032 skill, same structure)

- Append a **Clarification template** section to the pinned `create-quick-action` skill only. Tone, language, rules, and section structure of the skill are unchanged; only scope narrows to command generation: when the request is vague (unknown path such as "Download folder", ambiguous app match, unknown prerequisite), return `Clarify` with 2–4 pickable choices plus a free-text slot — the grill-with-docs frontier question, scoped down.
- Enforcement stays app-side (ADR-0030/0031): request checks before inference, output checks before any candidate is usable; `Clarify` never carries executable code; answering never widens disclosure (cloud re-asks); discovery stays bounded + locally bound via opaque references; shell inference is validated by non-executing compatibility checks, never trusted blindly. No custom/editable/remote skills (ADR-0032 holds).
- Frontend renders `Clarify` as radio choices reusing the existing find-pick pattern where paths are involved; plain-text clarify keeps today's notice rendering. No new execution path: generation/clarify never runs, tests, or stops anything.

### Modules and test seams

- One deep presence boundary (connect/set/clear/close + static payload) as the principal test seam; deterministic fake IPC for tests. One deep AI assistance seam already exists (`ai_generate_draft` → draft/clarify/refused/failed) — reuse it, no new dispatcher or pass-through shared module. Existing Quick Action create/update validation stays the saving seam; manual Test/Run stays the running seam. Windows process/shell mechanics stay with their current owner.

## Testing Decisions

- Test observable workflow behavior at the seams with deterministic fakes; no prompt-string snapshots or source-text assertions.
- Presence: connect/set/close calls with the exact static payload; Discord-absent connect failure = silent no-op with no user-visible error; no OAuth/token/user-ID access anywhere (payload + IPC-shape assertion); startup never blocked; exit clears; shipped binary stays under the size budget.
- Dialog: `!aiReady` = zero AI text/nodes; Add+ready defaults AI, Edit defaults Manual; tab flip preserves typed content both ways; `Use this draft` lands in Manual with values + announcement; applied-then-edited recheck blocks with escape hatch; Ctrl/Cmd+Enter generates vs submits correctly; keyboard/screen-reader operability of tabs, outcome, and clarify choices.
- Clarify: vague fixtures (unknown folder, ambiguous app, unknown prerequisite) return `Clarify` with choices and no usable draft; answering regenerates; no raw-path upload without fresh grant; destructive/7-only/unknown-module fixtures keep existing refuse/clarify verdicts (0018 corpus + `ai-eval-fixtures.json`).
- Prior art: 172 dialog gating tests, `aiDraftDialog`/`quickActionDialog` frontend tests, `ai_assist` request/output/redirect/binding tests, managed lifecycle controlled tests, backup merge tests, Settings dirty-guard tests, shared dialog a11y tests. Each slice runs relevant existing tests + frontend check/build + backend checks/tests as affected + ownership gate before sync.

## Out of Scope

- Dynamic presence (per-screen, counts, action names), art assets/images, Join/Spectate buttons, Settings toggle for presence, per-Quick-Action presence.
- Separate "Add with AI" dialog; durable AI Mode preference in Settings; AI in the compact dock/window; AI-triggered Run/Test/Stop/elevation/auto-run; repair loops; sandbox or safety guarantees.
- Arbitrary model-file import, training/fine-tuning, universal model-server compat, Microsoft-doc catalog, custom/editable/remote skills, whole-disk indexing, persistent chat history, silent cloud fallback.
- Manual privilege/tracking/logging changes; unrelated runner/watchdog fixes; Preset/Quick Launch execution changes.

## Further Notes

- Qualification state (0018): lightweight 1.5B tier + `llama-b10702` runtime qualified on one CPU machine; stronger tier + bounded existing-local/cloud sets + false-positive bound remain open (tickets 146/150/152/155). This round assumes 146–148 behavior unchanged and adds no new model/runtime qualification.
- Research impact: on delivery, extend `0006` (two-view application) + `0019` (single-textarea hero + tab scent) with dated decision updates; do not rewrite standing rules. New numbered note only if a genuinely new topic emerges.
- Glossary impact (planned, qualifier removed by owning delivery ticket): `AI Mode` (per-dialog view: AI hero vs manual fields; planned), `Clarification` (model question blocking generation until the user picks/answers; planned), `Presence activity` (static Discord `details`/`state` text; planned). Spec link is this file.

## Ticket map and integration ownership

| Ticket | Behavioral prerequisites | Likely paths / owner symbols | Shared contract and integration edits | Candidate wave |
| --- | --- | --- | --- | --- |
| [182 Discord presence backend](182-discord-presence-offline-static.md) | None — new module; requests Application ID from user | New presence owner module; app setup/exit wiring; `Cargo.toml` dep | Settles App ID constant (placeholder → user value), static payload, silent-fail + reconnect, no Settings/backup; supplies 185 | 1 — independent |
| [183 Dialog two-view](183-quick-action-ai-two-view-dialog.md) | 172 template + 175/176/179/180 dialog sections assumed; 167 copy rules | Add/Edit dialog template + caller readiness; `api.ts`/`types.ts` (no contract change) | Owns tab strip, view state, single hero, auto-flip-to-Manual review, clarify-choices render reuse; UI-heavy research rule applies | 1 — dialog owner (coordinate with 184) |
| [184 Scoped clarify skill](184-ai-clarify-template-scoped.md) | Existing draft/clarify seam + 0018 fixtures assumed | Pinned skill resources; prompt assembly + checks; deterministic fixtures | Appends Clarification template only (structure untouched); vague→choices→regenerate; no new execution/disclosure path; supplies 183/185 | 1 — skill owner (no dialog template edits) |
| [185 Round verification](185-round-verification-integration.md) | 182 + 183 + 184 contracts | Coordinator-owned: checks, packaging, docs | Combined verification, ADR/CONTEXT/research reconciliation, manual matrices, ownership gate + verified sync | 2 — after 182–184 |

The **spec-181 integration coordinator** (ticket 185) owns shared glossary/ADR/research reconciliation, parent status, 183↔184 dialog/skill handoff, backup/size overlap, and final combined verification. Workers update their own ACs/results, never rewrite global docs. Claims above are estimates to recheck against code at dispatch (CodeGraph first). If executed as a concurrent batch, follow `docs/agents/parallel-tickets.md` with one coordinator. No parallel implementation runs during this publication session.

## Acceptance and verification

- [x] User confirmed always-attempt static presence (ID requested in ticket), two-view tabs with single hero + auto-review in Manual, and scoped grill-style clarify.
- [ ] 182 verifies static payload, silent-absent behavior, no account access, exit clear, size + ownership gates.
- [ ] 183 verifies gating, defaults, tab scent/operability, single-hero, auto-flip review, recheck hatch, keyboard/DPI/light-dark.
- [ ] 184 verifies vague→clarify→regenerate with choices, no leaked draft, no disclosure widening, both shells, fixture battery.
- [ ] 185 records Rust/frontend checks, combined manual acceptance, reconciles CONTEXT/ADR/research status, publishes each completed unit through `node tools/ownership-gate.mjs` + verified `tools\sync.ps1 -Up` (twice, expect 0 copied).

No application behavior changed in this planning session.
