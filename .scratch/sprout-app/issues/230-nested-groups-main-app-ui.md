# 230 — Nested groups main-app UI

**Parent:** [221 — Collections & media (spec)](221-collections-media-spec.md)

**What to build:** The three main-app collections render two-level groups: a parent accordion with its direct items and indented child accordions. Child groups collapse independently, the parent collapses the whole subtree, counts reflect the subtree, and the ⋯ menu gains a one-level "Move into…" path so nesting is fully keyboard-operable. Deleting a parent explains that its subtree is ungrouped and every item stays.

**Blocked by:** 229 — Nested groups backend.

**Status:** ready-for-agent

- [ ] The main app renders parent accordion → direct items → indented child accordions. Child groups are independently collapsible and default expanded; parent collapse hides the entire subtree.
- [ ] Parent count badge = total items in the subtree; child badge = its own items. Counts update after assign/unassign/delete.
- [ ] Group menus are available at both levels. "Move into…" lists eligible top-level destinations for an item-only top-level group; for a child it lists "Top level" plus the other top-level groups. A group with children has no "Move into…". The menu stays one submenu level.
- [ ] Create defaults to top level. Rename/move errors (including destination name collisions) are shown inline and do not close the dialog/menu prematurely.
- [ ] The delete confirmation states that all items in the subtree stay and the subgroup structure is removed; deleting a child states its items return to ungrouped.
- [ ] While filtering, a parent stays if it or any descendant has a matching item; empty children drop; a parent whose only matches are in a child shows that child.
- [ ] Groups off stays fully dormant; re-enabling restores the tree. Child colors are independent of the parent.
- [ ] Pure tests cover tree derivation, subtree counts, and filter visibility. Component-level tests cover the Move into… menu and delete copy where the project already tests components. Manual evidence covers all three collections, flat/nested, and light/dark.
- [ ] `npm run check`, `npm run test`, `node tools/contrast-check.mjs`, and `node tools/ownership-gate.mjs` pass.

**Claims (recheck at dispatch):** `collectionGroups.svelte.ts`; `GroupAccordion`; the three main pages; group menu/dialogs; i18n. **Explicitly not built:** dock rendering (231); drag (232/233); group backup; a third level; color inheritance.
