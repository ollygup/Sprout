# 156 — Quick-access control round: dock visibility, export, Companion resets, width caps, site UA, dialog submit (spec)

**What to build:** One round of eight vertical slices in reporter order: (1) Logs page sections reorder to launch → actions → runs, mirroring the window tabs; (2) per-item dock visibility (`show_in_dock`) on launch entries, Quick Actions, and Clips — hidden rows leave the dock list and dock Start-all while the main app is unaffected; (3) single Quick Action export from the row menu as a one-element backup document, imported through existing Restore; (4) a Reload reset in the dock Companion bar for dead-end pages (e.g. mistyped logins); (5) user zoom for the Companion page (50–200%, per-site), independent of the height splitter; (6) an "Enable now" hint after adding a Companion site that activates it on-surface; (7) split dock-width caps — fixed stays 10–30%, auto-hide allows to 60%; (8) per-site Desktop/Mobile UA (default Mobile) so desktop-only sites like Teams can load; plus (9) the already-landed shared Dialog Enter-submit repair. Implemented via tickets 157–165.

**Blocked by:** none (round spec; implemented via tickets 157–165).

**Status:** landed in part — 157 done and verified 2026-09-07; 158–165 ready-for-agent.

## Problem Statement

Nine related control gaps, all reported together: the Logs page orders families runs → actions → launch while every tab surface orders launch → actions → clips; there is no way to keep an entry in the main app but out of the dock (no visibility column exists — both surfaces render the full ordered table); a single Quick Action cannot be shared without exporting the whole collection; a Companion page stuck on a bad login has no reset (native Back/Forward don't exist in the Tauri WebView JS surface); the Companion page zoom is automatic only (0.7–1.0 by width) with no user control; adding a Companion site gives no path to activation, which lives on a different page (Settings → Active site); the dock width caps at 30% for both modes although the reservation rationale only applies to fixed; mobile-UA-only Companion is blocked by desktop-only sites (Teams for Web is desktop-only); and Enter in any create/edit dialog dismissed it with nothing saved (clicks always worked).

## Solution

From the user's perspective: Logs reads Quick Launch runs, Quick Action runs, then Run folders; any launch entry, action, or clip can be hidden from the dock from its own row menu or edit dialog while staying fully usable in the main app — and each Start-all starts exactly what its own surface shows; any Quick Action exports from its row menu to a file that imports through the normal Restore flow; the Companion bar gains a Reload reset next to Open-externally; the Companion bar gains a zoom control that remembers per site while the height splitter is untouched; saving a site offers Enable-now right there and the pane appears; the width slider goes to 30% in fixed and 60% in auto-hide; each site can flip between Mobile and Desktop identity (Teams-type sites use Desktop); and Enter in dialog fields saves like the primary button while textareas keep Enter-newline with Ctrl+Enter to save.

## User Stories

1. As a Logs reader, I want families ordered launch → actions → runs, so the page matches the tabs I already know.
2. As a dock user, I want per-item Hide from dock, so clutter stays out of the strip without losing anything in the main app.
3. As a dock user, I want dock Start-all to start what the dock shows, and main Start-all to start what the main shows, so neither button ever runs something invisible.
4. As a Quick Action author, I want per-action Export in the row menu, so one command is shareable without a whole-collection backup.
5. As a Companion user stuck on a bad login, I want Reload in the Companion bar, so I reset without leaving the dock.
6. As a Companion reader, I want page zoom that remembers per site, so small text is fixable without touching pane height.
7. As a Companion user adding a site, I want Enable-now on the confirmation, so activation isn't a hunt through Settings.
8. As an auto-hide user, I want widths to 60%, so a wide overlay is possible while fixed stays a strip.
9. As a Teams user, I want a per-site Desktop switch, so the site loads instead of serving its mobile block page.
10. As a keyboard user, I want Enter in dialog fields to save, so every popup matches its button.

## Implementation Decisions

- **Logs order (0004:4 tab hygiene):** reorder the three rendered sections only (`logs/+page.svelte`); no on-disk moves (folder layout drives retention pruning), no History change.
- **Dock visibility (0006:4, 0008:1):** per-object metadata, so the toggle lives in each row's `⋯` menu plus its edit dialog — never the features gear, never Settings. Canonical name `show_in_dock`, default true (avoids confusion with `show_window`, which is console visibility). Dock filters rows and drops sections emptied by the filter (existing dock rule, 0004:2); main app unchanged; the flag travels in whole-app backup. Each Start-all starts what its surface shows: dock Start-all skips hidden via a backend input filter, main Start-all keeps starting all. `auto_run` startup runs are unaffected (background, not dock visibility). Group-Start plus a per-item start opt-out are recorded future work, not a second flag now.
- **Single-action export (ADR-0014/0026, 0006:4):** the file IS the `sprout-backup` document with a one-element `quick_actions` array — no new format (one-format rule), restoring through the ordinary merge by command+cwd identity. New backend command constructs it server-side; row-menu Export via the preset `saveDialog` pattern; import needs no UI (existing Restore). Payload-identity consequences (same command+cwd skips under any name) belong in the row's confirm copy.
- **Companion Reload (ADR-0022 scope):** one `IconButton` at the bar's left cluster calling the existing recreate logic (`companionRetry`); Reload is a reset, not browsing chrome, so the no-omnibox/tabs/history scope stands. Real Back/Forward stay rejected (Tauri 2.11.1 JS exposes no history API).
- **Companion zoom:** bar `- / % / +` control, 50–200%, persisted per site with auto fallback; the height-ratio splitter is untouched and independent.
- **Enable-now hint (0004:5, 0006:1/6):** transient `Notice` + existing `flash()` pattern carrying one Enable-now action (sets `companion_url`, fires the dock refresh) — not the persistent Settings dirty bar, which guards deferred saves and must not be reused. Wording "Site saved — Enable now".
- **Split width caps (ADR-0021, ADR-0011):** `window.rs` stays the single size source; fixed keeps 10–30% (reservation math: 60% of an ultrawide as a reserving AppBar is extreme), auto-hide allows 10–60% (overlay reserves nothing); one slider whose max follows the current mode, persisted per monitor. Record the overlay-vs-reservation policy as new UI/UX research.
- **Per-site UA:** default Mobile preserved; per-site Desktop override (`Windows NT 10.0 …` covers Win10 and Win11 both — the token is frozen, Win11 differs only via Client Hints) with a current Chrome token, Edge variant preferred since Teams first-classes Edge. Refresh the stale Chrome/131 pin at build time; Open-externally stays the fallback.
- **Dialog submit (research 0010, landed in 157):** pure decision helper plus the single `Dialog` call site; `defaultPrevented` precedence; exactly-once via pre-empting `preventDefault`.
- **Domain language:** `Dock visibility` = per-item `show_in_dock` (glossary update rides with 159); per-site UA preference rides with 165. No other glossary change.

## Testing Decisions

- Good tests assert external behavior (visible state, persisted prefs, emitted events), not pixels.
- Per ticket: `npm.cmd run check` 0 errors, relevant frontend slices green (`dialogSubmit`-style pure tests where a DOM is unavailable), ownership gate pass before sync; manual passes cover Fixed + auto-hide + floating, keyboard-only, and the docked Companion bar order.
- 157's evidence stands: 11 files / 143 tests green, check 0/0, gate pass, sync verified.

## Out of Scope

- Group-Start and per-item start opt-out (future; 159 records the seam).
- Companion Back/Forward/omnibox/tabs/JS bridge, floating Companion (ADR-0022 stands).
- A second backup/export format of any kind (ADR-0014 stands); sanitizing arbitrary command text (ADR-0026 qualifications stand).
- The wider AI-authoring round (spec 145 line, untouched).

## Further Notes

- Evidence: 0004 (rules 2/4/5), 0005:1, 0006 (patterns 1/4/6/8/11), 0007 (moment-of-use classification), 0008 (rules 1–2), 0010 (content-class split); ADRs 0011 (fixed-reserves vs auto-hide-overlays), 0014 (one backup format), 0021 (single size source), 0022 (Companion scope), 0026 (payload identity); `constants/window.rs` cap math; Tauri 2.11.1 WebView surface (close/position/size/zoom only); Microsoft Teams-for-Web desktop-only + frozen `NT 10.0` UA platform facts.
- Facts established in-round: no hide column exists on any of the three tables; both Start buttons share `start_quick_launch` on the full table; single-action export does not exist; dock Back/Forward are preview-iframe-only; zoom is auto-only; `/companion` cannot activate; mobile UA is the Teams blocker; dialog Enter never reached submit while clicks always worked; 0010's "implemented in Dialog" line predated any implementation and no Ctrl+Enter hint exists in any form.
- Ticket map:

| ticket | behavioral prerequisites | likely paths / owner symbols | shared contracts / integration edits | candidate wave |
| 157 (landed) | none | `src/lib/dialogSubmit.ts`, `dialogSubmit.test.ts`, `components/Dialog.svelte` | 0010 grammar; `requestSubmit` with per-form `onsubmit` | done 2026-09-07 |
| 158 (ready) | 157 | per-form `*FormDialog.svelte` hint lines | 0010 hint-under-field; `field__hint` styling | 1 |
| 159 (ready) | none | `db.rs` + `launch.rs`/`quick_actions.rs`/`clips.rs` columns+queries, `lib.rs` Start-all filter, dock + page lists, row menus + edit dialogs, `backup.rs` | `show_in_dock` default-true contract; Start-all filtered-input; backup carries flag | 1 |
| 160 (ready) | none | new export-single backend command, quick-actions row menu, `saveDialog` reuse | one-element `sprout-backup`; Restore merge identity copy | 1 (row-menu overlap with 159 — coordinate) |
| 161 (ready) | none | `quick-launch-window` companion bar + `companionRetry` | bar order: Reload beside Open-externally | 1 |
| 162 (ready) | 161 (bar order) | companion bar zoom control, per-site zoom memory | zoom independent of height splitter | 2 |
| 163 (ready) | none | `companion/+page.svelte` Notice+flash, `setCompanionUrl` activate | Enable-now action; dock refresh event | 1 |
| 164 (ready) | none | `constants/window.rs`, Settings slider, per-monitor memory | single size source; mode-following max | 1 |
| 165 (ready) | none | `settings.rs` site shape + tolerant read, companion create, site edit UI | default Mobile; Desktop override; token refresh | 1 |

## Amendment — 2026-09-08 (follow-on design round)

Spec 166 records accepted selective helper cleanup, hidden-from-dock card indicators, a progressively disclosed main-app Dock visibility filter, and a dock Companion saved-site selector. Its implementation remains pending. The old 158 hint-under-every-textarea requirement is superseded; 157 submission behavior remains required. The new presentation builds on the existing 159 visibility state and 162/165 site preferences. Source inspection on 2026-09-08 found hints, visibility, Reload, zoom and UA controls already implemented despite stale ready-for-agent headers; this is source evidence, not fresh runtime verification.

The earlier phrase Start-all starts what its surface shows does not accurately describe search: the main page calls startQuickLaunch without a selection, and backend start_quick_launch loads the full list. Spec 166 leaves the new filter's launch/reorder scope open for explicit decision. Do not infer a filtered-run contract from this historical prose. No blanket navigation fix is accepted.

## Amendment — 2026-09-09 (accepted follow-on scope)

Spec 166's second round is accepted. Tickets 168/169 add main-app Dock visibility discovery and combined search/visibility matching Start, labeled Start matching (N), disabled at zero. Main Start's old search-insensitive behavior is superseded only when the follow-on implementation lands. Reordering is disabled while filters are active; visibility resets to All on page exit. Ticket 170 adds on-dock site selection with saved-address replacement and persistent cookies/preferences; ticket 167 owns helper cleanup. Ticket 171 is a separate routing investigation, not a delivered fix. Existing historical ACs above are not evidence that these new requirements have shipped.
