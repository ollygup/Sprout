# 106 — Menu standard rollout: submenus, group create-by-assign, empty-group sweep

**What to build:** Rebuild all six menu builder sites on the round's ordering standard — primary/topical actions → organizational submenus (**Move to group** [Ungrouped ✓ | groups… | New group…], **Virtual desktop** per ticket 105) → Move up/Move down (disabled at ends) → separator → Remove (danger last); icons retained, checkmarks for current state. Library page additionally: remove the toolbar Add-group button and its flow; "New group…" opens a small prompt dialog (non-blank name validation) that creates-and-assigns in one step; the inline groups/desktops stacks in `items` (:~455–528) collapse into the two flyouts. Quick-actions/clips/group-header menus adopt the same skeleton (group submenu replacing their inline Ungrouped/groups block). Backend: after any unassign or delete transaction, sweep now-empty groups across all three collections (`DELETE FROM groups WHERE NOT EXISTS member`, preserving survivor order) — a group exists only while it has members.

**Blocked by:** 101 (submenu capability), 103 (dedup touches the same validators/dialogs this rebuild interacts with), 105 (desktop submenu lands first so the library rebuild happens once).

**Status:** done — code synced; keyboard-only + hover hands-on pass still owed a human

- [x] All six builders follow the standard; Remove always last behind a separator
- [x] "Move to group" flyout with Ungrouped checkmark, group list, New group… everywhere groups exist
- [x] Library toolbar Add-group button gone; New-group prompt dialog validates non-blank names
- [x] Create-and-assign is one gesture; new groups appear in correct position order
- [x] **Group names exclusive within their collection** (2026-08-25 ruling): `create_group`/`rename_group` reject a sibling whose trimmed name matches case-insensitively (excluding self); the New-group and rename dialogs render it inline; uniqueness binds to live rows only — deleting a group or dissolving it via empty-sweep frees the name for immediate reuse. Evidence: WCAG 2.5.3 Label-in-Name intent (visible labels are speech commands — duplicate destinations break voice targeting), ux.stackexchange #149205, app precedent (Products ticket 28, preset fork/import rejection)
- [x] Last member leaving dissolves the group on every path (unassign, delete item, explicit group delete keeps its return-to-ungrouped semantics)
- [x] Survivor group order preserved after sweeps; window tabs reflect changes live
- [ ] Keyboard-only pass: reach both flyouts, assign, create group, escape cleanly
- [x] cargo test covers sweep behavior ×3 collections + exclusivity (dup create rejected across case; rename onto sibling rejected; rename-to-self ok; delete-then-recreate allowed)
- [x] CONTEXT.md Group term gains the uniqueness clause when this lands (with implementation, per #85's glossary precedent)

**Backend note:** `groups.rs` `validate_group_name` stays blank-only; add a `colliding_group(conn, collection, name, except_id)` beside it — same placement discipline as ticket 103's collision checks (backup import validates through `validate_*` and must keep skip semantics).; vitest/check green

**Verification notes (2026-08-25):**

Frontend: all six builders now emit the standard ordering — primary/topical
verbs first, organizational submenus next (**Move to group**, and on the
library page **Virtual desktop** per ticket 105), Move up/Move down disabled
at their ends, separator, Remove danger-last. The three list pages share one
`moveToGroupChildren` builder on the groups manager (Ungrouped ✓ | groups in
user order | New group…), so the flyout can't drift between them; it renders
whenever Groups is enabled — even before a first group exists, since creation
is now assignment-born. "New group…" opens the shared GroupNameDialog via
`openCreateFor(item)`, and `submitName` creates then assigns in one gesture
(`“item” moved to <name>.`); blank names are refused inline as before and
backend exclusivity rejections land in the same error slot. The toolbar
Add-group buttons are gone from the library page per the ticket.

Two scope extensions beyond the ticket's literal text, flagged for review:

1. **Add-group buttons removed from Quick Actions and Clips too.** With the
   dissolve invariant ("a group exists only while it has members") a
   memberless group no longer survives: any later unassign/assign/delete
   transaction sweeps it server-side, so a toolbar-created empty group would
   silently vanish. Creation is therefore assignment-born on every page.
2. **The sweep also runs after assign transactions**, not only unassign/
   delete: a reassignment can strand the *previous* group empty
   (`groups.rs::sweep_empty_groups` runs inside `unassign_item`,
   `assign_item`, `delete_group`, and `OrderedList::delete` so item deletes
   sweep too). Survivor positions are untouched, preserving user order;
   window tabs stay live because every touched command already emits
   `quick-launch-changed`.

Exclusivity lives beside `validate_group_name` as specified:
`colliding_group` compares trimmed, lowercased names within the collection,
excluding self on rename (`rename-to-self` passes). Authored rejection copy:
"A group named 'X' already exists in {collection}." Backup import is
untouched (groups are machine-local and never exported).

Gates: `cargo test` 371 passed / 0 failed / 1 ignored (live probe),
`npm.cmd run check` 0 errors / 0 warnings, `vitest run` 36 passed,
`npm.cmd run build` clean. CONTEXT.md's Group term now carries the
uniqueness clause.
