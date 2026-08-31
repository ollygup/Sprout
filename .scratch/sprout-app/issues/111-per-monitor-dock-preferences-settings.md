# 111 — Per-monitor dock preferences in Settings

**What to build:** With two or more displays connected, Settings' Dock section gains a "Per-monitor" area listing every connected display (friendly label + resolution) with its own Edge and Mode selects, writing through the existing validators into the per-display dock memory. Single-display machines see nothing new — the area exists only when it has content. Global default controls remain above it as the fallback path.

Implements **Study A and B** from `docs/research/0011-natural-edge-reveal.md` (layman, bracket terms). An edge that touches another display by more than a tiny corner [monitor seam, >1 px overlap] is not a wall [not a cursor-stop] — no amount of sensitivity or delay fixes it [KDE #351175 #1 #59 9-year case], only turning that choice off does. A small corner touch [diagonal 1 px] stays allowed.

**Blocked by:** 110 — stable monitor identity (EDID).

**Status:** done

- [x] A command enumerates connected displays with label, resolution, and identity
- [x] Get/set commands read and write per-display Edge and Mode through the existing validators and persistence helpers
- [x] The Per-monitor area appears only when more than one display is connected; a single display shows today's unchanged Dock section
- [x] Saved per-display choices take effect on the next dock/restore for that display
- [x] Rows are keyboard-operable and labeled; styling uses existing tokens/components only
- [x] `svelte-check` clean; backend round-trip tests cover set → load per identity
- [x] Eligibility from live display arrangement [display arrangement] via the same system query as #110 — an edge (Sprout offers `left | right` only [edge]) is offered only when its full side has no >1 px overlap with another screen's rectangle [rcMonitor]; the top screen's left/right and the bottom screen's left/right are independent [display arrangement — vertical seam does not make left/right a seam]. Single geometry source for both identity and eligibility
- [x] Settings disables the ineligible `left`/`right` option with an inline reason (`Borders another display — cursor can't stop there`) and `aria-describedby` [the reason is described for screen readers]; the option stays visible. The `get`/`set` edge commands refuse the same edge with the identical string. The Quick Launch window top bar [edge arrows] shares the same rule (see 119) — disabled arrow + same tooltip/reason so both surfaces match
- [x] Eligibility is cached and recomputed on demand plus when screens are moved/added/removed [WM_DISPLAYCHANGE]; Settings re-renders and exposes the result for 119 via a shared pure helper (no second probe that could drift)
- [x] If a saved edge becomes a middle line [became a seam] (screen moved), the next dock/move/apply silently moves that screen to its opposite outer edge and saves it — no toast or popup [auto-migrate, no announcement]; the bar simply appears where the wall is; never forces floating. Hidden means no handle at all [off-screen] (see 119 Study B)

**Amendment 2026-08-30 — polish within same ACs (not 112-119 scope):**
- Deferred per-monitor `Edge`/`Mode` to `Save settings` (only Theme stays immediate per `0009` explicit-save / `0008` toggle rule; matches global `dock_mode/edge/state`). Batch-writes per display on Save; seam race surfaces as row-local `warn`/`danger` text. `grep per-monitor+save 0 hits in 112-119`.
- Flattened per-monitor to flat `article.knob` per display reusing `.knob` + `Select small` + tokens only (fixes `0005:5 same-kind controls share one treatment`, removes nested `per-monitor__row` card-in-card). `grep flatten 0 hits in 110-119`.
- Fixed `load()` → `loadDisplays()` ordering race (`dockEdge/Mode` fallback) and kept `{#if displays.length>1}` gate only (outer wall always exists for `>1` display per `0011 Study A`; both-edges-seam still shows disabled options with reason).
- Added DEV-only preview `?preview-per-monitor=1` guard `import.meta.env.DEV` in `src/routes/settings/+page.svelte:156` to see flattened knobs on single monitor without second display; tree-shaken out of production build.
