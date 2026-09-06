# 138 — Companion site names: `{url, name}`, unique both, dropdown shows name

**What to build:** Each Companion saved site carries an optional name end to end: the user names sites once, and everywhere a site is picked it shows the name (URL only as fallback).

**Blocked by:** none — can start immediately.

**Status:** ready-for-agent

## Scope

- Saved-site shape grows from strings to `{url, name}` with tolerant migrate-on-read of legacy string entries (blank name = URL).
- Uniqueness enforced both ways, trimmed and case-insensitive: duplicate URLs refused (whole-URL identity, trailing slashes ignored, per the ADR-0022 amendment) and duplicate names refused, each with a plain-language message.
- Site authoring surface edits names inline (add/rename/delete, position-preserving); the Settings active-site dropdown renders `name || url`; whole-app backup carries names; display-name helper shared by all callers (no per-surface fallback drift).

## ACs

- [x] Blank name renders the URL everywhere — nothing ever renders blank.
- [x] Duplicate URL and duplicate name are both refused (trimmed, case-insensitive) with a message stating which collided.
- [x] Legacy `string[]` payloads migrate on read without data loss; round-trip save keeps order and names.
- [x] Settings active-site dropdown lists names; selecting by name activates the right URL.
- [x] `npm.cmd run check` 0 errors; settings/companion/backup test slices green.

## Implementation notes

- Dedup stays whole-URL (not host+path) to match the audited behavior; name uniqueness is a separate check on the display label.
- Glossary: `Companion site` / `Companion site name` usages updated where the round touches them.

## Verification

- `npm.cmd run check`, settings/companion/backup tests; manual: add → name → duplicate-name refused → duplicate-URL refused → pick from Settings dropdown → backup/restore keeps names.
