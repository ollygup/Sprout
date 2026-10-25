//! Groups foundation (ticket 89): user-named buckets scoped per collection,
//! shared by the three machine-local lists — Launch entries, Quick Actions,
//! and Clips.
//!
//! Glossary (docs/CONTEXT.md): a **Group** is a user-named bucket within
//! exactly one collection. One table stores every collection's groups, each
//! row carrying a `collection` discriminator (`launch` / `action` /
//! `clip`), so namespaces are isolated at the data layer: a Quick Action
//! group accepts only Quick Actions, and likewise for Clips and Launch
//! entries (spec decision, ticket 85). Items hold a nullable `group_id`
//! column — at most one group per item — and deleting a group returns its
//! members to ungrouped instead of deleting them.
//!
//! Groups nest exactly two levels (spec 221 §C): a top-level row may hold
//! items and child groups, a child may hold only items. Names are exclusive
//! among siblings — `(collection, parent_id)` — so two parents may reuse a
//! child name. A group exists while it has a direct item or a child group;
//! the last one leaving dissolves it bottom-up. Deleting a parent ungroups
//! every item in its subtree and deletes only the structure.
//!
//! Groups are machine-local structure like desktop assignments (ticket 88):
//! never part of Presets, Plan, Run, or exports. `parent_id` is structural,
//! so it would ride along when groups enter the backup document (ADR-0026).

use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension, Result};
use serde::{Deserialize, Serialize};

use crate::ordered_list::OrderedList;

/// Which collection a Group lives in — the discriminator that keeps the
/// three namespaces apart at the data layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Collection {
    Launch,
    Action,
    Clip,
}

impl Collection {
    /// The item table whose rows this collection groups.
    pub fn table(self) -> &'static str {
        match self {
            Collection::Launch => "launch_entries",
            Collection::Action => "quick_actions",
            Collection::Clip => "clips",
        }
    }

    /// The display noun for user-facing messages.
    pub fn label(self) -> &'static str {
        match self {
            Collection::Launch => "Quick Launch",
            Collection::Action => "Quick Actions",
            Collection::Clip => "Quick Clips",
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Collection::Launch => "launch",
            Collection::Action => "action",
            Collection::Clip => "clip",
        }
    }

    pub(crate) fn from_str(value: &str) -> Option<Self> {
        match value {
            "launch" => Some(Collection::Launch),
            "action" => Some(Collection::Action),
            "clip" => Some(Collection::Clip),
            _ => None,
        }
    }

    /// The position discipline this collection's rows share.
    fn ordered_list(self) -> OrderedList {
        match self {
            Collection::Launch => OrderedList::LAUNCH_ENTRIES,
            Collection::Action => OrderedList::QUICK_ACTIONS,
            Collection::Clip => OrderedList::CLIPS,
        }
    }
}

/// A Group as stored: its collection namespace, its name, its optional
/// decorative color, and its parent. Position is sibling-scoped and internal
/// (never part of the payload) — reorders go through `move_group`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    pub id: i64,
    pub collection: Collection,
    pub name: String,
    /// `#rrggbb`, normalized lowercase, or `None` for neutral. Decorative
    /// only: never identity, ordering, dissolve, or backup (ADR-0026).
    pub color: Option<String>,
    /// The top-level group this one nests under; `None` = top level. A child
    /// never has children — the backend enforces depth 2 on every write.
    pub parent_id: Option<i64>,
}

const SELECT_GROUP_SQL: &str = "SELECT id, collection, name, color, parent_id FROM groups";

fn group_from_row(row: &rusqlite::Row) -> Result<Group> {
    let collection: String = row.get(1)?;
    Ok(Group {
        id: row.get(0)?,
        collection: Collection::from_str(&collection).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                1,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::other(format!(
                    "unknown group collection '{collection}'"
                ))),
            )
        })?,
        name: row.get(2)?,
        color: row.get(3)?,
        parent_id: row.get(4)?,
    })
}

/// Every Group of one collection as a flat tree: each top-level group in
/// sibling order, immediately followed by its own children in their sibling
/// order. The frontend rebuilds the two levels from `parent_id`.
pub fn list_groups(conn: &Connection, collection: Collection) -> Result<Vec<Group>> {
    let mut stmt = conn.prepare(
        "SELECT g.id, g.collection, g.name, g.color, g.parent_id
         FROM groups g
         LEFT JOIN groups parent ON parent.id = g.parent_id
         WHERE g.collection = ?1
         ORDER BY COALESCE(parent.position, g.position),
                  (g.parent_id IS NOT NULL),
                  g.position,
                  g.id",
    )?;
    let rows = stmt.query_map(params![collection.as_str()], group_from_row)?;
    rows.collect()
}

fn get_group(conn: &Connection, id: i64) -> Result<Option<Group>> {
    conn.query_row(
        &format!("{SELECT_GROUP_SQL} WHERE id = ?1"),
        params![id],
        group_from_row,
    )
    .optional()
}

/// [`get_group`] plus its internal position — delete/reorder need it.
fn get_group_at(conn: &Connection, id: i64) -> Result<Option<(Group, i64)>> {
    conn.query_row(
        "SELECT id, collection, name, color, parent_id, position FROM groups WHERE id = ?1",
        params![id],
        |row| Ok((group_from_row(row)?, row.get::<_, i64>(5)?)),
    )
    .optional()
}

/// Rejects names that could never serve as a bucket label.
pub fn validate_group_name(name: &str) -> std::result::Result<(), String> {
    if name.trim().is_empty() {
        return Err("Group name must not be empty".into());
    }
    Ok(())
}

/// Whether a sibling — same collection and same parent — already carries this
/// name, compared trimmed and case-insensitively. `except_id` excludes the
/// group being renamed or moved. Uniqueness binds to live rows only, so a
/// deleted or dissolved group frees its name immediately. `parent_id IS ?2`
/// matches NULL as NULL, keeping top-level names in their own set.
fn colliding_group(
    conn: &Connection,
    collection: Collection,
    parent_id: Option<i64>,
    name: &str,
    except_id: Option<i64>,
) -> Result<bool> {
    let needle = name.trim().to_lowercase();
    let mut stmt = conn.prepare(&format!(
        "{SELECT_GROUP_SQL} WHERE collection = ?1 AND parent_id IS ?2"
    ))?;
    let rows = stmt.query_map(params![collection.as_str(), parent_id], group_from_row)?;
    for sibling in rows {
        let sibling = sibling?;
        if Some(sibling.id) != except_id && sibling.name.trim().to_lowercase() == needle {
            return Ok(true);
        }
    }
    Ok(false)
}

fn collision_error(collection: Collection, name: &str, parent_id: Option<i64>) -> String {
    if parent_id.is_some() {
        format!(
            "A subgroup named '{}' already exists there — pick another name.",
            name.trim()
        )
    } else {
        format!(
            "A group named '{}' already exists in {} — pick another name.",
            name.trim(),
            collection.label()
        )
    }
}

/// Appends a top-level Group at the end of its collection's order (the next
/// free position among that collection's top-level groups — namespaces never
/// share an ordering). The trimmed name must be unique among top-level
/// siblings, case-insensitively. Children are only created by `move_group`.
pub fn create_group(
    conn: &Connection,
    collection: Collection,
    name: &str,
) -> std::result::Result<Group, String> {
    validate_group_name(name)?;
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    if colliding_group(&tx, collection, None, name, None).map_err(|e| e.to_string())? {
        return Err(collision_error(collection, name, None));
    }
    let position: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM groups
              WHERE collection = ?1 AND parent_id IS NULL",
            params![collection.as_str()],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO groups (collection, name, position) VALUES (?1, ?2, ?3)",
        params![collection.as_str(), name.trim(), position],
    )
    .map_err(|e| e.to_string())?;
    let id = tx.last_insert_rowid();
    tx.commit().map_err(|e| e.to_string())?;
    Ok(get_group(conn, id)
        .map_err(|e| e.to_string())?
        .expect("just inserted"))
}

/// Renames a Group in place; position, parent, and membership are untouched.
/// The trimmed name must stay unique among siblings — the same collection and
/// parent — case-insensitively, excluding the group itself.
pub fn rename_group(conn: &Connection, id: i64, name: &str) -> std::result::Result<(), String> {
    validate_group_name(name)?;
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let group = get_group(&tx, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "This group no longer exists — refresh and try again.".to_string())?;
    if colliding_group(&tx, group.collection, group.parent_id, name, Some(id))
        .map_err(|e| e.to_string())?
    {
        return Err(collision_error(group.collection, name, group.parent_id));
    }
    tx.execute(
        "UPDATE groups SET name = ?1 WHERE id = ?2",
        params![name.trim(), id],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

/// Accepts only a `#RRGGBB` value (no alpha, shorthand, or named/rgb/hsl
/// forms) and normalizes it to lowercase. The UI palette is just a set of
/// valid hex values, so adding a hue later needs no migration.
pub fn validate_group_color(color: &str) -> std::result::Result<String, String> {
    let trimmed = color.trim();
    let is_hex = trimmed.len() == 7
        && trimmed.starts_with('#')
        && trimmed[1..].chars().all(|c| c.is_ascii_hexdigit());
    if !is_hex {
        return Err("Group color must be a #RRGGBB hex value, like #1a7f64.".into());
    }
    Ok(trimmed.to_ascii_lowercase())
}

/// Writes one group's decorative color; `None` returns it to neutral. The
/// value never touches identity, order, membership, or dissolve — only the
/// stored presentation field.
pub fn set_group_color(
    conn: &Connection,
    id: i64,
    color: Option<&str>,
) -> std::result::Result<(), String> {
    let normalized = match color {
        Some(value) => Some(validate_group_color(value)?),
        None => None,
    };
    let changed = conn
        .execute(
            "UPDATE groups SET color = ?1 WHERE id = ?2",
            params![normalized, id],
        )
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err("This group no longer exists — refresh and try again.".into());
    }
    Ok(())
}

/// Dissolves every group across all three collections that has no direct item
/// and no child group. Runs inside the caller's transaction; survivor
/// positions are untouched, so the user's order is preserved (gaps are
/// harmless — lists order by position).
///
/// Bottom-up in one transaction: empty leaves go first, then any parent left
/// with no direct items and no remaining children. Two passes cover the
/// two-level limit, so naturally emptying a subgroup cascades to its parent.
/// Explicitly created empty groups dissolve on the next unassign/delete
/// transaction, never mid-create.
pub(crate) fn sweep_empty_groups(conn: &Connection) -> Result<usize> {
    let memberless = "NOT EXISTS (
            SELECT 1 FROM launch_entries WHERE launch_entries.group_id = groups.id
         ) AND NOT EXISTS (
            SELECT 1 FROM quick_actions WHERE quick_actions.group_id = groups.id
         ) AND NOT EXISTS (
            SELECT 1 FROM clips WHERE clips.group_id = groups.id
         )";
    let mut removed = conn.execute(
        &format!("DELETE FROM groups WHERE parent_id IS NOT NULL AND {memberless}"),
        [],
    )?;
    removed += conn.execute(
        &format!(
            "DELETE FROM groups WHERE parent_id IS NULL AND {memberless}
             AND NOT EXISTS (SELECT 1 FROM groups child WHERE child.parent_id = groups.id)"
        ),
        [],
    )?;
    Ok(removed)
}

/// Every sibling id of one level in user order.
fn sibling_ids(
    conn: &Connection,
    collection: Collection,
    parent_id: Option<i64>,
) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT id FROM groups
          WHERE collection = ?1 AND parent_id IS ?2
          ORDER BY position, id",
    )?;
    let rows = stmt.query_map(params![collection.as_str(), parent_id], |row| {
        row.get::<_, i64>(0)
    })?;
    rows.collect()
}

/// Removes a Group and returns every item it held to ungrouped — items are
/// never deleted. Deleting a child removes only that child; deleting a parent
/// removes its whole subtree (child groups are structure) after ungrouping
/// every direct and descendant item. Sibling positions compact at the
/// deleted group's own level. A missing id is an error, matching the other
/// collections' deletes.
pub fn delete_group(conn: &Connection, id: i64) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    let (group, position) =
        get_group_at(&tx, id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
    let children: Vec<i64> = {
        let mut stmt = tx.prepare("SELECT id FROM groups WHERE parent_id = ?1")?;
        let rows = stmt.query_map(params![id], |row| row.get::<_, i64>(0))?;
        rows.collect::<Result<Vec<_>>>()?
    };
    let table = group.collection.table();
    // Explicit null-out beside the schema's ON DELETE SET NULL: the rule
    // holds even where the foreign-keys pragma isn't enforcing.
    let mut subtree = vec![id];
    subtree.extend_from_slice(&children);
    for member in &subtree {
        tx.execute(
            &format!("UPDATE {table} SET group_id = NULL WHERE group_id = ?1"),
            params![member],
        )?;
    }
    // Children first so no row outlives its parent even without cascades.
    for child in &children {
        tx.execute("DELETE FROM groups WHERE id = ?1", params![child])?;
    }
    tx.execute("DELETE FROM groups WHERE id = ?1", params![id])?;
    // Compact the deleted group's own level: top-level siblings for a parent,
    // the parent's other children for a child. The deleted children's own
    // levels disappear with them, so no second compaction applies.
    tx.execute(
        "UPDATE groups SET position = position - 1
         WHERE collection = ?1 AND parent_id IS ?2 AND position > ?3",
        params![group.collection.as_str(), group.parent_id, position],
    )?;
    sweep_empty_groups(&tx)?;
    tx.commit()
}

/// The one reparent/reorder contract: moves `id` under `parent_id` (or to the
/// top level with `None`) at `to_position` (clamped), renumbering the old and
/// new sibling sets in one transaction. Moving within the same parent is a
/// reorder; `None` unnests.
///
/// Depth 2 is enforced here, not just in the UI: a parent must exist in the
/// same collection and be top level, and a group that has children can only
/// stay at the top level. Destination name collisions are refused before any
/// row is written, naming the colliding name.
pub fn move_group(
    conn: &Connection,
    id: i64,
    parent_id: Option<i64>,
    to_position: i64,
) -> std::result::Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let (group, _) = get_group_at(&tx, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "This group no longer exists — refresh and try again.".to_string())?;

    if parent_id == Some(id) {
        return Err("A group can't be moved into itself.".into());
    }
    if let Some(destination) = parent_id {
        let parent = get_group(&tx, destination)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "That group no longer exists — refresh and try again.".to_string())?;
        if parent.collection != group.collection {
            return Err(format!(
                "A {} group can't move inside {} — groups never cross collections.",
                group.collection.label(),
                parent.collection.label()
            ));
        }
        if parent.parent_id.is_some() {
            return Err(
                "Groups only nest two levels deep — pick a top-level group.".into(),
            );
        }
        let child_count: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM groups WHERE parent_id = ?1",
                params![id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if child_count > 0 {
            return Err(
                "This group has subgroups of its own — it can only stay at the top level.".into(),
            );
        }
    }
    if colliding_group(&tx, group.collection, parent_id, &group.name, Some(id))
        .map_err(|e| e.to_string())?
    {
        return Err(collision_error(group.collection, &group.name, parent_id));
    }

    let old_parent = group.parent_id;
    let mut destination = sibling_ids(&tx, group.collection, parent_id).map_err(|e| e.to_string())?;
    destination.retain(|&sibling| sibling != id);
    let index = to_position.clamp(0, destination.len() as i64) as usize;
    destination.insert(index, id);
    for (index, sibling) in destination.iter().enumerate() {
        tx.execute(
            "UPDATE groups SET parent_id = ?1, position = ?2 WHERE id = ?3",
            params![parent_id, index as i64, sibling],
        )
        .map_err(|e| e.to_string())?;
    }
    if old_parent != parent_id {
        let old_siblings =
            sibling_ids(&tx, group.collection, old_parent).map_err(|e| e.to_string())?;
        for (index, sibling) in old_siblings.iter().enumerate() {
            tx.execute(
                "UPDATE groups SET position = ?1 WHERE id = ?2",
                params![index as i64, sibling],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    tx.commit().map_err(|e| e.to_string())
}

/// Why an assignment could not be applied: the data-layer isolation rule
/// (a group only ever holds items of its own collection), a stale reference,
/// or an underlying storage error.
#[derive(Debug)]
pub enum AssignError {
    /// The target group belongs to a different collection than the item.
    CrossCollection { group_label: &'static str },
    /// The group or the item no longer exists.
    Missing,
    Storage(rusqlite::Error),
}

impl std::fmt::Display for AssignError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssignError::CrossCollection { group_label } => write!(
                f,
                "That group belongs to {group_label} — a group only holds items of its own collection."
            ),
            AssignError::Missing => {
                write!(f, "This group or item no longer exists — refresh and try again.")
            }
            AssignError::Storage(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for AssignError {}

impl From<rusqlite::Error> for AssignError {
    fn from(err: rusqlite::Error) -> Self {
        AssignError::Storage(err)
    }
}

/// Puts one item into one Group — the item's single group reference; any
/// previous membership is replaced. The collection discriminator is checked
/// against the group's stored collection here, at the data layer, so a
/// cross-collection assignment can never be persisted regardless of what the
/// caller sends.
pub fn assign_item(
    conn: &Connection,
    collection: Collection,
    item_id: i64,
    group_id: i64,
) -> std::result::Result<(), AssignError> {
    let tx = conn.unchecked_transaction()?;
    let group = get_group(&tx, group_id)?.ok_or(AssignError::Missing)?;
    if group.collection != collection {
        return Err(AssignError::CrossCollection {
            group_label: group.collection.label(),
        });
    }
    let changed = tx.execute(
        &format!(
            "UPDATE {} SET group_id = ?1 WHERE id = ?2",
            collection.table()
        ),
        params![group_id, item_id],
    )?;
    if changed == 0 {
        return Err(AssignError::Missing);
    }
    // The item may have left its previous group for the last time — that
    // group dissolves with this same transaction (ticket 106).
    sweep_empty_groups(&tx)?;
    tx.commit()?;
    Ok(())
}

/// Clears an item's group reference — back to ungrouped. Unassigning an
/// already-ungrouped item succeeds; an item that no longer exists errors.
/// The group it left, if now empty, dissolves in the same transaction
/// (ticket 106).
pub fn unassign_item(
    conn: &Connection,
    collection: Collection,
    item_id: i64,
) -> std::result::Result<(), AssignError> {
    let tx = conn.unchecked_transaction()?;
    let changed = tx.execute(
        &format!("UPDATE {} SET group_id = NULL WHERE id = ?1", collection.table()),
        params![item_id],
    )?;
    if changed == 0 {
        return Err(AssignError::Missing);
    }
    sweep_empty_groups(&tx)?;
    tx.commit()?;
    Ok(())
}

/// The drag & drop drop contract (ticket 232): assigns one item to a group
/// (or ungrouped) and moves it to a global list position in ONE transaction.
/// `to_position` is the item's index in the full collection after it is
/// removed — the same index `OrderedList::place_at` expects — so the item's
/// order inside its target group and Start-all's global order stay one
/// contract (spec 221 D.5).
///
/// Everything is validated before the first write: a missing item or group,
/// or a group of another collection, leaves the row and the list untouched.
/// Any failure after the writes rolls the whole transaction back, so a drop
/// can never half-apply. The item's old group, if left empty, dissolves in
/// the same transaction.
pub fn place_item(
    conn: &Connection,
    collection: Collection,
    item_id: i64,
    group_id: Option<i64>,
    to_position: i64,
) -> std::result::Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    if let Some(target) = group_id {
        let group = get_group(&tx, target)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "This group no longer exists — refresh and try again.".to_string())?;
        if group.collection != collection {
            return Err(format!(
                "That group belongs to {} — a group only holds items of its own collection.",
                group.collection.label()
            ));
        }
    }
    let changed = tx
        .execute(
            &format!("UPDATE {} SET group_id = ?1 WHERE id = ?2", collection.table()),
            params![group_id, item_id],
        )
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err("This item no longer exists — refresh and try again.".into());
    }
    collection
        .ordered_list()
        .place_at(&tx, item_id, to_position)
        .map_err(|e| e.to_string())?;
    sweep_empty_groups(&tx).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

/// One collection's items laid out for rendering: everything ungrouped
/// first, then each Group in user order with its own members.
/// Consumed by the per-page grouping surfaces landing next (tickets 90/91);
/// verified here so the ordering rule is already pinned.
#[allow(dead_code)]
pub struct GroupSections<T> {
    pub ungrouped: Vec<T>,
    /// Every group in user order, including empty ones — whether an empty
    /// section renders is the caller's call, not the ordering's.
    pub grouped: Vec<(Group, Vec<T>)>,
}

/// The pure ordering helper behind the per-collection lists (ticket 89):
/// ungrouped items first in their given order, then one section per group in
/// `groups` order, members keeping their given order within their section.
/// Items referencing an unknown group id count as ungrouped — a stale
/// reference never hides an item.
#[allow(dead_code)]
pub fn order_by_group<T>(
    items: Vec<T>,
    groups: Vec<Group>,
    group_of: impl Fn(&T) -> Option<i64>,
) -> GroupSections<T> {
    let index_of: HashMap<i64, usize> =
        groups.iter().enumerate().map(|(i, g)| (g.id, i)).collect();
    let mut sections = GroupSections {
        ungrouped: Vec::new(),
        grouped: groups.into_iter().map(|g| (g, Vec::new())).collect(),
    };
    for item in items {
        match group_of(&item).and_then(|id| index_of.get(&id).copied()) {
            Some(index) => sections.grouped[index].1.push(item),
            None => sections.ungrouped.push(item),
        }
    }
    sections
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clips::{self, ClipInput};
    use crate::launch::{self, LaunchEntryInput, LaunchEntryKind, LaunchShell};
    use crate::quick_actions::{self, QuickActionInput};

    fn conn() -> Connection {
        crate::db::init_at(&tempfile::tempdir().unwrap().into_path()).unwrap()
    }

    fn entry(name: &str) -> LaunchEntryInput {
        LaunchEntryInput {
            name: name.into(),
            kind: LaunchEntryKind::Command,
            target: "notepad.exe".into(),
            shell: Some(LaunchShell::None),
            show_window: false,
            desktop_id: None,
            show_in_dock: true,
        }
    }

    fn action(name: &str) -> QuickActionInput {
        QuickActionInput {
            name: name.into(),
            shell: quick_actions::QuickActionShell::Powershell,
            command: "echo hi".into(),
            cwd: None,
            stoppable: false,
            stop_command: None,
            note: None,
            auto_run: false,
            show_in_dock: true,
            pre_check: None,
            pre_fix: None,
        }
    }

    fn clip(name: &str) -> ClipInput {
        ClipInput {
            name: name.into(),
            content: format!("content of {name}"),
            show_in_dock: true,
            image: None,
        }
    }

    #[test]
    fn create_rename_list_work_per_collection_in_user_order() {
        let conn = conn();
        let first = create_group(&conn, Collection::Action, "Builds").unwrap();
        let second = create_group(&conn, Collection::Action, "Docker").unwrap();
        create_group(&conn, Collection::Clip, "Snippets").unwrap();

        rename_group(&conn, first.id, "Build scripts").unwrap();

        let actions = list_groups(&conn, Collection::Action).unwrap();
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].id, first.id);
        assert_eq!(actions[0].name, "Build scripts");
        assert_eq!(actions[0].collection, Collection::Action);
        assert_eq!(actions[1].id, second.id);

        // Namespaces never bleed into each other's lists.
        assert_eq!(list_groups(&conn, Collection::Clip).unwrap().len(), 1);
        assert!(list_groups(&conn, Collection::Launch).unwrap().is_empty());
    }

    #[test]
    fn group_positions_are_scoped_per_collection() {
        let conn = conn();
        // Two collections grow independently from position 0.
        let a_launch = create_group(&conn, Collection::Launch, "L1").unwrap();
        let an_action = create_group(&conn, Collection::Action, "A1").unwrap();
        let another_launch = create_group(&conn, Collection::Launch, "L2").unwrap();
        move_group(&conn, another_launch.id, None, 0).unwrap();
        move_group(&conn, an_action.id, None, 5).unwrap(); // clamped within its own namespace

        let launches = list_groups(&conn, Collection::Launch).unwrap();
        assert_eq!(launches.iter().map(|g| g.id).collect::<Vec<_>>(), vec![another_launch.id, a_launch.id]);
    }

    #[test]
    fn assigning_to_another_collections_group_is_rejected() {
        let conn = conn();
        let clip_group = create_group(&conn, Collection::Clip, "Snips").unwrap();
        let item = launch::create_launch_entry(&conn, &entry("Editor")).unwrap();

        let err = assign_item(&conn, Collection::Launch, item.id, clip_group.id).unwrap_err();
        assert!(
            matches!(err, AssignError::CrossCollection { .. }),
            "cross-collection assignment must be refused at the data layer, got: {err}"
        );
        // Nothing was written.
        let listed = launch::list_launch_entries(&conn).unwrap();
        assert_eq!(listed[0].group_id, None);
    }

    #[test]
    fn assign_reports_missing_groups_and_items() {
        let conn = conn();
        let action_group = create_group(&conn, Collection::Action, "Builds").unwrap();
        let item = quick_actions::create_quick_action(&conn, &action("Deploy")).unwrap();

        assert!(matches!(
            assign_item(&conn, Collection::Action, item.id, 99_999).unwrap_err(),
            AssignError::Missing
        ));
        assert!(matches!(
            assign_item(&conn, Collection::Action, 99_999, action_group.id).unwrap_err(),
            AssignError::Missing
        ));
    }

    #[test]
    fn item_holds_at_most_one_group_and_unassign_clears_it() {
        let conn = conn();
        let g1 = create_group(&conn, Collection::Action, "One").unwrap();
        let item = quick_actions::create_quick_action(&conn, &action("Backup")).unwrap();
        assign_item(&conn, Collection::Action, item.id, g1.id).unwrap();

        // g2 is born only after g1 is populated: an empty group does not
        // survive an intervening assign/unassign/delete (ticket 106).
        let g2 = create_group(&conn, Collection::Action, "Two").unwrap();
        assign_item(&conn, Collection::Action, item.id, g2.id).unwrap();
        let stored = quick_actions::list_quick_actions(&conn).unwrap();
        assert_eq!(stored[0].group_id, Some(g2.id), "the later assignment replaces the earlier one");

        unassign_item(&conn, Collection::Action, item.id).unwrap();
        let stored = quick_actions::list_quick_actions(&conn).unwrap();
        assert_eq!(stored[0].group_id, None);
    }

    #[test]
    fn deleting_a_group_returns_members_ungrouped_without_deleting_them() {
        let conn = conn();
        let g1 = create_group(&conn, Collection::Clip, "Work").unwrap();
        let kept = clips::create_clip(&conn, &clip("token")).unwrap();
        assign_item(&conn, Collection::Clip, kept.id, g1.id).unwrap();
        // Populated first — see the dissolve rule (ticket 106).
        let g2 = create_group(&conn, Collection::Clip, "Play").unwrap();
        let survivor = clips::create_clip(&conn, &clip("spare")).unwrap();
        assign_item(&conn, Collection::Clip, survivor.id, g2.id).unwrap();

        delete_group(&conn, g1.id).unwrap();

        // The member survives, ungrouped; the populated sibling survives.
        let stored = clips::list_clips(&conn).unwrap();
        assert_eq!(stored.len(), 2);
        assert_eq!(
            stored.iter().find(|c| c.id == kept.id).unwrap().group_id,
            None
        );

        // The remaining group closed ranks.
        let remaining = list_groups(&conn, Collection::Clip).unwrap();
        assert_eq!(remaining.iter().map(|g| g.id).collect::<Vec<_>>(), vec![g2.id]);
        move_group(&conn, remaining[0].id, None, 3).unwrap(); // still reorderable

        delete_group(&conn, 99_999).unwrap_err(); // missing id is an error
    }

    #[test]
    fn order_by_group_lists_ungrouped_first_then_groups_in_user_order() {
        let conn = conn();
        let g1 = create_group(&conn, Collection::Action, "First").unwrap();
        let in_g1 = quick_actions::create_quick_action(&conn, &action("c")).unwrap();
        assign_item(&conn, Collection::Action, in_g1.id, g1.id).unwrap();
        // Populated before the next group is born — the dissolve rule.
        let g2 = create_group(&conn, Collection::Action, "Second").unwrap();
        let in_g2 = quick_actions::create_quick_action(&conn, &action("a")).unwrap();
        assign_item(&conn, Collection::Action, in_g2.id, g2.id).unwrap();
        let free = quick_actions::create_quick_action(&conn, &action("b")).unwrap();

        // Reorder: Second now comes before First.
        move_group(&conn, g2.id, None, 0).unwrap();
        let groups = list_groups(&conn, Collection::Action).unwrap();

        let items = quick_actions::list_quick_actions(&conn).unwrap();
        let sections = order_by_group(items, groups, |item| item.group_id);
        assert_eq!(
            sections.ungrouped.iter().map(|i| i.id).collect::<Vec<_>>(),
            vec![free.id],
            "ungrouped stays on top"
        );
        assert_eq!(sections.grouped.len(), 2);
        assert_eq!(sections.grouped[0].0.id, g2.id);
        assert_eq!(sections.grouped[0].1.iter().map(|i| i.id).collect::<Vec<_>>(), vec![in_g2.id]);
        assert_eq!(sections.grouped[1].0.id, g1.id);
        assert_eq!(sections.grouped[1].1.iter().map(|i| i.id).collect::<Vec<_>>(), vec![in_g1.id]);
    }

    #[test]
    fn order_by_group_keeps_every_group_and_treats_unknown_refs_as_ungrouped() {
        let g1 = Group {
            id: 7,
            collection: Collection::Launch,
            name: "Solo".into(),
            color: None,
            parent_id: None,
        };
        // No items at all: the empty group is still handed back, user order trivially.
        let sections = order_by_group(Vec::<i32>::new(), vec![g1.clone()], |_| None);
        assert!(sections.ungrouped.is_empty());
        assert_eq!(sections.grouped.len(), 1);
        assert!(sections.grouped[0].1.is_empty());

        // A dangling group reference never hides an item.
        let sections = order_by_group(vec![1, 2], vec![g1], |i| if *i == 1 { Some(404) } else { None });
        assert_eq!(sections.ungrouped, vec![1, 2]);
    }

    #[test]
    fn group_names_are_exclusive_within_their_collection_case_insensitively() {
        let conn = conn();
        let builds = create_group(&conn, Collection::Action, "Builds").unwrap();
        // Populated immediately — an empty group does not survive an
        // intervening transaction (ticket 106).
        let first = quick_actions::create_quick_action(&conn, &action("a")).unwrap();
        assign_item(&conn, Collection::Action, first.id, builds.id).unwrap();

        // Same name, different case or padding, same collection → refused.
        let err = create_group(&conn, Collection::Action, "  builds  ").unwrap_err();
        assert!(err.contains("already exists"), "got: {err}");
        create_group(&conn, Collection::Clip, "Builds").unwrap(); // other namespace: fine

        // Renaming onto a sibling is refused; renaming to your own name is
        // not (self excluded).
        let other = create_group(&conn, Collection::Action, "Other").unwrap();
        let second = quick_actions::create_quick_action(&conn, &action("Deploy")).unwrap();
        assign_item(&conn, Collection::Action, second.id, other.id).unwrap();
        let err = rename_group(&conn, builds.id, "other").unwrap_err();
        assert!(err.contains("already exists"), "got: {err}");
        rename_group(&conn, builds.id, "BUILDS").unwrap();

        // Uniqueness binds to live rows only: deleting both frees the name
        // for immediate reuse.
        delete_group(&conn, builds.id).unwrap();
        delete_group(&conn, other.id).unwrap();
        let reused = create_group(&conn, Collection::Action, "Builds").unwrap();
        assert_eq!(reused.name, "Builds");
    }

    #[test]
    fn the_last_member_leaving_dissolves_its_group_on_every_path() {
        let conn = conn();
        let a = quick_actions::create_quick_action(&conn, &action("a")).unwrap();

        // Path 1 — explicit unassign. (Each group is filled as soon as it
        // exists: an empty group does not survive an intervening mutation,
        // ticket 106.)
        let g1 = create_group(&conn, Collection::Action, "One").unwrap();
        assign_item(&conn, Collection::Action, a.id, g1.id).unwrap();
        unassign_item(&conn, Collection::Action, a.id).unwrap();
        assert!(list_groups(&conn, Collection::Action).unwrap().is_empty());

        // Path 2 — reassignment (the old group loses its last member).
        let g2 = create_group(&conn, Collection::Action, "Two").unwrap();
        assign_item(&conn, Collection::Action, a.id, g2.id).unwrap();
        let g3 = create_group(&conn, Collection::Action, "Three").unwrap();
        assign_item(&conn, Collection::Action, a.id, g3.id).unwrap();
        let remaining = list_groups(&conn, Collection::Action).unwrap();
        assert_eq!(
            remaining.iter().map(|g| g.name.clone()).collect::<Vec<_>>(),
            vec!["Three"]
        );

        // Path 3 — the member itself is deleted.
        let c = clips::create_clip(&conn, &clip("token")).unwrap();
        let g4 = create_group(&conn, Collection::Clip, "Solo").unwrap();
        assign_item(&conn, Collection::Clip, c.id, g4.id).unwrap();
        clips::delete_clip(&conn, c.id).unwrap();
        assert!(list_groups(&conn, Collection::Clip).unwrap().is_empty());

        // A populated group survives an unrelated collection's sweep trigger.
        let e = launch::create_launch_entry(&conn, &entry("Editor")).unwrap();
        let launch_g = create_group(&conn, Collection::Launch, "Kept").unwrap();
        assign_item(&conn, Collection::Launch, e.id, launch_g.id).unwrap();
        unassign_item(&conn, Collection::Action, a.id).unwrap(); // trigger elsewhere
        assert_eq!(list_groups(&conn, Collection::Launch).unwrap()[0].id, launch_g.id);
    }

    #[test]
    fn sweeps_across_all_three_collections_preserve_survivor_order() {
        let conn = conn();

        // Every group is populated the moment it exists (the dissolve rule);
        // Launch gets a second populated group to prove relative order, and
        // one always-empty sibling per collection waits for the sweep.
        let e = launch::create_launch_entry(&conn, &entry("Editor")).unwrap();
        let first_l = create_group(&conn, Collection::Launch, "L first").unwrap();
        assign_item(&conn, Collection::Launch, e.id, first_l.id).unwrap();
        let drop_l = create_group(&conn, Collection::Launch, "L drop").unwrap();
        let e2 = launch::create_launch_entry(&conn, &entry("Browser")).unwrap();
        let second_l = create_group(&conn, Collection::Launch, "L second").unwrap();
        assign_item(&conn, Collection::Launch, e2.id, second_l.id).unwrap();
        let a = quick_actions::create_quick_action(&conn, &action("Deploy")).unwrap();
        let keep_a = create_group(&conn, Collection::Action, "A keep").unwrap();
        assign_item(&conn, Collection::Action, a.id, keep_a.id).unwrap();
        let drop_a = create_group(&conn, Collection::Action, "A drop").unwrap();
        let c = clips::create_clip(&conn, &clip("token")).unwrap();
        let keep_c = create_group(&conn, Collection::Clip, "C keep").unwrap();
        assign_item(&conn, Collection::Clip, c.id, keep_c.id).unwrap();
        let drop_c = create_group(&conn, Collection::Clip, "C drop").unwrap();

        // One unrelated transaction (unassigning an already-ungrouped item
        // succeeds) sweeps every memberless group in every collection.
        unassign_item(&conn, Collection::Clip, clips::create_clip(&conn, &clip("x")).unwrap().id).unwrap();

        let launches = list_groups(&conn, Collection::Launch).unwrap();
        assert_eq!(
            launches.iter().map(|g| g.name.clone()).collect::<Vec<_>>(),
            vec!["L first", "L second"],
            "survivors keep their user order"
        );
        assert_eq!(
            list_groups(&conn, Collection::Action)
                .unwrap()
                .iter()
                .map(|g| g.name.clone())
                .collect::<Vec<_>>(),
            vec!["A keep"]
        );
        assert_eq!(
            list_groups(&conn, Collection::Clip)
                .unwrap()
                .iter()
                .map(|g| g.name.clone())
                .collect::<Vec<_>>(),
            vec!["C keep"]
        );
        let _ = (drop_l, drop_a, drop_c);
    }

    #[test]
    fn group_color_accepts_hex_normalizes_and_clears() {
        let conn = conn();
        let group = create_group(&conn, Collection::Action, "Colored").unwrap();
        let item = quick_actions::create_quick_action(&conn, &action("Deploy")).unwrap();
        assign_item(&conn, Collection::Action, item.id, group.id).unwrap();

        set_group_color(&conn, group.id, Some("#AABBCC")).unwrap();
        assert_eq!(
            list_groups(&conn, Collection::Action).unwrap()[0].color.as_deref(),
            Some("#aabbcc")
        );

        for invalid in ["#abc", "#aabbccdd", "aabbcc", "red", "rgb(1, 2, 3)", "#gggggg"] {
            assert!(
                set_group_color(&conn, group.id, Some(invalid)).is_err(),
                "{invalid} must be refused"
            );
        }
        // A refused value never overwrites the stored color.
        assert_eq!(
            list_groups(&conn, Collection::Action).unwrap()[0].color.as_deref(),
            Some("#aabbcc")
        );

        set_group_color(&conn, group.id, None).unwrap();
        assert_eq!(list_groups(&conn, Collection::Action).unwrap()[0].color, None);
        assert!(set_group_color(&conn, 99_999, Some("#112233")).is_err());
    }

    #[test]
    fn colors_survive_rename_and_reorder_and_stay_per_group() {
        let conn = conn();
        let first = create_group(&conn, Collection::Action, "First").unwrap();
        let one = quick_actions::create_quick_action(&conn, &action("a")).unwrap();
        assign_item(&conn, Collection::Action, one.id, first.id).unwrap();
        let second = create_group(&conn, Collection::Action, "Second").unwrap();
        let two = quick_actions::create_quick_action(&conn, &action("b")).unwrap();
        assign_item(&conn, Collection::Action, two.id, second.id).unwrap();
        let neutral = create_group(&conn, Collection::Action, "Neutral").unwrap();
        let three = quick_actions::create_quick_action(&conn, &action("c")).unwrap();
        assign_item(&conn, Collection::Action, three.id, neutral.id).unwrap();

        set_group_color(&conn, first.id, Some("#112233")).unwrap();
        set_group_color(&conn, second.id, Some("#445566")).unwrap();
        rename_group(&conn, first.id, "Renamed").unwrap();
        move_group(&conn, first.id, None, 2).unwrap();

        let groups = list_groups(&conn, Collection::Action).unwrap();
        assert_eq!(
            groups.iter().map(|g| g.id).collect::<Vec<_>>(),
            vec![second.id, neutral.id, first.id]
        );
        assert_eq!(groups[0].color.as_deref(), Some("#445566"));
        assert_eq!(groups[1].color, None, "no color inheritance or spillover");
        assert_eq!(groups[2].color.as_deref(), Some("#112233"));
    }

    fn position_of(conn: &Connection, id: i64) -> i64 {
        conn.query_row(
            "SELECT position FROM groups WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .unwrap()
    }

    fn ids(groups: &[Group]) -> Vec<i64> {
        groups.iter().map(|g| g.id).collect()
    }

    #[test]
    fn list_groups_returns_the_flat_tree_in_sibling_order() {
        let conn = conn();
        let first = create_group(&conn, Collection::Action, "First").unwrap();
        let child_a = create_group(&conn, Collection::Action, "Child A").unwrap();
        let child_b = create_group(&conn, Collection::Action, "Child B").unwrap();
        let second = create_group(&conn, Collection::Action, "Second").unwrap();

        move_group(&conn, child_b.id, Some(first.id), 0).unwrap();
        move_group(&conn, child_a.id, Some(first.id), 1).unwrap();

        let listed = list_groups(&conn, Collection::Action).unwrap();
        assert_eq!(
            ids(&listed),
            vec![first.id, child_b.id, child_a.id, second.id],
            "a parent immediately precedes its children; each level keeps its own order"
        );
        assert_eq!(listed[0].parent_id, None);
        assert_eq!(listed[1].parent_id, Some(first.id));
        assert_eq!(listed[2].parent_id, Some(first.id));
        assert_eq!(listed[3].parent_id, None);
    }

    #[test]
    fn child_names_are_sibling_scoped_and_two_parents_may_reuse_a_name() {
        let conn = conn();
        let clients = create_group(&conn, Collection::Action, "Clients").unwrap();
        let personal = create_group(&conn, Collection::Action, "Personal").unwrap();

        let work_a = create_group(&conn, Collection::Action, "Work").unwrap();
        move_group(&conn, work_a.id, Some(clients.id), 0).unwrap();
        // The name is free at the top level again: uniqueness is sibling-only.
        let work_b = create_group(&conn, Collection::Action, "Work").unwrap();
        move_group(&conn, work_b.id, Some(personal.id), 0).unwrap();
        assert_eq!(
            list_groups(&conn, Collection::Action)
                .unwrap()
                .iter()
                .filter(|g| g.name == "Work")
                .count(),
            2,
            "two parents may use the same child name"
        );

        // A third "Work" under Clients collides with work_a — and the message
        // names the colliding name.
        let work_c = create_group(&conn, Collection::Action, "Work").unwrap();
        let err = move_group(&conn, work_c.id, Some(clients.id), 0).unwrap_err();
        assert!(err.contains("'Work'"), "got: {err}");

        // Rename is checked against the group's own sibling set.
        let design = create_group(&conn, Collection::Action, "Design").unwrap();
        move_group(&conn, design.id, Some(clients.id), 1).unwrap();
        let err = rename_group(&conn, design.id, "work").unwrap_err();
        assert!(err.contains("already exists"), "got: {err}");
        // The same name under another parent is fine.
        rename_group(&conn, work_b.id, "Design").unwrap();
        rename_group(&conn, work_a.id, "Work").unwrap();
    }

    #[test]
    fn move_reparents_unnests_and_renumbers_both_levels() {
        let conn = conn();
        let parent = create_group(&conn, Collection::Action, "Parent").unwrap();
        let first_child = create_group(&conn, Collection::Action, "First child").unwrap();
        let second_child = create_group(&conn, Collection::Action, "Second child").unwrap();
        move_group(&conn, first_child.id, Some(parent.id), 0).unwrap();
        move_group(&conn, second_child.id, Some(parent.id), 1).unwrap();
        assert_eq!(position_of(&conn, first_child.id), 0);
        assert_eq!(position_of(&conn, second_child.id), 1);

        // Same-parent move is a reorder, not a reparent.
        move_group(&conn, second_child.id, Some(parent.id), 0).unwrap();
        assert_eq!(position_of(&conn, second_child.id), 0);
        assert_eq!(position_of(&conn, first_child.id), 1);

        // Unnesting renumbers the old sibling set in the same transaction.
        let other = create_group(&conn, Collection::Action, "Other").unwrap();
        move_group(&conn, second_child.id, None, 1).unwrap();
        assert_eq!(
            position_of(&conn, first_child.id),
            0,
            "the old sibling set closes ranks"
        );
        let top: Vec<i64> = list_groups(&conn, Collection::Action)
            .unwrap()
            .iter()
            .filter(|g| g.parent_id.is_none())
            .map(|g| g.id)
            .collect();
        assert_eq!(top, vec![parent.id, second_child.id, other.id]);
        assert_eq!(position_of(&conn, second_child.id), 1);
    }

    #[test]
    fn move_refuses_depth_three_cycles_cross_collection_and_missing_parents() {
        let conn = conn();
        let parent = create_group(&conn, Collection::Action, "Parent").unwrap();
        let child = create_group(&conn, Collection::Action, "Child").unwrap();
        move_group(&conn, child.id, Some(parent.id), 0).unwrap();

        // Depth 3: a child can never become a parent.
        let err = move_group(&conn, parent.id, Some(child.id), 0).unwrap_err();
        assert!(err.contains("two levels"), "got: {err}");

        // Cycles.
        let err = move_group(&conn, child.id, Some(child.id), 0).unwrap_err();
        assert!(err.contains("itself"), "got: {err}");

        // A group that has children can only stay at the top level.
        let host = create_group(&conn, Collection::Action, "Host").unwrap();
        let err = move_group(&conn, parent.id, Some(host.id), 0).unwrap_err();
        assert!(err.contains("subgroups"), "got: {err}");

        // Cross-collection parenting.
        let clip_group = create_group(&conn, Collection::Clip, "Clips").unwrap();
        let err = move_group(&conn, child.id, Some(clip_group.id), 0).unwrap_err();
        assert!(err.contains("never cross collections"), "got: {err}");

        // A missing parent leaves everything untouched.
        let err = move_group(&conn, child.id, Some(99_999), 0).unwrap_err();
        assert!(err.contains("no longer exists"), "got: {err}");
        assert_eq!(
            list_groups(&conn, Collection::Action)
                .unwrap()
                .iter()
                .find(|g| g.id == child.id)
                .unwrap()
                .parent_id,
            Some(parent.id)
        );
    }

    #[test]
    fn sweep_dissolves_empty_subgroups_bottom_up() {
        let conn = conn();
        let parent = create_group(&conn, Collection::Action, "Parent").unwrap();
        let child = create_group(&conn, Collection::Action, "Child").unwrap();
        move_group(&conn, child.id, Some(parent.id), 0).unwrap();
        let item = quick_actions::create_quick_action(&conn, &action("a")).unwrap();
        assign_item(&conn, Collection::Action, item.id, child.id).unwrap();
        assert_eq!(list_groups(&conn, Collection::Action).unwrap().len(), 2);

        unassign_item(&conn, Collection::Action, item.id).unwrap();
        assert!(
            list_groups(&conn, Collection::Action).unwrap().is_empty(),
            "the emptied child goes first, then the parent left without either"
        );

        // A parent with its own direct item outlives an emptied subgroup.
        let parent2 = create_group(&conn, Collection::Action, "Parent 2").unwrap();
        let child2 = create_group(&conn, Collection::Action, "Child 2").unwrap();
        move_group(&conn, child2.id, Some(parent2.id), 0).unwrap();
        let child_item = quick_actions::create_quick_action(&conn, &action("child")).unwrap();
        assign_item(&conn, Collection::Action, child_item.id, child2.id).unwrap();
        let direct = quick_actions::create_quick_action(&conn, &action("direct")).unwrap();
        assign_item(&conn, Collection::Action, direct.id, parent2.id).unwrap();

        unassign_item(&conn, Collection::Action, child_item.id).unwrap();
        assert_eq!(ids(&list_groups(&conn, Collection::Action).unwrap()), vec![parent2.id]);
        unassign_item(&conn, Collection::Action, direct.id).unwrap();
        assert!(list_groups(&conn, Collection::Action).unwrap().is_empty());
    }

    #[test]
    fn deleting_a_parent_ungroups_its_whole_subtree_without_deleting_items() {
        let conn = conn();
        let parent = create_group(&conn, Collection::Action, "Parent").unwrap();
        let child = create_group(&conn, Collection::Action, "Child").unwrap();
        move_group(&conn, child.id, Some(parent.id), 0).unwrap();
        let nested = quick_actions::create_quick_action(&conn, &action("nested")).unwrap();
        assign_item(&conn, Collection::Action, nested.id, child.id).unwrap();
        let direct = quick_actions::create_quick_action(&conn, &action("direct")).unwrap();
        assign_item(&conn, Collection::Action, direct.id, parent.id).unwrap();
        let other = create_group(&conn, Collection::Action, "Other").unwrap();
        let survivor = quick_actions::create_quick_action(&conn, &action("survivor")).unwrap();
        assign_item(&conn, Collection::Action, survivor.id, other.id).unwrap();

        delete_group(&conn, parent.id).unwrap();

        let stored = quick_actions::list_quick_actions(&conn).unwrap();
        assert_eq!(stored.len(), 3, "no item is ever deleted");
        for id in [direct.id, nested.id] {
            assert_eq!(
                stored.iter().find(|item| item.id == id).unwrap().group_id,
                None,
                "every item in the subtree returns to ungrouped"
            );
        }
        assert_eq!(ids(&list_groups(&conn, Collection::Action).unwrap()), vec![other.id]);
        assert_eq!(
            position_of(&conn, other.id),
            0,
            "the top-level sibling set closes ranks"
        );
    }

    #[test]
    fn deleting_a_child_ungroups_only_its_items_and_keeps_the_parent() {
        let conn = conn();
        let parent = create_group(&conn, Collection::Action, "Parent").unwrap();
        let child = create_group(&conn, Collection::Action, "Child").unwrap();
        move_group(&conn, child.id, Some(parent.id), 0).unwrap();
        let child_item = quick_actions::create_quick_action(&conn, &action("child item")).unwrap();
        assign_item(&conn, Collection::Action, child_item.id, child.id).unwrap();
        let parent_item = quick_actions::create_quick_action(&conn, &action("parent item")).unwrap();
        assign_item(&conn, Collection::Action, parent_item.id, parent.id).unwrap();
        let sibling = create_group(&conn, Collection::Action, "Sibling").unwrap();
        move_group(&conn, sibling.id, Some(parent.id), 1).unwrap();
        let sibling_item = quick_actions::create_quick_action(&conn, &action("sibling item")).unwrap();
        assign_item(&conn, Collection::Action, sibling_item.id, sibling.id).unwrap();

        delete_group(&conn, child.id).unwrap();

        let listed = list_groups(&conn, Collection::Action).unwrap();
        assert_eq!(ids(&listed), vec![parent.id, sibling.id]);
        assert_eq!(listed[0].parent_id, None);
        assert_eq!(listed[1].parent_id, Some(parent.id));
        assert_eq!(
            position_of(&conn, sibling.id),
            0,
            "child positions compact under their parent"
        );
        let stored = quick_actions::list_quick_actions(&conn).unwrap();
        assert_eq!(
            stored.iter().find(|item| item.id == child_item.id).unwrap().group_id,
            None
        );
        assert_eq!(
            stored.iter().find(|item| item.id == parent_item.id).unwrap().group_id,
            Some(parent.id),
            "the parent keeps its own items"
        );
    }

    #[test]
    fn place_item_assigns_and_repositions_in_one_global_order() {
        let conn = conn();
        let a = quick_actions::create_quick_action(&conn, &action("a")).unwrap();
        let b = quick_actions::create_quick_action(&conn, &action("b")).unwrap();
        let c = quick_actions::create_quick_action(&conn, &action("c")).unwrap();
        let group = create_group(&conn, Collection::Action, "Drop").unwrap();

        // a (position 0) joins the group and moves to the global end.
        place_item(&conn, Collection::Action, a.id, Some(group.id), 2).unwrap();
        let stored = quick_actions::list_quick_actions(&conn).unwrap();
        assert_eq!(
            stored.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![b.id, c.id, a.id]
        );
        assert_eq!(
            stored.iter().find(|item| item.id == a.id).unwrap().group_id,
            Some(group.id)
        );

        // The same command returns it to ungrouped at the front; the emptied
        // group dissolves in the same transaction.
        place_item(&conn, Collection::Action, a.id, None, 0).unwrap();
        let stored = quick_actions::list_quick_actions(&conn).unwrap();
        assert_eq!(
            stored.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![a.id, b.id, c.id]
        );
        assert_eq!(
            stored.iter().find(|item| item.id == a.id).unwrap().group_id,
            None
        );
        assert!(list_groups(&conn, Collection::Action).unwrap().is_empty());
    }

    #[test]
    fn place_item_refuses_invalid_targets_before_writing_anything() {
        let conn = conn();
        let a = quick_actions::create_quick_action(&conn, &action("a")).unwrap();
        let b = quick_actions::create_quick_action(&conn, &action("b")).unwrap();
        let action_group = create_group(&conn, Collection::Action, "Keep").unwrap();
        assign_item(&conn, Collection::Action, b.id, action_group.id).unwrap();
        let clip_group = create_group(&conn, Collection::Clip, "Elsewhere").unwrap();
        let clip_item = clips::create_clip(&conn, &clip("token")).unwrap();
        assign_item(&conn, Collection::Clip, clip_item.id, clip_group.id).unwrap();

        let before = quick_actions::list_quick_actions(&conn).unwrap();

        let err = place_item(&conn, Collection::Action, a.id, Some(clip_group.id), 0).unwrap_err();
        assert!(err.contains("own collection"), "got: {err}");
        assert!(place_item(&conn, Collection::Action, 99_999, None, 0).is_err());
        assert!(place_item(&conn, Collection::Action, a.id, Some(99_999), 0).is_err());

        let after = quick_actions::list_quick_actions(&conn).unwrap();
        assert_eq!(after, before, "a refused drop leaves the list exactly as it was");
    }

    #[test]
    fn place_item_can_target_a_child_group_and_keeps_sibling_order() {
        let conn = conn();
        let parent = create_group(&conn, Collection::Action, "Parent").unwrap();
        let child = create_group(&conn, Collection::Action, "Child").unwrap();
        move_group(&conn, child.id, Some(parent.id), 0).unwrap();
        let first = quick_actions::create_quick_action(&conn, &action("first")).unwrap();
        let second = quick_actions::create_quick_action(&conn, &action("second")).unwrap();

        place_item(&conn, Collection::Action, second.id, Some(child.id), 1).unwrap();
        place_item(&conn, Collection::Action, first.id, Some(child.id), 1).unwrap();

        let stored = quick_actions::list_quick_actions(&conn).unwrap();
        assert_eq!(
            stored.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![second.id, first.id],
            "within a group, order follows the one global position"
        );
        assert_eq!(
            stored.iter().find(|item| item.id == first.id).unwrap().group_id,
            Some(child.id)
        );
        assert_eq!(
            ids(&list_groups(&conn, Collection::Action).unwrap()),
            vec![parent.id, child.id],
            "the parent and its child both survive"
        );
    }
}
