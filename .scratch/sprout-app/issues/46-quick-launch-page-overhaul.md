# 46 — Quick Launch page overhaul

**What to build:** The Quick Launch page rebuilt on the app's standard header pattern and structured by virtual desktop. The header matches every other tab (title + actions row + one-line count sub); the search box on top filters the *added entries* only; one "Add" button reveals the panel that actually searches the PC for installed apps (plus pick-a-file and add-command). Entries are grouped into accordions — one per virtual desktop, "Current desktop" for unassigned — each with a count, using the shared disclosure component. Per-row clutter collapses into a single ⋯ menu (desktop assignment with check marks, "New desktop…", move up/down, remove).

**Blocked by:** 45 — Shared disclosure & select components, Advanced section fix

**Status:** done — synced to the share; manual pass pending a human

- [x] Header matches the existing tab pattern: title + actions row (Start = primary with play icon, Add = secondary with plus icon) + concise sub ("N entries. The tray starts them together, in one click.") replacing the three-button header and the two-sentence description
- [x] Top search filters the added entries client-side via the shared search input ("Filter Quick Launch…"); empty states and count text respect the filter
- [x] "Add" toggles a single inline panel containing the installed-apps search (results add an entry on pick), "Pick a file…", and "Add command…"; the installed-app search no longer sits on the page permanently
- [x] Entries render as accordion groups — "Current desktop" for unassigned, one per virtual desktop (stale ids read "Desktop ?"), each with its entry count; accordions expanded by default, collapsible via the shared disclosure pattern; tray desktop-group semantics mirrored
- [x] Each row carries one ⋯ menu: Desktop assignment list (Current desktop / each desktop / "New desktop…", check mark on the current one), Move up, Move down, Remove; the per-row icon buttons (desktop, up, down, trash) and the redundant desktop chip are gone
- [x] Desktop-assignment surface stays fully hidden below Windows 11 24H2; stale-assignment labels and confirm-remove flow unchanged
- [x] `npm run check` 0 errors (done: 0 errors / 0 warnings, 32 vitest tests pass, vite build clean, 220 cargo tests pass); manual pass (filter, add app/command/file, group accordions, assign to a desktop, move/remove) needs a human with the app; synced to the share (done)
  - ContextMenu grew `disabled` and `separator` support (skipped by keyboard nav); SearchInput takes an optional `ariaLabel`; IconButton forwards extra attributes (the ⋯ trigger needs `data-ctx-trigger`) and its onclick receives the MouseEvent.
