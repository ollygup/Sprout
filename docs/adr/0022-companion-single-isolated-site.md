# Companion is one isolated site in the docked window only

> Latest status: amended 2026-09-09 for saved-site selection and switching lifecycle; accepted design, implementation pending in 170 under spec 166. See the final amendments; earlier text is preserved.

> Status: amended 2026-09-07 — original decision text preserved; see the executable-source audit amendment for current behavior and the 156-round amendment for readability zoom and per-site identity.

The Companion is a single user-chosen `https://` site shown in the lower portion of the docked Quick Launch window — never floating, never without an active saved site, and unrelated to Quick Actions, Presets, Plans, and Runs. It navigates a direct WebView2 child (no iframe, Android user agent, isolated `companion` profile with no `__TAURI__` bridge), sized by a splitter (25–60% of the dock height, default 40%, remembered per monitor). The saved-site list deduplicates by host+path (case-insensitive) rather than raw string. Audio is mute-only: the persisted mute flag is the source of truth and is healed onto the live WebView on every read and every creation, so silence survives restarts and WebView recreations; a fresh install starts unmuted. Leaving the pane always goes through the OS shell (`ShellExecuteW`) — the pane never hosts navigation chrome.

## Considered options

- **iframe inside the main webview.** Rejected: framing breaks sites that deny it, and it would share the app's origin context. A separate WebView2 with its own profile isolates the site and keeps app navigation working when the site refuses framing.
- **Multi-tab / omnibox / zoom / JS bridge.** Rejected scope: the Companion is a glanceable single-site surface, not a browser. Each of those turns it into a browser with its own update, history, and security story.
- **Floating companion.** Rejected: the pane's whole point is the docked morning bar with a site under it; floating keeps the palette small and predictable.

## Consequences

- `http://` and non-web schemes are refused at validation; only `https://` persists.
- Research `0012-companion-webview-feasibility` carries the feasibility evidence; this ADR carries the scope boundary so the next request ("just add tabs") has an answer.

## Amendment — 2026-09-05 (executable-source audit)

Saved-site deduplication uses the whole trimmed URL case-insensitively, ignoring trailing slashes (`src-tauri/src/settings.rs`, `dedup_companion_url_list`). It does not isolate host and path: different query strings or fragments remain distinct. The host-plus-path description is not the current identity algorithm.

Persisted mute remains authoritative, but applying it is asynchronous and best effort on creation and reads (`src-tauri/src/companion_audio.rs` and the child-created handling in `src/routes/quick-launch-window/+page.svelte`). This does not guarantee the absence of a brief initial sound before mute is applied. The Windows runtime uses the child WebView with its companion profile; browser preview uses an iframe. Docked-only visibility and teardown on leaving the docked form remain implemented.

The explicit Open externally action uses `external::open` through `src-tauri/src/lib.rs`. Child creation does not route every link navigation through that action, so “leaving the pane always goes through the OS shell” must not be read as a blanket navigation interception guarantee. These observations correct the description; they do not expand Companion’s product scope.

## Amendment — 2026-09-07 (156-round readability zoom and per-site identity)

Two refinements inside the glanceable single-site scope — omnibox, tabs, history chrome, floating Companion, and any JS bridge stay rejected. First, zoom: the rejected "zoom" meant browser-style zoom-as-navigation-chrome; readability zoom stays accepted as narrow-dock layout compensation (the existing width-derived auto zoom) extended to an explicit user control (50–200%, remembered per site), independent of the height splitter. Second, identity: the "Android user agent" line becomes the default rather than the whole policy — each saved site may override to a Desktop identity for desktop-only sites (Teams for Web is desktop-only and blocks mobile identities), with the `Windows NT 10.0` token covering current Windows releases and a current Chromium token refreshed at build time. Isolation (own profile, no bridge), one active site, and docked-only visibility are unchanged. Accepted in spec 156 (tickets 162, 165); implementation pending.

## Amendment — 2026-09-08 (saved-site selection, spec 166)

Accepted in the design interview; implementation pending. With multiple configured sites, the dock's Companion site label becomes a name (address fallback) plus chevron that reveals the saved sites and marks the active choice. Selecting an existing site is quick access; creating, editing and ordering sites remain in the main app. This refines the Settings-only activation placement without adding tabs, an omnibox, history chrome, a bridge, or floating Companion. Open externally remains a separate action. Selection lifetime and navigation failure policy are still under discussion in spec 166; this amendment does not accept a routing fix.

Research 0004 rules 2–3 supports on-surface frequent selection with authoring elsewhere. The earlier 0012 statement that the manager/Settings alone owns active-site selection is superseded to this extent. See ../../.scratch/sprout-app/issues/166-field-cleanup-dock-filter-companion-navigation-spec.md.

## Amendment — 2026-09-09 (accepted switching lifecycle, tickets 166/170)

The second design round is accepted. Choosing a different saved site replaces the live Companion page at that site's saved address, with its saved identity and zoom and the existing persistent Companion cookie profile. Returning to a site does not restore its prior route, unsaved form or a background tab; choosing the already-active site is not a reload. This settles the lifecycle question left open in the earlier amendment. The native routing report remains an evidence-gathering task in 171; no navigation interception or new-window policy is accepted. Implementation is pending in 170.
