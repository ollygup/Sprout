/// The Groups feature's logic, owned once (ticket 95): every collection page
/// (Quick Launch, Quick Actions, Quick Clips) instantiates one manager with
/// its own `GroupsCollection` key and noun, so the feature can no longer
/// drift between three hand-copied implementations — the same extraction
/// class as the run-state store (ticket 98) and the shared accordion (97).
/// Chrome stays with the page: where the switch sits (research 0008), how
/// rows and menus render. Off stays fully dormant — stored groups are never
/// shown or touched until the flag turns on.

import {
  assignToGroup,
  createGroup,
  deleteGroup,
  listGroups,
  moveGroup,
  renameGroup,
  unassignFromGroup,
  updateGroupsEnabled,
} from "./api";
import { createGroupCollapse } from "./groupCollapse.svelte";
import type {
  ContextMenuItem,
  ContextMenuState,
} from "./components/ContextMenu.svelte";
import type { Group, GroupsCollection } from "./types";

/** The naming dialog's subject: a fresh group (optionally carrying the item
 *  being assigned into it — ticket 106's create-and-assign) or one being
 *  renamed. */
export type GroupNaming =
  | { mode: "create"; item?: { label: string; id: number } }
  | { mode: "rename"; group: Group };

/** Everything the host surface contributes: its feedback channels and its
 *  reload. Mutations begin by clearing the error line and marking the page
 *  busy — exactly what the three former copies did inline. */
export interface CollectionGroupsHost {
  /** Begins a mutation round-trip: clear errors, disable the page's busy-gated controls. */
  begin(): void;
  /** Releases the page's busy flag; failures have already been surfaced via `fail`. */
  end(): void;
  /** A transient ok notice through the page's own flash channel. */
  flash(message: string): void;
  /** An error through the page's own error line — mutations are never silent. */
  fail(message: string): void;
  /** Re-fetches the page's data after any mutation that changed membership or order. */
  reload(): Promise<void>;
}

export function createCollectionGroups(options: {
  collection: GroupsCollection;
  /** This collection's plural noun for feedback copy ("entries", "actions", "clips"). */
  noun: string;
  host: CollectionGroupsHost;
}) {
  const { collection, noun, host } = options;

  let enabled = $state(false);
  let list = $state<Group[]>([]);
  const collapse = createGroupCollapse();

  let naming = $state<GroupNaming | null>(null);
  let nameDraft = $state("");
  let nameError = $state("");
  let savingName = $state(false);

  // Set only while the remove confirmation is up; nulled before the delete
  // round-trips so a slow double-confirm cannot fire twice.
  let removing = $state<Group | null>(null);

  async function guarded(run: () => Promise<void>) {
    host.begin();
    try {
      await run();
    } catch (e) {
      console.error(e);
      host.fail(String(e));
    } finally {
      host.end();
    }
  }

  return {
    get enabled() {
      return enabled;
    },

    get groups() {
      return list;
    },

    /** Sections exist only once at least one group does (absent-until-content,
     *  research 0004 rule 2). */
    get grouped() {
      return enabled && list.length > 0;
    },

    get collapse() {
      return collapse;
    },

    /** Applies the persisted flag from a Settings read — no save, no flash. */
    setEnabledFromSettings(on: boolean) {
      enabled = on;
    },

    /** The feature switch behind the page-features menu (research 0008):
     *  optimistic, reverted when the save fails. */
    async toggle() {
      const next = !enabled;
      enabled = next;
      try {
        await updateGroupsEnabled(collection, next);
        host.flash(
          next
            ? `Groups on — organize ${noun} into named sections.`
            : "Groups off — groups and assignments are kept but hidden."
        );
      } catch (e) {
        console.error(e);
        enabled = !next;
        host.fail("Couldn't save the Groups setting — try again.");
      }
    },

    /** Loads this collection's groups in user order and prunes stale
     *  collapse ids — call inside the page's load `Promise.all` so both
     *  fetches stay parallel. */
    async refresh() {
      const gs = await listGroups(collection);
      list = gs;
      collapse.prune(gs.map((g) => g.id));
    },

    get naming() {
      return naming;
    },
    get nameDraft() {
      return nameDraft;
    },
    set nameDraft(value: string) {
      nameDraft = value;
    },
    get nameError() {
      return nameError;
    },
    get savingName() {
      return savingName;
    },

    /** Ticket 106's create-and-assign: the "New group…" flyout item names
     *  the group and lands the triggering row in it as one gesture. A group
     *  exists only while it has members, so there is no memberless create. */
    openCreateFor(item: { id: number }, label: string) {
      nameDraft = "";
      nameError = "";
      naming = { mode: "create", item: { id: item.id, label } };
    },

    openRename(group: Group) {
      nameDraft = group.name;
      nameError = "";
      naming = { mode: "rename", group };
    },

    cancelNaming() {
      naming = null;
    },

    /** Create/rename share one dialog contract: an empty name is refused
     *  inline, a backend rejection lands in the same error slot, success
     *  closes the dialog and reloads through the host. */
    async submitName() {
      if (!naming) return;
      const name = nameDraft.trim();
      if (!name) {
        nameError = "Group name must not be empty.";
        return;
      }
      savingName = true;
      nameError = "";
      try {
        if (naming.mode === "create") {
          const created = await createGroup(collection, name);
          if (naming.item) {
            await assignToGroup(collection, naming.item.id, created.id);
            host.flash(`“${naming.item.label}” moved to ${created.name}.`);
          } else {
            host.flash(`Group “${name}” created.`);
          }
        } else {
          await renameGroup(naming.group.id, name);
          host.flash(`Group renamed to “${name}”.`);
        }
        naming = null;
        await host.reload();
      } catch (e) {
        console.error(e);
        nameError = String(e);
      } finally {
        savingName = false;
      }
    },

    get removing() {
      return removing;
    },

    requestRemove(group: Group) {
      removing = group;
    },

    cancelRemove() {
      removing = null;
    },

    /** Deleting a group returns its members to ungrouped — it never deletes
     *  them (ticket 89). */
    async removeGroup() {
      if (!removing) return;
      const group = removing;
      removing = null;
      await guarded(async () => {
        await deleteGroup(group.id);
        host.flash(
          `Group “${group.name}” removed — its ${noun} are back in the ungrouped list.`
        );
        await host.reload();
      });
    },

    /** Assign/unassign through the data layer, which refuses cross-collection
     *  attempts outright. */
    assign(
      item: { id: number },
      itemLabel: string,
      groupId: number | null
    ) {
      return guarded(async () => {
        if (groupId === null) {
          await unassignFromGroup(collection, item.id);
          host.flash(`${itemLabel} moved to the ungrouped list.`);
        } else {
          await assignToGroup(collection, item.id, groupId);
          const target = list.find((g) => g.id === groupId);
          host.flash(`${itemLabel} moved to ${target?.name ?? "the group"}.`);
        }
        await host.reload();
      });
    },

    reorderGroup(id: number, toPosition: number) {
      return guarded(() => moveGroup(id, toPosition).then(host.reload));
    },

    /** The "Move to group" flyout (ticket 106): Ungrouped ✓ | groups in user
     *  order | "New group…" — create-and-assign in one gesture (research 0006
     *  pattern 10's create-and-place fusion). Checkmarks mark the item's
     *  current membership. */
    moveToGroupChildren(
      item: { id: number; group_id: number | null },
      label: string
    ): ContextMenuItem[] {
      return [
        {
          label: "Ungrouped",
          checked: item.group_id === null,
          onselect: () => this.assign(item, label, null),
        },
        ...list.map((g) => ({
          label: g.name,
          checked: item.group_id === g.id,
          onselect: () => this.assign(item, label, g.id),
        })),
        {
          label: "New group…",
          icon: "plus",
          onselect: () => this.openCreateFor(item, label),
        },
      ];
    },

    /** One ⋯ menu per group header, on the round's ordering standard
     *  (ticket 106): Rename, order, separator, Remove danger-last. The page
     *  owns the toggle-off check against its own menu state. */
    groupMenu(
      group: Group,
      anchor: HTMLButtonElement,
      viaKeyboard: boolean
    ): ContextMenuState & { groupId: number } {
      const index = list.indexOf(group);
      return {
        groupId: group.id,
        open: true,
        label: `Actions for group ${group.name}`,
        anchor,
        focusFirst: viaKeyboard,
        returnTo: anchor,
        items: [
          {
            label: "Rename",
            icon: "pencil",
            onselect: () => this.openRename(group),
          },
          {
            label: "Move up",
            icon: "chevron-up",
            disabled: index <= 0,
            onselect: () => this.reorderGroup(group.id, index - 1),
          },
          {
            label: "Move down",
            icon: "chevron-down",
            disabled: index >= list.length - 1,
            onselect: () => this.reorderGroup(group.id, index + 1),
          },
          { label: "", separator: true, onselect: () => {} },
          {
            label: "Remove",
            icon: "trash",
            danger: true,
            onselect: () => (removing = group),
          },
        ],
      };
    },
  };
}

/** The ungrouped-first view every grouped list renders: plain rows first,
 *  then one entry per group in user order. While a filter is active, empty
 *  sections drop out so no section header floats over nothing. */
export function groupView<T extends { group_id: number | null }>(
  groups: Group[],
  items: T[],
  matches: (item: T) => boolean,
  filtering: boolean
): { ungrouped: T[]; sections: { group: Group; rows: T[] }[] } {
  return {
    ungrouped: items.filter((i) => i.group_id === null && matches(i)),
    sections: groups
      .map((g) => ({
        group: g,
        rows: items.filter((i) => i.group_id === g.id && matches(i)),
      }))
      .filter((s) => !filtering || s.rows.length > 0),
  };
}

/** A group's total membership — count badges keep showing it even while a
 *  filter hides some of the section's rows. */
export function countMembers<T extends { group_id: number | null }>(
  items: T[],
  groupId: number
): number {
  return items.filter((i) => i.group_id === groupId).length;
}
