# 171 - Diagnose Companion links that appear to do nothing

**Parent:** [166](166-field-cleanup-dock-filter-companion-navigation-spec.md)
**Status:** ready-for-investigation; reporter-specific conclusion awaits a concrete site/button reproduction. Not a ready-to-implement routing fix.
**Blocked by:** no implementation ticket for evidence gathering. A verified reproduction and any resulting product-policy decision are prerequisites to a repair ticket.

## Report and established facts

The reporter describes website buttons changing routes or URLs but appearing not to work in Companion. No exact site, button or expected target has been supplied. The source audit found a direct isolated native child WebView, no app-owned navigation/new-window callback, and no established app veto of ordinary routing. Browser-preview iframe behavior is not native-runtime evidence.

The bar displays the saved URL, Reload returns to that saved URL, and Open externally opens that saved URL rather than the current page. These are concrete recovery limitations, not proof of the dead-button cause. Source already contains per-site Mobile/Desktop identity and zoom despite stale earlier ticket headers.

## Investigation scope and owners

Use the diagnosing-bugs skill when executing this ticket. CodeGraph first. Likely owners: `src/routes/quick-launch-window/+page.svelte` child creation/lifetime/toolbar, Tauri child-WebView creation path, `src-tauri/src/settings.rs` site identity, existing external-opening command. Inspect pinned Tauri/WebView2 support and primary Microsoft documentation through research 0012 before selecting hooks.

Gather exact configured URL, action, before/after address, expected result, identity setting, and whether a normal browser opens a new tab. Independent progress is allowed now: create controlled test pages/fixtures for ordinary navigation, fragment/history routing, redirects, `_blank`, `window.open`, authentication return, custom-protocol handoff and actual load failure. Do not use synthetic success to claim the reporter's issue is resolved. Use a separate test profile where needed; avoid changing real account/session data.

## Acceptance criteria

- [ ] Record the supplied reproduction, or explicitly retain the missing-reproduction limitation. Separate the user's report, reproducible facts and hypotheses.
- [ ] Establish a native-runtime behavior matrix for same-document routes, full navigation, redirects, popups, auth/site identity constraints, failure feedback and external recovery. Document environment and each observation.
- [ ] Trace the failing case to a supported mechanism or record why available evidence cannot establish its cause. Distinguish suppressed popups, stale chrome, child recreation, script/site errors and unsupported authentication.
- [ ] Evaluate saved-address versus live-page external recovery and login/opener/profile implications; recommend concrete behavior without assuming opening every popup externally preserves a workflow.
- [ ] Extend research 0012 with evidence and citations; propose a bounded repair ticket and required ADR/policy amendment if justified. Otherwise record a specific outstanding prerequisite. Do not mark a repair shipped.
- [ ] Any diagnostics/fixtures are scoped and documented; no tabs, arbitrary JS bridge, relaxed isolation, reload-on-every-route or blanket navigation policy is introduced speculatively.

Ticket 170 owns the accepted saved-site selector and lifecycle, independently of this diagnosis. Coordinate any observations against its eventual implementation. The spec-166 integration coordinator owns shared documentation reconciliation. No production code fix is authorized merely by marking investigation work complete.
