# 127 — Companion audio: toolbar mute + playing indicator

**What to build:** The docked Companion toolbar gains a mute/unmute toggle and a playing indicator; loud/soft stays with the site player and the OS Volume Mixer entry (documented in help text), with Open-externally as escape.

**Blocked by:** none — can start immediately.

**Status:** ready-for-agent

## Scope

- Surfaces: docked Companion toolbar (mute toggle + indicator), Settings/Companion help line (mixer guidance). Floating window and no-URL state gain no audio chrome (content-gated, 0004:2/0006:11).
- Mute via WebView2 `IsMuted`, indicator via `IsDocumentPlayingAudio`/`IsMutedChanged`; persisted global mute (default unmuted). No volume level — API is mute-only (0013:3).
- Reuse shared toolbar/button treatment, tokens only; toggle keyboard-accessible with `aria-pressed` + `aria-label`, indicator tooltip-only (no new component).

## ACs

- [ ] Audible site + docked pane → indicator shows playing; mute toggles silence audibly and persists across restart; unmute restores.
- [ ] No Companion URL or floating window → no mute/indicator chrome anywhere.
- [ ] Help text states loud/soft lives in the site player + Windows Volume Mixer (that session entry), not in Sprout.
- [ ] `npm.cmd run check` 0 errors; keyboard-only mute + reduced-motion pass.

## Verification

- `npm.cmd run check`, related `npm.cmd test -- --run`
- Manual: play → indicator on → mute → mixer shows muted → unmute → indicator/22050audio back; hide/reveal keeps state.
