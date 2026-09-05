# Companion is one isolated site in the docked window only

> Status: amended 2026-09-05 — original decision text preserved; see the executable-source audit amendment for current behavior and implementation gaps.

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
