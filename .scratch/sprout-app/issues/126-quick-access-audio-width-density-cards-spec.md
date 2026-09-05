# 126 — Quick-access round: companion audio, dock width, density, compact action cards, create-dialog extras (spec)

**What to build:** One round of six vertical slices in reporter order: (1) Companion mute toggle + playing indicator in the docked toolbar, loud/soft left to site + OS mixer; (2) dock-width % setting, Settings-only, docked-only, per-monitor; (3) dock density Compact/Default/Large in the surface features menu, tokens only; (4) compact docked/floating Quick Action cards with fixed-width full-height icon-only Run/Stop; (5) conditional Group picker on Quick Action creation; (6) per-action auto-run at Sprout start with Stop preserved. Elevation and restart re-attach are explicitly parked (research 0013), not deferred work. Implemented via tickets 127–132.

**Blocked by:** none (round spec; implemented via tickets 127–132)

**Status:** ready-for-agent (spec; implementation in 127–132)

## Problem Statement

Companion audio can't be muted without muting the system; the dock is one fixed width (340px) that is roomy on small screens and cramped on large ones with many items; large fonts force excess scrolling with no compact option; Quick Action cards are taller than Launch/Clip cards because of their buttons; new actions can't be grouped or auto-run at creation, forcing post-creation assignment passes.

## Solution

From the user's perspective: the docked Companion toolbar gains a mute toggle, a playing indicator, and a volume-mixer shortcut button pointing loud/soft control at the site player and the Windows Volume Mixer entry; Settings gains a dock-width % slider (docked only, both Fixed and auto-hide, remembered per monitor, never draggable to avoid accidents); the Quick Launch surface gains a density switch (Compact/Default/Large) that rescales list text via existing type tokens; docked and floating Quick Action rows become `[flex text | fixed full-height icon Run/Stop]` with spinner for Stopping, same grammar as today (text → details, button → Run/Stop); the Quick Action create dialog gains a Group picker only when groups exist (default ungrouped, with New-group create-and-place) and an Advanced-collapsed auto-run flag that fires each action once per Sprout start in list order with Stop still available.

## User Stories

1. As a Companion listener, I want to mute/unmute the pane from its toolbar, so I don't touch system volume.
2. As a Companion listener, I want to see at a glance whether the pane is playing audio, so mute state is never a mystery.
3. As a Companion listener, I want loud/soft guidance toward the site player and the OS mixer, so I don't hunt for a slider that can't exist.
4. As a dock user, I want a wider (or restored narrow) dock set in Settings, so long names truncate less without accidental drags.
5. As a multi-monitor user, I want each screen's dock width remembered, so 1080p and 4K both feel right.
6. As an auto-hide user, I want the same width setting as a fixed user, so switching modes never resets my layout.
7. As a user with many items, I want a Compact density, so I scroll less; as a large-text user, I want Large without breaking layout.
8. As a Quick Action user, I want dock rows as compact as Launch/Clip rows with an unmissable Run/Stop target, so scanning is uniform.
9. As a keyboard/touch user, I want the compact Run/Stop operable without hover, with a visible focus ring and an announced state, so nothing is pointer-only.
10. As a user with action groups, I want to place a new action into a group at creation, so no second assignment pass is needed.
11. As a user with no groups, I want no group field at all, so the dialog stays minimal until the feature has content.
12. As a user with startup actions, I want flagged actions to run once per Sprout start exactly as if I clicked Run (Stop included), so "docker start"-style setups are automatic.
13. As a user, I want a failing auto-run action never to block the rest, so one bad command doesn't sink startup.

## Implementation Decisions

- **Seams, highest-first:** Companion mute/indicator behind the existing Companion toolbar + child-WebView seam (mute via WebView2 `IsMuted`, indicator via `IsDocumentPlayingAudio`); dock width behind the dock-geometry seam (`constants/window.rs` stays the single size source — min = today's width, ratio + per-monitor memory mirroring edge/mode discipline); density as a surface features-menu switch (0006:8) mapped only onto `--text-*` tokens; cards by restyling the shared run control's dock/window usage (fixed icon width, flex text) while the main-app page keeps icon+text; create-dialog extras as two Advanced-collapsed fields on the existing Quick Action form (no new dialog).
- **Audio scope:** persisted global mute (default unmuted) + transient playing indicator + toolbar volume-mixer shortcut; no volume level (API is mute-only — parked in 0013); content-gated toolbar chrome (no URL / floating → no audio controls, 0004:2/0006:11). The earlier help-line wording is superseded by the shortcut: a moment-of-use launcher beats a config-surface paragraph (0006:1/0006:4/0004:2).
- **Width scope:** % of monitor width, effective width clamped between today's width and ≤60% (cap confirmed by in-ticket research — 60% of an ultrawide as a *fixed* AppBar is extreme); docked only, floating stays fixed; both Fixed and auto-hide share it (gate stays floating-vs-docked per 0012:35).
- **Data:** `auto_run` flag on Quick Action create/update/list + whole-app backup (machine-local, never Presets/exports, like all quick-access state); Group picker writes existing `group_id` membership, no schema change; density + width + mute are Settings/preferences with per-monitor memory where the dock already keeps it.
- **Conventions:** tokens + `Button`/`Icon`/`PageHeader`/`Dialog`/`Disclosure`/`PageFeaturesButton` only, no ad-hoc colors/sizes; icon-only buttons carry `aria-label` + tooltip; `role="switch"` + `aria-checked` for instant switches, checkboxes stay for Save-deferred dialog flags (0008:2); `prefers-reduced-motion` freezes the spinner.

## Testing Decisions

- Good tests assert external behavior (visible state, persisted prefs, emitted events), not pixel values or internal measurement.
- Prior art: Companion settings round-trip + geometry regression tests (ticket 125), `QuickActionRunControl` width/state tests (ticket 124), per-monitor dock-memory tests (ticket 110/111), group-membership tests (ticket 89/90).
- Each ticket: `npm.cmd run check` 0/0, relevant `npm.cmd test -- --run` + `cargo test` slice green; manual passes at 340px and a wide width, both Fixed and auto-hide, floating spot-check, per-monitor recall, keyboard-only run-through, reduced-motion on.

## Out of Scope

- Elevated Quick Actions, restart pid re-attach, Companion volume slider — parked with rationale in research 0013. Entire-card/hover card alternatives were decided (not parked) in 0004's applied cases; blanket toggle conversion was refused under 0008:2, with per-ticket audits (ticket 128 step 0) instead of a conversion pass.
- Multi-tab Companion / omnibox, floating-width resizing, main-app density, Preset/export membership for any of this (all machine-local).

## Further Notes

- Evidence: 0004 (rules 1/2/4/5), 0005 (rules 1/2), 0006 (patterns 1/2/5/6/8/11–14), 0007 (dialog vs switch classification), 0008 (rules 1–3), 0011 (reveal/hover ownership), 0012 (Companion isolation, splitter 25–60%, floating-vs-docked gate), 0013 (parked alternatives); NN/g touch-target 1cm + WCAG 2.5.8/2.5.5 + WIG icon-button/focus/keyboard rules; WebView2 `IsMuted` mute-only API.
- Ticket map: 127 audio → 128 width (+switch-audit step 0) → 129 density → 130 cards (width-agnostic) → 131 group picker → 132 auto-run (blocked by 131, same dialog) → 134 row-shell convergence (shared `QuickLaunchRow` shell + uniform density-following heights; 130's 44px floor retired per user decision, Compact clears AA).
