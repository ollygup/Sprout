# 125 — companion single-site browser pane (chosen HTTPS site, isolated, dock-only)

**What to build:** The dock hosts one isolated WebView2 in its lower portion and directly navigates to the chosen HTTPS site as a narrow responsive surface. There is no multi-tab browser, and sites that refuse embedded browsers fall back to an explicit external-browser action.

**Blocked by:** 121, 122 — reuses the same dock window/chrome (`quick_window.rs:88` `QUICK_LAUNCH_WINDOW`) that 121's foreground seam and 122's minimal header touch; also reuses the picker/settings pattern from 122's candidate merge.

**Status:** implementation follow-up in verification

## Scope

- **Backend + settings:** `src-tauri/src/settings.rs` / `db.rs` persist `companionUrl: string|null` (`null`=off), `companionHeightRatio: number` (0.25–0.60, default 0.40), and `companionUrlList: string[]`. Per-monitor height memory follows the existing identity-first dock-memory seam and falls back to the global ratio. Research note `docs/research/0012-companion-webview-feasibility.md` and the glossary's **Companion** entry record the durable result.
- **Frontend dock:** `src/routes/quick-launch-window/+page.svelte` creates one child `Webview` label `companion` at the measured content-frame bounds. It directly navigates to validated HTTPS URLs with an Android Chrome identity and a persistent isolated `companion` data directory. Tauri's `unstable` Cargo feature is required for this child-WebView API. At cramped logical widths, a 70–100% width-derived zoom keeps the site's effective responsive layout near 320 CSS pixels without changing the dock or native bounds. The toolbar can open the active URL through the Windows default browser. Sites may still refuse embedded browsers; one failed URL remains in a stable error state until Retry or a URL change.
- **Main-app surface:** Settings uses the shared `Disclosure` and owns Active site + Pane height. The Companion manager reuses `PageHeader`/`Dialog`/`Button`/`ConfirmDialog` and owns saved-site add/edit/remove/reorder only. URL entry is a plain text field with URL input mode and ligatures disabled so typed `https://` punctuation remains visible; only full HTTPS URLs are accepted.
- **Visibility:** content-gated per 0004:2 / 0006:11 — `companionUrl==null` means no WebView, splitter, or Companion toolbar; the floating window never shows the pane.
- **Splitter:** the horizontal draggable divider clamps to 25–60%, updates `companionHeightRatio` live, and persists per monitor. Auto-hide sliver and reveal behavior remain unchanged.

## ACs

- [x] No companion URL set → dock renders without a pane, splitter, or Companion toolbar — empty feature occupies no chrome.
- [x] Any valid HTTPS URL is accepted without a site-specific allowlist and Sprout attempts direct navigation in the native child WebView below the shared toolbar; if the site refuses embedded browsers, Companion shows the stable Retry / Open externally state.
- [x] The child uses its persistent isolated `companion` profile for cookies/auth and is not an `Action` run (no log folder or `quick_action_runs` events).
- [x] Splitter ratio is clamped to 25–60%, persisted globally and per display, and ignored while floating.
- [x] Floating Quick Launch renders no Companion chrome or placeholder; Settings remains the discoverability/configuration home and explains that the active site appears only while docked.
- [ ] Fixed and auto-hide use the same live child WebView; repeated reveal → hide cycles preserve the full pane layout, and hiding only moves the parent offscreen without closing the child or its Chromium/audio processes.
- [x] A saved active-site change replaces the live child WebView immediately, including when a previous creation is still in flight.
- [x] Sprout forwards its concrete light/dark theme to WebView2 as the browser color-scheme preference; supporting sites may honor it, while arbitrary site-authored colors are not forcibly recolored.
- [x] Saving Fixed ↔ Auto-hide immediately applies the final persisted global/per-display state without reopening either window.
- [x] Research `0012` and the CONTEXT Companion entry match the shipped implementation; frontend tests, svelte-check, and `cargo check` pass.

## Out of scope

- Multi-tab companion or omnibox inside the pane (single-tab only).
- Desktop `SetParent` HWND embedding of arbitrary `exe` (rejected in 0012: `electron/electron#10547`/`#26729`, `sweetwisdom/electron-native-windows` `WS_POPUP→WS_CHILD` dance, `stackoverflow 170800` `AttachThreadInput` hangs — brittle for CEF/Chromium like Spotify desktop).
- Command blob spillover (`quick_actions.command TEXT` `db.rs:121` — Q13 left as-is).

## Verification

- `npm.cmd test -- --run`: 71/71 passed.
- `npm.cmd run check`: 0 errors / 0 warnings.
- `cargo check`: passed for v0.8.0 with seven pre-existing warnings.
- Full `cargo test`: 420 passed, 6 unrelated environment/timing failures, 3 ignored; Companion settings tests passed.
- Manual smoke test: Add site, native YouTube load, adaptive narrow-width presentation, OS external-open path, fixed dock visibility. This is evidence for the generic HTTPS-navigation contract, not a YouTube-specific acceptance criterion.

## Closeout corrections (2026-09-03)

- Add site now submits through the shared Button's `kind="submit"` API; URL punctuation remains visible while typing.
- The manager uses the shared PageHeader, has a direct Back to Settings action, removes dividers and duplicated Active site / Pane height controls, and follows the existing flat row/dialog patterns.
- Child-WebView bounds begin below the Companion toolbar, preventing click interception and cursor flicker. Creation failures no longer auto-retry and flicker.
- The runtime identity is Android Chrome, not iPhone Safari, because it matches WebView2's Chromium engine.
- Native Back/Forward history and `_blank` interception described in the original proposal did not ship through the current Tauri child-WebView surface; the toolbar's explicit Open externally action is the supported escape.
- The native frame still uses the measured below-toolbar bounds. A width-derived WebView2 zoom now compensates only when Windows scaling makes the site's CSS viewport unusually narrow; no pane-width, UA, or site-specific override was introduced.

## Follow-up findings (2026-09-04)

- Settings Save no longer fails with `a webview with label quick-launch already exists`. Tauri 2.11.5's `get_webview_window` intentionally rejects a native window once it contains a differently labeled child WebView; `quick_window.rs` now resolves the persistent `Window` parent through one multi-WebView-safe seam. A source-contract regression test prevents parent lifecycle code from returning to `get_webview_window`.
- Fixed/docked → auto-hide/docked now applies far enough for the first real edge reveal to work, but the second reveal can collapse the auto-hide refusal banner into a one-character-wide vertical column. Screenshot: `C:\Users\admin\AppData\Local\Temp\codex-clipboard-fb5f9ad3-5ad1-4422-bc62-ff890552a4cf.png`.
- Saving a different active Companion URL is still delayed or sometimes does not replace the displayed site. Treat the current serialized recreation logic as unverified; reproduce the exact save → event → close/create sequence before changing it.
- Post-fix automated evidence: `npm.cmd test -- --run` passed 78/78, `npm.cmd run check` passed with 0 errors / 0 warnings, `cargo check` passed with seven pre-existing warnings, and `cargo test quick_window::tests::` passed 10/10.

## Follow-up findings (2026-09-04, emit-seam + banner round)

- URL staleness root cause: `lib.rs`'s `emit_quick_launch_changed` / `emit_valid` and the floating eligibility probe still resolved the dock through `get_webview_window`, which yields `None` once the `companion` child exists. A URL-only save persisted, correctly changed no geometry, then had its final event silently dropped — the dock never ran `refreshCompanion`. All four resolutions now share `quick_window`'s native-`Window` seam (no event, payload, or target changed, so entry/action/clip sync is untouched). New source-contract tests pin both files; they went red before the fix and green after.
- Banner collapse root cause: the single flex row cannot fit ~226 CSS px at 150% scaling, so the action held its width while the reason wrapped one character per line. The banner keeps its copy, order, `role="status"`, warn tokens, and shared Button, but stacks the action below an icon + reason row (research 0004: fit must hold at real device DPI).
- Automated evidence: `npm.cmd test -- --run` 80/80, `npm.cmd run check` 0/0, `cargo check` seven pre-existing warnings, `cargo test` 426 passed / 0 failed / 3 ignored, `quick_window` 10/10.
- Still needs live eyes: immediate URL swap on save (including mid-creation), 3 real reveal → move-away cycles with banner legibility, audio continuity with unchanged renderer PIDs, theme forwarding on a `prefers-color-scheme` site. The immediate-apply and repeated-reveal ACs stay unchecked until observed.

## Follow-up findings (2026-09-04, banner-copy round)

- Live report: URL swap is immediate in fixed and auto-hide, hiding works — but saving the Companion URL in auto-hide shows the refusal banner. Diagnosis: the refusal is genuine (another bar owns that edge's auto-hide slot, so `set_autohide` honestly errors), and the banner state likely predates the URL save — the save only re-marks it identically while the repaired emit finally lets the dock *see* the long-standing state. The banner copy was the bug: "stays pinned" is false, because the driver slides the strip regardless of registration (appbar.rs, CONTEXT Quick Launch dock). Copy now reads "{reason}. Hiding still works — the strip slides on its own while that edge stays busy." Same order, same Move action, no token/component changes. Regression test went red → green.
- Automated evidence: `npm.cmd test -- --run` 81/81, `npm.cmd run check` 0/0. No Rust changes this round.
- Open question for the reporter: is the Windows taskbar set to auto-hide on that same edge, and was the banner already visible right after the Fixed → Auto-hide save (before any URL save)? A yes confirms the state predates the URL save entirely.

## Follow-up findings (2026-09-04, spurious-refusal round)

- Live report + log (`unexpected reservation grant ×3`, one `auto-hide blocked` refusal on a Companion-URL save; fixed bottom taskbar, right-edge auto-hide dock, single `sprout.exe` — no zombie twin). Ruled out with evidence: companion reload touches no Win32 state (all inside our HWND); `current_edge_hwnd` resolves live every call (no stale hwnd); `reserve()` sends only `QUERYPOS`/`SETPOS` (cannot disturb slot ownership); a fixed taskbar never claims an auto-hide slot (slots are per-edge). Root cause: `reconcile_saved_settings` unconditionally reset `settled=None` (kicking the driver into a fresh settle → the grant lines) and re-ran `apply_dock_mode` (re-probing the slot → the refusal line) on *every* save, including dock-irrelevant ones.
- Fix: the tail now runs only when the live state has not converged on the persisted prefs (`needs_reestablish` — unit-tested truth table; fresh docks, undocks, edge switches, and mode flips all converge through their own paths, which already register + emit, so nothing real is skipped). Refusals now also log the slot owner (`describe_autohide_owner`: us / empty / dead hwnd / foreign hwnd + class) while the user-facing message stays stable.
- Automated evidence: `npm.cmd test -- --run` 83/83, `npm.cmd run check` 0/0, `cargo check` seven pre-existing warnings, `cargo test` 427 passed / 0 failed / 3 ignored. Regression pins went red → green.
- Acceptance (live): save a Companion URL in auto-hide → no grant lines, no new `blocked` line, no banner change. Then flip Fixed → auto-hide once: success clears any standing refusal; a refusal prints the owner line — send those exact lines back and the holder is identified (dead hwnd ⇒ restart Explorer; live foreign bar ⇒ move edges or live with the informational banner).

## Follow-up findings (2026-09-04, closeout round)

- Reporter confirms live: a saved active-site change replaces the pane immediately in both fixed and auto-hide, including a rapid double-save where the second save lands mid-creation — the last save always wins, no stuck stale page. In-flight caveat struck 2026-09-04; that AC is fully closed.
- Reporter cannot run the repeated reveal → hide + renderer/audio-PID + audio-continuity checks, so that AC stays unchecked. What is code-established: hiding only ever moves the parent HWND offscreen (the driver never closes/hides/recreates the child; the child closes only on URL change, float, or window close), and the refusal banner is display-only (it gates no motion). What is still unobserved: 3 real reveal → move-away cycles preserving the full pane, unchanged PIDs, and uninterrupted playback.
- Fixed ↔ Auto-hide AC checked 2026-09-04 on the passed Test 1 round-trip
  (pre-gating flip observed applying + gating provably preserves the flip path
  + post-gating round-trip witnessed live). Only the audio/reveal-cycles AC
  remains open, deferred to a separate test run.
- Docs AC checked: re-read research `0012` and the CONTEXT `Quick Launch dock` / `Companion` entries against the shipped code on 2026-09-04 — every behavioral claim matches (direct navigation, Android UA, isolated `companion` profile, splitter discipline, floating-vs-docked gating, immediacy, theme contract). The three fix rounds changed no documented contract: the emit seam and owner logging are unspecified implementation detail, the gated reconcile preserves the documented immediacy, and the banner stack/copy is presentation 0012 never specifies. Dated verification counts inside 0012 stay as history; current evidence is `npm.cmd test -- --run` 83/83, `npm.cmd run check` 0/0, `cargo check` seven pre-existing warnings, `cargo test` 427 passed / 0 failed / 3 ignored. No research/CONTEXT edits needed.
- Design note (chat-only until now): the strip painting over the taskbar's corner during reveal is by design — the revealing strip is deliberately topmost because the shell does not maintain z-order for self-driven motion. Unrelated to the auto-hide slot refusal.
- Still open: the slot holder's identity (no owner line ever reported back — needs one refused probe on a build containing `describe_autohide_owner`).

## Deferred live acceptance (agreed 2026-09-04 — run after this ticket)

Build under test: a clean `tauri dev` containing the `needs_reestablish` gating
and `describe_autohide_owner` (both already merged + synced). Companion URL set
throughout, so every step runs in the original failure context.

### Test 1 — Fixed ↔ Auto-hide immediacy round-trip (closes that AC)

1. Settings → dock Docked + Fixed → Save. Expect within ~1–2 s, no reopen: the
   strip turns fixed (maximized windows shrink to make room).
2. Settings → mode Auto-hide → Save. Expect within ~1–2 s, no reopen: the strip
   hides offscreen (other windows regain full width). No refusal banner is
   expected; if one appears, copy the exact `auto-hide: edge registration
   refused — …` dev-log line (it names the holder).
3. Push the cursor into the docked edge, hold past the ~200 ms dwell → the strip
   reveals; move away → it hides again.
4. Settings → mode Fixed → Save. Expect the fixed strip back within ~1–2 s.
5. Regression context: with the Companion URL still set, save a *different*
   active URL while in auto-hide. Expect the pane to swap immediately AND the
   dev log to stay free of `unexpected reservation grant` / `auto-hide blocked`
   lines (the gating fix's whole point).

Pass = every save lands in ~1–2 s without reopening, reveal/hide works after
each flip, URL-only saves are log-quiet.

**Passed 2026-09-04 (reporter):** round-trip works — each save applies without
reopening, reveal/hide works after every flip. Fixed ↔ Auto-hide AC checked on
that basis; the assumption is the reporter's earlier "tested, works" for this
exact procedure — correct it here if that read was wrong.

### Test 2 — Audio continuity (deferred past this ticket — keep its AC open)

1. Companion URL on an audible site; start playback in the pane.
2. Move the cursor away → strip hides. Listen ~30 s while hidden.
3. Optional strength: compare WebView2 renderer/audio-service PIDs before vs
   after hiding (Task Manager details) — unchanged means the child survived.
   (A frozen no-code interval once showed unchanged PIDs, but audible
   continuity itself was never human-checked — that is what this test owns.)
4. Reveal → hide 2–3×: playback never restarts, full pane layout intact.

Pass = uninterrupted audio + stable PIDs + intact layout. Report back and the
reveal-cycles AC gets checked then.
