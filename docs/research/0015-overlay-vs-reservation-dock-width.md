# 0015 — Overlay-vs-reservation dock-width policy: fixed stays a strip, auto-hide may overlay wide

**Date:** 2026-09-07 · **Question:** should the dock-width slider share one cap across visibility modes, or split caps by mode — and where does that policy live so a future width change cannot drift it?

## Sprout ground truth (measured, not inferred)

- Single size source is `src-tauri/src/constants/window.rs` (ADR-0021): floating palette `340×460`, dock floor `340`, dock width as monitor-percentage with per-mode caps, auto-hide driver constants, main-window sizes. `tauri.conf.json` declares no windows. The frontend mirrors numeric values for sliders with identical fallbacks and never re-derives them.
- Fixed reserves workspace via `ABM_SETPOS` (`src-tauri/src/appbar.rs:reserve`, `src-tauri/src/quick_window.rs:dock`); auto-hide registers for coordination only and slides over content, reserving nothing (ADR-0011 amendment 2026-08-21, ticket 63; reservation-release in `settle_mode`). Other windows keep full size whether an auto-hide strip is hidden or revealed.
- Width derivation is `% of full monitor (rcMonitor), floored at 340, capped at the mode's cap` (`dock_width_px_for_mode`): stored `%` clamps into `10–60` first, then into the mode's cap, so a broken stored value can neither collapse nor explode the strip; a degenerate monitor falls back to the floor. Undock narrows a widened dock back to the floor; the floating palette stays exactly the floor.
- Caps: fixed `10–30%`, auto-hide `10–60%`, stored `10–60%` (`DOCK_WIDTH_MAX_PCT_FIXED/AUTOHIDE/ABSOLUTE`). The Settings slider maximum follows the current mode (global and per-monitor rows); switching modes re-clamps the edited value honestly (55 in auto-hide becomes 30 in fixed). Stored values validate against the absolute range; the dock applies the mode's cap, so a stored 60 never explodes a fixed strip.
- Cap math: 60% of 3440 is 2064 as a fixed AppBar — wider than a 1080p monitor, leaving 1376 for apps. 30% keeps any reservation at most a third of the screen (1920→576, 2560→768, 3440→1032, 5120→1536). As an overlay the same 2064 costs no workspace, so the reservation objection does not transfer.

## Sources (primary first)

- Microsoft Learn, *Application Desktop Toolbars* (AppBar reservation: `ABM_SETPOS` reserves workspace from the work area; auto-hide bars coordinate without reserving) — https://learn.microsoft.com/en-us/windows/win32/shell/application-desktop-toolbars
- Microsoft Learn, *Responsive design techniques* — show/hide or re-architect content when the available window cannot keep it usable — https://learn.microsoft.com/en-us/windows/apps/design/layout/responsive-design
- Jakob Nielsen, *Progressive Disclosure*, NN/g (2006) — disclose specialized options upon request; the frequency split decides placement — https://www.nngroup.com/articles/progressive-disclosure/
- Repo standing rules: 0004 rule 2 (frequency split), 0008 rule 1 (placement follows persistence/scope — a window-global width knob lives in Settings beside the other dock knobs, deferred to Save), 0006 pattern 8 (view-scoped switches live on-surface; global concerns centralize).

## Findings

1. **Reservation cost is the cap's justification, not width itself.** A fixed strip permanently takes workspace from maximized windows, so its cap answers "how much workspace may one bar claim" (a third). An overlay takes none while hidden and covers content only while revealed, so its cap answers "how wide may a temporary overlay run before it stops being a strip" (over half is still a strip when it costs nothing at rest). One cap for both modes either starves the overlay or over-claims workspace.
2. **The mode, not the monitor, selects the cap.** Monitor width already scales the pixel result (`% of rcMonitor`); the mode selects which `%` range applies. Per-monitor memory keeps each display's `%`; the live mode caps it on apply. Switching modes therefore re-derives width without rewriting stored preferences — honest and reversible.
3. **Clamp, never refuse, stored values.** A stored 55 under fixed is not corrupt — it is a valid auto-hide preference on the wrong mode. Refusing it (validation error, collapsed strip, exploded strip) punishes a legal cross-mode value. Clamping on apply keeps both modes' preferences intact across switches.

## Rules for Sprout

1. **Caps live in `constants/window.rs` and nowhere else (ADR-0021).** A width change starts by scanning that file; adding a second source is a review failure. The frontend mirrors with identical fallbacks and never re-derives.
2. **Fixed 10–30, auto-hide 10–60, stored 10–60.** The slider maximum follows the edited mode; mode switches re-clamp the edited value; the backend clamps broken values exactly as before (clamp + default fallback, never collapse/explode). Undock-narrows-to-floor is unchanged.
3. **Classify width as a window-global Settings concern (0008 rule 1).** It reshapes every dock tab on every monitor; it stays beside the other dock knobs, deferred to Save. No per-surface width switch, no in-dock width editor.

## Verdict

**Split the caps by mode in the single size source; one slider whose maximum follows the mode.** Fixed stays a strip because it reserves; auto-hide may overlay wide because it does not. Accepted in ticket 164 (ADR-0021 amendment); implementation carries the per-mode derivation, the mode-following slider, and the contract tests.
