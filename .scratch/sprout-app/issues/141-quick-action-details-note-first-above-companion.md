# 141 — Action details: note-first, command as scent, above the Companion

**What to build:** The Quick Action details popup becomes a readable note-first surface: full note on top, command collapsed to a short scent, internally scrollable, and always above the Companion pane.

**Blocked by:** 140 (yields the stabilized Companion pane this dialog must sit above).

**Status:** ready-for-agent

## Scope

- Quick Actions only (Launch entries run, Clips copy — neither opens this dialog); docked and floating; read-only in the window (full configuration stays in the main app).
- Note renders fully above the command; command shows at most ~3 lines plus ellipsis with Show-command and Copy affordances (note present → collapsed behind the affordance; note absent → truncated scent with a main-app hint). Full text always available in the main app.
- Dialog keeps its internal scroll (contents never clip past Close/Run); while any dialog is open the native Companion child yields (hidden/moved) and is restored after — CSS layering alone cannot cross a native child window.
- Tokens and shared `Dialog`/details components only; Run/Stop/Stopping control untouched.

## ACs

- [x] Long-command action opens a compact popup: full note visible, command ≤3 lines + `…`, Show-command reveals the rest, Copy works.
- [x] Popup scrollbar, Close and Run/Stop are all reachable with Companion enabled in docked mode; nothing paints behind the pane.
- [x] Floating and both dock modes pass; keyboard-only open → read → copy → close; reduced-motion clean.
- [x] `npm.cmd run check` 0 errors.

## Implementation notes

- Sizing context: today's full-width dialog in the narrow dock leaves a ~160px mono column that wraps into a wall — truncation plus the native yield fixes both halves together.
- Unreadable-wall vs hidden is settled policy (0004:2–3, 0006:13–14): rare, unbounded, level-2 content collapses to scent in level-1.

## Verification

- `npm.cmd run check`, dialog/frontend tests; manual: long-command + note action and note-less action, docked-with-Companion + floating, Fixed + auto-hide.

## Follow-up 1 — 2026-09-06 — yield was denied at runtime

- Symptom survived restart: the note still painted behind the Companion pane.
- Cause: the yield calls `Webview.hide()/show()` (`plugin:webview|webview_hide/_show`), which Tauri denies unless declared — `quick-launch.json` granted create/close/position/size/zoom but not hide/show, and the denial only reached `console.error`, so the pane silently stayed. Proven against the framework source (`tauri-2.11.5/src/webview/plugin.rs`) plus a failing-then-passing capability test.
- Fix: declared `core:webview:allow-webview-hide` + `allow-webview-show`; re-assert hide in `tauri://created` (construction-time hide can lose to backend creation); refusals now also reach the window error line. Backend rebuild required — capabilities compile in.

## Follow-up 2 — 2026-09-06 — restore failed on a stale handle

- Symptom after the permission fix: closing the popup bannered "Couldn't restore the companion pane — webview not found".
- Cause: the backend resolves hide/show by label and answers `WebviewNotFound` when the manager holds no live "companion" child; the cached frontend handle had outlived its child (a close racing its null-out), and every call through it failed while bounds syncs kept throwing on the dead label instead of recreating. Proven against the framework source (`tauri-2.11.5` `error.rs` + `webview/plugin.rs` `get_webview`).
- Fix: `liveCompanionChild()` resolves the live child by label before hide/show (adopting it, URL tag included, so no spurious recreate); a `companionWebviewBorn` flag (set on created, cleared on every null-out) tells a dead child (drop + silent recreate) from one still registering (leave alone); bounds-sync failures on a born-stale handle null it and re-queue instead of throwing forever. Genuine refusals still reach the error line; stale-handle recovery stays silent.

## Follow-up 3 — 2026-09-06 — toggle/hint gated on measured overflow

- Wart: `Show command` (and the note-less main-app hint) rendered even when the command already showed fully — the toggle flipped to `Hide` with no visible change.
- Fix: `commandOverflows` measured post-paint only (`scrollHeight > clientHeight` while clamped, re-run on action/open/expand/resize/fonts, never in render); toggle renders iff `showFullCommand || commandOverflows`, hint iff note-less + overflowing + collapsed. Copy stays always visible per the hover-reveal rejection in research 0004.

## Follow-up 4 — 2026-09-06 — yielded pane no longer reads "Loading"

- Symptom: hiding the native child exposed the `Loading {url}…` placeholder beneath it, reading as a stuck load behind the dialog.
- Fix: the wrapper keeps reserving the native bounds (no layout shift, no resync churn); only the text gates on the dialog being closed.

## Follow-up 5 — 2026-09-06 — unborn yield stayed noisy + set_theme was never declared

- Console residue after the stale-handle fix: opening/closing the dialog while the child was still registering logged `webview not found` from the yield hide/show (and its label-heal); every load also logged `set_theme not allowed` 10+ times. Behavior correct in both cases; the noise was the bug.
- Cause (yield): hide()/show() on a handle between `new Webview()` and its `tauri://created` event throw WebviewNotFound until registration lands (slow on cold WebView2 profile init) — the born flag healed state correctly but each queued call still logged. Cause (theme): `syncNativeWindowTheme` calls `Window.setTheme`, which neither the quick-launch nor the main capability declared — same denial class as the earlier hide/show denial, predating it; the theme module runs in both windows (the layout starts it, the dock restores it).
- Fix: the yield effect returns early while unborn (the created callback already re-asserts the hide when the dialog sits above, so the skip loses nothing); declared `core:window:allow-set-theme` in both capabilities and pinned both in the permission test. Backend rebuild required — capabilities compile in.
