# 143 — Companion height falls back to default after undock → re-dock

**What to build:** three-part fix (all frontend, no backend change): Settings save merges fresh companion values for untouched knobs (touched + diverged refuses honestly); toggle-path per-monitor resolve retries boundedly and surfaces failures; drag persist failures surface in the window error line.

**Blocked by:** none — DB evidence closed it (global 0.41 stale vs per-monitor 0.6; restart OK ⇒ stores fine, toggle-path-only).

**Status:** implemented — user manual pass pending

## Scope

- Docked and floating; single- and multi-monitor; splitter drag and Settings Pane-height paths.
- Constants untouched: 0.25–0.60 clamp + 0.40 default stay single-sourced in `settings.rs` (frontend mirrors with identical fallbacks, never re-derives).

## ACs

- [x] Undock (auto-hide → float) → re-dock (→ auto-hide) keeps the configured Companion height (bounded re-resolve + surfaced failures; verified by contract tests — live toggle pass belongs to the user matrix below).
- [x] A height save that fails surfaces in the window error line; drag-applied-but-unsaved is impossible to mistake for saved (persist + resolve errors banner; Settings refuses on conflict instead of clobbering).
- [x] `npm.cmd run check` 0 errors; companion/height test slices green (127 frontend tests green, `check` 0/0).

## Follow-up 1 — 2026-09-06 — DB evidence: global 0.41 stale vs per-monitor 0.6

- Read-only DB query proved the split: `settings.companion_height_ratio` 0.41 (stale global) vs `quicklaunch.companion.height_ratio.\\.\DISPLAY1` 0.6 (live per-monitor intent). Restart hits per-monitor (works); toggle re-dock missed it and landed on the stale global.
- (1) Settings `save()` re-reads the store: untouched companion knobs ride fresh values, touched-but-diverged refuses ("Discard and re-apply your edits"), manager-owned site list always fresh. (2) `refreshCompanion` retries the monitor resolve 3×150ms and banners only a genuine failure (missing entry stays a silent global fallback). (3) Drag persist + resolve failures banner in the window error line.

## Implementation notes

- Two stored values by design (ticket 140 AC: per-monitor wins, global fallback), not one: 1080p and 4K keep their own heights. The bug is not the two layers.
- Prime suspect: the configured value never reached the store — `persistCompanionRatio` swallows every failure (`catch (e) { console.error(e) }`, inner `catch {}`), so in-memory height works while docked and any re-resolve (undock → re-dock, restart) falls back to the 0.40 default. Ruled out: backend dock/undock touching companion settings (no companion references in `quick_window.rs` outside comments); duplicate constants (single source verified).
- Runner-up: per-monitor key flip (identity- vs device_name-keyed memory in `db.rs`/`resolve_display_keys`) between drag-time write and post-redock read, combined with a default global. Decisive test: Settings Pane-height value (global read) + restart behavior — default after restart proves never-persisted.

## Follow-up 2 — 2026-09-06 — user evidence narrows it

- Restart restores configured ⇒ stores hold the value; bug is toggle-path-only. Backend dock/undock writes no companion settings (verified `quick_window.rs`, `toggle_quick_launch_dock`).
- In-memory ratio has exactly one writer to default: `refreshCompanion` computing `perMonitor ?? global` as 0.40. So on the toggle path the per-monitor read missed AND global read default.
- Confirmed clobber vector for the default global: Settings `save()` is a full-state write (`companion_height_ratio` always included) while `load()` runs only on mount — a page mounted before a splitter drag saves the stale 0.40 baseline over the configured global on any later unrelated save. (Settings never re-loads on `quick-launch-changed`.)
- Remaining unknown: why the per-monitor read misses specifically on re-dock (monitor-null transient? backend throw swallowed by `catch {}`? different monitor after float-center?). Candidates for hardening: bounded re-resolve retry when the monitor is absent; surface (not swallow) resolve failures.
- To the "single constant?" question: no — two layers are the ticket-140 contract (per-monitor wins, global fallback); constants (0.25–0.60, default 0.40) are already single-sourced in `settings.rs`.

## Verification

- `npm.cmd run check`, companion/height frontend tests + `cargo test` settings slice; manual: set height via drag and via Settings → undock → re-dock → restart, single + multi-monitor.

## Follow-up 3 — 2026-09-06 — the loop was broken in both directions, not just the toggle

- User evidence that re-centered the diagnosis: a Settings height save moves nothing on the live pane, and a divider drag never shows up in Settings. Both verified in code.
- Settings → dock: a saved global height was shadowed by any surviving per-monitor entry (per-monitor wins, so the knob read dead) and nothing pushed back. Fix: on a one-screen machine the save now writes the global value into that display's memory too (the file's own single-vs-multi rule, same as the width memory); several displays keep distinct memories with the global as fallback. The previously silent per-monitor writer (`set_companion_height_ratio_for_display`) now notifies like every sibling companion writer, so the save lands live.
- Dock → Settings: the page loaded once on mount and never listened to `quick-launch-changed`, so it displayed (and could save back) stale companion values. Fix: a silent background refresh of untouched companion knobs on that event (touched knobs keep user edits; the save-time conflict check still guards those); the manager-owned site list always rides fresh.
- Earlier 143 work (save-merge, toggle retry, persist surfacing) stands as complementary hardening; the DB split (global 0.41 vs per-monitor 0.6) was its symptom, not its cause.

## Follow-up 4 — 2026-09-06 — unborn bounds syncs stayed noisy

- Console residue: `syncCompanionWebview bounds failed webview not found` logged on every pass while a fresh child was still registering (worse across rapid recreates, e.g. active-site switches), then self-healed at birth. Behavior correct; the noise was the bug.
- Cause: the bounds pass called setPosition/setSize/setZoom on the unborn handle; the born flag correctly kept the handle (nulling it would fork a duplicate creation) but each attempt still logged.
- Fix: the bounds pass returns early while unborn; a creation timeout (10s) banners loudly if registration never lands (the quiet passes stay silent by design, so a stuck birth must expire loudly), and a late birth clears that banner. Also removed the synchronous hide right after construction — it could only lose to backend creation, and the created callback owns the first yield.
