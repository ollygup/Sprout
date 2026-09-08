# 166 - Field cleanup, dock visibility discovery, and Companion navigation (spec)

**Status:** design interview in progress; first-round decisions accepted 2026-09-08. Not ready for implementation.

## Verified baseline

The 2026-09-08 source audit found five permanent Ctrl+Enter hints, persisted per-item dock visibility with menu/edit controls but no card indicator or list filter, and a Companion toolbar displaying the saved address as plain text. Reload, per-site zoom and identity already exist in source despite stale ready-for-agent headers in spec 156. These are source observations, not renewed runtime verification. Full field inventory: [research 0017](../../../docs/research/0017-field-helper-audit.md).

Companion has no app-owned navigation/new-window handling identified by that audit. URL changes are not an established cause of the reported dead button. Open externally uses the saved address rather than the live page. A concrete site/button reproduction remains needed.

## Accepted first round

1. Remove the five repeated Ctrl+Enter hint lines; preserve shared dialog submission behavior. Across all Sprout-authored fields, remove obvious tutorials and duplicate explanations, shorten useful guidance, and retain constraints, defaults, errors and consequential behavior. Correct stale auto-hide copy and improve missing field labels. Research 0017 is the per-field review inventory; exact replacement copy still requires implementation review.
2. Launch entries, Quick Actions and Clips hidden from the dock show a quiet informational eye-off indicator with the meaning **Hidden from dock**. Do not dim or imply the item is disabled; existing menu/edit actions change visibility.
3. Add a **Dock visibility** filter beside search with exclusive **All / Shown in dock / Hidden from dock** choices. The user explicitly requires progressive disclosure: reveal choices from a clear filter trigger rather than showing a permanent choice strip or two checkboxes. A non-default selection remains visible after closing the menu, with a clear reset; opening/focusing search remains text search. Filter persistence, content gating and Start-all semantics are unsettled below.
4. With multiple configured sites, the Companion site label becomes **Site name + chevron**, disclosing saved alternatives with the active site marked. Address is the fallback for an unnamed site. Open externally stays separate; site authoring stays in the main app. One active site, dock-only visibility and isolation remain required. This is not an omnibox or a tab strip.

Evidence: research 0004 rules 2-3 (frequency and quick-access/configuration split), 0005 rule 4 (search/filter toolbar), 0006 patterns 12/14 (conditional metadata), 0008 rule 1 (classify a list filter separately from feature activation), and 0010's 2026-09-08 hint evidence. Research supports these design hypotheses; no Sprout usability study has yet validated exact placement.

## Decision frontier

Follow-up source check: main Start-all ignores text search; reordering on all three collection pages uses full-list/group neighbors even when search hides them. Companion site switching closes and recreates the child at the saved URL with the selected site's UA/zoom and a persistent shared Companion browser profile; no background tab is kept alive.

- List filter lifetime: reset on returning to a page, retain per page during the session, or persist across launches?
- Quick Launch Start scope while search/visibility filtering is active: start the matching rows or retain all-entry execution with explicit labeling?
- Reordering while search or visibility filtering is active: disable move controls until filters clear, or define movement among matching rows without unexpectedly crossing invisible neighbors?
- Filter content gating: when no item is hidden, omit the redundant control; ensure an active filter and its reset never disappear if the last matching item changes state. Proposed, not yet accepted.
- Site switching lifecycle: navigate to the selected saved address or attempt to restore a last visited page? Background tabs are excluded by ADR-0022. Unsaved remote-page state needs an honest contract.
- Routing: obtain exact site/button and expected behavior; distinguish ordinary navigation, same-document routing, new-window requests and auth/site restrictions before selecting a fix. No navigation policy or scope expansion accepted yet.

## Dependencies and reconciliation

- Submission behavior remains owned by 157. The visible-hint obligations in 109, 114 and 158 are superseded by this round; do not re-add them while cleanup is pending.
- Visibility presentation extends 159's persisted flag; no new hidden/disabled domain flag. Spec 156's surface Start-all statement needs clarification once filter scope is answered.
- Site picker reuses the active-site concept and existing site preferences from 156/162/165. ADR-0022 is amended to permit on-dock saved-site selection; site lifecycle details remain open.
- No dependency on the entire AI-authoring spec 145. When implementing cleanup, reconcile any fields introduced since the source audit rather than claiming the inventory covers future UI.

## Validation and delivery still required

After the frontier is settled, write coherent implementation tickets with prerequisites and owner paths before marking ready. Verify helper cleanup on every inventoried surface, keyboard/accessible names, indicator meaning, filter empty/reset states, filtered actions and reordering, and Companion picker at narrow widths with native child-WebView layering. Test routing against an actual reproduction; preserve isolation and expose honest failures. No implementation AC is complete in this spec.
