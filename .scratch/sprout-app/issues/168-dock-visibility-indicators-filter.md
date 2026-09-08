# 168 - Reveal dock-hidden cards and filter their main-app lists

**Parent:** [166](166-field-cleanup-dock-filter-companion-navigation-spec.md)
**Status:** ready-for-agent
**Blocked by:** none in this round; builds on 159's existing per-item visibility state.
**Delivery coupling:** ship the Quick Launch filter with 169's matching launch scope, not ahead of it.

## Outcome

Launch entries, Quick Actions and Clips hidden from the dock remain fully usable in the main app and show a quiet eye-off annotation meaning **Hidden from dock**. Users can inspect hidden/shown items through a progressively disclosed filter without adding permanent option rows.

## Accepted interaction contract

- Place an informational eye-off glyph in card metadata alongside existing annotations, consistently across all three collections. Show it only for dock-hidden items. Give it the tooltip and accessible meaning Hidden from dock; do not dim the card, imply disabled state, nest a toggle in row activation, or alter existing run/copy/detail actions. Existing menu/edit controls change visibility.
- Put one clear **Dock visibility** filter trigger beside search in the shared toolbar. Reveal exclusive **All / Shown in dock / Hidden from dock** choices on activation. Do not put them in the feature-enable menu, inside search focus, or in a permanent checkbox strip.
- Show the trigger when the full collection contains any dock-hidden item OR the visibility choice is not All. Inspect the full collection before text search, so a query cannot make the filter vanish. With no hidden items and All, omit the redundant control.
- Keep non-default selection visible after dismissal, with an explicit reset to All. Preserve the control and reset in zero-result states and when the last hidden item becomes shown. Reset only visibility when using its reset; text search remains independently editable.
- Filter state is page-local, defaults to All, and resets when leaving the page. Do not persist it to Settings, backup, or browser storage. Typing, data refresh, or editing an item does not reset it.
- Match text search AND visibility. `show_in_dock=true` means Shown in dock; false means Hidden from dock. Use the existing missing-value/default-visible compatibility behavior. Preserve relative collection/group order; omit groups with no matching rows without changing saved membership.
- Disable every reorder path (move actions, shortcuts, drag if present) while text search or visibility filtering restricts the list. Use the existing search normalization to determine an effective query. A concise contextual reason tells users to clear filters to reorder. Filtering never writes a new order.

## Ownership and shared contract

Likely paths: `src/routes/+page.svelte`, `quick-actions/+page.svelte`, `clips/+page.svelte`, existing card/menu components and `PageHeader` toolbar composition. Reuse shared primitives/tokens, frontend-design and web-design-guidelines; no ad-hoc page variants. Own filter state, the single derived matching collection per page, indicator and reorder guards. Ticket 169 consumes the Quick Launch matching collection and active-filter condition; no independent launch-specific predicate.

Ticket 167 may edit help copy in the same pages; restrict overlap by region. Ticket 169 integrates the Quick Launch Start button and API call after this contract is available. The spec-166 coordinator owns cross-ticket integration and shared docs. No storage migration or new hidden-state flag.

## Acceptance criteria

- [ ] All three main-app collections render conditional, accessible Hidden from dock annotations without changing normal card actions.
- [ ] The filter follows the content gate, clear-trigger disclosure, exclusive choices, active-state visibility, reset and page-exit lifetime above.
- [ ] Combined text/visibility matching is the one collection used for display and exported to Quick Launch Start integration; group collapse/scroll viewport do not change which items match.
- [ ] Hide/show/edit/delete and external data refresh update results correctly; the last-hidden-item and zero-match cases always have a recovery path.
- [ ] All reorder entry points refuse to reorder under an effective text query or non-All visibility choice; clearing filters restores ordinary ordering controls.
- [ ] Verify keyboard opening/selection/Escape/focus restoration, accessible state, light/dark, narrow main-window layout, and empty/single/multiple/grouped lists.
- [ ] Add meaningful behavioral coverage for combined predicates, default-visible legacy items, last-hidden reset access and reorder gating. `npm.cmd run check` and relevant tests pass.
- [ ] Combined acceptance with 169 proves Start cannot launch an entry excluded by these filters before this filter is delivered.

Research: 0004 rule 2 (content/frequency disclosure), 0005 rule 4 (toolbar), 0006 patterns 12/14 (conditional metadata), 0008 rule 1 (filter versus feature setting). These are accepted design applications, not claims of completed usability testing.
