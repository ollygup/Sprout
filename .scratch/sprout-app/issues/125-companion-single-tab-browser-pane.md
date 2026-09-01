# 125 — companion single-tab browser pane (any https URL, mobile UA, isolated, dock-only)

**What to build:** The dock hosts a single WebView2 in its bottom ~40% that can load any `https://` web app (YouTube, Music, Spotify Web, etc.) as a mini mobile UI — no multi-tab, fully isolated except for the splitter and URL.

**Blocked by:** 121, 122 — reuses the same dock window/chrome (`quick_window.rs:88` `QUICK_LAUNCH_WINDOW`) that 121's foreground seam and 122's minimal header touch; also reuses the picker/settings pattern from 122's candidate merge.

**Status:** ready-for-agent

## Scope

- **Backend + settings:** new keys in `src-tauri/src/settings.rs` / `db.rs` `settings` JSON: `companionUrl: string|null` (active URL, `null`=off) and `companionHeightRatio: number` (0.25–0.60, default 0.40) + `companionUrlList: string[]` (saved list edited in main app). Per-monitor `dock per-monitor` memory in `quick_window.rs` already exists for `left_eligible`/`right_eligible` (`types.ts:486` `QuickLaunchDockState`) — extend to `companionHeightRatio` per monitor (falls back to settings). Research note `docs/research/0012-companion-webview-feasibility.md` + glossary `docs/CONTEXT.md` **Companion** update land in this ticket.
- **Frontend dock:** `src/routes/quick-launch-window/+page.svelte:621` `qlw__tabs` + `quick_window.rs` `WINDOW` hosting: create one `Webview` label `companion` at `x:0 y:0.6*h width:340 height:0.4*h` derived from the dock monitor rect (display arrangement `docs/CONTEXT.md:89` virtual-screen). Mobile `UserAgent` (`iPhone`/`Android`) so 340px gets mobile layouts. Direct navigation to the URL (not iframe) so `X-Frame-Options` never blocks (`learn.microsoft.com/webview2/concepts/frames` + `youtube/api-samples#140` — `watch→embed` only matters for framing). In-page links navigate in-place; `_blank`/`NewWindowRequested` → `shell.open` external. Back/Forward `IconButton`s only when `canGoBack`/`canGoForward` (`0004:2` show if you can). Separate WebView2 profile/partition, no `__TAURI__`/`invoke`/`listen` exposure — not a `QuickAction` runner (`quick_actions.rs:52` / `engine/windows` untouched).
- **Main-app surface:** Settings page section + companion manager page/dialog reusing `PageHeader.svelte`/`PageFeaturesButton.svelte`/`Dialog.svelte` — list saved URLs (add/edit/remove, reorder via `ordered_list.rs` position discipline if needed, dedup trimmed case-insensitive on host+path), pick active; machine-local only, never in Preset exports/backups beyond the `settings` row (ADR-0009 spirit).
- **Visibility:** content-gated `0004:2` / `0006:11` — `companionUrl==null` → no Webview, no splitter, no chrome; floating (`constants/window.rs:9` `WINDOW_HEIGHT 460`) never shows the pane.
- **Splitter:** horizontal draggable divider `0006:7` Disclosure-like but horizontal, clamped 25–60%, updates `companionHeightRatio` live and persists per-monitor; `AUTOHIDE_SLIVER_PX` / reveal dwell (`window.rs:19`) unchanged.

## ACs

- [ ] No companion URL set → dock renders exactly as today (no pane, no splitter, no header button) — empty feature must not occupy chrome.
- [ ] Set active to `https://music.youtube.com` → dock bottom 40% shows that site in mobile layout, scroll/controls work, header tabs above unchanged; `Back` appears after navigating in-pane.
- [ ] Set to `https://open.spotify.com` → loads and plays (auth in isolated profile, cookies kept); not an `Action` run (no log folder, no `quick_action_runs` events).
- [ ] Splitter drag from 40% → 55% persists after `toggleQuickLaunchDock` undock/redock and across restart per monitor; floating window ignores it.
- [ ] research `0012` + CONTEXT Companion entry written; `npm.cmd run check` + `cargo test` (settings round-trip for `companionUrl`/`companionHeightRatio`) green.

## Out of scope

- Multi-tab companion or omnibox inside the pane (single-tab only).
- Desktop `SetParent` HWND embedding of arbitrary `exe` (rejected in 0012: `electron/electron#10547`/`#26729`, `sweetwisdom/electron-native-windows` `WS_POPUP→WS_CHILD` dance, `stackoverflow 170800` `AttachThreadInput` hangs — brittle for CEF/Chromium like Spotify desktop).
- Command blob spillover (`quick_actions.command TEXT` `db.rs:121` — Q13 left as-is).

## Verification

- `cargo test` (settings migration companion keys, splitter clamp)
- `npm.cmd run check` 0/0
- Manual docked laptop 1080p + 4K screenshot: pane 40% mobile layout for both; splitter drag persists.
