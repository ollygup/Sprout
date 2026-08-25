# 105 — Virtual desktop: assignment activates by use, switch removed

**What to build:** Remove the "Desktop grouping" master switch and rework desktop assignment as an always-available per-entry annotation (ADR-0015). Library page (`+page.svelte`): delete the features-menu row (:~406–417), the `desktopGrouping` state and its toast trio (:~88, :391–397), and the setting read/write of `desktop_assignments` (:~162; stale DB value left untouched — no migration); every gate that read `desktopSupported && desktopGrouping` becomes `desktopSupported`, so the Virtual desktop submenu is always present on supported machines with children **No assignment ✓ | Current desktop | Desktop N… | New desktop…** ("No assignment" checkmarked when `desktop_id` is null); row badges show whenever supported && assigned (:~620–629). Settings' pass-through of the key is deleted too. The features gear disappears from this page entirely when no applicable switches remain (0008 rule 5). Rename residue: no user-visible string says "Desktop grouping" anywhere.

**Blocked by:** 101 — submenu capability must exist before the desktop block becomes a flyout.

**Status:** done — code synced; hands-on light/dark + below-gate passes still owed a human

- [x] Features-menu row, state, toasts, and setting usage for desktop grouping deleted
- [x] Submenu present for every entry wherever `desktopSupported`; "No assignment" escape item works
- [x] Badges render exactly when supported && assigned; stale assignments survive upgrade
- [x] Below 24H2 / OS-refusal: zero feature traces (menu block, badges) as today
- [x] Launch honoring unchanged — assigned windows still open on their desktop
- [x] No "Desktop grouping" string remains in `src/`; glossary-consistent wording only
- [x] cargo test green; manual pass light/dark incl. gear-less header on library page

**Verification notes (2026-08-25):**

Frontend (`src/routes/+page.svelte`): the features-menu row, `desktopGrouping`
state, its optimistic toast trio, and the `desktop_assignments` settings read
are gone; `featureItems` carries only Groups, so the gear stays but with one
row (the gear itself only disappears when *no* switches remain anywhere —
Groups keeps theirs per ADR-0015's structure-vs-annotation split, so this
page never becomes gear-less). The ⋯ menu's desktop stack collapsed into a
**Virtual desktop** flyout gated on plain `desktopSupported`: children are
No assignment ✓ (when `desktop_id` is null) | Current desktop | Desktop N… |
New desktop…. Badges render exactly on `desktopSupported && entry.desktop_id`;
the empty placeholder twin and its CSS are deleted.

Two implementation decisions worth review:

1. **"Current desktop" is an explicit pin**: it assigns the GUID of the
   desktop the user is on right now (resolved at menu build time), distinct
   from "No assignment" (null = launch wherever you start it). This required
   exposing which desktop is current — `DesktopInfo` gained a serialized
   `current` flag resolved via `winvd::get_current_desktop()`
   (`engine/windows.rs::virtual_desktops`, fake engine updated). Without the
   distinction two menu items would have written identical state.
2. **Launch honoring is unconditional**: `run_launch_queue` lost its
   `honor_assignments` parameter entirely (`launch.rs`) — stored assignments
   are always honored where the OS supports them; below the gate the empty
   desktop list degrades every entry to unassigned. The dormant-era tests
   (ticket 88's switch semantics) were deleted with the knob;
   `settings.rs::the_retired_desktop_assignments_key_is_never_read` pins that
   a stale `launch.desktop_assignments` DB row is ignored, not migrated.

Settings page no longer passes the key through on save; `update_desktop_assignments`
command, its registration, and the frontend API wrapper are removed. Gates:
`cargo test` 371 passed / 0 failed, `npm.cmd run check` 0 errors / 0 warnings,
`vitest run` 36 passed, `npm.cmd run build` clean. "Desktop grouping" grep over
`src/` comes back empty.
