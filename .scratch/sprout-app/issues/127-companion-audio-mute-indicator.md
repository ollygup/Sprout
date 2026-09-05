# 127 — Companion audio: toolbar mute + playing indicator + volume-mixer shortcut

**What to build:** The docked Companion toolbar gains a mute/unmute toggle, a playing indicator, and a volume-mixer shortcut button; loud/soft stays with the site player and the OS Volume Mixer entry (opened directly by the shortcut — the earlier Settings help line is superseded by it), with Open-externally as escape.

**Blocked by:** none — can start immediately.

**Status:** ready-for-agent (reopened — mixer shortcut folds in; AC3 superseded)

## Scope

- Surfaces: docked Companion toolbar (mute toggle + indicator + mixer shortcut). Floating window and no-URL state gain no audio chrome (content-gated, 0004:2/0006:11).
- Mute via WebView2 `IsMuted`, indicator via `IsDocumentPlayingAudio`/`IsMutedChanged`; persisted global mute (default unmuted). No volume level — API is mute-only (0013:3). Loud/soft guidance is the moment-of-use shortcut button, not a Settings paragraph (0006:1/0006:4/0004:2).
- Reuse shared toolbar/button treatment, tokens only; toggle keyboard-accessible with `aria-pressed` + `aria-label`, indicator tooltip-only (no new component). Mixer shortcut opens `ms-settings:apps-volume` via the existing external-open seam.

## ACs

- [x] Audible site + docked pane → indicator shows playing; mute toggles silence audibly and persists across restart; unmute restores.
- [x] No Companion URL or floating window → no mute/indicator chrome anywhere.
- [x] ~~Help text states loud/soft lives in the site player + Windows Volume Mixer (that session entry), not in Sprout.~~ Superseded: the mixer shortcut button below is the guidance (moment-of-use launcher beats config-surface paragraph); the Settings paragraph is removed.
- [x] `npm.cmd run check` 0 errors; keyboard-only mute + reduced-motion pass.
- [x] Toolbar gains an "Open volume mixer" shortcut (mute ↔ mixer ↔ external order) opening Settings › System › Sound › Volume mixer; failure surfaces an honest error; floating/no-URL gain no new chrome; check/keyboard/reduced-motion pass.

## Verification

- `npm.cmd run check`, related `npm.cmd test -- --run`
- Manual: play → indicator on → mute → mixer shows muted → unmute → indicator/audio back; hide/reveal keeps state.
- Manual (mixer shortcut): docked → shortcut opens the Volume mixer page; playing session adjustable there; keyboard-only run-through.
