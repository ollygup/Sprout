# 136 — Settings scale, Companion sites, height, and action-details round (spec)

**What to build:** One round of five vertical slices in reporter order: (1) zero build warnings — delete the truly-dead, annotate the test-only honestly; (2) Companion site names (`{url, name}`, unique both, URL fallback, dropdown shows name); (3) Settings scale — four `Disclosure` groups on the one page plus multi-keyword local search, no nested rail children; (4) Companion launch height — saved ratio applied immediately on cold launch, per-monitor for the actual dock monitor; (5) Quick Action details — note-first, command collapsed to scent, Companion child hidden while the dialog is open. Implemented via tickets 137–141.

**Blocked by:** none (round spec; implemented via tickets 137–141)

**Status:** ready-for-agent

## Problem Statement

Settings has grown into one long ungrouped page that will keep growing; the build emits warnings for unused imports/functions with no stated reason, so real warnings hide; Companion saved sites are bare URLs so the Settings active-site dropdown shows an unreadable URL instead of a name; the Companion pane ignores its saved height (e.g. 50%) on launch until the user touches the docked splitter; and the Quick Action details popup renders the full multi-line command as an unscrollable wall that slides behind the Companion pane, so the end can't be read.

## Solution

From the user's perspective: Settings stays one page with four labeled `Disclosure` groups (General / Dock / Companion / Backup & housekeeping) plus a filter box that matches labels, synonyms, values and descriptions (typing `light`, `dark` or `theme` all find the theme knob); the build is warning-free with every remaining suppression carrying its reason; each Companion site gets an optional name (blank = URL), names and URLs are both unique, and the Settings dropdown lists names; the Companion opens at its saved height on every launch with no drag needed; and the action popup leads with the note, keeps the command to a 3-line scent with a Show-command affordance, scrolls internally, and always sits above (Companion yields while it is open).

## User Stories

1. As a Settings user, I want knobs grouped by type (General / Dock / Companion / Backup), so I can scan without reading the whole page.
2. As a Settings user, I want collapsed group headers to summarize their state (e.g. Dock — auto-hide, left, 18%), so collapsed still tells the truth.
3. As a Settings user, I want a filter that matches synonyms and values (`light`/`dark` find theme; `width` finds dock width), so I don't memorize knob placement.
4. As a Settings user, I want filtering to narrow groups to matches with an honest empty state, so no-match is never a blank stare.
5. As a keyboard/screen-reader user, I want the filter and groups fully operable with visible focus and announced state, so nothing is pointer-only.
6. As a Companion user, I want to name each saved site, so the picker shows `Chill mixes` instead of a URL.
7. As a Companion user with no name entered, I want the URL itself as the display text, so nothing ever renders blank.
8. As a Companion user, I want duplicate URLs and duplicate names both refused with a plain message, so the list stays unambiguous.
9. As a Companion user, I want names to survive backup/restore, so a rebuilt machine keeps my labels.
10. As a Companion user with height at 50%, I want 50% on cold launch, so no splitter touch is ever needed.
11. As a multi-monitor user, I want each screen's Companion height remembered for the screen the dock is actually on, so 1080p and 4K both feel right.
12. As a Quick Action user opening a details popup, I want the note first and fully, so the *why* leads.
13. As a Quick Action user with a long command, I want at most a 3-line scent plus Show-command/Copy, so the popup never becomes a wall (full text stays in the main app).
14. As a docked-with-Companion user, I want the popup fully readable above the pane with its scrollbar and Close/Run reachable, so nothing hides behind the pane.
15. As a contributor, I want zero build warnings with every suppression reasoned, so the next warning means something.

## Implementation Decisions

- **Seams, highest-first:** warnings at the owning modules (no behavior seam — delete vs `cfg(test))`/`allow`-with-reason only); site names behind the existing settings/companion persistence seam (`companion_url_list` grows from strings to `{url, name}` with a tolerant migrate-on-read of legacy string entries, whole-URL dedup preserved per the ADR-0022 amendment); Settings groups as `Disclosure` sections on the existing page (`PageHeader` unchanged) with the filter over the existing `SearchInput` grammar; height behind the existing dock/companion geometry seam (order the launch reads before native-child create/size, resolve the *actual* dock monitor instead of the first-display proxy, per-monitor wins with global fallback); dialog behind the shared `Dialog` + details-dialog seam with a Companion yield (hide/move the native child while any dialog is open — CSS layering cannot cross a native HWND).
- **Settings navigation (research 0014):** Disclosure sections over page splits at current scale; no nested app-rail children, no top tabs, no side-rail sub-pages. Rail labels stay ≤3 words if children ever graduate (`General/Dock/Companion/Backup`); graduation only at ≥7 sections, deep-link/search demand, or a section growing its own surface.
- **Details content (0004:3, 0006:13–14):** dock dialog is level-1 fast read-only; the main-app Quick Actions page stays the level-2 home of the full command. Note renders fully above; command collapses (note present) or truncates to scent (note absent). Scope is Quick Actions only — Launch entries run, Clips copy, neither opens this dialog.
- **Conventions:** design-system tokens and shared components only (`PageHeader`, `Dialog`, `Disclosure`, `SearchInput`, `Button`, `Icon`); icon-only controls carry `aria-label` + tooltip; instant switches stay `role="switch"` (no conversion of Save-deferred checkboxes per 0008:2); single size source stays `constants/window.rs` (frontend mirrors with identical fallbacks, never re-derives).
- **Domain language:** `Companion site` = one entry in the saved-site list; `Companion site name` = its optional display label, fallback the URL. Glossary updates ride with ticket 138.

## Testing Decisions

- Good tests assert external behavior (visible state, persisted prefs, emitted events), not pixel values or internal measurement.
- Prior art: settings round-trip tests, companion validation/clamp coverage, dock per-monitor memory tests, run-control width/state tests, `svelte-check` 0/0 gate.
- Each ticket: `npm.cmd run check` 0 errors, `cargo check` 0 warnings, relevant `cargo test` + frontend test slices green; manual passes at 340px and wide dock, Fixed + auto-hide + floating, single + multi-monitor, keyboard-only, reduced-motion.

## Out of Scope

- Nested app-rail Settings children, top-tab Settings, per-group deep routes (`/settings/dock`…) — refused until the 0014 graduation threshold.
- Companion multi-tab/omnibox/zoom/JS bridge, volume slider, floating Companion (ADR-0022 scope stands).
- Elevated Quick Actions, detached-service running state (ADR-0017 stands).
- Blanket checkbox→toggle conversion (refused under 0008:2; per-ticket instant-effect audits only).
- Deleting the three stale `src-tauri/*.rs` root duplicates — owned by the git device, not this round (they ride along untouched).

## Further Notes

- Evidence: 0004 (frequency split, ≤2 disclosure levels, tab hygiene 1–3 words), 0005 (one PageHeader, search-in-toolbar grammar), 0006 (patterns 1/4/7/8/13–14, disclosure anatomy 9), 0008 (placement-follows-persistence, scent, switch-reads-value), 0014 (nested-rail verdict: sections now, search before hierarchy, graduation threshold); ADRs 0011 (window read-only, dock/undock + edge live in window), 0017 (hidden unelevated runs, note as no-behavior metadata), 0021 (single size source), 0022 (single isolated docked-only site, whole-URL dedup, mute-only audio), 0028 (tokens/components/research lock).
- Facts established in-round: 7 Rust warnings (3 truly-dead incl. the never-constructed Store seam pair, 4 test-only incl. the clamp helper whose only caller is its own test) with 0 frontend warnings; launch race (`load()` never sets companion state, `onMount` fires three reads concurrently, first native-child sizing measures pre-layout rect at the 0.40 init); first-display proxy for per-monitor ratio; dialog geometry (480-wide dialog in a 340 dock ≈312px, ~160px mono column) plus native-HWND stacking.
- Ticket map: 137 warnings → 138 site names → 139 settings groups + search (blocked by 138, builds over the new site shape) → 140 height → 141 dialog (blocked by 140, yields the stabilized pane). All five demoable alone; 138/141 are the two user-visible payoffs.
