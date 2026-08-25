# 101 — Shared ContextMenu gains one-level submenus

**What to build:** Extend `ContextMenu.svelte` with optional nesting: a `ContextMenuItem` may carry `children: ContextMenuItem[]`, rendered as a flyout panel beside the parent row. Pointer: opens on hover, closes when the pointer leaves both the parent item and the flyout, or on outside click/Escape. Keyboard: ArrowRight or Enter opens and focuses the child list, ArrowLeft/Escape returns focus to the parent, ArrowUp/Down navigate children, Tab closes everything. ARIA: parent gets `aria-haspopup="true"` + `aria-expanded`, child container keeps the menu/menuitem pattern; disabled parents never open. Flyout positioning clamps to the viewport like the root menu (opens right, mirrors left near the edge). Separators/disabled skipping rules apply within children as at root. No builder changes yet — capability only, proven by the existing menus rendering unchanged.

**Blocked by:** none.

**Status:** done — synced to the share; hover-feel and screen-reader passes still pending a human

- [x] `ContextMenuItem.children?: ContextMenuItem[]` supported at arbitrary depth 1
- [x] Hover open/close with leave-grace so diagonal pointer travel doesn't slam shut
- [x] Full keyboard traversal per above; focus restore into root on Escape from child
- [x] `aria-haspopup`/`aria-expanded` on parents; child rows are real `menuitem`s
- [x] Viewport clamping incl. left-edge mirror; no clipped flyouts (`max-height` + inner scroll added for long child lists)
- [x] Existing six builder sites render byte-identically without using `children`
- [x] `npm.cmd run check` 0 errors/warnings; `vitest run` green
- [ ] Hands-on pass: hover grace feels right; screen reader announces submenu open/close

**Verification notes (2026-08-25):**

`ContextMenu.svelte` rewritten in place: roving-tabindex root preserved exactly (flat menus untouched), plus `openIndex`/`childActiveIndex` state, a fixed-positioned `.ctx-submenu` nested inside the root container so outside-click `contains()` keeps working, positioning effect that mirrors left near the right edge and clamps vertically, a 240 ms hover-close grace timer cancelled by entering either side, and keyboard routing — ArrowRight/Enter opens and focuses first focusable child, ArrowLeft/Escape returns focus to the parent row, Up/Down/Home/End operate inside the child list. Parent rows carry a trailing chevron-right glyph and never fire `onselect`. The owed first-hand Notion flyout citation is still open — fold it into ticket 106's menu work or 107's audit.

**First-hand citation owed:** while implementing, verify Notion's actual flyout mechanics (hover delay, keyboard model) against the live app/help and record findings as research 0006 pattern 10 amendments — cite what was observed, not assumed.
