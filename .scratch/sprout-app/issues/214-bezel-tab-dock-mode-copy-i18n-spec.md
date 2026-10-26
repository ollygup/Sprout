# 214 — Bezel-tab dock mode + human-friendly copy with file-based i18n (spec)

**Status:** delivered 2026-09-19 — tickets 215–220 applied + validated (see Acceptance and verification). No application behavior changed in this spec file itself; behavior landed in the owning tickets.

**Parents / related (read before implementing):**
- [52 Quick Launch window](52-quick-launch-window.md) + [53 dock AppBar](53-quick-launch-dock-appbar.md) + [55 dock/floating UX](55-quick-launch-window-dock-floating-ux-sync-and-action-control-spec.md) + [57 dock settings + live sync](57-quick-launch-dock-settings-and-live-sync.md) + [60 dock auto-hide](60-dock-auto-hide.md) + [63 auto-hide blocked edge](63-dock-auto-hide-blocked-on-taskbar-owned-edge.md) + ADR-0011 — dock/AppBar foundation. This round adds a third `dockMode` beside `fixed` / `auto-hide`; existing two modes unchanged.
- [111 per-monitor dock prefs](111-per-monitor-dock-preferences-settings.md) + [128 dock width settings-only](128-dock-width-settings-only.md) + [164 split dock width caps](164-split-dock-width-caps.md) + ADR-0020 (monitor identity/seam) + ADR-0021 (single size source + per-mode caps) — bezel Y/height memory follows the same identity-key pattern; open-width cap decision lands here.
- [112 layered autohide reveal gate](112-layered-autohide-reveal-gate.md) + [113 reveal tuning knobs](113-reveal-tuning-knobs-advanced.md) + ADR-0019 — bezel is click-gated, not hover-gated; reveal knobs stay auto-hide-only.
- [33 UI copy rewrite](33-ui-copy-rewrite-plain-technical-style.md) — prior round set plain technical style (no "Sprout never…", no wordplay, single "Loading…"). This round keeps those bans and adds Discord/Notion human voice (2nd person, verb-led) on top — warmth without wordplay, not a silent overwrite.
- [181 Discord presence two-view spec](181-discord-presence-ai-two-view-clarify-spec.md) + [182 offline static presence](182-discord-presence-offline-static.md) + [204 custom input controls Discord frame](204-custom-input-controls-discord-frame.md) + ADR-0033 — presence strings (`details`/`state` allowlist) join the same copy dictionary.
- ADRs: 0011 (AppBar dock, zero-width auto-hide reservation), 0019 (driver owns motion), 0020 (identity + seam eligibility), 0021 (size source + per-mode caps), 0028 (design system + disclosure), 0029 (one owner per Windows command), 0033 (static presence), 0034 (motion tokens). **No ADR text changes in this round:** all decision-boundary moves (third mode, new geometry constants, new store keys, dictionary seam) ship as dated amendments inside the owning tickets below, not here.
- Research: web findings synthesized in this spec (Samsung Edge handle: Size/Width/Transparency + Left/Right + vertical drag + lock; Windows 1–2px auto-hide trigger as undiscoverability anti-pattern; Fitts edge law; Discord/Notion microcopy rules). No new `docs/research/` file — findings are small enough to live here.
- Glossary: no `docs/CONTEXT.md` change in this round. Planned terms for the owning tickets to add with a `planned` qualifier + this spec link: **bezel tab**, **bezel Y position**, **copy dictionary** (see Shared understanding §8–10).

## Problem Statement

Hover-open docks fire by accident: the whole screen edge is a trigger, so grazes slide the panel out over your work. And across Settings, tooltips, and hints, the copy reads robotic — long, passive, jargon-heavy sentences users skim past instead of understanding.

## Solution

Ship a third dock mode, **bezel tab**: collapsed, only a small tab protrudes from the screen edge — hover merely peeks it out a little wider, and only a click opens the full panel. It parks at a user-dragged height on the left or right edge, remembered per display. In parallel, rewrite dock/Settings/presence copy in a Discord/Notion human voice and move every string behind a file-based copy dictionary (`en.json` now, `zh-CN.json` after) so identical text is reused and translation is a file swap, not a hunt.

## User Stories

1. As a dock user, I want a mode that never opens on hover, so grazes stop interrupting me.
2. As a bezel user, I want a visible-but-small tab protruding from the edge, so I can find it without it shouting.
3. As a bezel user, I want hover to only peek the tab wider (never open it), so I get feedback without commitment.
4. As a bezel user, I want a click anywhere on the tab/peek to open the full panel, so the target is forgiving.
5. As a bezel user, I want to drag the tab up/down the edge and have it stay there, so it sits where my hand expects.
6. As a left/right-edge user, I want the side as an explicit choice (global default + per-display override), so handedness and layout win.
7. As a multi-monitor user, I want each display to remember its own tab height-position, so replugging doesn't scramble me.
8. As an open-panel user, I want click-outside, Esc, and tab-toggle to close it — but not focus loss alone — so alt-tab and copy-paste don't slam it shut.
9. As a first-run bezel user, I want a tooltip plus a one-time pulse, so I don't conclude the app vanished.
10. As a multi-monitor user, I want the global Dock width slider disabled with a link to the per-display widths, so I stop turning a knob that does nothing.
11. As a settings reader, I want short human sentences ("Wider fits longer names.") instead of robotic ones, so I understand at a glance.
12. As a future Simplified-Chinese user, I want every string keyed in files (not the database), so translation ships without migrations.

## Shared understanding (grill close-out, R1 Q1–Q7 + R2 Q8–Q11)

1. **Scope:** new third `dockMode` (`bezel`) co-existing with `fixed` / `auto-hide`; hover-open keeps its behavior untouched.
2. **Trigger:** full-edge hover trigger is replaced by the tab only. Hover = peek (visual widen, no open). Click = open. Strict separation from hover mode.
3. **Collapsed geometry (research-backed, exact constants in ticket 215):** ~12–14px visible width; hover peek ~22–26px. Never 1–2px (Windows-taskbar anti-pattern) and never a permanent ~2cm rail.
4. **Height is monitor-relative (user-confirmed):** `bezel_h_ratio` ≈ 12% of the docked monitor's height, clamped to physical-px bounds (proposed 64–160px, ticket 215 fixes numbers) — not a fixed pixel height.
5. **Placement:** pure Y-drag, no Top/Center/Bottom presets. Stored as `bezel_y_ratio 0..1` under the existing per-monitor identity key pattern (EDID identity preferred, device-name fallback — ticket 216), clamped on load, center fallback on disconnect/resize. Side = existing Left/Right setting + per-display override.
6. **Open/close:** open expands to the full dock width (cap decision in ticket 215 — proposed: auto-hide-like overlay cap, reserves nothing extra). Close = click-outside + Esc + tab toggle. Focus loss alone does NOT close.
7. **Width grey-out (user-confirmed sub-scope):** when `displays.length > 1`, the global Dock width slider is disabled with hint text linking to `#per-monitor-title`. Single display = enabled. Good design because the global knob otherwise silently does nothing.
8. **Copy scope V1:** Settings `knob__label` + `knob__hint` + dock tooltips + presence strings. Errors/onboarding/empty states are a follow-up.
9. **Voice:** Discord/Notion pattern — 2nd person, specific verb-led labels, front-loaded keywords, ≤140 chars per hint, no passive robot voice, no fancy words; ticket 33's bans (no "Sprout never…", no wordplay) still hold.
10. **Dictionary:** one file-based source of truth, `src/lib/copy/en.json` keyed `section.key` (e.g. `dock.bezel.tooltip`); duplicate literals reuse keys; i18n-ready loader with EN fallback; `zh-CN.json` scaffold ships with the loader but fills only after EN is final (user-confirmed sequencing). Never in the database.
11. **Motion/discoverability:** peek + open/close reuse ADR-0034 tokens (`--dur-slow:280ms`, `--ease-spring`); dock driver untouched beyond bezel placement (ADR-0019 boundary respected).

## Current vs accepted vs proposed

- Current (verified via CodeGraph/source): `dockMode ∈ {auto-hide, fixed}` (`+page.svelte:104-107`); per-monitor edge/mode/width-% memory via `memory_key(identity, monitor)` (`quick_window.rs:716-752`); auto-hide collapses to zero-width reservation off-screen with wall-push trigger; no i18n infra (`grep i18n|locale` hits only locale-formatting uses); copy lives inline as `knob__label`/`knob__hint` literals.
- Accepted-but-unimplemented assumed: per-mode width caps (164/ADR-0021 amendment), reveal knobs (112/113) — bezel neither depends on nor redefines them; reveal knobs stay auto-hide-only.
- Proposed here: items 1–11 above. No change to `fixed`/`auto-hide` behavior, no new Windows-invocation site (extend `quick_window`/`appbar` owners per ADR-0029), no database shape change beyond two numeric prefs beside existing dock memory.

## Seams (for implementer + reviewer confirmation)

One owner per area, existing seams preferred: bezel geometry + collapsed/open placement extend the `quick_window`/`appbar` dock owner (ADR-0029 — new `bezel` branch beside `settle_mode`/`apply_dock_mode`, constants in `constants/window.rs` single size source, Svelte reads dimensions via Tauri commands, never JS constants); Y/height prefs ride the existing `db` dock-memory seam beside `save_dock_edge/mode/width_pct`; Settings owns the mode option + drag affordance + width grey-out link; new `src/lib/copy/` dictionary (+ loader, no `shared/` module — single consumer pair EN/zh-CN, thin pass-through forbidden by conventions); presence strings consumed from the dictionary at the existing presence seam. No new seams beyond the copy dictionary.

## Design rules every implementer applies

- **Codebase-design vocabulary (docs/agents/conventions.md):** deep modules at clean seams; deletion test; extend the dock/display owners, never a second invocation site; ownership gate must pass.
- **Constants (docs/agents/conventions.md):** all bezel geometry (collapsed/peek widths, height ratio + clamps, Y ratio) lives in `constants/window.rs`; frontend mirrors numbers for sliders only with backend validation as truth.
- **UI/UX:** tokens from `src/lib/styles/tokens.css` + components from `src/lib/components/` only; cite the applied rule per change. No ad-hoc colors/type/radii/dimensions.
- **Planning (docs/agents/planning.md):** distinguish current/accepted/proposed; assumed pendings recorded (164 caps); ADR amendments ship in owning tickets, never silently.

## Out of Scope

- Touching `fixed`/`auto-hide` trigger behavior; top/bottom edges; full-height sliver variant; per-app tab content; tab theming beyond tokens; reveal-knob retuning; errors/onboarding/empty-state rewrite; machine translation of `zh-CN` before EN freezes; any database-backed copy store.

## Ticket map and integration ownership (DELIVERED — tickets 215–220 applied + validated 2026-09-17, revalidated 2026-09-19)

| Ticket | Size | Behavioral prerequisites | Likely paths / owner symbols | Shared contract and integration edits | Candidate wave |
| --- | --- | --- | --- | --- | --- |
| 215 Bezel mode backend (collapsed/peek/open geometry + zero reservation) | L | None — first mover, defines the contract | `src-tauri/src/quick_window.rs` (`settle_mode`, `apply_dock_mode`, `dock`), `src-tauri/src/appbar.rs` (rect/placement), `src-tauri/src/constants/window.rs` (bezel constants — single size source) | Owns contract: `mode="bezel"`, collapsed/peek widths, `bezel_h_ratio` + clamps, open-width cap (propose auto-hide-like overlay; ADR-0021 amendment here), zero-reservation collapsed (ADR-0011/0019 amendments); exposes dimensions via Tauri commands | 1 — with 219 |
| 216 Bezel Y-position store + drag + per-monitor restore | M | 215 (store keys + geometry contract) | `src-tauri/src/db.rs` + `quick_window.rs` (`memory_key`, `monitor_refs`), Quick Launch window drag affordance | Owns `bezel_y_ratio` key under identity-key pattern (ADR-0020 amendment), clamp + center-fallback rules; per-display side override reuses existing edge memory | 2 — after 215 |
| 217 Peek/open/close interaction + motion + discoverability | M | 215 (geometry + open/close hooks) | Quick Launch window page, `tokens.css` motion tokens (`--dur-slow`, `--ease-spring`; ADR-0034 untouched) | Owns: hover-peek (no open), click-toggle, click-outside + Esc close, no focus-loss close, tooltip + one-time pulse copy keys (consumes 219's dictionary) | 2 — after 215, needs 219's keys (coordinate) |
| 218 Settings UI (mode option + edge + Y + width grey-out link) | S | 215 + 216 (mode value + Y pref exist) | `src/routes/settings/+page.svelte` (dock group, per-monitor section `#per-monitor-title`) | Owns: `bezel` in `dockModeOptions` + per-display mode lists, Y control, global-width `disabled` when `displays.length>1` + anchor-link hint; no behavior logic beyond binding | 3 — after 215/216 |
| 219 Copy voice guide + `en.json` dictionary + wiring | L | None (content work; key names coordinated with 215/217) | New `src/lib/copy/en.json` + loader, Settings + dock + presence surfaces, `src/lib/api.ts`/`types.ts` if seam changes | Owns: voice rules doc, key schema `section.key`, EN rewrite of V1 scope (incl. presence strings per ADR-0033), duplicate-literal → key-reuse pass, EN-fallback loader; ticket 33 bans hold | 1 — with 215 |
| 220 `zh-CN.json` scaffold + fallback + translation (after EN) | M | 219 (EN keys frozen) | `src/lib/copy/zh-CN.json` + loader fallback, language setting surface | Owns: scaffold with TODO values, fallback-to-EN proof, full Simplified-Chinese translation of 219's keys, language switch wiring; no new keys of its own | 3 — after 219 |

Claims above are estimates to recheck against code at dispatch (CodeGraph first). File overlap (Settings page across 218/219) is integration, not a dependency: 219 owns string keys, 218 owns control wiring; combined verification on the dock Settings group.

**Post-delivery follow-up (2026-10-26):** [243 — Bezel outside-click close across windows](243-bezel-outside-click-close.md) extends 217's click-outside rule to button-downs that land in another window (the shared native hook). It changes no accepted design decision from this round — same modes, same click-gated open/close, focus loss still never closes.

## Acceptance and verification

- [x] User confirmed third mode, click-only, peek-on-hover, ratio-based tab height, Y-drag with per-monitor identity memory, click-outside + Esc close, width grey-out with link, EN-first then zh-CN file-based dictionary incl. presence strings (grill R1–R2).
- [x] 215 verifies collapsed/peek/open rects per monitor/DPI, zero reservation collapsed, cap honored, `check` + Rust tests + ownership gate — done 2026-09-17, revalidated 2026-09-19 (`cargo test` 713 passed / 0 failed; bezel subset 13/13; `npm run check` 0 errors; ownership gate pass).
- [x] 216 verifies drag → persist → redock restore per display, replug/disconnect clamp + center fallback — done 2026-09-17, revalidated 2026-09-19 (`bezel_y_ratio` per-monitor identity memory in `db.rs`/`quick_window.rs`, drag + keyboard nudge, clamp + center fallback tests pass).
- [x] 217 verifies hover never opens, click toggles, outside/Esc closes, focus loss doesn't, motion tokens + contrast + screen-reader names — done 2026-09-17, revalidated 2026-09-19 (bezel interaction tests pass; 73/73 across copy + bezelY + bezelSettings + bezelInteraction).
- [x] 218 verifies `bezel` option appears (global + per-display), Y control binds, global width disables with working anchor link exactly when `displays.length>1` — done 2026-09-17, revalidated 2026-09-19 (options + Y binding + grey-out link present in Settings; `npm run check` 0 errors; ownership gate pass).
- [x] 219 verifies no inline duplicate of a keyed string in V1 scope, voice rules followed (noun titles + Discord-tone descriptions per ADR-0028 amendment 2026-09-17, superseding the spec-214 2nd-person rule; ≤140 chars; ticket-33 bans hold), loader falls back to EN, `npm run check` 0 errors — done 2026-09-16, revalidated 2026-09-17/19 (copy tests 42/42).
- [x] 220 verifies missing-key fallback, full zh-CN coverage of 219's keys, language switch without restart — done + extended 2026-09-17, revalidated 2026-09-19 (`en.json`/`zh-CN.json` 1182/1182 keys, 0 missing, 0 extra; full vitest 442/443 with the single pre-existing `managedSettings.close` "Available to install" failure from ticket 191, untouched).

## Close-out notes (2026-09-19 revalidation)

- As-built geometry differs from the §Shared-understanding proposal: collapsed 20px (not 12–14px) and peek 60px (not 22–26px) per `constants/window.rs` — hit-threshold + 3× forgiving-target rule; height ratio 12% clamped 64–160px and open cap 60% overlay as proposed. Proposal numbers above remain historical; the constants file is the size source.
- Dictionary grew since ticket 220's 1178-key extension (now 1182/1182, still in lockstep) — key-set deltas belong to later work, not this round.
- Follow-up (not this edit): `docs/CONTEXT.md` still carries the `planned` qualifier on **bezel tab**, **bezel Y position**, and **copy dictionary**. Per planning rules the qualifier is now due for removal since the owning tickets marked verified delivery.

No application behavior changed in this spec file — behavior delivered in tickets 215–220 (see above).
