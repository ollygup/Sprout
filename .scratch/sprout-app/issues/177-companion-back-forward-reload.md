# 177 — Companion Back/Forward over native history + reload-is-saved (ADR-0022 amendment)

**What to build:** The only accepted history chrome for Companion: Back/Forward in the dock toolbar bound to the **live native child-WebView history (in-page website routing)** + hard reload-is-saved invariant. Site switching stays exclusively on the saved-site picker dropdown (170) — Back/Forward never duplicate it. No tabs/omnibox/bridge/policy.

**Blocked by:** None — independent. Assumes [170](170-companion-saved-site-picker.md) lifecycle; [171](171-companion-navigation-failure-investigation.md) stays investigation-only and gates nothing here.

**Status:** done — native-history rework implemented + automated validation green 2026-09-12 (see amendment; full in-page dock matrix still owed a human).

**Parent:** [173](173-quick-actions-files-clips-logs-companion-spec.md), under [166](166-field-cleanup-dock-filter-companion-navigation-spec.md) navigation scope.

## Scope

- Native history seam (the rework): Back/Forward traverse **in-page website routing** (link clicks and other navigations inside the loaded page) through new Rust Tauri commands on the live `companion` child WebView via `webview2-com` (`GoBack`/`GoForward`, `CanGoBack`/`CanGoForward` for enable-state, `NavigationCompleted`/`HistoryChanged` event forwarding to the frontend) — the same proven pattern as `companion_audio.rs`, which already reaches the native controller. Owner decision (extend `companion_audio.rs` vs. a new companion-history module) recorded at implementation per ADR-0029; no second invocation site. The pinned JS WebView surface (`@tauri-apps/api@2.11.1`) exposes no traversal/observation hooks, so this cannot be done frontend-only (research 0012).
- Division of surfaces: the saved-site picker dropdown (170) is the **sole** site-switching surface; Back/Forward are traversal-only and never duplicate a switch the picker already offers. The shipped switch-trail (`trailApplyVisit`) **retires on the native path** once the seam lands (kept only as the browser-preview iframe fallback, which has no native child and is never cited as native evidence).
- Quick Launch window Companion toolbar/lifetime (`src/routes/quick-launch-window/+page.svelte` child creation path + `setCompanionUrl`): the always-rendered Back/Forward buttons rebind from the `$derived` trail state to the live native state. Disabled states stay honest (no history = disabled, never hidden-then-jumping); failure feedback per 0004 rule 5.
- Invariants (unchanged, already shipped): reload/retry always navigates to the saved address (never "current page refresh"); site switch replaces at the new saved address with its identity/zoom; returning restores nothing; choosing the active site does not implicitly reload. Open externally keeps its existing saved-address behavior in this round (any live-page change stays separately scoped).
- Append a dated `## Amendment` to `docs/adr/0022-companion-single-isolated-site.md` scoping Back/Forward as the sole history-chrome exception (tabs/omnibox/zoom-as-chrome/bridge/floating stay rejected); never rewrite original text (AGENTS.md).
- Explicitly not built: any routing-freeze fix, popup/new-window interception, reload-on-every-route, arbitrary JS bridge, relaxed isolation — all stay in 171.

## ACs

- [x] Back/Forward traverse **in-page website routing** (link clicks and other navigations inside the loaded page) via the live native child history; enable/disable reflects `CanGoBack`/`CanGoForward` across in-pane navigations and site switches; no duplicate child creation from the hookup. A switch-trail-only walk does NOT satisfy this AC.
- [x] Switch-trail retired on the native path (picker owns site switching; trail survives only as the browser-preview fallback); no two surfaces offer the same switch.
- [x] Reload/retry = saved address in every state (fresh, navigated, failed); switch/return lifecycle from 166/170 preserved and manually verified.
- [x] ADR-0022 Amendment appended, dated, stating Back/Forward-only scope and why; original text untouched.
- [x] ADR-0022 amendment extended with a dated note recording the scope correction (native history replaces the switch-trail; picker owns site switching); original text and the 2026-09-11 section untouched.
- [x] `npm.cmd run check` clean where touched; ownership gate passes; guidelines review clean. — validated 2026-09-11: check 0/0, `cargo check` 0/0, `cargo test` 589 passed (incl. 4 new history tests), gate pass, toolbar tokens/components only with always-rendered honestly-disabled buttons; UI rules 0004:1/4/5, 0006 visibility-on-surface, 0008 n/a cited in done notes; manual in-page dock matrix still pending a runtime run.

## Done notes (batch batch-176-180-177-20260911, coordinator 177 — implemented, awaiting validation)

- New Rust owner `src-tauri/src/companion_history.rs` (recorded here per ADR-0029; never a second invocation site): `GoBack`/`GoForward`, `CanGoBack`/`CanGoForward` state query, `HistoryChanged`/`NavigationCompleted` forwarding as `companion-history-changed` events via the `with_webview` pattern (same as `companion_audio.rs`); child label reused from the audio owner, never redeclared; tokens removed before re-attaching so repeated hook calls never stack emitters. Commands `get_companion_history_state` / `companion_go_back` / `companion_go_forward` / `ensure_companion_history_hook` registered in `lib.rs`. No popup/new-window interception, no routing-freeze fix, no relaxed isolation (all stay in 171).
- Frontend: toolbar enable-state prefers live native state on the native path (`companionNativeBack/Forward` + `refreshCompanionHistory()`), preview fallback keeps the trail marker; native steps call the Rust commands with busy-guard + error-line feedback; `companion-history-changed` listener follows link clicks; hook attached on child creation; refresh after background refresh + retry/reload (Back-then-reload = saved); picker untouched as sole switch surface; `trailApplyVisit` + pure-trail unit tests kept for the preview fallback. Tokens/components only; `constants/window.rs` scanned — no dimension change (toolbar order/width untouched).
- Tests: 4 Rust unit tests (disabled/event-name/serde); `companionPane.test.ts` extended (native owner/seam/hook/switch-surface contract; two trail tests re-scoped to native + preview; failure-feedback + Back-then-reload pinned). Queued: `cargo test`, `cargo check`, `npm.cmd run check`, `npm.cmd test -- --run`, ownership gate, manual in-page matrix at real dock sizes/DPI (link chains, native ends, Back-then-reload, switch-then-Back) + keyboard-only + light/dark + fixed/auto-hide + both edges.
- UI rules applied: 0004:1 (Back/Forward always present, never hidden-then-jumping) + 0004:4/5 (honest disabled at native ends, mid-step/mid-switch wait, failure line on failed step), 0006 visibility-on-surface (toolbar owns traversal; picker owns switching; no second switch surface), 0008 n/a (no opt-in switch). 171 NOT claimed resolved (no reproduction exists).

## Reframe (2026-09-11) — why this ticket is incomplete

The shipped Back/Forward walks the session switch-trail (saved addresses Sprout itself showed). The saved-site picker dropdown (170) already switches sites, so for site switches the two surfaces duplicate each other — while the actual issue, **routing inside the website itself** (inbox → message → link), is covered by neither. The trail was an honest fallback (no native observation exists in the pinned JS surface), not the fix. This ticket is reframed around the native seam; the trail retires on the native path when it lands.

## Changes required (native-history rework)

- Rust (`src-tauri/src/`, ADR-0029 owner — extend `companion_audio.rs` or record a new companion-history owner, never a second invocation site; `webview2-com 0.38` is already a direct dependency): new Tauri commands driving the live `companion` child (`GoBack`/`GoForward`), a `CanGoBack`/`CanGoForward` state query, and `NavigationCompleted`/`HistoryChanged` forwarding as frontend events; command registrations in `lib.rs`. No popup/new-window interception, no routing-freeze fix, no relaxed isolation (all stay in 171).
- Frontend (`src/routes/quick-launch-window/+page.svelte`, `src/lib/api.ts`/`types.ts` seam only): rebind the toolbar buttons from the `$derived` trail state to the live native state on the native path; retire `trailApplyVisit` usage there (keep `trailApplyVisit` + the 12 companion-trail tests scoped to the browser-preview iframe fallback); dropdown untouched as the sole switch surface; reload-is-saved path untouched. Tokens/components only; `constants/window.rs` scanned before any dimension claim.
- Tests: Rust tests for the new commands/state query/event shape; extend `companionPane.test.ts` (native-state contract; trail tests re-scoped to preview fallback). `npm.cmd run check` 0/0; `node tools/ownership-gate.mjs` passes.
- Docs: dated extension to the 2026-09-11 ADR-0022 section (never rewrite); cite applied 0004/0006 rules again in the done notes (UI-heavy rule stays binding).
- Verification adds: in-page matrix (link chains within one site, Back/Forward across them, enable-state at native trail ends, Back-then-reload = saved address, switch-then-Back semantics) at real dock sizes/DPI, keyboard-only, light/dark, fixed/auto-hide, both edges — on top of the existing reload/switch/return/failure matrices.

## Validation (batch batch-174-175-177-178-179-20260911, 2026-09-11)

- AC1 left OPEN (partial): Back/Forward traverse the session's Sprout-shown saved addresses through the single serialized switch queue (one child — `new Webview(` occurs exactly once, pinned by contract tests; honest disabled states; preview keeps iframe fallback). In-pane link traversals inside the native child are NOT observed: the pinned JS Webview surface (`@tauri-apps/api@2.11.1`) exposes no traversal/observation hooks and no Rust nav commands or nav-event forwarding exist (verified by search; research 0012 pins the same). The ADR amendment scopes this boundary honestly. Follow-up needed for true native history: new Rust commands via webview2-com `GoBack`/`GoForward` + `CanGoBack`/`CanGoForward` observation (allow-list extension, separate ticket). 171's reporter issue is NOT claimed resolved.
- `cargo test` unaffected (frontend-only). `npm.cmd run check`: 0/0. `npm.cmd test -- --run`: 212 passed (incl. 12 new companion-trail tests). Ownership gate: pass. UI rules applied: 0004:2 (on-surface frequent selection), 0004:3 (L1 boundary kept), 0004:5 (honest disabled + failure feedback), 0006 visibility-on-surface; 0008 n/a (no opt-in switch). `web-design-guidelines`/`frontend-design` skills unavailable to the worker — rules applied directly + manual guidelines pass. Manual dock matrix (340px/DPI/keyboard/light-dark/fixed-autohide/both edges) pending.
- Merged with 178's Clips regions of the same page (disjoint; coordinator merge, both preserved).

## Implementation notes

- Likely owners to recheck at dispatch via CodeGraph: Companion creation/lifetime/toolbar, Tauri child-WebView path, `settings.rs` site identity, pinned Tauri/Wry/WebView2 support + research 0012 before selecting hooks. Do not claim 171's reporter issue resolved — no reproduction exists.
- Design system only; toolbar fits the 340px dock width without new dimensions (scan `constants/window.rs` before any dimension claim).

## Verification

- `npm.cmd run check` + ownership gate; manual at real dock sizes/DPI, keyboard-only, light/dark, fixed/auto-hide, both edges: Back/Forward matrix (including in-page link chains, native trail ends, Back-then-reload, switch-then-Back), reload/switch/return matrix, failure pane still offers Try again + Open externally.

## Amendment — 2026-09-12 (rework landed; YouTube Music constraint recorded)

- The native-history rework is implemented as specified above: `companion_history.rs` owner (GoBack/GoForward, CanGoBack/CanGoForward state, HistoryChanged/NavigationCompleted forwarding as `companion-history-changed`), three commands registered, toolbar rebound to live native state with the trail kept for the preview fallback only. Automated evidence on this tree: `cargo test` 597 passed (incl. 4 history tests), `npm.cmd run check` 0/0, vitest 248 passed, ownership gate pass. Temporary `[DEBUG-hist]` instrumentation used during diagnosis is removed (verified zero remnants).
- Reporter-tested in-page behavior (YouTube Music track switches): Back steps the native list only — a view change that creates no session-history entry has no back-step, identical to the browser's own Back button (Edge-differential). That is a site constraint, not a defect: single Back/Forward steps are the same WebView2 calls Edge makes, so per-site history quirks reproduce faithfully.
- Still owed a human: the full in-page dock matrix (link chains, native ends, Back-then-reload, switch-then-Back) at real dock sizes/DPI, keyboard-only, light/dark, fixed/auto-hide, both edges.
