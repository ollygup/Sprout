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
//! Groups are machine-local structure like desktop assignments (ticket 88):
//! never part of Presets, Plan, Run, or exports.

use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension, Result};
use serde::{Deserialize, Serialize};

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
}

/// A Group as stored: its collection namespace plus its name. Position is
/// internal (order within the collection) and never part of the payload —
/// reorders go through `move_group`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    pub id: i64,
    pub collection: Collection,
    pub name: String,
}

const SELECT_GROUP_SQL: &str = "SELECT id, collection, name FROM groups";

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
    })
}

/// Every Group of one collection, in user order.
pub fn list_groups(conn: &Connection, collection: Collection) -> Result<Vec<Group>> {
    let mut stmt = conn.prepare(&format!(
        "{SELECT_GROUP_SQL} WHERE collection = ?1 ORDER BY position, id"
    ))?;
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
        "SELECT id, collection, name, position FROM groups WHERE id = ?1",
        params![id],
        |row| {
            Ok((
                group_from_row(row)?,
                row.get::<_, i64>(3)?,
            ))
        },
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

/// Appends a Group at the end of its collection's order (the next free
/// position among that collection's groups only — namespaces never share an
/// ordering).
pub fn create_group(conn: &Connection, collection: Collection, name: &str) -> Result<Group> {
    let tx = conn.unchecked_transaction()?;
    let position: i64 = tx.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM groups WHERE collection = ?1",
        params![collection.as_str()],
        |row| row.get(0),
    )?;
    tx.execute(
        "INSERT INTO groups (collection, name, position) VALUES (?1, ?2, ?3)",
        params![collection.as_str(), name.trim(), position],
    )?;
    let id = tx.last_insert_rowid();
    tx.commit()?;
    Ok(get_group(conn, id)?.expect("just inserted"))
}

/// Renames a Group in place; position and membership are untouched.
pub fn rename_group(conn: &Connection, id: i64, name: &str) -> Result<()> {
    let changed = conn.execute(
        "UPDATE groups SET name = ?1 WHERE id = ?2",
        params![name.trim(), id],
    )?;
    if changed == 0 {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    Ok(())
}

/// Removes a Group: its members go back to ungrouped (their `group_id` is
/// nulled — the items themselves are never touched) and the collection's
/// remaining groups close ranks so positions stay gapless. A missing id is
/// an error, matching the other collections' deletes.
pub fn delete_group(conn: &Connection, id: i64) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    let (group, position) =
        get_group_at(&tx, id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
    // Explicit null-out beside the schema's ON DELETE SET NULL: the rule
    // holds even where the foreign-keys pragma isn't enforcing.
    tx.execute(
        &format!(
            "UPDATE {} SET group_id = NULL WHERE group_id = ?1",
            group.collection.table()
        ),
        params![id],
    )?;
    tx.execute("DELETE FROM groups WHERE id = ?1", params![id])?;
    tx.execute(
        "UPDATE groups SET position = position - 1
         WHERE collection = ?1 AND position > ?2",
        params![group.collection.as_str(), position],
    )?;
    tx.commit()
}

/// Moves a Group to `to_position` within its own collection's order
/// (clamped), renumbering the rest — mirroring the item lists' reorder.
pub fn move_group(conn: &Connection, id: i64, to_position: i64) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    let collection = get_group(&tx, id)?
        .ok_or(rusqlite::Error::QueryReturnedNoRows)?
        .collection;
    let mut ids: Vec<i64> = {
        let mut stmt = tx.prepare(
            "SELECT id FROM groups WHERE collection = ?1 ORDER BY position, id",
        )?;
        let rows = stmt.query_map(params![collection.as_str()], |row| row.get::<_, i64>(0))?;
        rows.collect::<Result<Vec<_>>>()?
    };
    let from = match ids.iter().position(|&candidate| candidate == id) {
        Some(index) => index,
        None => return Ok(()),
    };
    ids.remove(from);
    let clamped = to_position.clamp(0, ids.len() as i64) as usize;
    ids.insert(clamped, id);
    for (index, group_id) in ids.iter().enumerate() {
        tx.execute(
            "UPDATE groups SET position = ?1 WHERE id = ?2",
            params![index as i64, group_id],
        )?;
    }
    tx.commit()
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
    let group = get_group(conn, group_id)?.ok_or(AssignError::Missing)?;
    if group.collection != collection {
        return Err(AssignError::CrossCollection {
            group_label: group.collection.label(),
        });
    }
    let changed = conn.execute(
        &format!(
            "UPDATE {} SET group_id = ?1 WHERE id = ?2",
            collection.table()
        ),
        params![group_id, item_id],
    )?;
    if changed == 0 {
        return Err(AssignError::Missing);
    }
    Ok(())
}

/// Clears an item's group reference — back to ungrouped. Unassigning an
/// already-ungrouped item succeeds; an item that no longer exists errors.
pub fn unassign_item(
    conn: &Connection,
    collection: Collection,
    item_id: i64,
) -> std::result::Result<(), AssignError> {
    let changed = conn.execute(
        &format!("UPDATE {} SET group_id = NULL WHERE id = ?1", collection.table()),
        params![item_id],
    )?;
    if changed == 0 {
        return Err(AssignError::Missing);
    }
    Ok(())
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
        }
    }

    fn action(name: &str) -> QuickActionInput {
        QuickActionInput {
            name: name.into(),
            command: "echo hi".into(),
            cwd: None,
            stoppable: false,
            stop_command: None,
        }
    }

    fn clip(name: &str) -> ClipInput {
        ClipInput {
            name: name.into(),
            content: format!("content of {name}"),
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
        move_group(&conn, another_launch.id, 0).unwrap();
        move_group(&conn, an_action.id, 5).unwrap(); // clamped within its own namespace

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
        let g2 = create_group(&conn, Collection::Action, "Two").unwrap();
        let item = quick_actions::create_quick_action(&conn, &action("Backup")).unwrap();

        assign_item(&conn, Collection::Action, item.id, g1.id).unwrap();
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
        let g2 = create_group(&conn, Collection::Clip, "Play").unwrap();
        let kept = clips::create_clip(&conn, &clip("token")).unwrap();
        assign_item(&conn, Collection::Clip, kept.id, g1.id).unwrap();

        delete_group(&conn, g1.id).unwrap();

        // The member survives, ungrouped.
        let stored = clips::list_clips(&conn).unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].id, kept.id);
        assert_eq!(stored[0].group_id, None);

        // The other collection's group is untouched and closes ranks.
        let remaining = list_groups(&conn, Collection::Clip).unwrap();
        assert_eq!(remaining.iter().map(|g| g.id).collect::<Vec<_>>(), vec![g2.id]);
        move_group(&conn, remaining[0].id, 3).unwrap(); // still reorderable

        delete_group(&conn, 99_999).unwrap_err(); // missing id is an error
    }

    #[test]
    fn order_by_group_lists_ungrouped_first_then_groups_in_user_order() {
        let conn = conn();
        let g1 = create_group(&conn, Collection::Action, "First").unwrap();
        let g2 = create_group(&conn, Collection::Action, "Second").unwrap();
        let in_g2 = quick_actions::create_quick_action(&conn, &action("a")).unwrap();
        let free = quick_actions::create_quick_action(&conn, &action("b")).unwrap();
        let in_g1 = quick_actions::create_quick_action(&conn, &action("c")).unwrap();
        assign_item(&conn, Collection::Action, in_g2.id, g2.id).unwrap();
        assign_item(&conn, Collection::Action, in_g1.id, g1.id).unwrap();

        // Reorder: Second now comes before First.
        move_group(&conn, g2.id, 0).unwrap();
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
        let g1 = Group { id: 7, collection: Collection::Launch, name: "Solo".into() };
        // No items at all: the empty group is still handed back, user order trivially.
        let sections = order_by_group(Vec::<i32>::new(), vec![g1.clone()], |_| None);
        assert!(sections.ungrouped.is_empty());
        assert_eq!(sections.grouped.len(), 1);
        assert!(sections.grouped[0].1.is_empty());

        // A dangling group reference never hides an item.
        let sections = order_by_group(vec![1, 2], vec![g1], |i| if *i == 1 { Some(404) } else { None });
        assert_eq!(sections.ungrouped, vec![1, 2]);
    }
}
