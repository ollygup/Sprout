# 166 - Field cleanup, dock visibility discovery, and Companion navigation (spec)

**Status:** design accepted; ticket package finalized 2026-09-09. Implementation is pending in 167-170; 171 is a separate investigation, not a promised routing fix. User authorized ticket creation and sync.

## Verified baseline

The 2026-09-08 source audit found five permanent Ctrl+Enter hints, persisted per-item dock visibility with menu/edit controls but no card indicator or list filter, and a Companion toolbar displaying the saved address as plain text. Reload, per-site zoom and identity already exist in source despite stale ready-for-agent headers in spec 156. These are source observations, not renewed runtime verification. Full field inventory: [research 0017](../../../docs/research/0017-field-helper-audit.md).

Companion has no app-owned navigation/new-window handling identified by that audit. URL changes are not an established cause of the reported dead button. Open externally uses the saved address rather than the live page. A concrete site/button reproduction remains needed.

## Accepted first round

1. Remove the five repeated Ctrl+Enter hint lines; preserve shared dialog submission behavior. Across all Sprout-authored fields, remove obvious tutorials and duplicate explanations, shorten useful guidance, and retain constraints, defaults, errors and consequential behavior. Correct stale auto-hide copy and improve missing field labels. Research 0017 is the per-field review inventory; exact replacement copy still requires implementation review.
2. Launch entries, Quick Actions and Clips hidden from the dock show a quiet informational eye-off indicator with the meaning **Hidden from dock**. Do not dim or imply the item is disabled; existing menu/edit actions change visibility.
3. Add a **Dock visibility** filter beside search with exclusive **All / Shown in dock / Hidden from dock** choices. The user explicitly requires progressive disclosure: reveal choices from a clear filter trigger rather than showing a permanent choice strip or two checkboxes. A non-default selection remains visible after closing the menu, with a clear reset; opening/focusing search remains text search. The accepted lifetime, gate and action scope are below.
4. With multiple configured sites, the Companion site label becomes **Site name + chevron**, disclosing saved alternatives with the active site marked. Address is the fallback for an unnamed site. Open externally stays separate; site authoring stays in the main app. One active site, dock-only visibility and isolation remain required. This is not an omnibox or a tab strip.

Evidence: research 0004 rules 2-3 (frequency and quick-access/configuration split), 0005 rule 4 (search/filter toolbar), 0006 patterns 12/14 (conditional metadata), 0008 rule 1 (classify a list filter separately from feature activation), and 0010's 2026-09-08 hint evidence. Research supports these design hypotheses; no Sprout usability study has yet validated exact placement.

## Accepted second round

Follow-up source check: main Start-all ignores text search; reordering on all three collection pages uses full-list/group neighbors even when search hides them. Companion site switching closes and recreates the child at the saved URL with the selected site's UA/zoom and a persistent shared Companion browser profile; no background tab is kept alive.

- The visibility filter resets to All on leaving the page. It is not a saved preference. Data refresh and in-page edits preserve an active filter.
- Show its trigger only when the full collection contains hidden items or a non-All visibility filter is active. Text search must not hide the trigger. Keep active state and reset accessible even when results are empty or the last hidden item becomes shown.
- Text search and visibility selection intersect. Main Quick Launch starts only matching entries, labeled **Start matching (N)** while either filter is active; zero matches disables Start. Collapsed groups and scrolling do not change the matching set. Unfiltered Start retains full-list behavior.
- Disable reorder controls while either effective text search or a non-All visibility filter is active. Clear filters to reorder; never reorder through invisible neighbors.
- Site switching opens the selected saved address with its saved identity/zoom and persistent Companion cookies. Returning does not restore a last route, unsaved form, or background tab. Site authoring remains in the main app; choosing the active site does not implicitly reload it.
- Routing evidence is still missing. Ticket 171 gathers native-runtime evidence and the exact site/button before a repair is proposed. This does not block the accepted cleanup/filter/picker implementation and does not authorize speculative navigation policy.

## Refreshed baseline - 2026-09-09

After the user reported shared changes, guarded Up twice and Down each reported zero copies. The sync-managed inventories both contained 452 files with no local-only or missing files. Raw comparison found 450 byte-identical files; spec 166 and research 0017 were equal after normalizing text encoding/line endings. Build caches and inert git data are outside the sync comparison.

Current source reconfirms full-list batch Start, radio-capable ContextMenu, and one-child Companion switching. AI authoring has added Shell, request/context textareas and provider/endpoint/model settings. There are now five submit shortcut hints plus one generate shortcut hint. Ticket 167 and research 0017 include these additions and preserve the AI fields' generate/stop-propagation behavior. Accepted AI scope remains governed by ADR-0030/0031/0032; stale implementation-pending prose is not evidence that these fields are absent.

## Dependencies and reconciliation

- Submission behavior remains owned by 157. The visible-hint obligations in 109, 114 and 158 are superseded by this round; do not re-add them while cleanup is pending.
- Visibility presentation extends 159's persisted flag; no new hidden/disabled domain flag. 168 owns display/filter/reorder state; 169 supplies matching batch execution. Deliver their Quick Launch changes together.
- Site picker reuses the active-site concept and existing site preferences from 156/162/165. ADR-0022 records on-dock selection and the accepted saved-address lifecycle.
- No dependency on the entire AI-authoring spec 145. When implementing cleanup, reconcile any fields introduced since the source audit rather than claiming the inventory covers future UI.

## Ticket map and integration ownership

| Ticket | Behavioral prerequisites | Likely paths / owners | Shared contract and integration edits | Candidate wave |
| --- | --- | --- | --- | --- |
| [167 - Field guidance](167-selective-field-guidance-cleanup.md) | Existing 157 grammar and current AI editor; no new-round dependency | FormDialog components; Settings, Companion manager, Plan and collection copy | All-field audit; preserve AI generate versus ordinary submit; helper-only overlaps with 168/170 | 1 |
| [168 - Visibility discovery](168-dock-visibility-indicators-filter.md) | Existing 159 visibility state | Three collection routes, ContextMenu, PageHeader toolbar and row metadata | Own matching collection, active-filter flag, content gate and reorder guards; supplies 169 | 1; deliver Quick Launch with 169 |
| [169 - Start matching](169-start-matching-quick-launch.md) | 168 matching-list contract for frontend integration | Main Quick Launch Start; api.ts; lib.rs start_quick_launch/launch_entries | Optional authoritative ID subset: omitted=all, empty=reject, stale ID=reject, saved order; reuse batch queue | 2; backend/tests can start in 1 |
| [170 - Site picker](170-companion-saved-site-picker.md) | Existing active-site and per-site UA/zoom from 162/165 | Dock Companion toolbar/lifecycle; existing setCompanionUrl; manager edit preference preservation | Existing active-site store and one child; preference edit overlap with 167 coordinated | 1 |
| [171 - Routing investigation](171-companion-navigation-failure-investigation.md) | Evidence gathering can start; reporter-specific conclusion needs reproduction | Companion creation/lifetime, pinned WebView2/Tauri, research 0012 | No production fix or new policy; record cause and a bounded follow-up if supported | Independent research; not a gate for 167-170 |

The **spec-166 integration coordinator** owns shared glossary/ADR/research reconciliation, parent status, overlapping page integration and final combined verification. Workers update their own ACs and results rather than each rewriting global docs. Claims above are estimates to recheck at dispatch. 167/168/170 can make independent behavioral progress, but shared files require coordinated integration; this table is not permission for independent sync sessions. If executed as a concurrent ticket batch, follow docs/agents/parallel-tickets.md with one coordinator. No parallel implementation is being run during this ticket-publication session.

Ready means requirements are agreed, not shipped or tested. The 169 dependency is real; 168's Quick Launch filter must not be delivered without matching launch semantics. 171 may remain unresolved without delaying the agreed UI work.

## Acceptance and verification

- [x] User confirmed first-round cleanup, indicator/filter, and site selector.
- [x] User confirmed filter gate/lifetime, matching Start, reorder restriction and saved-address switching lifecycle.
- [x] Implementation/investigation tickets include owner claims, exclusions, dependencies and verification requirements.
- [ ] 167 verifies every inventoried field, including new AI fields; retained guidance and accessible names remain adequate.
- [ ] 168 and 169 jointly verify hidden-state meaning, all filter combinations, zero-match/reset behavior, disabled reordering and no excluded launches.
- [ ] 170 verifies selection, preference preservation and native child-WebView layering at real dock sizes/DPI, keyboard-only, light/dark, fixed/auto-hide and both edges.
- [ ] Integration coordinator records relevant frontend/Rust checks and combined manual acceptance, reconciles pending glossary/ADR status and publishes each completed unit through ownership gate and verified sync.
- [ ] 171 records reproducible routing evidence or a specific outstanding prerequisite; any repair remains separately scoped.

The documentation package is complete; these unchecked items are future implementation/investigation work. No application behavior changed in this planning session.
