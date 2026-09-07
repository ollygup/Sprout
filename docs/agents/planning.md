# Planning and ticket slicing — agent reference

> Read this file when: you grill with docs, create/revise a spec, or split
> work into tickets. Planning can proceed while earlier implementation is pending.

## Keep design and implementation status distinct

- Before a new round, read `docs/CONTEXT.md`, the ADR index and relevant
  decisions/amendments, plus related open specs and ticket status. When
  asserting current behavior, verify source (CodeGraph first when indexed)
  and relevant evidence; accepted prose and `ready-for-agent` are not proof
  that a feature exists.
- In each spec, distinguish current behavior, accepted-but-unimplemented
  behavior (linked spec/tickets), and this round's proposed changes. Record
  which pending decisions the proposal assumes. No need to wait for all
  earlier tickets to finish before discussing or documenting the next round.
- Keep `CONTEXT.md` a glossary. Accepted future vocabulary may be added with
  a short `planned` qualifier and a spec link; keep implementation status,
  ACs, dependencies, and detailed behavior in the tracker. Mark verified
  delivery in the owning ticket before removing the qualifier.
- If a new round changes an accepted decision, append a dated ADR amendment
  and update affected specs/tickets and their dependencies in the same unit
  of work. Do not silently overwrite a term or leave contradictory active
  requirements for implementers. Concurrent planning uses a pinned baseline;
  reconcile changed decisions before publishing its findings.

## Make tickets suitable for parallel work

Before marking a set ready for implementation, provide a table in the parent
spec: ticket, behavioral prerequisites, likely paths/owner symbols, shared
contracts or integration edits, and candidate parallel wave. Put each
ticket's claims and exclusions in its own text too. Claims are estimates to
recheck against code at dispatch, not permanent file locks.

- Prefer coherent, independently verifiable slices around existing owners.
  Avoid broad refactors/formatting or repeated global-doc edits in every
  ticket. Assign common documentation and registration edits to a named
  integration owner. Do not split into shallow modules or duplicate Windows
  invocation knowledge merely to avoid file overlap.
- Settle shared interface/data contracts before concurrent consumers start;
  record any small prerequisite ticket separately. Keep genuine behavioral
  dependencies even if they reduce concurrency. File overlap by itself does
  not require a dependency: plan integration under `parallel-tickets.md`.
- List which tickets may start together and why. For unavoidable overlap,
  name the integration owner and combined verification. Independent work
  within a blocked ticket may be explicitly carved out, but do not declare
  the full ticket ready by assuming an unfinished prerequisite exists.
- Later specs depend on the specific earlier tickets whose behavior they
  need, rather than the entire planning parent when that is unnecessary.

For spec 145, the existing ticket map makes 146 (qualification) and 147
(shell compatibility) initial concurrent candidates. Ticket 148 depends on
both. After 148, 149, 151, and 153 are behavioral concurrency candidates,
subject to a real path/owner/contract preflight; the map alone does not prove
their edits are disjoint. A new grill may use the accepted design now while
keeping these implementation prerequisites explicit.
