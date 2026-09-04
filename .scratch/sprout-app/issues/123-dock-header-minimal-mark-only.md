# 123 — dock header minimal: wordmark → mark only + mark opens main app

**What to build:** The Quick Launch window/dock header stops wasting 60px on the wordmark at 340px, and the remaining mark becomes a second entry point to the main window (user isn't forced through tray → right-click → Open Sprout when it is closed).

**Blocked by:** none.

**Status:** done

## Scope

- File: `src/routes/quick-launch-window/+page.svelte:448` `header.qlw__bar`.
- When `dock.docked == true`: remove `<h1 class="qlw__title">Quick Launch</h1>` (keep `SproutMark size=16` `+page.svelte:452` as `data-tauri-drag-region={titleBarDragRegion(dock.docked)}` `src/lib/quickLaunchTitleBar.ts`). Edge arrows (`chevron-left`/`chevron-right`) + `IconButton undock` + `x` close stay — the decorative `qlw__dock-hint` (`dock-left`/`dock-right` 13px accent icon) is **removed** (see Validation — hint useless, 2026-09). Floating window (`dock.docked==false`) keeps the text for discoverability — one-header caveat per `0005-page-chrome-consistency.md:1`.
- `src-tauri/src/lib.rs`: expose `open_main_window` seam as a `#[tauri::command]` `open_main_window_cmd` that calls the robust `crate::open_main_window(app)` (focus/show/unminimize if already open, else recreate at `constants::window::MAIN_WINDOW_*` per ADR-0013 / `src-tauri/src/constants/window.rs` single-size source; handles minimized/hidden and "already exists" race, see Validation — white screen). Register in `invoke_handler`.
- `src/lib/api.ts`: add `openMainWindow(): Promise<void>` → `invoke("open_main_window_cmd")`.
- `+page.svelte` header: the mark becomes a button (not decorative `span`):
  - `<button class="qlw__mark" aria-label="Open Sprout" title="Open Sprout" data-tauri-drag-region="false" onclick={openMain}>` wrapping `<SproutMark size={16} />`. `data-tauri-drag-region="false"` excludes the hit-target from the header's `deep` drag when floating — the header itself remains draggable, the button stays clickable (Tauri drag script contract).
  - `openMain` is single-flight guarded (`openingMain` flag, 500 ms debounce) → `await openMainWindow()` with best-effort error console; no banner needed (main open failure is already logged in `open_sprout` path). Prevents double-invoke race that produced white screen on 2nd click.
  - Works docked *and* floating for consistency (floating still shows title text, mark click target is identical). `dock.docked` conditional only controls title visibility, not clickability.
  - Insert a flex spacer when docked (`<span class="qlw__spacer" aria-hidden="true">`) so removal of the `flex:1` title doesn't left-cluster the right-side controls — header stays `mark left, controls right` without wrap.
- Styling: reuse `src/lib/styles/tokens.css` tokens only — `var(--space-*)`, `var(--radius-sm)`, `var(--bg-hover)`, `var(--border)`, `var(--ring)`; no ad-hoc colors/radii per AGENTS.md design rule. Match `IconButton` quiet hover treatment on the mark button: transparent → `var(--bg-hover)` on hover, `border-color: var(--border)` on quiet-hover, `focus-visible` ring `2px solid var(--ring)` + offset. 30×30 hit-target (inline-flex center, ticket 123 follow-up fixed 28→30) so docked header at 340px stays ≤ ~205px total (hint removed saves 21px) and never triggers `Tabs.svelte:178` degradation. Docked header now symmetric `var(--space-2)` both sides (was mirrored 16/8) for visual centering — 4px off-center fixed.
- Cite `0006-notion-design-patterns.md:5` relocating familiar controls has cost — mark keeps brand scent vs icon-only tab. Mark→home validates as near-zero relearning (universal "logo goes home" vocabulary, NN/g Consistency heuristic; cf. 0006:5's warning — cost here is repaid by redundant non-tray entry point). Not a durable toggle nor per-use scope, so correctly *not* in page-features menu (0008 rule 1 classify-first) nor a dialog (0007).
- Cite `0004-rule 2` minimal-until-content (title hidden when space-constrained docked, not in floating) and `0008-rule 1` placement-by-persistence for above. Hint removal cites `0004:1` show navigation if you can but never waste chrome on redundant adornment, and `0006:6` one reserved accent — hint spent accent on non-action.

## Validation — mark-click to open main app (user request)

- **Decision: VALIDATED — ship.**
- **Why it wins:** Single chrome-neutral affordance (0 extra px at 340) for the user's stated pain ("main closed → tray → right-click → Open" is 2 precise clicks on a tiny target) when main's `close` destroys the webview and only the tray stays resident (`lib.rs:on_window_event`, `tray::open_sprout`). Reuses existing `lib.rs:795 open_main_window` seam (already shared by tray + single-instance hook) — no new state/persistence.
- **Discoverability/cost:** `aria-label`/`title="Open Sprout"`, keyboard `focus-visible` ring, hover `bg-hover` make it perceivable; undetectable users still have tray. Cost is a logo-home idiom users already learned elsewhere → lowest class of relearning per 0006:5.
- **Alternatives rejected:**
  - Dedicated `Open Sprout` IconButton in header: +30px at 340 (pushes Tabs into `short`/`icon` fit, violates `0004:1` show nav if you can and adds chrome for a redundant navigation).
  - Header double-click / right-click menu: undiscoverable, no keyboard/a11y, conflicts with Tauri double-click maximize.
  - Tray-only: preserves the reported friction; dock is the only always-visible surface when main is gone (ADR-0011), so housing a home link there is proximity-correct per 0006:4.
- **Risks mitigated:** floating drag vs click → `data-tauri-drag-region="false"` on the button (header stays `deep`); docked drag already `false`, no conflict. Robust `open_main_window` handles both `Some(window)->show/unminimize/focus` and `None->build` so recreating after destroy and focusing when already open need no fork; frontend single-flight prevents double-invoke race.

## Validation — dock side hint icon useless (2026-09 follow-up)

- **Decision: VALIDATED — remove `qlw__dock-hint`.**
- **Evidence:** Docked position is already conveyed by the window's physical location on screen (strongest signal) *and* by the chevron buttons' disabled state (`chevron-left` disabled on left, `chevron-right` disabled on right) with `title`/`aria-label` ("Dock to the left edge" etc., plus `SEAM_REASON` when ineligible). The hint is `aria-hidden`, no `title`, no interaction — user must hover the *arrows* anyway to read where they would go. It is the only header element that spends the reserved accent (`var(--accent)`, `0006:6`) on a non-action.
- **Cost of keeping:** 13px icon + 8px gap = 21px at 340px, where tab degradation (`Tabs.svelte:178`) is already tight (research `0004` constraint `DOCK_WIDTH===340` physical). violates `0004:1` (don't waste chrome on redundant adornment) and `0004:2` (frequency: hint is read rarely, but occupies chrome always).
- **Alternatives rejected:** Keep hint for at-a-glance edge confirmation — rejected because disabled arrow + screen position already give at-a-glance confirmation with no extra chrome; making hint interactive (click to switch) would duplicate the arrows. Tooltip on hint would still require hover, same as arrows.
- **Cite:** `0004:1`/`0004:2`, `0006:6` one reserved accent, `0005` one-header caveat not implicated.

## Validation — white screen on 2nd mark click (2026-09 bug report)

- **Decision: VALIDATED — fix as described.**
- **Repro:** 1) Close main window (X destroys webview, tray stays). 2) Click dock mark → main appears. 3) Without waiting, click mark again *while main is already open* (or double-click quickly) → main shows full white, doesn't load. Root cause: `open_main_window` only did `set_focus()` on existing window; on Windows a minimized/hidden or not-yet-visible window stays white until `show()`/`unminimize()` — and a double-invoke races `WebviewWindowBuilder` with label "main" ("already exists") leaving the new webview in a half-built white state. Frontend had no single-flight guard, so two concurrent `invoke("open_main_window_cmd")` could interleave.
- **Fix:** Backend `open_main_window` now does `show()` + `unminimize()` + `set_focus()` for existing window, treats `set_focus` failure as signal to destroy and recreate, and handles "already exists" build error by focusing existing instead of failing. Frontend `openMain` is single-flight (`openingMain` flag, 500 ms debounce) so double-click cannot issue two concurrent invokes.
- **Cite:** ADR-0013 single size source still respected; no new persistence; failure is logged not surfaced as dock error (same as `open_sprout`).

## Validation — blank main window at boot (2026-09 follow-up)

- **Root cause:** WebView2 defers navigation while its parent window is hidden, but showing a normal window before Svelte mounts exposes WebView2's blank startup surface. Handle validity and Vite readiness therefore could not distinguish this state.
- **Fix:** New main windows are created visible with a transparent native/webview background and omitted from the taskbar. The main Svelte layout sends `main_window_ready` from `onMount`; Rust then makes the native window opaque, restores its taskbar entry, shows/unminimizes it, and focuses it. Repeated open requests wait while that handshake is pending. The existing 800 ms zombie-window grace remains authoritative for close→reopen races.
- **Evidence:** Clean-app-data runtime reached a visible, non-layered `Sprout` window after the mounted handshake. Lifecycle decision tests cover loading, ready, and zombie states; `npm.cmd run check`, `npm.cmd run build`, and the focused Rust tests pass.

## Validation — event-thread hang on close→reopen via dock/tray/single-instance (2026-09 hang, hypothesis #1)

- **Decision: VALIDATED — fix as described.**
- **Repro (all 3 entry points):** 1) Close main (X → `on_window_event` queues `destroy()` on event thread, `main_close_time` set `lib.rs:2222`). 2) Immediately click dock mark (`open_main_window_cmd` `lib.rs:1865` → `request_open_main_window`) / tray `Open Sprout` (`tray.rs:98`) / second-instance launch (`lib.rs:2169`) within `≤800 ms` → every native window `IsHungAppWindow` true, never recovers. `tools/repro-main-window-lifecycle.ps1` drives this exact lifecycle (close → reopen → minimize+foreground → close → reopen → final open) and asserts `hung==false` + `main Count==1`.
- **Root cause (captured, not theorized):** `open_main_window` `lib.rs:983` held every retry sleep on the **event thread** — `800 ms` grace (`1001`) + `7×120 ms` (`1042,1066,1112`) inside the `for _ in 0..7` loop plus `IsWindow`/zombie checks. The same thread must process the queued `destroy()` for the `main` label to clear. Sleep blocked it, so the retry loop waited for work it prevented — classic self-deadlock; every `get_webview_window("main")` kept returning the zombie and `build` hit `already exists`.
- **What was attempted (did NOT fix hang):** robust `is_zombie_window` (`lib.rs:847` — `!IsWindow || elapsed<800`) + `existing_main_window_action` + `show/unminimize/set_focus` for existing window and `already exists→focus` handling (`Validation — white screen on 2nd mark click`). Correct for white-screen on rapid reopen *while open* but still called `open_main_window` directly on tray/IPC/single-instance event thread → sleeps still blocked `destroy()`.
- **Fix that works (single shared seam):** `lib.rs:1128` `request_open_main_window` — `AppState:82` `main_window_opening:AtomicBool` single-flight + `1137` `tauri::async_runtime::spawn_blocking(|| open_main_window(&app))`. All post-start requests (`tray.rs:98`, `lib.rs:1866`, `2169`) only enqueue (`Ok(())` if already opening); retry sleeps run on the blocking pool, event thread stays free to process `destroy()` and WebView2 COM teardown. Boot `lib.rs:2257` stays sync `open_main_window` — no `main_close_time` race, no delay.
- **Why this way:** single seam vs 3× duplication — deletion test passes: removing `request_open_main_window` forces tray/IPC/single-instance to reimplement `AtomicBool + spawn_blocking` independently (`lib.rs:76` seam doc, `codebase-design` deletion test). Lean `destroy-on-close` kept vs hidden-window (least memory per `lib.rs:996`); 800 ms grace covers slow COM teardown (500 ms insufficient, see `is_zombie_window:845`). Coalesce-on-busy (`swap(true)` → `Ok`) is correct — open is idempotent, second intent satisfied by first rebuild; if first fails flag cleared for retry.
- **Evidence:** `repro-main-window-lifecycle.ps1` `GREEN: main window opened and the Sprout event loop remained responsive` (no `IsHungAppWindow`), `lib.rs:white_screen_tests` 7 passed (`valid_hwnd_but_recent_close_is_zombie`, `close_reopen_race_needs_grace`), `cargo test` filtered green (previous full 292), `npm.cmd run check` 0 errors.
- **Cite:** ADR-0013 single size source still respected (`constants::window::MAIN_WINDOW_*`); AGENTS.md `constants/window.rs` single source.

## ACs

- [x] Docked header at 340px physical shows mark (16px) *as a button* + dock-edge arrows + undock + close (no `qlw__dock-hint`) with no flex wrap / no tab degradation triggered by header width alone (verify `Tabs.svelte:178` `display:flex` `hug-left`).
- [x] Header drag still moves the window when floating (`deep`), and is blocked when docked (`false`); single-click on the mark never drags — it is excluded from the drag region.
- [x] Clicking (or keyboard-activating) the mark focuses the main window when it exists (including minimized/hidden), and recreates + focuses it when it was destroyed (boot-to-tray or X close), both docked and floating — verified without using the tray menu. Rapid double-click (≤200 ms) does not produce white screen.
- [x] Mark button has `aria-label="Open Sprout"` and `title="Open Sprout"`, visible hover (`var(--bg-hover)`) + `focus-visible` ring (`var(--ring)`), 30px hit-target via tokens.
- [x] No decorative dock-hint icon is rendered in either docked edge; current edge is still perceivable via window position + disabled chevron state.
- [x] Floating palette still shows `Quick Launch` text.
- [x] `npm.cmd run check` 0 errors, `cargo check` 0 errors.
- [x] Regression 2026-09-04: close→reopen via any post-start entry point never blocks the event thread — all route through the off-thread single-flight seam (`tools/repro-123-event-thread.ps1` GREEN).

## Verification

- `npm.cmd run check` 0 errors, `cargo test` `white_screen_tests` 7 passed, `cargo check` 0 errors.
- `tools/repro-main-window-lifecycle.ps1` GREEN — no `IsHungAppWindow`, `main Count==1` after close→reopen sequence (tray/dock/single-instance all via `request_open_main_window`); keep `tools/repro-*` harness (paired with `lib.rs:66` stress harness precedent) unless explicitly retiring.
- Visual: dock on left/right, resize to 340px, screenshot before/after; tab labels stay `full` with 2 tabs. No `qlw__dock-hint` in DOM when docked.
- Interaction: with main window open (including minimized via taskbar), click dock mark → main shows/unminimizes/focuses. Destroy main (X), click dock mark (and again floating mark) → main recreates at `MAIN_WINDOW_*` and focuses, tray not used. Rapid double-click mark (2 clicks ≤200 ms) → main still loads, no white. Close→immediate reopen (≤800 ms) via any of dock/tray/second launch → no hang, `destroy` settles on freed event thread. Keyboard: Tab to mark → Enter/Space opens main. Hover shows `Open Sprout` tooltip + bg-hover. Drag header (floating) still moves window; clicking mark doesn't.

## Regression (2026-09-04 — invisible frame + dead X)

- **Report:** clicking the docked header's mark opened the main app as an invisible frame, and the main window then ignored X.
- **Root cause:** the validated off-thread seam (`request_open_main_window`: `main_window_opening` single-flight + `spawn_blocking`) was absent from the code — `open_main_window_cmd`, `tray::open_sprout` (tray menu + `open_sprout_cmd`, i.e. the dock-mark handler), and the single-instance hook all called the sleeping `open_main_window` synchronously on the event thread (800 ms close-grace + up to 7×120 ms zombie retries). That thread must also process the queued `destroy()` the retry loop waits on — a self-deadlock: the rebuilt window never revealed and `CloseRequested` never processed.
- **Fix:** restored the seam as one small-interface module (`lib.rs` `request_open_main_window`; `AppState.main_window_opening`; all three post-start callers enqueue through it; boot stays on the direct sync call — no close race there, ADR-0013 size source untouched). Frontend unchanged: its single-flight guard plus backend coalescing cover double-click, and `open_if_docked` no-ops from the dock.
- **Evidence:** `tools/repro-123-event-thread.ps1` (kept per the harness convention above) went RED pre-fix (no seam, all 3 entry points blocking) → GREEN post-fix; `cargo test` 427 passed / 0 failed / 3 ignored; `npm.cmd run check` 0 errors / 0 warnings; `cargo check` clean (pre-existing `walker.rs` warnings only).
- **Not yet run:** `tools/repro-main-window-lifecycle.ps1` end-to-end (needs all Sprout processes closed + Vite on :1420); run it when convenient and expect `GREEN … event loop remained responsive`.
