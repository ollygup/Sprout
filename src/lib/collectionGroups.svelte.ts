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
  setGroupColor,
  unassignFromGroup,
  updateGroupsEnabled,
} from "./api";
import { createGroupCollapse } from "./groupCollapse.svelte";
import { normalizeGroupColor } from "./groupColor";
import type {
  ContextMenuItem,
  ContextMenuState,
} from "./components/ContextMenu.svelte";
import type { Group, GroupsCollection } from "./types";
import { t } from "./copy";

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
  host: CollectionGroupsHost;
}) {
  const { collection, host } = options;

  // The collection's plural noun for feedback copy, resolved per call so a
  // language switch never leaves a stale noun in a flash message.
  function groupNoun(): string {
    return t(
      collection === "launch"
        ? "groups.noun.entries"
        : collection === "action"
          ? "groups.noun.actions"
          : "collection.clips.many",
    );
  }

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

  // The color dialog's subject and field state (ticket 225). Swatch and
  // "No color" picks apply immediately; the custom field applies on submit.
  let coloring = $state<Group | null>(null);
  let colorDraft = $state("");
  let colorError = $state("");
  let savingColor = $state(false);

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
            ? t("groups.flashOn").replace("{noun}", groupNoun())
            : t("groups.flashOff"),
        );
      } catch (e) {
        console.error(e);
        enabled = !next;
        host.fail(t("groups.saveFail"));
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

    get coloring() {
      return coloring;
    },
    get colorDraft() {
      return colorDraft;
    },
    set colorDraft(value: string) {
      colorDraft = value;
    },
    get colorError() {
      return colorError;
    },
    get savingColor() {
      return savingColor;
    },

    openColor(group: Group) {
      colorDraft = group.color ?? "";
      colorError = "";
      coloring = group;
    },

    cancelColor() {
      coloring = null;
    },

    /** Applies one color (or clears it) and reloads. A custom value is
     *  validated here first so an invalid hex never reaches the backend and
     *  the field keeps its inline error. */
    async applyColor(value: string | null) {
      if (!coloring) return;
      const group = coloring;
      let normalized: string | null = null;
      if (value !== null) {
        normalized = normalizeGroupColor(value);
        if (!normalized) {
          colorError = t("groups.colorInvalid");
          return;
        }
      }
      savingColor = true;
      colorError = "";
      try {
        await setGroupColor(group.id, normalized);
        coloring = null;
        host.flash(
          (normalized ? t("groups.colorSet") : t("groups.colorClear")).replace(
            "{name}",
            group.name,
          ),
        );
        await host.reload();
      } catch (e) {
        console.error(e);
        colorError = String(e);
      } finally {
        savingColor = false;
      }
    },

    /** Create/rename share one dialog contract: an empty name is refused
     *  inline, a backend rejection lands in the same error slot, success
     *  closes the dialog and reloads through the host. */
    async submitName() {
      if (!naming) return;
      const name = nameDraft.trim();
      if (!name) {
        nameError = t("groups.nameEmpty");
        return;
      }
      savingName = true;
      nameError = "";
      try {
        if (naming.mode === "create") {
          const created = await createGroup(collection, name);
          if (naming.item) {
            await assignToGroup(collection, naming.item.id, created.id);
            host.flash(
              t("groups.movedToGroup")
                .replace("{item}", naming.item.label)
                .replace("{group}", created.name),
            );
          } else {
            host.flash(t("groups.created").replace("{name}", name));
          }
        } else {
          await renameGroup(naming.group.id, name);
          host.flash(t("groups.renamed").replace("{name}", name));
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
          t("groups.removed")
            .replace("{name}", group.name)
            .replace("{noun}", groupNoun()),
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
          host.flash(t("groups.movedToUngrouped").replace("{item}", itemLabel));
        } else {
          await assignToGroup(collection, item.id, groupId);
          const target = list.find((g) => g.id === groupId);
          host.flash(
            t("groups.movedToGroup")
              .replace("{item}", itemLabel)
              .replace("{group}", target?.name ?? t("groups.menuGroupFallback")),
          );
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
          label: t("menu.ungrouped"),
          checked: item.group_id === null,
          onselect: () => this.assign(item, label, null),
        },
        ...list.map((g) => ({
          label: g.name,
          checked: item.group_id === g.id,
          onselect: () => this.assign(item, label, g.id),
        })),
        {
          label: t("menu.newGroup"),
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
        label: t("groups.menuLabel").replace("{name}", group.name),
        anchor,
        focusFirst: viaKeyboard,
        returnTo: anchor,
        items: [
          {
            label: t("common.rename"),
            icon: "pencil",
            onselect: () => this.openRename(group),
          },
          {
            label: t("groups.colorMenu"),
            swatch: group.color,
            onselect: () => this.openColor(group),
          },
          {
            label: t("menu.moveUp"),
            icon: "chevron-up",
            disabled: index <= 0,
            onselect: () => this.reorderGroup(group.id, index - 1),
          },
          {
            label: t("menu.moveDown"),
            icon: "chevron-down",
            disabled: index >= list.length - 1,
            onselect: () => this.reorderGroup(group.id, index + 1),
          },
          { label: "", separator: true, onselect: () => {} },
          {
            label: t("common.remove"),
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
