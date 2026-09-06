# 144 — Companion Off leaves the WebView2 child alive (+ always-on RAM trim)

**What to build:** backend destroy-on-off guarantee plus frontend orphan sweep plus LOW memory target: `set_companion_url` and bulk `update_settings` destroy the live `companion` WebView by label whenever the saved URL normalizes to off; the dock's off-branch sweeps `getByLabel("companion")` immediately and once more past the cold-init window; every audio-state read pins the child to WebView2's LOW memory usage target.

**Blocked by:** none — code evidence closed it (no Rust path ever closed the `companion` child; frontend close is best-effort on the cached JS handle only).

**Status:** done - repro loop 6/6 green (was 1/6), 461 backend tests green (incl. new off-forms test), svelte-check 0 errors, vitest 137/137, ownership gate pass, synced to the share; live Task-Manager confirmation (extra `msedgewebview2.exe` vanishes on Off) still owed a human

## Scope

- Companion off-paths only: Settings Active-site Off select (bulk `update_settings`), companion-manager active-site removal (`set_companion_url(null)`), undock teardown (frontend sweep only — no setting changes).
- Constants untouched: off-definition stays `normalize_companion_url(...) == None` in `settings.rs` (both writers share it, never re-derive).
- No new Tauri commands or events; no scope change to ADR-0022 (single isolated site, docked only).

## ACs

- [x] Setting Companion to Off destroys the live native child even when the dock frontend's close misses (in-flight creation, stale handle) — backend destroys by label in both writers before emitting.
- [x] No orphaned `companion` renderer survives an off transition: frontend off-branch sweeps by label immediately plus one gated late re-sweep past the 10 s birth timeout.
- [x] Companion WebView2 runs on the LOW memory usage target at all times a pane is loaded (healed on every audio-state read: creation, dock playing poll, mute toggle).
- [x] Late-landing orphans are reaped in ~1 s, not ~12 s (bounded gated poll), and stale refresh runs can't resurrect state (run-id guard) — follow-up 5.
- [x] `tools/repro-companion-off-teardown.ps1` 8/8 green; `cargo test` 461 passed/0 failed; `npm.cmd run check` 0 errors; vitest 137 passed; ownership gate pass.

## Problem

- With a companion site active, setting Companion to Off persisted the off state but could leave the native `companion` WebView2 child alive — an orphaned renderer holding ~150–200 MB until restart.

## Finding

- Teardown was frontend-only and best-effort: `syncCompanionWebviewOnce` closes the cached JS handle, but no Rust code ever closed the child (verified: `companion_audio.rs` only read the handle via `get_webview`). A `close()` landing before backend registration finished (cold profile init is slow), or a stale cached handle, left a native child with no JS owner and nothing ever reaped it.
- Off depended solely on `quick-launch-changed` reaching the dock's JS; nothing backend-side guaranteed destruction.
- No always-on memory mitigation existed: mute is healed on every read, but the memory target was never set.

## Fixes

- `src-tauri/src/companion_audio.rs`: new `destroy_webview()` (best-effort close of the live child by `COMPANION_WEBVIEW_LABEL`); new `apply_memory_profile()` casting to `ICoreWebView2_19` and setting `LOW` (best-effort, idempotent, pre-1.0.2739 runtimes keep Normal); `current_state` applies the profile on every successful live read.
- `src-tauri/src/lib.rs`: `set_companion_url` destroys when normalized is None; `update_settings` destroys when `companion_url` normalizes to None (the Settings Off-select path) — both before `emit_quick_launch_changed`, so the frontend reconciles after.
- `src/routes/quick-launch-window/+page.svelte`: off-branch sweeps `Webview.getByLabel("companion")` right after closing the cached handle; arms one late re-sweep at birth-timeout + 2 s only when a child existed, skipped if the pane came back.
- `src-tauri/src/settings.rs`: `companion_off_forms_normalize_to_none` test locks None/empty/blank to off plus the full-save off round-trip.

## Follow-up 5 — 2026-09-06 — off read as white → bare site → gone at ~15–20 s on a weak device

- User evidence that re-centered the timing: after Off the pane goes white, then the bare site repaints with no toolbar/frame, then it vanishes by itself at ~15–20 s. Clears-by-itself confirmed (no clicks), so this is the in-flight-creation orphan race, not a permanent leak: an already-constructing child registers after the teardown with no JS owner (`created` callback's `companionWebview !== wv` early return) and no Svelte frame, paints late on weak hardware, and the single 12 s backstop reaps it late.
- Fix, frontend-only: the single delayed sweep becomes a bounded poll — sweep now, then every 1 s while the pane stays gone, gated per-tick on same-URL + `!useWebview` (a rapid off→on stops it untouched, so the toggle is never disabled and the new child is never at risk), hard-capped past the cold-init window, one poller at most (re-arm clears), cleared on unmount. Orphan lifetime drops from ~12 s to ~1 s after landing.
- Same pass, stale-write guard: `refreshCompanion` takes a monotonic run id and returns before assigning when superseded — a pre-save settings read landing after an off-save can no longer reassign the old URL (permanent resurrection) or clobber a rapid off→on. Last starter wins; the newer run performs the sync and audio reads itself.
- Off→on safety reviewed, no disable needed: creation always closes by label first (no duplicates), sync passes serialize, the poll/timer gates exclude a live pane, closes are all `catch {}` — worst residue is a transient white flash.

## Verification

- `tools/repro-companion-off-teardown.ps1`: 1/6 before → 6/6 after the destroy fix → 8/8 after follow-up 5 (bounded gated poll + stale-run guard).
- `cargo test --lib`: 461 passed, 0 failed, 3 ignored (companion slice 7/7 incl. the new test).
- `npm.cmd run check`: 0 errors, 0 warnings. `npm.cmd run test`: 137/137.
- `node tools/ownership-gate.mjs`: pass (62 owned references).
- Manual (owed a human): with a site active, note the extra `msedgewebview2.exe` in Task Manager → Settings Companion Off → save → the process exits within seconds; re-select a site → pane returns.
