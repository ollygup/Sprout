# 171 - Diagnose Companion links that appear to do nothing

**Parent:** [166](166-field-cleanup-dock-filter-companion-navigation-spec.md)
**Status:** controlled fixture and source investigation complete; native matrix and reporter-specific conclusion remain blocked on the prerequisites below. Not a ready-to-implement routing fix.
**Blocked by:** no implementation ticket for evidence gathering. A verified reproduction and any resulting product-policy decision are prerequisites to a repair ticket.

## Report and established facts

The reporter describes website buttons changing routes or URLs but appearing not to work in Companion. No exact site, button or expected target has been supplied. The source audit found a direct isolated native child WebView, no app-owned navigation/new-window callback, and no established app veto of ordinary routing. Browser-preview iframe behavior is not native-runtime evidence.

The bar displays the saved URL, Reload returns to that saved URL, and Open externally opens that saved URL rather than the current page. These are concrete recovery limitations, not proof of the dead-button cause. Source already contains per-site Mobile/Desktop identity and zoom despite stale earlier ticket headers.

## Investigation scope and owners

Use the diagnosing-bugs skill when executing this ticket. CodeGraph first. Likely owners: `src/routes/quick-launch-window/+page.svelte` child creation/lifetime/toolbar, Tauri child-WebView creation path, `src-tauri/src/settings.rs` site identity, existing external-opening command. Inspect pinned Tauri/WebView2 support and primary Microsoft documentation through research 0012 before selecting hooks.

Gather exact configured URL, action, before/after address, expected result, identity setting, and whether a normal browser opens a new tab. Independent progress is allowed now: create controlled test pages/fixtures for ordinary navigation, fragment/history routing, redirects, `_blank`, `window.open`, authentication return, custom-protocol handoff and actual load failure. Do not use synthetic success to claim the reporter's issue is resolved. Use a separate test profile where needed; avoid changing real account/session data.

## Acceptance criteria

- [x] Record the supplied reproduction, or explicitly retain the missing-reproduction limitation. Separate the user's report, reproducible facts and hypotheses.
- [ ] Establish a native-runtime behavior matrix for same-document routes, full navigation, redirects, popups, auth/site identity constraints, failure feedback and external recovery. Document environment and each observation.
- [x] Trace the failing case to a supported mechanism or record why available evidence cannot establish its cause. Distinguish suppressed popups, stale chrome, child recreation, script/site errors and unsupported authentication.
- [x] Evaluate saved-address versus live-page external recovery and login/opener/profile implications; recommend concrete behavior without assuming opening every popup externally preserves a workflow.
- [x] Extend research 0012 with evidence and citations; propose a bounded repair ticket and required ADR/policy amendment if justified. Otherwise record a specific outstanding prerequisite. Do not mark a repair shipped.
- [x] Any diagnostics/fixtures are scoped and documented; no tabs, arbitrary JS bridge, relaxed isolation, reload-on-every-route or blanket navigation policy is introduced speculatively.

Ticket 170 owns the accepted saved-site selector and lifecycle, independently of this diagnosis. Coordinate any observations against its eventual implementation. The spec-166 integration coordinator owns shared documentation reconciliation. No production code fix is authorized merely by marking investigation work complete.

## Investigation record — 2026-09-09

### Report

The only report remains “buttons changing routes or URLs appear not to work.”
No site, button, before/after address, expected destination, browser identity,
normal-browser behavior, screenshot, console entry, or network trace was
supplied. The reporter issue is not reproduced or resolved.

### Reproducible facts

- Baseline `batch-151-170-171-20260909-01` pins Tauri 2.11.5,
  `tauri-runtime-wry` 2.11.4, Wry 0.55.1, and `webview2-com` 0.38.2.
- The native Companion child is created directly from
  `src/routes/quick-launch-window/+page.svelte` with creation/error callbacks,
  but no app-owned navigation, completion, live-source, external-scheme, or
  new-window callback. Open externally receives the saved `companionUrl`.
- Pinned Wry source marks a Windows `NewWindowRequested` handled when the host
  supplies no handler. That is a supported silent-popup mechanism in this
  configuration, but the missing reporter action prevents matching it to the
  report.
- `tools/companion-navigation-fixture/server.mjs` provides controlled fragment,
  history, full-load, redirect, popup, auth-return, custom-protocol, and aborted
  transport cases plus `/events` request evidence.
- `node tools/repro-companion-navigation.mjs` ran on Node 24.19.0 and passed all
  eight controlled contracts. This is fixture evidence, not native evidence.
- The baseline also contains Back/Forward state for the browser-preview iframe;
  it has no native-WebView history connection. Ticket 170 owns reconciliation
  because its accepted scope rejects history chrome; ticket 171 changes no
  production source.

The cited support, full controlled matrix, saved/live recovery analysis, and
ranked hypotheses are in `docs/research/0012-companion-webview-feasibility.md`.

### Pending native matrix

Prerequisite A: host the unchanged local fixture at a trusted HTTPS origin.
The local harness intentionally binds HTTP loopback, while production Companion
validation accepts only HTTPS. Do not weaken validation or put the HTTP URL in
a real profile. Prerequisite B: run Sprout with an isolated app-data directory
and capture the WebView2 runtime version. Prerequisite C for a reporter-specific
conclusion: obtain the exact site and action plus permission to use any required
test account in the isolated profile.

For the coordinator's native pass:

1. Run `node tools/repro-companion-navigation.mjs`; require `RESULT 8/8`.
2. Run `node tools/repro-companion-navigation.mjs --serve` and publish/reverse-
   proxy that exact loopback fixture to the approved trusted HTTPS test origin.
3. Launch the coordinator candidate with a new isolated `LOCALAPPDATA`, save
   only the trusted fixture root, dock Companion, and record candidate ID,
   Windows version, WebView2 runtime version, identity, dock edge/mode, and URL.
4. Restart the fixture before each row. Exercise fragment, `pushState`, full
   load, redirects, `_blank`, `window.open`, in-profile auth return, custom
   protocol, and aborted load. For every row record visible page/readout,
   `/events`, whether a new surface or OS prompt appeared, and Sprout feedback.
5. Repeat popup rows under both Mobile and Desktop identities. Compare the
   toolbar's saved-address external action with the current route; for auth,
   verify an external browser has no implied Companion cookie or opener state.
6. If UI plus `/events` cannot classify the result, use a temporary diagnostic
   build at the native child-creation seam to capture navigation ID, redirect,
   source change, new-window URI/user-initiation/disposition, external-scheme
   initiation, and completion `IsSuccess`/`WebErrorStatus`. That diagnostic
   contract requires separate path ownership and is not implemented by 171.

### Hypotheses and decision

Ranked hypotheses are: silent popup suppression; successful navigation hidden
by saved-address chrome; embedded-auth identity/profile refusal; custom-
protocol handoff; actual load/script failure. Each has a distinct fixture/server
or native-event prediction in research 0012.

Saved-address recovery is stable but loses the live route. Live-address
recovery can carry transient credentials into a different browser profile and
still cannot preserve cookies or opener semantics. Keep the saved action until
the native/reporter evidence supports a policy. No repair ticket or ADR
amendment is justified yet; the exact reporter reproduction plus the trusted-
HTTPS native matrix are the outstanding prerequisites.
