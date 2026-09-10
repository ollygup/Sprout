# 169 - Start only the Quick Launch entries matching the active filters

**Parent:** [166](166-field-cleanup-dock-filter-companion-navigation-spec.md)
**Status:** implemented batch-149-168-20260909 (rode with 168) — published via sync (joint manual verify pending)
**Blocked by:** 168's matching-list and active-filter contract for frontend integration. Backend selection validation and tests may start independently against the contract below.
**Delivery coupling:** integrate and deliver with 168's Quick Launch filter.

## Outcome

With text search or a non-All dock-visibility filter active, the main Quick Launch button reads **Start matching (N)** and starts exactly the matching entries. N counts matches across groups, including collapsed groups; it is not a viewport count. Zero matches disables Start and must never fall back to launching everything. Without an active filter, existing full-list Start behavior remains.

## Existing owners and implementation contract

Current source: `src/lib/api.ts` `startQuickLaunch()` invokes `start_quick_launch` without arguments. `src-tauri/src/lib.rs` `start_quick_launch` loads all saved entries and delegates to `launch_entries`, then the capped batch queue. Individual `start_launch_entry` uses a different single-entry path that foregrounds existing apps; do not implement filtered Start by looping individual calls.

Extend the existing batch command and wrapper with an optional list of selected entry IDs:

- Omitted selection means the current full-list behavior for existing callers.
- Explicit empty selection rejects without launching anything. It is never equivalent to omission.
- Resolve selected IDs from authoritative saved entries, deduplicate and retain canonical saved order. Do not accept caller-authored command bodies or order as launch truth.
- If any selected ID no longer exists, reject the request before starting a batch and report that the list changed; refresh the list so the user can retry. Do not silently broaden selection or launch a partial unexpected set.
- Feed the selected saved records through existing `launch_entries` once, preserving batch concurrency, single-flight, existing-app behavior, logging and completion/error feedback.
- Main frontend reads IDs, count and active-filter state from 168's matching collection at activation time. No second search/visibility predicate. Refresh/filter changes after dispatch do not change the already-requested batch.
- Dock batch selection, single-entry launch and startup auto-run behavior remain unchanged. Dock-hidden entries are runnable in main-app All/Hidden results, as agreed; hidden from dock is not disabled.

These contracts settle empty/stale selection behavior for implementation; no new execution owner, queue, per-item opt-out or Group-Start feature is introduced. Likely paths: `src/routes/+page.svelte`, `src/lib/api.ts`, `src-tauri/src/lib.rs`, existing launch tests and suitable pure selection tests. Follow existing Windows-operation ownership. Ticket 168 owns matching/filter/reorder state; 169 owns the Start integration and batch input validation. The spec-166 coordinator owns combined acceptance and final shared-doc status.

## Acceptance criteria

- [x] Text-only, visibility-only and combined filtering use Start matching (N) and launch exactly the matching saved IDs; count includes matching collapsed-group entries.
- [x] No filter keeps the existing full-list label/path. Zero matches disables the button; a direct empty-ID request launches nothing and returns an appropriate error.
- [x] Authoritative selection handles duplicates, saved order and missing IDs according to the contract; no request can fall back to all entries accidentally.
- [x] One capped batch is used; existing single-flight, skips/foreground distinction, logs, completion events and errors remain intact. No loop of individual launches.
- [x] Tests with harmless fixtures/stubs cover subset, omitted, empty, duplicate, unknown/stale IDs and combined filters without running user commands. Verify no excluded record reaches the queue.
- [ ] Verify filter refreshes, count/label changes, failure recovery, hidden entries, collapsed groups and the 168 zero-match/reset/reorder behavior together.
- [x] `npm.cmd run check`, relevant frontend tests, `cargo check` and relevant Rust tests pass. Record results and limitations; apply existing UI skills/tokens to label changes.

The accepted behavior supersedes search-insensitive main Start described by the 156 baseline. ADR-0018's batch pipeline remains the implementation owner; existing unrelated launch limitations are not part of this ticket.
