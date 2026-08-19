# 12 — Products page rebuild with context menus

**What to build:** The Products page (renamed from Library) redesigned in the seed-catalog identity: packet-style cards (two-tone header band, display-type name, counts, quiet body) with no icon-button clutter. A shared context-menu system — right-click or a quiet hover-revealed `⋯` affordance open the same menu (Edit · More info · Remove), with full keyboard support: Tab focuses the card, Enter/Space opens the menu, arrows navigate, Escape closes. Empty states, search no-match states, and loading copy rewritten as invitations; loading text is dynamic (rotating phrases, never repeated in a session). Remove moves into the context menu with its confirm dialog.

**Blocked by:** 11 — App shell and design foundation

**Status:** done

- [x] Product cards render in seed-packet style with no visible icon buttons at rest; Edit/More info/Remove only via context menu or `⋯`
- [x] Right-click and `⋯` open the same context menu (`aria-haspopup`); menu keyboard-operable (Tab to card, Enter/Space opens, arrows navigate, Escape closes); menu never steals focus on open
- [x] Remove requires confirmation, then removes the product and flashes confirmation
- [x] Search works as before; empty state, no-match state, and subline rewritten as invitations; loading messages rotate dynamically
- [x] "Custom install step" language replaces "no winget id" negative framing on cards
- [x] Page error state shows what happened + next step, never a raw backend string
- [x] `npm run check` 0 errors, `cargo test` green, `cargo check` clean, `npm run build` ok