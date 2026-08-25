/// Session-only collapse state for the Groups feature (tickets 89–93).
///
/// Every surface that renders Groups — the Quick Launch, Quick Actions, and
/// Clips pages plus the Quick Launch window's tabs — collapses group sections
/// the same way: collapsed ids last only for the session, sections default
/// expanded (accordions organize without hiding), and stale ids are pruned
/// after each load because SQLite can reuse a deleted group's id.

/** One collapse store per mounted surface. */
export function createGroupCollapse() {
  let collapsed = $state<Set<number>>(new Set());

  return {
    isOpen(groupId: number): boolean {
      return !collapsed.has(groupId);
    },

    toggle(groupId: number): void {
      const next = new Set(collapsed);
      if (next.has(groupId)) {
        next.delete(groupId);
      } else {
        next.add(groupId);
      }
      collapsed = next;
    },

    /** Group ids can be reused by SQLite after a delete — a stale collapse
     *  entry must never hide a future group. Call after every load with the
     *  ids of the groups that still exist. */
    prune(existingIds: number[]): void {
      collapsed = new Set(
        [...collapsed].filter((id) => existingIds.includes(id))
      );
    },
  };
}
