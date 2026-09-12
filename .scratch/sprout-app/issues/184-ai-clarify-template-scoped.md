# 184 — AI clarify template, scoped-down grill (skill structure untouched)

**What to build:** The pinned `create-quick-action` skill gains a scoped clarification template — vague requests ask back with pickable choices instead of guessing — with zero change to the skill's tone, language, rules, or section structure.

**Blocked by:** None — can start immediately. Assumes the existing draft/clarify seam + 0018 fixture corpus; supplies 183/185. Makes no dialog-template edits (183 owns the view).

**Status:** ready-for-agent

**Parent:** [181 — Discord + AI two-view + clarify (spec)](181-discord-presence-ai-two-view-clarify-spec.md). Extends [ADR-0032](../../../docs/adr/0032-model-recommendations-and-skills-ship-with-app.md) skill content (dated Amendment in this unit); ADR-0030/0031 boundaries unchanged.

## Scope

- Pinned skill resources + prompt assembly + request/output checks + deterministic fixtures only. Appends one **Clarification template** section to `create-quick-action` skill: when the request is vague (unknown path e.g. "Download folder", ambiguous app match, unknown prerequisite), return `Clarify` with 2–4 pickable choices plus a free-text slot — the grill-with-docs frontier question scoped to command generation.
- Enforcement stays app-side: request checks before inference, output checks before usability; `Clarify` carries no executable code; answering never widens disclosure (cloud re-asks); discovery stays bounded + locally bound via opaque references; shell inference validated by non-executing checks. No custom/editable/remote skills, no new execution/disclosure path.
- Explicitly not built: dialog rework (183), new provider/runtime support (146/150–152), diagnosis skill changes (153), persistent chat history, whole-disk indexing.

## ACs

- [ ] Skill diff is append-only to the Clarification section: tone/language/rules/structure of existing sections byte-identical except the new template; NOTICES/attribution retained; bundled-resource packaging unchanged.
- [ ] Vague fixtures (unknown folder, ambiguous app, unknown prerequisite) return `Clarify` with choices and expose no usable draft via preview/stream/Copy/Save; answering with a pick regenerates; unknown/stale/modified references rejected and rebound locally.
- [ ] Both shells covered; destructive/7-only/missing-module fixtures keep existing refuse/clarify verdicts; edited-candidate recheck preserved; zero script-execution calls across generate/clarify/validate/cancel/save (managed startup + fixed read-only discovery distinguished, as before).
- [ ] Cloud path: no request before consent, no raw discovery data without preview+grant, no destination/redirect bypass, no credential leakage (assert outbound payloads).
- [ ] Relevant backend checks/tests + fixture battery green; `node tools/ownership-gate.mjs` passes.

## Implementation notes

- Sources: 0018 corpus + `ai-eval-fixtures.json` + shared-rules/diagnose skills as prior art; spec 145 testing decisions for seam choice (highest reusable seam, one deep module). Claims above are estimates to recheck against code at dispatch (CodeGraph first).

## Verification

- Fixture + combination + path-binding checks per spec 145 (malformed/truncated/unsupported/cancel/timeout/uncertain/refused, spaces/quotes/metachars/Unicode/dupes/missing/stale/reparse escapes); packaging check (installed app serves pinned skills without checkout); ownership gate before sync.
