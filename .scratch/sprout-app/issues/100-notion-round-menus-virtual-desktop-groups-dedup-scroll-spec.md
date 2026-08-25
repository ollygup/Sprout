# 100 — UX round: context menus with submenus, virtual desktop without a switch, group create-by-assign, payload dedup, accordion alignment, mini-window scroll (spec)

**What to build:** One UX round that fixes six reported issues as a single coherent design move: the Quick Launch window's lists become fully reachable by scroll in both docked and floating forms; duplicate items are rejected per-list by payload identity; the shared accordion aligns its body with its title text and adopts Notion's caret disclosure; every context menu is rebuilt on one standard — frequent actions, organizational submenus (Move to group, Virtual desktop), reorder, then destructive last — with the shared menu component gaining real submenu capability; virtual-desktop assignment loses its master switch and activates with first use, Favorites-style; and groups are created from inside that same "Move to group" submenu, dissolving when empty, so the toolbar Add-group button disappears. Implemented via tickets 101–106, audited by 107.

**Blocked by:** none (round spec; implemented via tickets 101–106, audited by 107).

**Status:** ready-for-agent

## Problem Statement

Six defects and design debts compound on the same surfaces. The Quick Launch window's overflow cannot be reached when item lists exceed the visible height, in either docked or floating form. Clips, launch entries, and quick actions can each be added twice with no guard. The group accordion's body sits at an indent that matches neither the container edge nor the title-text start, and its disclosure icon points down when closed and up when open — inverted against every tree convention users know. Entry menus were loosely modeled on Notion's but flattened: organizational choices (group membership, desktop assignment) stack inline with verbs, mixing three action classes in one undifferentiated column. Desktop assignment carries a features-menu switch whose on-state changes nothing until entries are assigned one by one — a control that fails its own contract — while a separate toolbar button creates empty group placeholders before any content exists. And the feature is labeled "Desktop grouping", which names nothing.

## Solution

The Quick Launch window keeps its fixed palette geometry; every tab's list scrolls internally in both variants. Each list gains payload-identity deduplication: launch entries reject a second `(kind, target)`, quick actions a second `(command, cwd)` (both trimmed, case-insensitive), clips a second identical `content` (trimmed, case-sensitive) — names stay free everywhere, existing duplicates are grandfathered, bulk surfaces skip-and-report instead of erroring. `Disclosure` swaps to a filled caret (▸ closed → rotates 90° to ▾ open) and `GroupAccordion` indents body content to the title-text start, matching Notion toggle anatomy. `ContextMenu` grows one-level submenus (hover + keyboard flyouts), and all six builder sites adopt one ordering standard with checkmarks for current state and danger styling on Remove. Virtual-desktop assignment drops its switch entirely: wherever Windows supports it (24H2+), every entry menu offers the Virtual desktop submenu with an explicit "No assignment" escape item, activation is the first assignment, and opting out is per-entry — Notion's Favorites model, recorded in ADR-0015. Groups keep their opt-in toggles (structure vs annotation, research 0006 pattern 12), but creation moves into the submenu ("New group…" prompt dialog fuses naming with assigning), and a group exists only while at least one member belongs — empties dissolve server-side.

## User Stories

**Mini window scroll**

1. As a user of the floating Quick Launch window, I want to reach every listed item even when more exist than fit in the palette, so nothing hides below the fold.
2. As a user of the docked strip, I want the same complete reachability, so docking never costs me content.
3. As a user, I want size, positioning, and docking behavior unchanged, so scrolling is the only thing that moves.

**Duplicate prevention**

4. As a user adding a launch entry for an app already listed, I want the save blocked with an inline message, so my list stays one-entry-per-app.
5. As a user writing a quick action whose command and working directory already exist, I want that flagged at save, so I don't accumulate twin actions under different names.
6. As a user pasting clip text already stored, I want the duplicate refused inline, so clips stay unique by content.
7. As a user editing an item, I want the same check applied excluding the item itself, so renaming without changing payload always succeeds.
8. As a user importing a backup or re-adding from installed-app search, I want existing items skipped with a count rather than erroring, so bulk flows stay friction-free.
9. As a user with pre-existing duplicates, I want them left alone, so upgrades never mutate my data behind my back.

**Accordion alignment & disclosure icon**

10. As a user scanning grouped lists, I want section content aligned with its group title's text start, so children visibly belong to their header.
11. As a user, I want the disclosure control to read as a caret — right when closed, down when open — matching Notion and every native tree, so expand/collapse needs no relearning.
12. As a user, I want the change applied everywhere sections appear (pages, window tabs, composer rows, form sections), so one interaction is learned once.
13. As a user of the Quick Launch window's flush layout, I want it left flush as today, so the narrow strip keeps its density.

**Context menus**

14. As a user, I want organizational choices gathered into submenus ("Move to group", "Virtual desktop"), so verbs and destinations stop competing in one column.
15. As a keyboard user, I want to open a submenu with ArrowRight/Enter and leave with Escape/ArrowLeft, with focus announced correctly, so menus are fully operable without a pointer.
16. As a pointer user, I want submenus to open on hover and close when I leave both menu levels, so navigation feels native.
17. As a user, I want actions ordered frequent → organizational → reorder → Remove last after a separator, so destruction is never adjacent to everyday verbs.
18. As a user, I want checkmarks marking the current group and desktop, so state reads without opening anything.
19. As a user, I want icons kept on menu items (matching current-Notion convention and app idiom), with danger color reserved for destructive rows.

**Virtual desktop assignment**

20. As a user on Windows 11 24H2+, I want desktop assignment offered directly in each entry's menu without enabling anything first, so capability appears at the moment of use.
21. As a user who assigned entries, I want launches honored and badges shown wherever supported, so assignments are visible facts, not settings.
22. As a user who changed my mind about one entry, I want "No assignment" at the top of the submenu, so opting out is one click per entry.
23. As a user who changed my mind about all of them, I accept unassigning per entry instead of a master switch, because reassignment is cheap and a switch that does nothing until content exists is noise (ADR-0015).
24. As a user below the 24H2 gate, I want no trace of the feature anywhere, as today.
25. As a user, I want the vocabulary "Virtual desktop" throughout — menu, badges, dialogs — with "Desktop grouping" gone entirely.

**Groups create-by-assign**

26. As a user organizing a list, I want "New group…" inside the Move-to-group submenu, so naming a bucket and placing an item into it are one gesture.
27. As a new user, I want no Add-group button on the toolbar, so structure never appears before content justifies it.
28. As a user moving the last item out of a group, I accept the group dissolving with its name and order, since empty placeholders have no owner to serve.
29. As a user deleting a populated group from its header menu, I want members returned ungrouped as today, so explicit deletion stays non-destructive to items.
30. As a user with Groups toggled off, I want stored groups untouched but invisible, exactly as the toggle promises today.

**Docs**

31. As a future session, I want research 0006 extended (caret anatomy, menu structure, Favorites-style activation, annotation-vs-structure rule) and ADR-0015 written, so these decisions cite evidence permanently.

## Implementation Decisions

- **Scroll fix**: root-cause first (suspects: Tabs panel `overflow:hidden` chain at `quick-launch-window/+page.svelte:1227–1245`, docked-sliver geometry); fix inside `.qlw__list` flex chain; window geometry constants in `constants/window.rs` untouched.
- **Dedup**: backend validators return typed errors (`launch.rs` `validate_launch_entry`, `quick_actions.rs` `validate_quick_action`, `clips.rs` `validate_clip` gain duplicate-payload checks against the DB, excluding self on update); dialogs render the error inline and stay interactive (the ticket-28 lesson); backup import keeps skip-and-count; installed-app search / file-picker add paths skip silently with a notice; no schema migration, no cleanup of existing duplicates.
- **Disclosure**: swap the `chevron` icon for a filled triangle glyph rotating 90° open (transform-only transition preserved, reduced-motion collapse preserved); labeled and icon-only modes both update via the single component.
- **Alignment**: `GroupAccordion` body indent becomes icon column + gap (title-text start); `flush` variant stays 0.
- **Submenus**: `ContextMenuItem` gains optional `children`; flyout opens on hover or ArrowRight, closes on Escape/ArrowLeft/outside click/disabled parent; `role="menu"`/`menuitem` semantics with `aria-haspopup`/`aria-expanded`; positioning clamped like the root menu; disabled parents unflyable.
- **Menu standard**: builders ordered primary/topical → organizational submenus → Move up/Move down (disabled at ends) → separator → Remove (danger). Checkmarks mark current group/desktop. Icons retained per item.
- **Virtual desktop**: remove the features-menu switch row and all "Desktop grouping" strings; gates become `desktopSupported && (assignments exist || menu open)` for surfaces, plain `desktopSupported` for the submenu; submenu = No assignment ✓ | Current desktop | Desktop N… | New desktop…; row badges show whenever supported && assigned; the `desktop_assignments` setting key stops being read/written (stale value left harmlessly in DB, no migration).
- **Groups**: group-menu Rename/Move up/Move down/Remove unchanged; creation moves into the entry submenu via a small name prompt dialog (non-blank validation) that creates-and-assigns in one call; toolbar Add-group button removed from the library page; backend sweeps empty groups after any unassign/delete transaction (`DELETE … WHERE NOT EXISTS member`) across all three collections; ordering of surviving groups preserved.
- **Glossary/docs**: done this session — CONTEXT.md Virtual desktop term + Launch entry rewrite + Group existence rule; ADR-0015; research 0006 patterns 9–12 + supersessions; research 0008 applied-history supersession.

## Testing Decisions

- Backend behavior tests against temp databases: dedup validators reject exact payloads (trimmed/case variants) and pass distinct ones, self-exclusion on update works; empty-group sweep fires on unassign/delete and preserves survivor order; launch honoring of assignments unaffected by switch removal (feature now unconditional where supported).
- Frontend verification at established seams: `svelte-check` zero errors/warnings, `vitest run` green, manual passes over library page, quick-actions, clips, and the window/dock in light+dark — covering scroll reachability in both variants, submenu hover + full keyboard traversal, caret states, badge visibility, and New-group dialog flow.
- Audit ticket 107 applies the web-interface-guidelines review over every changed component.

## Out of Scope

- Removing the Groups opt-in switches (deliberate divergence — structure keeps a switch).
- Drag-and-drop reordering anywhere; nested submenus deeper than one level; bulk unassignment tools.
- Resizing or growing the floating mini window.
- Any migration touching stored duplicates, stale `desktop_assignments` values, or ticket history (tickets 44/85/88 remain as written).

## Further Notes

- Evidence base: research 0006 patterns 9–12 (caret anatomy; menu structure incl. the corrected icons finding; Favorites content-gating verified first-hand; annotation-vs-structure rule), research 0008 rules 1–5 plus its new supersession line, ADR-0015. Session decisions Q1–Q21 of the grilling transcript are consolidated here; disagreements resolved: icons stay (user correction verified), dedup keys payload not names, VD loses its switch with evidence.
- The old v1 spec moved to `00-sprout-app-spec.md` this session; `docs/specs/` retired — round specs live only in this tracker.
