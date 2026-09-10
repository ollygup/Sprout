# Companion WebView feasibility

## Question

Can the Quick Launch dock host one chosen HTTPS site as a narrow responsive surface — without becoming a multi-tab browser or embedding an arbitrary desktop `exe` — and what limits, splitter shape, and profile isolation does that require?

## Sources

- WebView2 `ICoreWebView2Settings` UserAgent + `IsPasswordAutosaveEnabled` docs — `learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2settings`
- WebView2 frames `X-Frame-Options` semantics — `learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/frames` + YouTube API samples issue `#140` (`watch→embed` only matters when framing)
- Electron embedding discussion `electron/electron#10547` / `#26729` (`SetParent`-embedding of Chromium/CEF windows)
- Native embedding attempt `sweetwisdom/electron-native-windows` (`WS_POPUP→WS_CHILD` dance) + StackOverflow `170800` (`AttachThreadInput` hangs)
- Tauri v2 WebView / WebviewWindow docs — `tauri.app/reference/webview` + `tauri.app/develop/calling-rust/` (invoke isolation per partition)
- Tauri 2.11.5 `webview/plugin.rs` — JavaScript child-WebView creation returns `UnstableFeatureNotSupported` unless the Cargo `unstable` feature is enabled
- MDN `X-Frame-Options` + `Content-Security-Policy: frame-ancestors`
- Microsoft, *Responsive design techniques* — show/hide or re-architect content when the available window cannot keep it usable: https://learn.microsoft.com/en-us/windows/apps/design/layout/responsive-design
- Microsoft, *Guidelines for app settings* — durable behavior/preferences belong on the dedicated Settings page, Settings stays consistent across app contexts, and changes should reflect immediately: https://learn.microsoft.com/en-us/windows/apps/design/app-settings/guidelines-for-app-settings
- WebView2 `CoreWebView2Profile.PreferredColorScheme` — profile-level Light/Dark/Auto selection sets browser UI and the website-facing `prefers-color-scheme` media feature: https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2profile
- CSSWG, *Media Queries Level 5* `prefers-color-scheme` — this feature communicates a preference that website-authored styles may honor; it does not recolor authored page content: https://www.w3.org/TR/mediaqueries-5/#prefers-color-scheme
- Tauri 2.11.5 Webview/Window APIs and pinned runtime source — child `Webview` has no JavaScript theme setter, but `Window.setTheme` exists; the Windows runtime creates child WebViews with their parent window's theme and propagates later window theme changes to every child: https://v2.tauri.app/reference/javascript/api/namespacewebview/ and https://v2.tauri.app/reference/javascript/api/namespacewindow/#settheme and https://github.com/tauri-apps/tauri/blob/tauri-v2.11.5/crates/tauri-runtime-wry/src/lib.rs#L4293-L4304 and https://github.com/tauri-apps/tauri/blob/tauri-v2.11.5/crates/tauri-runtime-wry/src/lib.rs#L5062-L5068

## Decision

**Embed a single WebView2 in the dock — direct navigation, not an `<iframe>` — with an Android Chrome identity, an isolated data store (no `__TAURI__` exposure), and a draggable horizontal splitter clamped 25–60%.**

- **Host:** `quick_window.rs` `QUICK_LAUNCH_WINDOW` (the same dock window/chrome 121's foreground seam and 122's minimal header touch). One child `Webview` label `companion` uses the measured DOM content-frame rectangle, so its native surface starts below the shared Companion toolbar and follows splitter resizing. No second window, no omnibox inside the pane (single-site only).
- **Navigation:** WebView2 navigates *directly* to the `https://` URL. `X-Frame-Options` / `frame-ancestors` do not block top-level navigation because there is no framing (`learn.microsoft.com/webview2/concepts/frames`). This removes the iframe-specific YouTube refusal, but it does not guarantee every site supports embedded browsers; a site or authentication flow may still refuse WebView2.
- **External escape:** the Companion toolbar's **Open externally** action validates the active HTTPS URL in Rust and asks Windows to open it in the default browser via `ShellExecuteW`. The current implementation does not intercept a site's `_blank` / `window.open` requests or create Companion tabs.
- **History chrome:** the current Tauri child-WebView JavaScript surface does not expose native history state, so the shipped native pane does not claim Back/Forward capability. The browser-preview iframe keeps its limited host-side history behavior only for development preview.
- **Mobile layout:** WebView2 uses an Android Chrome UserAgent so its advertised browser engine matches WebView2's Chromium engine. An iPhone Safari identity is not interchangeable: it can cause a site to deliver a WebKit/Safari-specific client to Chromium. No viewport meta is injected — each site's own responsive rules still own the page layout. The measured native bounds remain exact, while WebView2 zoom gives widths below 320 logical pixels an effective layout width near 320 CSS pixels (never smaller than 70% zoom). At normal logical widths the page stays at 100%. This compensates for the fixed 340-physical-pixel dock becoming unusually narrow in CSS pixels at Windows display scaling without widening the dock or impersonating a different browser.
- **No presentation knob:** the adaptive scale is an internal narrow-surface correction, not another durable user preference. Research 0008 rule 1 classifies the existing Active site and Pane height choices as Settings concerns; it does not justify exposing implementation-level page zoom beside them. Research 0005's shared chrome also stays unchanged — the Companion toolbar and measured content frame remain the single presentation.
- **Isolation:** the child uses a persistent `companion` WebView2 data directory separate from the main `quick-launch` webview and main window; `__TAURI__` / `invoke` / `listen` are not enabled — not a `QuickAction` runner (`quick_actions.rs:52` / `engine/windows` untouched). Cookies/auth stay in the Companion profile, not in the app's other partitions. No log folder, no `quick_action_runs` events.
- **Tauri feature gate:** `@tauri-apps/api/webview` child-WebView creation requires Tauri's Cargo `unstable` feature in 2.11.5. The JavaScript constructor can exist and type-check without that feature, but every creation request fails at runtime with `UnstableFeatureNotSupported`; compilation and a visible fallback alone are not sufficient verification.
- **Splitter:** horizontal draggable divider (0006:7 Disclosure-like but horizontal) between the tabs list and the pane, clamped 25–60% — updates `companionHeightRatio` live and persists per monitor (falls back to `settings.companionHeightRatio` 0.40). Storage keys `quicklaunch.companion.height_ratio.<monitor-id>` mirror the dock edge/mode per-monitor memory (`db.rs` / `quick_window.rs` `memory_key` discipline, ticket 110 identity-first). `toggleQuickLaunchDock` undock/redock keeps the ratio; floating (`constants/window.rs:9` `WINDOW_HEIGHT 460`) never shows the pane so the ratio is inert there. `AUTOHIDE_SLIVER_PX` / reveal dwell (`window.rs:19`) unchanged — companion does not touch the auto-hide gate.
- **Visibility:** content- and mode-gated (`0004:2` / `0006:2,11` — `companionUrl==null` or `dockState==floating` → no visible WebView, splitter, Companion tab/button, or empty placeholder in Quick Launch). Floating Quick Launch is only 340×460 and cannot keep both its primary launcher and a useful arbitrary web app surface; Microsoft's responsive guidance explicitly allows showing/hiding or re-architecting content at constrained window sizes. The dedicated Settings/Companion manager remains the discoverability and authoring home, consistent with `0004:2–3`, `0008:1`, and Microsoft's Settings guidance. It says that Companion appears when Quick Launch is docked rather than hiding configuration based on the current window mode. The gate is **floating versus docked**, never fixed versus auto-hide: both dock visibility modes host the same live Companion, and an auto-hide transition hides/repositions its native surface without destroying the WebView so media/session state can continue in the background.
- **Mode and setting immediacy:** changing the active site while the dock exists navigates/reloads that same live Companion immediately; changing it while floating becomes the URL used on the next dock. Saving floating/docked or fixed/auto-hide applies to the open Quick Launch immediately. These are durable Settings preferences, but Microsoft's Settings guidance and research `0008:2` still require visible state changes to take effect without reopening the window.
- **Theme contract:** Sprout maps its current Light/Dark choice to the Quick Launch parent window theme; changing only Svelte's `data-theme`/CSS tokens is insufficient, so the theme path must also call Tauri `Window.setTheme`. Tauri 2.11.5 then initializes the child WebView2 with that native theme and forwards later parent-window theme changes to all child WebViews. WebView2 applies the value at profile level and exposes it to any directly navigated HTTPS origin as `prefers-color-scheme`, so cross-origin navigation is not a blocker. This is preference negotiation, not CSS ownership: Sprout chrome follows its exact tokens, while the remote page follows only if its authors support `prefers-color-scheme` (and may instead honor its own account/local theme or fixed branding). Sprout must not promise matching accent colors or inject blanket CSS into arbitrary sites. The implementable/testable wording is: **“Companion forwards Sprout's light/dark setting as the WebView2 preferred color scheme; sites that support the browser color-scheme preference follow it.”**

## Rejected

- **Desktop `SetParent` HWND embedding of arbitrary `exe` (e.g. Spotify desktop):** brittle for CEF/Chromium apps — the community's `WS_POPUP→WS_CHILD` + `AttachThreadInput` dance (`electron-native-windows`) hangs and breaks on focus/DPi (`stackoverflow 170800`); Electron issues `#10547` / `#26729` document CEF's refusal to be reparented. Web apps via WebView2 are the stable seam.
- **Multi-tab companion or omnibox inside the pane:** single-site only per ticket scope — no tab strip, no address bar inside the pane; the saved URL list lives in the main app and is deduplicated by trimmed, case-insensitive host + path.
- **Iframe framing of arbitrary sites:** blocked by `X-Frame-Options: SAMEORIGIN/DENY` on YouTube/Spotify/etc. — direct WebView navigation avoids framing entirely; no embed URL rewrite needed.
- **JS-exposed bridge in the companion partition:** no `__TAURI__.invoke` / `listen` in the companion partition — prevents a compromised site from invoking Rust. Active site and pane height live in Settings; the Companion manager owns saved-site CRUD and ordering only.

## Consequences

- New settings keys `settings.companion_url` (`string|null`), `settings.companion_height_ratio` (`0.25–0.60` default `0.40`), `settings.companion_url_list` (`string[]` JSON) in `settings.rs` / `db.rs` metas; per-monitor ratio memory in `quick_window.rs` / `db.rs` mirroring existing `quicklaunch.dock.*` discipline; `db::validate_product` unaffected.
- Tauri commands cover the active URL, height ratio, saved list, per-display ratio, and OS-default-browser escape. All are machine-local and never enter Preset exports.
- Frontend dock: `src/routes/quick-launch-window/+page.svelte` creates the native child WebView only when docked + active URL. An iframe exists only for non-Tauri browser preview; it is not the Windows runtime fallback. Splitter drag persists per monitor and auto-hide is unchanged. `companionZoomForWidth` scales only cramped logical widths; it does not alter the native frame bounds, Android Chrome identity, or site-owned responsive rules.
- Build configuration: `src-tauri/Cargo.toml` enables Tauri's `unstable` feature because the child-WebView JavaScript API is feature-gated; a source-contract regression test keeps the feature and isolated `dataDirectory` paired with the implementation.
- Main app: Settings uses a shared `Disclosure` for the optional capability and owns Active site + Pane height. `src/routes/companion/+page.svelte` reuses `PageHeader`, `Dialog`, `Button`, and `ConfirmDialog` for saved-site add/edit/remove/reorder only; it returns directly to Settings and does not duplicate the two live controls.
- Verification on 2026-09-03: `npm.cmd test -- --run` passed 71/71, `npm.cmd run check` passed with 0 errors / 0 warnings, and `cargo check` passed for v0.8.0 with seven pre-existing warnings. A manual YouTube smoke test confirmed the native surface renders in the fixed dock and the adaptive scale avoids its most compressed responsive layout. YouTube is smoke evidence only: the contract remains an attempted direct navigation for any valid HTTPS URL, with an honest external-browser escape because individual sites and authentication flows may still refuse embedded browsers.

## Implementation evidence

- The dock frontend creates one child `Webview` labeled `companion`, using the measured content-frame DOM rectangle in logical pixels so the native surface begins below the shared toolbar instead of covering it.
- The child uses the `companion` data directory for persistent isolated cookies and an Android Chrome identity compatible with WebView2's Chromium renderer.
- The child keeps the content frame's exact measured bounds and applies a width-derived 70–100% zoom after creation and on later bounds updates; this changes the page's effective responsive viewport without clipping or widening Sprout's dock.
- Creation failures stop after one attempt and expose stable retry / external-browser actions; a reactive effect must not automatically recreate the same failed URL or the failure UI flickers.

## Evidence update — 2026-09-08: routing and apparently inert buttons

**Research only; no root cause or new navigation policy accepted.** Frequent
URL changes alone do not establish incompatibility. [Microsoft navigation events](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/navigation-events)
distinguishes document loads, redirects and same-document changes. A fragment
can update SourceChanged without NavigationStarting; redirects can produce
multiple starts with one navigation ID. Document-start observation alone is
therefore insufficient to represent every location change.

Investigate distinct categories:

- Ordinary routes: verify navigation and child lifetime; distinguish the live
  location from the saved launch URL. A stale toolbar does not prove failure.
- Popups: [NewWindowRequested](https://learn.microsoft.com/en-us/dotnet/api/microsoft.web.webview2.core.corewebview2.newwindowrequested)
  lets a host supply a WebView or mark a request handled; handling without a
  destination suppresses opening. `_blank` and `window.open` require a separate
  policy. External opening may break opener-dependent workflows (inference).
- Load errors: inspect IsSuccess and WebErrorStatus in [NavigationCompleted](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2navigationcompletedeventargs).
  Distinguish cancellation/superseding navigation from actual load failure.
- Authentication/site behavior: the [WebView2 user-data folder](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/user-data-folder)
  owns cookies and browser data. Sprout's isolated profile therefore cannot
  promise session continuity on an external-browser escape. Site scripts,
  browser identity and authentication restrictions need concrete reproduction.

**Proposed investigation:** obtain site, exact button, before/after URL and
whether a normal browser opens a new tab. Cover same-origin routes,
fragment/history changes, redirects, user popups, login return, load failure
and external escape. Do not add tabs, relax isolation or reload on every URL
change speculatively. Reconcile any selected policy with ADR-0022 and pending
per-site browser identity work in spec 156 before implementation.

## Decision update — 2026-09-08

ADR-0022's final amendment and spec 166 now accept choosing an existing saved site directly from the dock through a name-plus-chevron selector. This supersedes Settings-only active-site selection, while main-app authoring, dock-only visibility and profile isolation remain required. Navigation failure policy and switching lifecycle remain open; no routing repair is accepted by this selection decision.

## Decision update — 2026-09-09

The saved-site lifecycle question is settled by ADR-0022's final amendment and ticket 170: select the saved address and replace the single live page while keeping the existing persistent profile and site preferences. No background tab or last-route restoration is promised. Ticket 171 carries the unresolved navigation report; no speculative routing repair or new-window policy follows from approval of the picker.

## Evidence update — 2026-09-09: controlled navigation harness (ticket 171)

### Scope and limitation

The report still lacks the exact site, button, starting URL, expected target,
browser identity, and whether the same action opens a normal-browser tab. The
reporter symptom has therefore **not** been reproduced or resolved. The
unattended local harness in `tools/repro-companion-navigation.mjs` instead
locks down eight controlled stimuli so a later native run can identify which
mechanism matches the report. Its HTTP contract check is not a native WebView2
observation.

Baseline `batch-151-170-171-20260909-01` pins Tauri 2.11.5,
`tauri-runtime-wry` 2.11.4, Wry 0.55.1, and `webview2-com` 0.38.2 in
`src-tauri/Cargo.lock`. The Companion is still constructed from JavaScript in
`src/routes/quick-launch-window/+page.svelte` with only `tauri://created` and
`tauri://error` listeners. Those events report child creation, not subsequent
document navigation. The toolbar's external action still receives
`companionUrl`, the saved address, rather than a live page address.

### Pinned platform evidence

- [Microsoft's navigation sequence](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/navigation-events)
  says a network navigation raises `NavigationStarting` through
  `NavigationCompleted`, redirects raise repeated starts under one navigation
  ID, and same-document navigation does not raise `NavigationStarting` or
  change that ID. `SourceChanged` can report a fragment change without a
  network request.
- [Tauri 2.11.5's Rust builder](https://docs.rs/tauri/2.11.5/x86_64-pc-windows-msvc/tauri/webview/struct.WebviewBuilder.html)
  has `on_navigation`, `on_new_window`, and `on_page_load`. The JavaScript
  [`Webview` surface](https://v2.tauri.app/reference/javascript/api/namespacewebview/)
  used by Companion exposes creation/manipulation and Tauri event methods, but
  no WebView2 `SourceChanged`, `NewWindowRequested`, or
  `NavigationCompleted` callback. Moving observation to the Rust creation seam
  would be an implementation decision, not a JavaScript listener addition.
- The pinned [Wry 0.55.1 Windows backend](https://github.com/tauri-apps/wry/blob/wry-v0.55.1/src/webview2/mod.rs#L1679-L1753)
  always registers WebView2 `NewWindowRequested`; when the builder supplies no
  new-window handler it calls `SetHandled(true)` and supplies no destination.
  Companion supplies no handler. This is a source-established mechanism for a
  `_blank` or `window.open` request to appear inert. It is the leading
  hypothesis, but it is not proof that the reporter's unknown button requests
  a new window. A current [untriaged Tauri report](https://github.com/tauri-apps/tauri/issues/15872)
  on Wry 0.55.1 additionally claims `target=_blank` may fail to reach even an
  installed handler on Windows; that report is corroborating risk, not an
  established Sprout fact.
- [Microsoft's `NewWindowRequested` contract](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/winrt/microsoft_web_webview2_core/corewebview2newwindowrequestedeventargs?view=webview2-winrt-1.0.3856.49)
  says `Handled=true` without `NewWindow` returns a dummy, immediately closed
  `WindowProxy`; `Handled=false` without `NewWindow` opens an uncontrolled
  popup. A policy that merely sends the URL to another browser cannot preserve
  a site script's usable opener `WindowProxy` (inference).
- [`NavigationCompleted`](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2navigationcompletedeventargs?view=webview2-1.0.3967.48)
  exposes `IsSuccess` and `WebErrorStatus`. The existing child-creation error
  listener cannot distinguish a cancelled/superseded navigation, external
  handoff, and a transport failure.
- [Microsoft's external-scheme event](https://learn.microsoft.com/en-us/dotnet/api/microsoft.web.webview2.core.corewebview2.launchingexternalurischeme?view=webview2-dotnet-1.0.3856.49)
  occurs between `NavigationStarting` and a `NavigationCompleted` whose result
  is `ConnectionAborted`; the default dialog can vary with browser/user
  settings and origin trust. A generic failed completion must therefore not be
  presented as a failed web load without classifying the scheme.
- WebView2 stores cookies, permissions, cache, and DOM storage in its
  [user-data folder](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/user-data-folder).
  Sprout deliberately uses a separate persistent Companion data directory.
  The default browser does not share that directory, so external recovery
  cannot promise the Companion login session (inference from the separate
  stores).

### Controlled matrix

| Case | Deterministic fixture signal | Native signal needed | Status |
| --- | --- | --- | --- |
| Fragment | `#fragment-target`; no request | `SourceChanged`, no `NavigationStarting` | Fixture verified; native pending |
| `history.pushState` | live readout becomes `/history-state`; no request | source/history change, no new document ID | Fixture verified; native pending |
| Full navigation | `/full` request and `full-document` marker | normal start/completion with success | Fixture verified; native pending |
| Redirect | 302 `/start`, 307 `/middle`, final marker | repeated starts, same navigation ID, successful completion | Fixture verified; native pending |
| `_blank` | request appears only if a target actually loads | `NewWindowRequested`, disposition, visible destination | Fixture verified; native pending; silent deny predicted |
| `window.open` | separately named popup target | same as above plus returned opener behavior | Fixture verified; native pending; silent deny predicted |
| Auth return | matching cookie/state succeeds; a no-cookie return reports profile mismatch | redirect IDs, cookie continuity, identity setting | Fixture verified; native pending |
| Custom protocol | `sprout-fixture://` user-gesture link | external-scheme event/dialog/OS result and `ConnectionAborted` completion | Fixture verified; native pending |
| Load failure | server destroys the connection | failed completion and exact `WebErrorStatus`; stable Sprout feedback | Fixture verified; native pending |

The verifier was run as `node tools/repro-companion-navigation.mjs` on Node
24.19.0 and returned `RESULT 8/8 controlled contracts passed`. It is
red-capable for a changed/broken fixture contract, not for the unsupplied
reporter action.

### Recovery analysis and hypotheses

Opening the saved address is predictable and avoids treating a transient
redirect as a durable site selection, but it loses the current route, form,
and auth-return parameters. Opening the live address would preserve more route
context, yet may expose one-time codes or other sensitive query data to a
different browser profile, still lacks Companion cookies, and cannot preserve
an opener relationship. Automatic external opening of every popup is therefore
not a neutral repair. If product policy later permits it, it needs an explicit
user-initiated HTTPS rule, visible destination/feedback, and a decision about
opener-dependent flows; custom protocols need a separate confirmation policy.

Ranked, falsifiable hypotheses for the reporter case:

1. The button requests `_blank`/`window.open`; the controlled popup target will
   not hit the server under the current handler-free child, while ordinary
   navigation will.
2. Navigation succeeds but Sprout's saved-address toolbar remains unchanged;
   the page marker and server request will change while Sprout chrome does not.
3. An auth flow rejects embedded identity or requires state from another
   profile; changing Mobile/Desktop identity or completing the return in the
   same isolated profile will change the outcome.
4. The button launches a custom protocol; WebView2 will show an
   external-scheme sequence rather than an ordinary successful document load.
5. The target really fails to load or page script throws; native completion
   status or DevTools console/network evidence will go red while child creation
   remains successful.

### Decision boundary

No repair ticket or ADR amendment is opened yet. The exact reporter action and
one native matrix run against a trusted HTTPS copy of this fixture are the
specific prerequisites. If popup suppression is then confirmed, the bounded
decision is a new-window disposition only (deny with feedback, navigate the
single Companion, or explicitly open user-initiated HTTPS externally), with an
ADR-0022 amendment before implementation. It must not add tabs, an arbitrary
bridge, relaxed profile isolation, or blanket redirect/navigation interception.
