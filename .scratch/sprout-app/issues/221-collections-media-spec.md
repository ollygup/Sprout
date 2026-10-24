# 221 — Collections & media: group colors, clip viewer, nested groups, drag & drop, dock layout (spec)

**Status:** planning package only — no application behavior changed here. Tickets 224–233 and 238 are `ready-for-agent` (created next in this unit).

**Parents / related (read before implementing):**

- [ADR-0016](../../../docs/adr/0016-groups-isolated-dissolve-dormant.md) — groups are isolated, dissolve when empty, and go fully dormant when off. **Amended by this spec:** two-level nesting, sibling-scoped names, subtree dissolve/delete, and color as presentation data.
- [ADR-0021](../../../docs/adr/0021-single-size-source-dock-width.md) — single size source and dock-width formula. **Amended by this spec:** the 420 CSS px dock group threshold is a content breakpoint, not a reusable window dimension; it lives with the dock layout, not in `constants/window.rs`.
- [ADR-0026](../../../docs/adr/0026-machine-local-boundary-backup-identities.md) — machine-local boundary. Group color is an explicit exception: it is local presentation and is **never** written to an export or backup. Groups themselves remain machine-local. Current gap: the backup document does not carry groups at all; this round does not add that.
- [ADR-0028](../../../docs/adr/0028-design-system-disclosure-rules.md) — tokens/components and disclosure rules. Color is user data, not a design token; chrome stays token-only and color is decorative-only.
- [ADR-0029](../../../docs/adr/0029-one-source-of-truth-per-windows-command.md) — one owner per Windows command. The clip viewer owns the low-level mouse hook; the amendment records the owner.
- [ADR-0034](../../../docs/adr/0034-motion-tokens-ease-out-restrained-spring.md) — motion tokens; drag/reorder motion reuses them.
- [52 Quick Launch window](52-quick-launch-window.md), [53 dock AppBar](53-quick-launch-dock-appbar.md), [214 bezel spec](214-bezel-tab-dock-mode-copy-i18n-spec.md) — dock surfaces the viewer opens over.
- [89–106 groups](89-groups-foundation-per-collection-domain.md) — group data, menus, dissolve, and dormant behavior this round extends.
- [178 image clips](178-image-clips.md) — image-only Clips and the copy path the viewer reuses.
- [212 unified chrome](212-unified-window-chrome-spec.md) — window chrome grammar the viewer follows.
- Research: [0004 progressive disclosure](../../../docs/research/0004-progressive-disclosure-and-clips.md), [0006 Notion patterns](../../../docs/research/0006-notion-design-patterns.md), [0015 dock width policy](../../../docs/research/0015-overlay-vs-reservation-dock-width.md), plus this round's viewer/drag/bezel evidence in Further Notes.

## Problem Statement

From the user's side:

- Groups all look the same. In a long Launch, Quick Actions, or Clips list, there is no at-a-glance way to tell one area from another.
- Image Clips can be copied but not inspected. Right-click opens a text-style menu; there is no way to see the image large, zoom into detail, or verify the right image before copying.
- Groups are one level deep. A real collection mixes broad areas with sub-areas ("Work" containing "Client A", "Client B"), so everything lands in one flat bucket.
- Reorganizing means opening the ⋯ menu for every move. There is no direct manipulation, and no way to drag a card into a group or reorder groups by hand.
- The per-monitor dock card in Settings squeezes three control rows into one horizontal card and overflows; the dock-size controls are hard to use.
- The dock has no narrow-width group strategy. At the default strip width, group headers and rows compete for space; nested groups would be unreadable without one.

## Solution

Ship five connected changes over the three main-app collections and the dock:

1. **Group colors.** A group can carry one color: a curated hue or any `#RRGGBB`. It renders as a 3 px accent bar, a dot in the group menu, a tint of at most 12 %, and a 1 px outline. Color is decorative only and stays on this machine — never in an export or backup.
2. **Clip image viewer.** One reusable floating window opens from an image Clip row (main app or dock), on the monitor that invoked it. It remembers its size, is not always-on-top, fits/zooms/pans the image, copies it, and closes on Esc or an outside click.
3. **Nested groups.** Two levels maximum. A top-level group may hold items and child groups; a child group holds only items. Names are unique among siblings, not across the whole collection. Groups dissolve bottom-up; deleting a parent ungroups its whole subtree but never deletes an item.
4. **Drag & drop.** A Notion-style six-dot grip appears on hover/focus on cards and group headers in the three main-app collections. Stage 1 drags cards (reorder and group assignment) and reorders top-level groups; stage 2 reparents/unnests groups. The ⋯ menu remains the complete keyboard path; the dock is never draggable.
5. **Dock layout.** The per-monitor Settings card stacks vertically. In the dock, groups render nested when the tab panel is at least 420 CSS px wide and flatten below that; the parent is the only collapse level.

## User Stories

1. As a Sprout user, I want to give a group a color from a curated palette, so I can scan my collections by area instead of reading every name.
2. As a Sprout user, I want to enter any `#RRGGBB` value, so the curated palette does not limit my organization.
3. As a Sprout user, I want one continuous 3 px accent bar down a colored group's leading edge, so I can identify it while scrolling.
4. As a Sprout user, I want a color dot in the group menu, so I can see the current color before changing it.
5. As a Sprout user, I want a calm wash on a colored group and its hue on a row when I hover it, so the group reads as one region without hurting text contrast.
6. As a Sprout user, I want to clear a group's color, so it can return to neutral.
7. As a color-blind user, I want color to be decorative and never the only signal, so I can still use every group by name, position, and count.
8. As a privacy-conscious user, I want group colors to stay on this machine, so an export or backup never carries my visual setup.
9. As a Sprout user, I want colors to survive rename, reorder, and reparent, so organizing does not lose them.
10. As a Sprout user, I want to right-click an image Clip to open it large, so I can inspect it before copying.
11. As a Sprout user, I want the viewer to open on the monitor I invoked it from, so it appears where I am looking.
12. As a Sprout user, I want the viewer to remember its size, so it opens how I left it.
13. As a Sprout user, I want the viewer not to be always-on-top, so it does not cover other work when I switch away.
14. As a Sprout user, I want Esc and a click outside the viewer to close it, so dismissal is quick.
15. As a Sprout user, I want Ctrl+wheel and the +/− controls to zoom, so I can inspect fine detail.
16. As a Sprout user, I want `0` to fit, `1` for 100 %, and Ctrl+0 to reset, so I can jump to useful scales.
17. As a Sprout user, I want to drag-pan a zoomed image and double-click to toggle fit/100 %, so navigation feels direct.
18. As a Sprout user, I want a Copy button in the viewer, so I can copy the image without going back to the row.
19. As a keyboard or screen-reader user, I want the viewer's controls labeled and focusable, so I can use it without a mouse.
20. As a Sprout user, I want two levels of groups, so I can separate categories inside a category.
21. As a Sprout user, I want a top-level group to hold both items and child groups, so the structure matches how I think about the collection.
22. As a Sprout user, I want two different parents to be able to use the same child name, so "Work" can exist under both "Clients" and "Personal".
23. As a Sprout user, I want to move a group into another group, so I can reorganize without deleting and recreating it.
24. As a Sprout user, I want to move a child group back to the top level, so I can undo nesting.
25. As a Sprout user, I want a group with children to stay at the top level, so the hierarchy never exceeds two levels.
26. As a Sprout user, I want a parent group to collapse its entire subtree, so I can hide a large area in one click.
27. As a Sprout user, I want child groups to collapse independently in the main app, so I can focus on one sub-area.
28. As a Sprout user, I want deleting a parent group to keep every item in its subtree, so I never lose data.
29. As a Sprout user, I want empty groups to dissolve from the bottom up, so stale structure disappears without leaving empty parents.
30. As a Sprout user, I want nested groups to render in the dock when it is wide enough, so the dock matches the main app.
31. As a Sprout user, I want the dock to flatten nested groups when it is narrow, so labels and counts stay readable.
32. As a Sprout user, I want a six-dot grip to appear on hover or focus, so I know what is draggable without permanent chrome.
33. As a Sprout user, I want to drag a card between groups, so I can reorganize directly.
34. As a Sprout user, I want to drag a card within a group, so I can set its order without using the menu.
35. As a Sprout user, I want to drag top-level groups to reorder them, so the list matches my priorities.
36. As a Sprout user, I want to drag an item-only group into another group, so nesting is a direct gesture.
37. As a Sprout user, I want invalid drops refused with a reason, so I cannot create a third level or a cycle.
38. As a keyboard user, I want the ⋯ menu to perform every move, so drag is never required.
39. As a Sprout user, I want Esc to cancel a drag, so a mistake is recoverable before it is saved.
40. As a Sprout user, I want no drag in the dock, so the compact surface stays predictable.
41. As a Sprout user, I want the per-monitor dock card to stack vertically, so the edge/mode/width/bezel controls are not squeezed.
42. As a Sprout user on a narrow dock, I want group rows to truncate cleanly, so nothing overflows the strip.
43. As a Sprout user on a wide dock, I want child group headers indented, so the hierarchy is visible at a glance.

## Current vs accepted vs proposed

**Current (verified in source, not prose):**

- `groups` has `id`, `collection`, `name`, `position` only; no parent, no color. `Group` carries `id`, `collection`, `name`.
- Group names are unique across the whole collection, case-insensitively, trimmed.
- A group dissolves when no item in any collection references it; `delete_group` nulls direct members and compacts sibling positions.
- The three main pages render flat `groupView` sections through the shared `GroupAccordion`; the dock builds flat `launchSections`/`actionSections`/`clipSections` and renders the same accordion in its flush variant.
- Image Clips already exist (`clip_images`, PNG/JPEG, 5 MB cap, one image per Clip). Left-click copy works in the main app and dock. Right-click opens the shared context menu; there is no image viewer window.
- `ContextMenu` supports exactly one submenu level. `GroupAccordion` is single-level.
- The per-monitor Settings card is an `<article class="knob">` with three `.knob__input` rows; `.knob.knob--stack` exists and is used by managed-list knobs.
- The dock has no group layout breakpoint; labels truncate, but there is no narrow-width flattening strategy.
- The three item lists have global `position` columns and `move_*` commands; group membership is a separate `group_id`. There is no atomic "assign + reposition" command.
- No drag controller exists. `Icon` has `dots` (three horizontal dots) but no six-dot grip.
- The backup document carries Products, Presets, Launch entries, Quick Actions, and Clips — **not groups**. ADR-0026's statement that groups ride in the whole-app backup is accepted but unimplemented.

**Accepted-but-unimplemented assumed by this round:**

- ADR-0026 says groups are machine-local and carried by the whole-app backup. They are not in the backup document today. This round does not add group backup; when it lands, group `color` must be omitted. Nested `parent_id` is structural and should be carried when groups enter backup.
- ADR-0016's isolation/dissolve/dormant rules remain in force; only the one-level restriction and whole-collection name scope change.

**Proposed here:**

- Group `color` and two-level `parent_id`; sibling-scoped names; bottom-up dissolve; subtree delete that ungroups but never deletes items.
- The clip viewer window, its zoom/pan/copy surface, and the outside-click hook.
- A shared pointer-based drag controller with two stages and a transactional card placement command.
- The per-monitor card stack fix and the dock's 420 CSS px group layout.
- ADR amendments: 0016 (nesting/names/delete), 0021 (content breakpoint clarification), 0026 (color excluded from backup), 0029 (viewer hook owner). ADR-0025 is unchanged.

## Implementation Decisions

### A. Group color

1. **Storage.** `groups.color` is nullable text. `NULL` means neutral. A stored value is either a curated palette id or a `#RRGGBB` string. The backend validates: `#` plus exactly six hex digits, no alpha, no shorthand, no named/rgb/hsl forms; normalize to lowercase. Invalid values are refused with a readable message, never stored.
2. **Payload.** `Group` gains `color: string | null`. `list_groups` returns it; a `set_group_color` command writes it. Color never participates in identity, ordering, dissolve, name collision, search, or backup identity.
3. **Curated palette.** The color UI owns a list of ten named hues (red, orange, amber, green, teal, blue, indigo, violet, pink, slate). Each has a light/dark accent value that passes the project's non-text contrast gate against the surface it sits on. The backend accepts any valid hex, so adding a palette hue later needs no migration. Exact values and contrast evidence land with the color ticket.
4. **Rendering.** A colored group is one object, not a framed box: a single 3 px accent bar runs the full height of the section at its leading edge (header and body, pill ends), the body carries a wash derived from the color, and hovering a row inside the group paints that row with the same hue. The 1 px colored outline is gone; no tinted border replaces it. Text, icons, badges, focus rings, and menu surfaces stay token colors. Color is decorative: the group name, count, position, and menu remain the only required signals. Amended 2026-10-24 — exact geometry, wash and hover formulas, and the AA text rule are in the amendment at the end of this spec (research 0028).
5. **Picker.** The group ⋯ menu gains a Color row showing the current dot. Selecting it opens a small dialog with the ten swatches, a custom `#RRGGBB` field, and "No color". It uses the existing dialog contract (focus trap, Escape cancels, inline error, immediate apply on choose). Choosing a swatch applies and closes; the custom field applies on submit.
6. **Scope.** Child groups can carry their own color; there is no inheritance. A neutral child inside a tinted parent stays neutral. Colors survive rename, reorder, and reparent; dissolve/delete discards them.
7. **Local-only.** Export/backup serializers must never emit `color`. Presets, plans, runs, and Quick Action files never carry it. A regression test asserts the backup document shape has no group color; when group backup is added, the test must still hold.

### B. Clip image viewer

1. **One window, one route.** A single `clip-viewer` window and viewer route. `open_clip_viewer(clip_id, source_window)` raises the existing window, sends it the new clip id, and focuses it; otherwise it creates the window at the remembered size, centered on the monitor that owns `source_window`. One viewer at a time, never a second window.
2. **Window behavior.** Frameless with the unified chrome grammar (drag region, compact title, Copy, close), resizable, not always-on-top, not a dock. Size is remembered in machine-local settings; position is always centered on the invoking monitor, clamped to its work area. Default size is the smaller of 960×720 and 80 % of the work area; minimum 480×360. Closing destroys the window; the next open recreates it at the remembered size.
3. **Content.** The viewer resolves the Clip by id through the clips API (no image bytes in URLs or events), shows the image, its name (or "Image"), and its pixel dimensions/mime. A missing/deleted Clip shows a friendly empty state and closes on Esc. Copy uses the existing image clipboard command and shows the same copied feedback as the row.
4. **Open triggers.** Right-click anywhere on an image Clip row in the main app opens the viewer. Right-click on an image Clip row in the dock opens the viewer. The row's ⋯ menu gains "Open image" as its first item (the keyboard path). Shift+F10/context-menu key always opens the menu, never the viewer. Text Clips keep the existing right-click menu.
5. **Zoom and pan.** Zoom range 25–800 %. Fit is contain-with-margin; 100 % is one image pixel per physical display pixel (device-pixel-ratio aware). Ctrl+wheel zooms at the cursor; `+`/`−` zoom at the viewport center; `0` fits; `1` goes to 100 %; Ctrl+0 resets to fit and centers; double-click toggles fit ↔ 100 % at the cursor; drag-pan uses pointer capture when the image is larger than the viewport; plain wheel pans vertically, Shift+wheel horizontally. Arrow keys pan when zoomed. Controls are labeled buttons with visible focus; the zoom percentage is announced.
6. **Dismissal.** Esc closes. While the viewer is open, a low-level mouse hook owned by the viewer module watches for a button-down outside the viewer's HWND and its WebView child; after a 200 ms grace from open, the first outside click closes the viewer. The click is not swallowed, so the dock/underlying window still receives it. The hook is installed only while the viewer is open and unhooked on close/destroy. If the hook cannot be installed or UIPI makes it unreliable (elevated target), the viewer falls back to focus-loss close after the same grace, ignoring focus loss to its own child WebView/dialogs.
7. **Dock interaction.** The viewer is a separate top-level window; opening it does not change the dock's mode, reveal state, or reservation. Closing it returns focus to the source window. Opening another image from the dock reuses the same viewer. No viewer-in-dock, no drag from the viewer, no always-on-top toggle.
8. **Ownership.** The viewer window builder, close command, size memory, and mouse hook live in one viewer module (ADR-0029). The hook is not added to the dock/appbar owner.

### C. Nested groups

1. **Schema.** `groups.parent_id` is a nullable self-reference to a top-level group in the same collection. `NULL` = top level. A child's parent must be top level; a parent may have items and children; a child may not have children. The backend enforces depth 2 in every write path, not just the UI.
2. **Payload and order.** `Group` gains `parent_id: number | null`. Positions are sibling-scoped: top-level groups have one position sequence per collection; children have one sequence per parent. `list_groups` returns a flat list with `parent_id`; the frontend builds the tree. `move_group(id, parent_id, to_position)` is the single reparent/reorder contract: it validates, renumbers the old and new sibling sets, and commits in one transaction. `null` parent moves to top level.
3. **Validation.** Refuse: a parent that does not exist, belongs to another collection, or is itself a child; a move that would give a group children while it is a child; a move into itself; a name collision in the destination sibling set. Errors name the reason and the colliding name.
4. **Names.** Unique among siblings: top-level names are unique among top-level groups in the collection; child names are unique among the children of the same parent. Comparison is trimmed and case-insensitive. Different parents may reuse a name. Create defaults to top level; rename and move validate against the destination sibling set.
5. **Existence and dissolve.** A group exists while it has at least one direct item **or** at least one child group. `sweep_empty_groups` deletes bottom-up in one transaction: child groups with no items first, then parents with no items and no remaining children. Natural emptying cascades upward. Re-enabling Groups after off restores the stored tree untouched.
6. **Delete.** Explicit delete of a child ungroups its items and deletes it. Explicit delete of a parent ungroups **every item in its subtree** (direct and descendant), deletes its child groups, then deletes the parent. No item is ever deleted. Confirmation copy states that items stay and subgroup structure is removed. Sibling positions compact at both levels.
7. **Reparent menu (keyboard path).** A group's ⋯ menu gains "Move into…" when eligible. For an item-only top-level group, it lists the other top-level groups; for a child, it lists "Top level" plus the other top-level groups. A group with children has no "Move into…" (it can only reorder at top level). The menu stays one submenu level. This path must exist before drag stage 2 and must remain after it.
8. **Main-app rendering.** Parent accordion → direct items → child accordions indented. Child accordions are independently collapsible (default expanded); parent collapse hides the entire subtree. Parent count badge = total items in the subtree; child badge = its own items. Group menus are available at both levels. While filtering, a parent stays if it or any descendant has a matching item; empty children drop; a parent with matches only in a child shows that child.
9. **Dock rendering.** The threshold is the tab panel's **content width**, measured by a container query, not the window width. At ≥ 420 CSS px: parent header, direct items, then each child sub-header indented and **always expanded** (no child chevron); the parent chevron collapses the whole subtree. Below 420: one flattened list — parent header, then all items in tree order (parent direct first, then each child's items in child order), with no child sub-headers. A parent with visible descendants but no direct items still renders. A parent whose whole subtree is hidden drops. The dock has no group menus and no colour editing; a group's colour still renders there read-only (ticket 241).
10. **Toggle off.** Groups off stays fully dormant: the tree is neither shown nor touched, no group affordance appears, and re-enabling restores it. The dock flatten/nest logic only runs when Groups is on.
11. **Color.** Per-group, no inheritance. Child colors are independent; a child without color is neutral inside a tinted parent.

### D. Drag & drop

1. **One controller.** A shared pointer-events drag controller (not HTML5 drag-and-drop) serves all three collection pages. It owns drag state, the 4 px start threshold, the ghost/insertion line, edge auto-scroll, drop-target resolution, Esc cancel, and cleanup. It never persists; pages supply item/group data and callbacks.
2. **Grip.** A six-dot grip appears on hover/focus at the card's leading edge and on group headers. Only the grip starts a drag; the rest of the card stays clickable. The grip is a focusable button with an accessible name and instructions. Keyboard users use the ⋯ menu — Move up/down, Move to group, Move into… — never a simulated drag.
3. **Stage 1.** All three main-app collections. Cards drag within and between sections: the drop resolves to a target group (including ungrouped) and a global list position, then one transactional `place_item(collection, item_id, group_id, to_position)` command assigns and repositions. Top-level groups reorder by dragging their grip. Child groups are not draggable in stage 1; their menu reorder remains.
4. **Stage 2.** Child groups drag between parents and to top level; item-only top-level groups drag into another top-level group; groups with children reorder at top level only. Drop on a group header = nest into it (if eligible); drop on an insertion line between groups = reorder at that level; drop on the top-level empty area = unnest. A group with children is a valid parent but never a valid child. A drop on the current parent is a no-op. Invalid targets show a not-allowed cursor and a short reason.
5. **Ordering contract.** Card order within a group follows the global item `position`; dragging a card changes that global position, so each group's visible order stays consistent and Start-all keeps following the global list order. Group order is sibling-scoped.
6. **Limits.** No drag in the dock, no drag of images from the viewer, no cross-collection drag, no third level, no drag while a filter is active (the drop targets would be ambiguous; the menu remains available). Esc cancels; reduced motion removes the ghost animation but keeps the insertion line; 100–200 % DPI and pen pointers are supported.
7. **Failure handling.** `place_item` and `move_group` are transactional. If a drop fails, the page reloads and surfaces the error through its existing error line; the UI never leaves a half-applied move.

### E. Per-monitor Settings card

1. The per-monitor `<article>` uses the existing `knob knob--stack` variant so the label, edge/mode selects, width row, and bezel row flow vertically at full width. No new CSS pattern, no new token, no new size constant.
2. The width readout keeps the existing `effectiveWidthPx` formula and per-mode caps. The only change is layout.
3. The global dock-width card and the per-monitor card keep their existing labels/IDs/search keys; settings search must still find both.

### F. Dock group layout

1. The 420 CSS px threshold is a content breakpoint owned by the dock group layout. It is not added to `constants/window.rs` (ADR-0021 amendment).
2. At and above the threshold, child headers are indented and always expanded; the parent is the only collapse. Below it, the subtree flattens. The threshold applies to the tab panel, not the window, because bezel chrome, padding, and the scrollbar reduce content width.
3. The same layout pass fixes the current narrow-width overflow: labels truncate, count badges stay visible, and rows never exceed the strip. This is the dock-layout bug ticket.

## Seams

- **Groups data/behavior:** `groups.rs` remains the single owner of group semantics (create/rename/move/delete/assign/sweep). Extend it with `parent_id`, sibling-scoped validation, `move_group(parent, position)`, and bottom-up sweep; do not add a second group store or a parallel tree table.
- **Group UI:** `collectionGroups.svelte.ts` remains the shared manager; extend it with tree derivation, color actions, and the reparent menu. `GroupAccordion` remains the one section component; add a nested variant rather than a second accordion.
- **Item placement:** `ordered_list.rs` remains the position discipline; add one transaction-aware `place_item` path used by drag. Do not duplicate position math in the drag controller.
- **Drag:** one new `collectionDrag` controller shared by the three pages. No per-page drag implementations. Drop-target resolution is a pure function so it can be tested without pointer events.
- **Clip viewer:** one new viewer module owns the window, size memory, close command, and mouse hook. The clips API gains only a by-id read if none exists; no image bytes in events. The hook owner is recorded under ADR-0029.
- **Dock layout:** the dock page owns the container-query mode and the flatten/nest derivation; the shared accordion gains only a flush/nested presentation. No new dock window or size constant.
- **Settings layout:** the existing `knob--stack` variant is the seam; no new card component.

## Design rules every implementer applies

- **Codebase design:** deep modules at clean seams; apply the deletion test; prefer the highest existing seam; one adapter is inlined; no second group store, drag controller, or viewer window.
- **UI/UX:** tokens from `src/lib/styles/tokens.css`, components from `src/lib/components/`; no ad-hoc type sizes, radii, or chrome. Color values are user data, but all chrome around them is token-only. Cite the applied research rule (0004, 0006, 0015) in the ticket. Every deviation is recorded for review before shipping.
- **Accessibility:** color is never the only signal (WCAG 1.4.1); non-text contrast for the accent bar and menu dot (1.4.11); text contrast against composited colored surfaces (1.4.3); keyboard paths for every drag move; focus-visible rings; screen-reader labels for grip, viewer controls, and the zoom readout.
- **Windows ownership:** the viewer's low-level hook lives in the viewer module; extend ADR-0029's inventory, never duplicate. No new Windows call in the dock/page.
- **Data safety:** no item is deleted by group delete; no color in export/backup; no cross-collection move; no third level; all multi-row moves are transactional.
- **Planning:** distinguish current/accepted/proposed; record assumed pendings (bezel mode shipped in 215–218; image clips shipped in 178); no silent ADR overwrite — amendments are dated and appended.

## Testing Decisions

Good tests assert external behavior, not implementation details. Prefer the highest existing seam: pure functions for layout/tree/drop/zoom math, backend tests at the `groups.rs`/`db.rs` data seam, and manual matrices for real window/DPI/OS behavior.

- **Backend (`groups.rs`, existing test style):** create/rename with sibling-scoped names; two parents may share a child name; move into/out of a parent; refuse depth 3, cycles, cross-collection parents, and collisions; bottom-up dissolve; parent delete ungroups the whole subtree and keeps items; child delete ungroups its items; color validation (valid hex, invalid alpha/shorthand, clear); `place_item` atomicity (assign + position in one transaction, rollback on failure).
- **Backend (`db.rs`):** WAL is enabled on every connection; `busy_timeout` is 5000; migrations still run; foreign keys still hold; worker and app connections use the same initializer. (Storage spec 222 owns the concurrency test.)
- **Frontend pure modules (Vitest, matching `dockSettings.test.ts`/`windowChrome.test.ts` style):** tree derivation and counts; filter visibility; dock layout mode at 419/420/421 CSS px; color parsing/normalization; drop-target resolution (valid/invalid/nested/reorder); zoom clamp/fit/100 %/reset math; viewer size clamping.
- **Component tests (only where the project already does them):** the group color dialog's validation/focus contract; the viewer's control labels and keyboard handlers; the per-monitor card's stack contract. No jsdom layout assertions pretending to test CSS.
- **Manual matrix:** three collections × flat/nested × colored/neutral × drag stages; dock at 340/420/800 px in fixed/auto-hide/bezel, light/dark, 100/150/200 % DPI; viewer from main and dock, multi-monitor, Esc/outside click, UIPI/elevated fallback, copy feedback; per-monitor Settings at 1/2 displays and 200 % DPI.
- **Prior art:** `groups.rs` tests for CRUD/dissolve/isolation; `ordered_list.rs` tests for position compaction; `dockSettings.test.ts`, `windowChrome.test.ts`, `bezelInteraction.test.ts` for pure UI logic; `clips.rs` tests for image validation.

## Out of Scope

- Adding groups (or their colors) to the backup document; that remains the ADR-0026 gap. When it lands, `color` stays out and `parent_id` is included.
- Group colors in Presets, Plans, Runs, Quick Action files, or any export.
- Three or more group levels; cross-collection groups; tags or multi-membership.
- Viewer editing, annotation, rotation, slideshow, next/previous navigation, or a second viewer window.
- Viewer always-on-top, viewer-in-dock, or dragging images out of the viewer.
- Drag in the dock, drag while filtering, touch-specific gestures, or a keyboard "drag mode".
- Per-monitor viewer position memory; only size is remembered.
- A user-managed custom palette, color names, gradients, patterns, or alpha.
- SurrealDB or any storage-engine change (see spec 222).
- Redesigning the dock's existing bezel/auto-hide behavior or width formula.

## Further Notes

**Research basis.** The group/color/nesting/drag decisions follow the standing Notion patterns (research 0006): a hover grip is the drag affordance, group headers are the collapse surface, nesting is direct manipulation, and menus remain the complete non-drag path. Progressive disclosure (0004) caps the dock at one collapse level and hides affordances until content exists. Dock width policy (0015) keeps the strip/reservation split; this round adds a content breakpoint, not a new window size. Color is treated as data with decorative rendering; WCAG 1.4.1 (use of color) and 1.4.11 (non-text contrast) govern the accent bar, dot, and outline. The viewer follows desktop image-viewer conventions (`0` fit, `1` 100 %, Ctrl+wheel zoom, drag-pan, double-click toggle). The outside-click hook is a `WH_MOUSE_LL` hook installed only while the viewer is open; Windows low-level hooks require a message loop on the installing thread, and UIPI can make them unreliable against elevated windows, hence the focus-loss fallback. The exact hook lifecycle and fallback are owned by the viewer module (ADR-0029).

**Why two drag stages.** Stage 1 is the high-value, low-risk path: cards between/within sections and top-level group order. Stage 2 is the nested reparent/unnest path, which needs the tree contract, invalid-target rules, and the transactional group move. Shipping stage 1 first keeps the controller and drop-target vocabulary stable before adding nested targets.

**Why flatten below 420.** The dock's default width is about 346 px; two levels of indentation plus labels, count badges, and the parent chevron do not fit legibly. Flattening preserves every item and the parent header while dropping only the child sub-headers. Widening the dock (auto-hide/bezel may go to 60 %) earns the nested view. The parent remains the only collapse, so the dock never asks the user to manage two disclosure levels in a strip.

**Why delete ungroups the subtree.** The standing rule is that deleting a group returns members to ungrouped and never deletes items. For a parent, "members" means every item in the subtree; child groups are structure, not items. Promoting children would surprise the user with new top-level groups and create name collisions. Ungrouping the subtree is the least destructive interpretation and matches the existing confirmation voice.

## Ticket map and integration ownership

| Ticket | Behavioral prerequisites | Likely paths / owner symbols | Shared contract and integration edits | Candidate wave |
| --- | --- | --- | --- | --- |
| [224 Settings per-monitor dock card stack](224-settings-per-monitor-dock-card-stack.md) | None | Settings per-monitor card; existing `.knob--stack`; settings search test | None — visual-only; no size/token change | 0 |
| [225 Group colors](225-group-colors.md) | None | `groups.rs` (validation, `color`), `db.rs` migration, `Group` type/API, `collectionGroups.svelte.ts`, `GroupAccordion`, new color dialog, i18n, backup-shape test | `Group.color` contract; palette module; color never in export/backup | 1 |
| [226 Clip viewer shell](226-clip-viewer-shell.md) | None | New viewer module/window/route; clips by-id read; viewer size settings; `Icon` copy/close; i18n | `open_clip_viewer` command + `clip-viewer` label; size memory | 2 |
| [227 Viewer zoom/pan](227-clip-viewer-zoom-pan.md) | 226 | Viewer route; pure zoom/pan module; i18n | Zoom/fit/100 %/reset contract | 2 |
| [228 Viewer outside-click hook](228-clip-viewer-outside-click.md) | 226 | Viewer Windows module; ADR-0029 owner; hook lifecycle | Hook + focus-loss fallback; no dock owner change | 2 |
| [229 Nested groups backend](229-nested-groups-backend.md) | None (schedule after 225 for the shared schema file) | `groups.rs`, `db.rs` migration, API commands, `Group` type | `Group.parent_id`; `move_group(parent, position)`; sibling names; bottom-up sweep; subtree delete | 3 |
| [230 Nested groups main-app UI](230-nested-groups-main-app-ui.md) | 229 | `collectionGroups.svelte.ts`, `GroupAccordion`, three pages, group menu, dialogs, i18n | Tree view, subtree counts, Move into…, delete copy | 3 |
| [231 Nested groups dock layout](231-nested-groups-dock-layout.md) | 230 | Dock page, flush accordion, pure layout function | 420 CSS px content contract; flatten/nest; narrow-width fix | 3/4 |
| [232 Drag & drop stage 1](232-drag-drop-stage-1.md) | 230 | New `collectionDrag` controller; three pages; group headers; `ordered_list.rs` `place_item`; API | Drag state/drop contract; atomic card placement | 4 |
| [233 Drag & drop stage 2](233-drag-drop-stage-2.md) | 232, 230 | Drag controller; `move_group`; group headers | Nested drop rules; invalid-target reasons | 5 |
| [240 Group color anatomy and row hover](240-group-color-anatomy-row-hover.md) | 225 | `GroupAccordion` (bar, per-card tint, published hover surface), three main-app collection pages (`.rack__row` tint + hover), `tools/contrast-check.mjs` | Colored-group anatomy: full-height bar, 8 %/14 % card tint, hover `color-mix`, no outline | 1 (follow-on) |
| [241 Dock group color](241-dock-group-color.md) | 240 | Dock's three `GroupAccordion` call sites, flush colored anatomy, `QuickLaunchRow` tint + hover | Colour renders read-only in the dock; controls stay in the main app | 1 (follow-on) |
| [242 Shared collection row shell](242-shared-collection-row-shell.md) | 230, 238 | New row shell component; three main-app pages; `QuickLaunchRow`; `GroupAccordion` published properties | One owner for the row card: chrome, tint, hover; behaviour-neutral | 2 (refactor) |
| [238 Collections & media verification](238-collections-media-verification.md) | 224–233, 240, 241 | All of the above | Combined matrix; no new behavior | Final |

**Concurrency notes.** 224, 225, and 226 can start together (different surfaces). 227 and 228 both follow 226 and touch different parts of the viewer (controls vs hook), so they can run together after 226. 229 and 230 are sequential because they share the group payload/UI. 231 follows 230. 232 follows 230; 233 follows 232. 225 and 229 both touch `groups.rs`/`db.rs`; sequence them rather than running concurrently. 234 (storage) shares `db.rs` with 225/229; schedule it after those migrations or give one integration owner for `db.rs`. Claims above are estimates to recheck against code at dispatch (CodeGraph first).

## Acceptance and verification

- [x] User confirmed the round's decisions: colors are local-only; two levels maximum; mixed items and subgroups; sibling-scoped names; bottom-up dissolve; parent delete ungroups the subtree; child groups with children are refused; dock flatten below 420 CSS px; viewer controls and outside-click behavior; drag stages and grip-only drag; per-monitor card fix.
- [x] 224 verifies the per-monitor card stacks without overflow at 1/2 displays and 100/150/200 % DPI. (2026-10-24: 24/24 CDP matrix runs green; pre-fix row layout control RED — see ticket Evidence.)
- [ ] 225–228, 240, and 241 verify color, the colored-group anatomy and row hover in the main app and the dock, viewer, zoom/pan, and outside-click behavior, including local-only export/backup.
- [ ] 229–231 verify nested groups in data, main app, and dock, including delete/dissolve and the 420 px threshold.
- [ ] 232–233 verify drag stages, invalid drops, keyboard menu parity, and transactional failure handling.
- [ ] 238 runs the combined manual matrix and records evidence.
- [ ] ADR amendments 0016, 0021, 0026, and 0029 are appended with dates and rationale; no contradictory active requirement remains.
- [ ] No application behavior changed in this planning session.

## Amendment — 2026-10-24 (group colour anatomy + row hover)

Ticket 225 shipped §A.4 as written: a header-only 3 px bar, a 10 % sRGB body tint, and a 1 px inset outline in the group hue. Using it surfaced two problems. The header stub and the separately outlined body read as a box inside a box, and hovering a row inside a coloured group shows no trace of the group's colour — every row paints `--bg-hover` or nothing over the wash. Research [0028](../../../docs/research/0028-collection-color-systems.md) re-derived the anatomy from Notion, Discord, Linear, GitHub, Material 3, Fluent, Apple HIG, WCAG 2.2 and CSS Color 5. Stories 3 and 5 and §A.4.4 are superseded below; §A.1–3 and §A.5–7 stand unchanged (storage, payload, palette, picker, scope, local-only).

**Geometry — one object.** A coloured group draws a single 3 px bar at its leading edge spanning the section's full height — header and body, pill ends — in `--group-color` in both themes. The header-only stub is removed. The header and body keep their reserved leading inset, so titles and rows keep the shared start they have today.

**Wash.** The body (not the header) carries `color-mix(in oklab, var(--group-color) 8%, var(--bg-page))` in light and 14 % in dark. `in oklab` is `color-mix()`'s default interpolation and keeps all ten hues perceptually even; dark needs the higher alpha because the same alpha over `#191919` moves chroma far less than over `#ffffff`.

**No outline.** The 1 px coloured inset outline is deleted and nothing tinted replaces it. If a collection later needs separation from the page it is a neutral `1px solid var(--border)` — never the group hue (Notion, Linear, GitHub and Windows all separate with neutral strokes or none).

**Hover carries the hue.** Rows inside a coloured group paint `color-mix(in oklab, var(--group-color) 14%, var(--bg-hover))` on hover; neutral groups and ungrouped rows paint `var(--bg-hover)`. The mix is opaque, so it composites once over the wash instead of stacking, and a nested group cannot compound two washes. Row hover becomes uniform across Clips and Quick Actions; the Launch home rows are display-only and take no hover state, since a hover wash there would imply a click that does not exist. It is a state layer carrying the group's identity, not a second tint.

**Text on the tinted hover.** Secondary text is promoted to `--text` while a coloured row is hovered. `--text-muted` measures 3.79–3.88:1 light and 4.23–4.31:1 dark against the composited hover surface (AA needs 4.5), and 4.52/4.65 on plain `--bg-hover` — the pair has no headroom, so no tint that still reads as a hover keeps it. On the 8 %/14 % wash `--text-muted` holds 4.59–4.66 / 4.89–4.99, so it stays there. Numbers and reasoning: 0028's addendum.

**Contract.** `GroupAccordion` publishes the derived hover surface and hover text as inherited custom properties (`--group-row-hover`, `--group-row-hover-text`); the pages apply them in one rule and never read the group's internal class. A neutral group resets both, so a neutral child inside a coloured parent (229/230) inherits no hue (§A.6).

**Fallback.** `@supports not (background: color-mix(in oklab, red, blue))` → wash transparent (the bar still identifies the group), hover `var(--bg-hover)`, hover text `--text-muted`.

**Dock unchanged.** §B.9 still passes no colour to the dock's group headers; dock rows stay colourless.

**Accessibility.** The bar clears 3:1 against the page in both themes (existing gate); the hover is a ≥3 Oklab-lightness step from the wash (measured 5.75–6.38 light, 7.28–7.65 dark at 14 %); focus-visible rings and menu surfaces are untouched. Colour remains decorative: name, count, position and menu are the required signals.

**Known dead token.** `.rack__row` declares `background: var(--bg-card)`, a token defined nowhere in `tokens.css` — rows compute transparent today, which is why the wash currently shows through them. Ticket 240 replaces it with an explicit `transparent` in the three collection rows; other `--bg-card` users are out of its scope.

**Ownership.** Ticket [240](240-group-color-anatomy-row-hover.md) owns implementation; 238's combined matrix verifies it.

## Amendment — 2026-10-24 (dock shows group colour read-only)

§B.9's "no group menus and no color editing" was read by ticket 240 as "no colour in the dock". That was too broad. The decision is about **controls**: the dock has no group menus, no colour picker, no rename, no move, no delete — every write stays in the main app. Colour is **display data**, so a group's hue must render in the dock exactly as it does in the main app; a dock that hides it makes the colour feel like a main-app-only decoration.

Ticket [241](241-dock-group-color.md) implements the read-only rendering:

- The dock's three `GroupAccordion` call sites pass `color`, so the same full-height 3 px bar and 8 %/14 % wash appear in the strip.
- The flush variant cannot reuse the main app's padded card (the strip is 340–420 px and its rows are flush), so `.group--colored.group--flush` insets the section by `--space-2` for the bar and drops the wash's padding. Row width and vertical rhythm stay the strip's.
- Dock rows are `QuickLaunchRow`, not `.rack__row`. Their hover paints the same `--group-row-hover` surface (inherited from the section) over the existing accent border, and their secondary text uses the published `--group-row-hover-text`. Neutral and ungrouped rows keep the clear card.
- §B.9's control rule stands unchanged: the dock still exposes no group menu and no colour editing.

This supersedes the "Dock unchanged" paragraph in the 240 amendment above. Everything else in 240 stands.

## Amendment — 2026-10-24 (colour sits on the items, not the region)

The 240 amendment tinted the group's **body** (`--group-row-bg` over `.group__rows`), so the hue filled the region — including the empty space above, below and between the cards. Side by side, that read as a coloured *panel* with items floating on it: the colour drew a box the user never asked for, and the "colored group" looked like a different surface rather than a set of colored items. On a 340–420 px dock strip the same wash became most of the window.

**Decision.** The hue belongs to the **items**. A colored group renders:

- the full-height 3 px bar (unchanged — it is the identifier),
- a plain page-surface body (no wash, no frame),
- each row card tinted with `--group-row-bg` = `color-mix(in oklab, var(--group-color) 8%, var(--bg-page))` in light, 14 % in dark,
- the same hover surface (`--group-row-hover`) and hover text (`--group-row-hover-text`) as before.

The empty space between cards stays page surface, so the group reads as "these items are blue", not "this region is blue". Neutral groups publish no tint; their rows stay transparent, exactly as before.

**Where it lives.** The mix is still defined **once**, in `.group--colored` (`GroupAccordion`), together with its dark twin and the `@supports not (color-mix…)` fallback. Row chrome stays owned by each surface: the three main-app pages set `background: var(--group-row-bg, transparent)` on their own `.rack__row`, and the dock's `QuickLaunchRow` does the same on `.dock-row`. The accordion never learns row class names, and no page carries a colour literal.

**Why the consumption line repeats.** Svelte scopes component styles, so a page's `.rack__row` selector compiles to `.rack__row.svelte-<hash>`; a global rule of equal specificity would win or lose by bundle order. Publishing one property and consuming it where the row's box is already styled is the seam that keeps the colour single-sourced without a fragile global override. The genuine duplication is the row **chrome** itself (three near-identical `.rack__row` blocks plus the dock row); that predates this round and is captured in ticket [242](242-shared-collection-row-shell.md) as a deepening opportunity, not solved here.

**Supersedes:** the wash paragraph and the "Known dead token" note in the 240 amendment, and the wash bullet in the dock amendment above (the bar, the flush inset, the hover and the read-only rule all still stand).

**Accessibility.** The tint is the same colour the wash was, so the shipped text pairs and their ratios are unchanged; `tools/contrast-check.mjs` now labels it the card tint. Colour remains decorative: name, count, position and menu are the required signals.

**Ownership.** Ticket [240](240-group-color-anatomy-row-hover.md) owns the anatomy; [241](241-dock-group-color.md) owns the dock rendering; 238's combined matrix verifies both.



