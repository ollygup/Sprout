# Comments — agent reference

> Read this file when: you write or edit code comments. Otherwise skip it.

- MUST NOT add WHAT comments — WHEN the code needs one to be understood → MUST fix the naming/interface instead.
- MUST use WHY comments only, self-contained (readable without opening a tracker).
- MUST point to an ADR by name for durable rationale, MUST NOT point to a ticket number (tickets close/renumber; ADRs don't).
- External constraints (upstream bugs, library quirks) MAY cite the issue link directly.
- MUST NOT add history trails in comments (ticket 1 → 5 → 10) — that's `git log`/`blame`'s job, not the code's.
