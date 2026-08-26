# 109 — UX friction round: stable per-monitor dock prefs, layered auto-hide reveal, unsaved-changes guards, Enter-to-submit, Quick Action notes (spec)

**What to build:** One UX round that removes five reported frictions as a single coherent pass: docking preferences become visible and editable per connected display in Settings, and the memory behind them switches from slot numbers Windows can reshuffle to hardware-derived identities; the auto-hide dock stops revealing on accidental edge contact — reveal now requires a deliberate push (direction-gated travel into the edge, held briefly, exactly at the sliver), with two quiet tuning knobs under Advanced; leaving Settings with unsaved changes becomes impossible to do silently — a sticky Save/Discard bar appears while dirty and both rail navigation and window close are intercepted with an explicit three-way choice; Ctrl+Enter submits every dialog form so the multi-line authoring fields finally answer the keyboard; and Quick Actions gain an optional free-form Note — authored as plain formatted text, rendered read-only in a row-click details dialog, marked by a content-gated glyph wherever rows render. Implemented via tickets 110–118, dock work first.

**Blocked by:** none (round spec; implemented via tickets 110–118).

**Status:** ready-for-agent

## Problem Statement

Five frictions compound on daily surfaces. A user docking the Quick Launch window on different screens cannot see or set each screen's preference anywhere — it is remembered invisibly per monitor, and the underlying keys are Win32 slot names that Windows may reassign when displays are replugged or reordered, quietly landing a saved edge on the wrong panel. The auto-hide dock reveals the instant the cursor enters an interior 8 px band at the docked edge, so aiming at corner UI near that edge — a close button, a scrollbar — overshoots into the seam and the strip slides out over the target; the same instant-touch class flashes the dock during routine crossings between neighboring monitors. Settings has no dirty tracking at all: change a knob and navigate away, and the edit vanishes without a word — closing the window loses it identically. In every create/edit dialog the field users type most into is a multi-line textarea where Enter does nothing (it makes newlines), so saving always costs a reach for the mouse or a Tab walk. And Quick Actions have nowhere to record anything about themselves — setup steps, reminders, context — so knowledge about an action lives outside the app or not at all.

## Solution

**Per-monitor dock prefs**: Settings' Dock section grows a content-gated "Per-monitor" area, present only when two or more displays are connected, listing each display (friendly label + resolution) with Edge and Mode selects that write through the existing validators onto per-display storage. Beneath it, dock memory is re-keyed to a hardware-derived identity — make+product code pulled from the display configuration at runtime — with an automatic fallback to device-name keys for displays that expose no usable identity (virtual, RDP) and a legacy-key read fallback so nothing saved before the upgrade is lost. All geometry stays runtime-derived from whichever monitor the dock occupies; no constant encodes a resolution, DPI, or monitor count.

**Reveal gate**: revealing a hidden dock requires the cursor to be within the sliver's own width of the edge (the reserved invisible zone *is* the target — the separate interior band disappears), to be traveling predominantly *into* the edge (along-edge-dominant motion accumulates nothing), and to hold there through a short dwell (~200 ms) that cancels the moment the cursor leaves the band. Grazes along the seam, overshoot-and-rebound, and cross-monitor transits all fail some layer and never reveal; a deliberate push passes all of them. Hide-side hysteresis is untouched. Two knobs — reveal delay and sensitivity — sit under an Advanced disclosure for those who want them; defaults ship tuned and quiet.

**Unsaved-changes guards**: comparing current fields against the loaded snapshot (post-clamp, so clamped numerics don't fake dirtiness), a fixed bottom bar appears whenever anything differs — warning text plus Save and Discard — and stays pinned regardless of scroll. Rail navigation away, and closing the main window (× / Alt+F4), are intercepted while dirty with one dialog: **Save changes / Discard changes / Keep editing**, initial focus on Keep editing, Escape meaning Keep editing, focus restored afterwards. The viewport is never scrolled anywhere to deliver the warning (documented counter-pattern), and state changes announce politely.

**Enter to submit**: one shared handler gives every dialog the standard split — Ctrl+Enter submits from anywhere including multi-line textareas (hint shown beneath each), while single-line inputs keep their native plain-Enter submit. Validation runs identically either way. No app-exclusive combinations anywhere.

**Notes**: a Quick Action gains an optional Note — free-form text whose purpose is whatever its writer wants — authored as plain text supporting simple bullets and numbered lists, stored raw, rendered read-only. Clicking a row opens a centered details dialog (the same grammar Products already use) showing the action with its rendered note beside Run/Edit; rows carrying a note show a small glyph in every list, including the compact window/dock lists where the glyph appears alone. Notes ride along in whole-app backups and never touch Presets, Plans, Runs, or Preset exports.

## User Stories

**Per-monitor dock preferences**

1. As a user with two or more displays, I want Settings to list each connected display with its own Edge and Mode pickers, so I can lay out my docking without dragging the window to each screen in turn.
2. As a user who replugs or rearranges displays, I want each panel's saved dock preference to follow the physical monitor by its hardware identity, so Windows renumbering slots can't hand my left-edge choice to the wrong panel.
3. As a user whose secondary display reports no usable hardware identity (a virtual or remote display), I want preferences stored per device name as before, so the feature degrades instead of breaking.
4. As a user who upgraded with saved dock memories, I want them still honored after the update, so nothing resets behind my back.
5. As a user with a single display, I want Settings to look exactly as today, so the new surface never appears where it has nothing to offer.
6. As a user of any monitor arrangement — mixed DPI, any resolution, either edge — I want identical reveal and docking behavior on every screen, so nothing is tuned to one machine.

**Auto-hide reveal gate**

7. As a user aiming at a button near the docked edge, I want sliding along the seam not to reveal the dock, so my cursor's detours stop summoning a strip over my target.
8. As a user whose pointer rebounds off the edge mid-overshoot, I want that fly-through not to reveal the dock, so accidental contact stays silent.
9. As a user moving between two monitors across a shared seam, I want routine crossings not to flash the dock, so transit stays clean.
10. As a user deliberately summoning the dock, I want the strip out promptly after I press into the edge, so intent still feels immediate.
11. As a user, I want the trigger zone to be the actual outermost pixels — the reserved sliver itself — not a deeper interior band, so near-misses stay near-misses.
12. As a user who wants control anyway, I want reveal delay and sensitivity available under Settings → Advanced, so tuning exists without cluttering the default surface.
13. As a user, I want hiding behavior unchanged, so the familiar exit rhythm doesn't move.

**Unsaved-changes guards**

14. As a user editing Settings, I want an always-visible signal the moment anything differs from what's saved, so dirty state is never a surprise I discover by navigating.
15. As a user with a dirty Settings page, I want Save and Discard reachable without scrolling, so acting on the reminder is one gesture from anywhere on the page.
16. As a user who tweaked a numeric field that gets clamped, I want the clamp not to count as an edit, so the bar doesn't nag about nothing.
17. As a user switching pages while dirty, I want an explicit choice — save, discard, or keep editing — so navigation never silently destroys my changes.
18. As a user closing the window while dirty, I want the same explicit choice, so Alt+F4 is not a discard button.
19. As a keyboard user, I want Escape to mean keep editing and the safe choice focused first, so panic-pressing Escape never destroys work.
20. As a screen-reader user, I want dirty-state changes announced politely without focus theft, so I stay oriented.
21. As a user who chose Save in the intercept dialog, I want navigation to continue once the save lands, so the trip I started still happens.

**Enter to submit**

22. As a user pasting multi-line text into a new Clip, I want Ctrl+Enter to save it without leaving the textarea, so keyboard entry finishes the job the keyboard started.
23. As a user writing a multi-line command, I want plain Enter to keep making newlines, so my script is never fired half-written.
24. As a user filling single-line fields, I want plain Enter to submit as it already does, so the common case keeps its habit.
25. As a user in any dialog — products, presets, launch commands, groups — I want the same submit rule everywhere, so one lesson covers the app.
26. As a learner, I want a visible hint on multi-line fields telling me the submit key, so the convention is discoverable, not folklore.

**Notes**

27. As a user with an action that needs preparation, I want to attach whatever text I like to it — steps, reminders, context — so the knowledge lives with the action.
28. As a note author, I want simple bullets and numbered lists in my text, so structure costs nothing more than typing.
29. As a user scanning Quick Actions, I want a small mark on rows carrying a note — including in the mini window and dock — so I know to look before running.
30. As a user wanting the full text, I want clicking the row to open the action's details with the note rendered read-only alongside Run/Edit, so reading never risks firing.
31. As a user backing up the whole app, I want notes included, so they survive machine moves like everything else local.
32. As a user who clears a note, I want it gone completely — no ghost marks, no residue.

## Implementation Decisions

- **Monitor identity**: resolve a GDI display name → EDID manufacturer+product pair via the display-configuration API at runtime; the identity string becomes the per-display storage suffix. Fallback chain: identity unavailable → device-name key (virtual/RDP/headless). Reads try the identity key, then the legacy device-name key, so pre-existing memories survive. Known accepted limit: two identical monitor models share one identity and therefore one preference. Topology-independence is a standing rule — geometry derives at runtime from the docked monitor's own rect; no constant encodes resolution, DPI, or monitor count.
- **Per-monitor Settings area**: enumerated displays surface as label + resolution rows with Edge and Mode selects reusing the existing validators and persistence helpers; the area renders only when more than one display is connected (absent-until-content rule); global default controls above remain the fallback path.
- **Reveal gate**: trigger zone derives from the sliver constant itself (single size source — no second width value); reveal requires accumulated toward-edge travel above a sensitivity threshold *and* a sustained presence interval, evaluated on the existing poll loop; samples dominated by along-the-edge motion are discarded; the pending reveal cancels the moment the cursor exits the band, which is also what makes inter-monitor transit inert. Hide hysteresis, animation timing, and reservation behavior unchanged. Decision logic lives behind a pure-function seam (the established pattern for dock geometry tests).
- **Reveal knobs**: two validated settings keys — delay (milliseconds) and sensitivity threshold — surfaced under an Advanced disclosure inside the Dock section, defaults equal to the shipped constants; the driver honors them on subsequent evaluations.
- **Dirty tracking & bar**: snapshot comparison against loaded values performed post-clamp; the bar is position-fixed at the page bottom with warning text + Save + Discard (two buttons max, fixed placement — the documented save-bar pattern); appearance/disappearance announced via a polite live region; never scrolls the user.
- **Leave guards**: rail navigation checks the dirty flag before routing; the main window registers a close-requested guard while dirty. One three-way dialog serves both paths with consequence-named buttons; initial focus on Keep editing; Escape resolves to Keep editing; focus returns to the trigger afterwards; Save completes, then navigation proceeds.
- **Submit keys**: the shared Dialog component owns one keydown handler — Ctrl/Cmd+Enter invokes the form's submit path; multi-line fields show a hint line; single-line native form submission untouched; validation errors render inline exactly as button-driven submits do.
- **Notes data**: nullable text column added to Quick Actions via the established migration pattern; the edit payload extends with the field; create/update/list carry it; whole-app backup merge preserves it; Presets/Plans/Runs/exports untouched (machine-local, per glossary).
- **Notes rendering & surfaces**: a minimal read-only formatter supporting paragraphs, `-`/`*` bullets, and `1.`-style ordered lists — everything else escaped verbatim; authored raw in a textarea with a hint; row click opens a centered details dialog built on the shared Dialog primitives (focus management, Escape, restore) matching the Product-details grammar; the carrying-a-note glyph renders on every list surface, content-free (glyph only) on compact ones.
- **Glossary/docs**: already landed this session — CONTEXT.md **Note** term; research 0003 edge-hijack addendum (with the layman layer table and topology-independence rule); research 0009 (unsaved-changes evidence); research 0010 (submit-key conventions); research 0006 patterns 13–14 (row-click detail surfaces, existence glyphs).

## Testing Decisions

- Good tests assert external behavior through existing seams only: temp-database round-trips (the groups/db test precedent) for notes CRUD, backup passthrough, and legacy-key fallback reads; pure-function tests over the reveal-gate decision logic covering the four canonical cases — graze, fly-through, deliberate push, cross-seam transit (the settle/edge-hit geometry tests are prior art).
- Validator tests extend to the new settings keys (clamps and refusals) following the existing validate-and-default shape.
- Frontend: `svelte-check` clean everywhere; a small unit suite for the note formatter (store-level tests are the prior art); manual verification checklists cover keyboard-only dialog flows, dirty-bar interactions in light+dark, the replug/reorder scenario for dock-memory stability, and the single-monitor absence of the per-monitor area.
- Identity derivation itself is Win32-bound and verified manually via the replug checklist rather than a unit seam; only its fallback selection logic is pure-tested with injected inputs.

## Out of Scope

- Serial-number-grade monitor identity (twin identical models intentionally share one preference).
- A per-monitor docked-vs-floating choice (transient window state, not a stored preference).
- Auto-save for Settings (explicit save remains; these knobs have machine effects).
- Rich-text note editing beyond bullets/ordered lists; notes on Clips or Launch entries (extraction to a shared annotation store deferred until a second owner actually exists).
- Changing global-default dock controls, dock animation timing, or reservation behavior.
- The ABM_\*EX multi-monitor registration follow-up (remains flagged in research 0003 as its own item).

## Further Notes

- Evidence base: research 0003 addendum (Windows taskbar internals; GNOME PressureBarrier, KDE screen edges, macOS dwell; triangle-filter debunked; the layer-by-layer benefit table; topology-independence rule), research 0009 (Discord/GitLab intercept practice; scroll-to-warning documented as counter-evidence; accessibility recipe), research 0010 (submit conventions split by content class, with official receipts), research 0006 patterns 13–14 (peek/detail grammar; content-gated glyphs).
- The grilling transcript's decisions (Q1–Q15 and the Round-2 confirmations) are consolidated here; user corrections applied: the design is explicitly monitor-count/device agnostic, and the Note is purpose-free by definition.
- Machine diagnostics from this session explain the reported left/right asymmetry (AppBar reservation + pointer-hotspot geometry) but are evidence only, never design inputs.
