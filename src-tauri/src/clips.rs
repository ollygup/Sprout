//! Quick Clips (ticket 78): the machine-local, hand-authored plain-text list
//! with one-click re-copying — storage, validation, and ordering.
//!
//! Glossary (docs/CONTEXT.md): a **Clip** is a machine-local piece of plain
//! text stored for one-click re-copying; authored by hand (pasted into the
//! add dialog), ordered by the user, never captured in the background. All
//! editing happens in the main app's Quick Clips page; the Quick Launch
//! window's third tab is read-only (ticket 79). Machine-local like Quick
//! Actions — never part of Presets, Plan, or Preset exports.

use rusqlite::{params, Connection, OptionalExtension, Result};
use serde::{Deserialize, Serialize};

/// The editable shape of a Clip, as the frontend sends it. The stored record
/// ([`Clip`]) adds the id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipInput {
    /// Display name; an untitled clip stores the empty string and its
    /// content's first line stands in wherever the name is shown — the list
    /// stays readable without forcing the user to invent names.
    pub name: String,
    /// The plain text a copy puts back on the clipboard.
    pub content: String,
}

/// A Clip as stored: the input plus its library id. Position is internal
/// (order within the list) and never part of the payload — reorders go
/// through `move_clip`. `group_id` is the clip's optional Group membership
/// (ticket 89), assigned through the groups commands only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clip {
    pub id: i64,
    #[serde(flatten)]
    pub clip: ClipInput,
    /// The one Group this clip belongs to (`None` = ungrouped). Not part of
    /// the edit payload — assignments go through `assign_to_group`.
    #[serde(default)]
    pub group_id: Option<i64>,
}

/// Rejects clips that could never serve their purpose: blank text has nothing
/// to copy. The name adds no rejection by design — optional means optional,
/// and an empty name is a valid stored state.
pub fn validate_clip(clip: &ClipInput) -> std::result::Result<(), String> {
    if clip.content.trim().is_empty() {
        return Err("clip text can't be empty".into());
    }
    Ok(())
}

fn clip_from_row(row: &rusqlite::Row) -> Result<Clip> {
    Ok(Clip {
        id: row.get(0)?,
        clip: ClipInput {
            name: row.get(1)?,
            content: row.get(2)?,
        },
        group_id: row.get(3)?,
    })
}

/// Every Clip in list order (position, then insertion order).
pub fn list_clips(conn: &Connection) -> Result<Vec<Clip>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, content, group_id
         FROM clips ORDER BY position, id",
    )?;
    let rows = stmt.query_map([], clip_from_row)?;
    rows.collect()
}

/// Fetches one Clip by id — the clipboard-write command's lookup.
pub fn get_clip(conn: &Connection, id: i64) -> Result<Option<Clip>> {
    conn.query_row(
        "SELECT id, name, content, group_id FROM clips WHERE id = ?1",
        params![id],
        clip_from_row,
    )
    .optional()
}

/// The one INSERT shape for a Clip, position as the trailing placeholder —
/// shared by `create_clip` and `append_clip`.
const INSERT_CLIP_SQL: &str = "INSERT INTO clips (name, content, position)
     VALUES (?1, ?2, ?3)";

/// Appends a clip at the end of the list (the next free position). Name and
/// content store trimmed; an untitled clip persists the empty string.
pub fn create_clip(conn: &Connection, clip: &ClipInput) -> Result<Clip> {
    let id = crate::ordered_list::OrderedList::CLIPS.create_at_end(
        conn,
        INSERT_CLIP_SQL,
        &[&clip.name.trim(), &clip.content.trim()],
    )?;
    Ok(get_clip(conn, id)?.expect("just inserted"))
}

/// [`create_clip`]'s shape inside a caller-owned transaction — the whole-app
/// backup's merge appends every clip under ONE transaction.
pub(crate) fn append_clip(conn: &Connection, clip: &ClipInput) -> Result<()> {
    crate::ordered_list::OrderedList::CLIPS
        .append_at_end(conn, INSERT_CLIP_SQL, &[&clip.name.trim(), &clip.content.trim()])
        .map(|_| ())
}

/// Replaces a clip's text and name in place (same id). Position and the
/// Group reference are untouched — reorders go through `move_clip`, group
/// changes through `assign_to_group`/`unassign_from_group` (ticket 89).
pub fn update_clip(conn: &Connection, clip: &Clip) -> Result<()> {
    conn.execute(
        "UPDATE clips SET name = ?1, content = ?2 WHERE id = ?3",
        params![clip.clip.name.trim(), clip.clip.content.trim(), clip.id],
    )?;
    Ok(())
}

/// Removes a clip and compacts the positions so the list stays gapless.
pub fn delete_clip(conn: &Connection, id: i64) -> Result<()> {
    crate::ordered_list::OrderedList::CLIPS.delete(conn, id)
}

/// Moves a clip to `to_position` (clamped to the list), renumbering the rest.
/// The list is small (user config), so the same read-all-renumber-write
/// approach as the other ordered lists is the obviously-correct one.
pub fn move_clip(conn: &Connection, id: i64, to_position: i64) -> Result<()> {
    crate::ordered_list::OrderedList::CLIPS.move_to(conn, id, to_position)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        crate::db::init_at(&tempfile::tempdir().unwrap().into_path()).unwrap()
    }

    fn input(name: &str, content: &str) -> ClipInput {
        ClipInput {
            name: name.into(),
            content: content.into(),
        }
    }

    #[test]
    fn validation_rejects_blank_text_only() {
        assert!(validate_clip(&input("", "")).is_err());
        // Whitespace-only text is blank too.
        assert!(validate_clip(&input("", "   \n\t")).is_err());
        // The name is optional — an untitled clip is valid by design.
        assert!(validate_clip(&input("", "docker compose up -d")).is_ok());
        assert!(validate_clip(&input("greeting", "hello")).is_ok());
    }

    #[test]
    fn crud_roundtrips_across_reopen() {
        let dir = tempfile::tempdir().unwrap().into_path();
        {
            let conn = crate::db::init_at(&dir).unwrap();
            let first = create_clip(&conn, &input("reply", "Thanks for the report!"))
                .unwrap();
            let second = create_clip(&conn, &input("", "git status --short")).unwrap();
            create_clip(&conn, &input("addr", "127.0.0.1")).unwrap();
            assert_eq!(first.id, 1);
            assert_eq!(second.id, 2);
            let list = list_clips(&conn).unwrap();
            assert_eq!(list.len(), 3);
            assert_eq!(
                list.iter().map(|c| c.clip.content.as_str()).collect::<Vec<_>>(),
                vec![
                    "Thanks for the report!",
                    "git status --short",
                    "127.0.0.1"
                ]
            );
            // Update keeps the position.
            let mut updated = list[1].clone();
            updated.clip.content = "git status".into();
            update_clip(&conn, &updated).unwrap();
        }
        // Re-open: everything survives the connection.
        let conn = crate::db::init_at(&dir).unwrap();
        let list = list_clips(&conn).unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].clip.name, "reply");
        assert_eq!(list[1].clip.content, "git status");
        // An untitled clip stores the empty name.
        assert_eq!(list[1].clip.name, "");
        assert_eq!(list[2].clip.name, "addr");
    }

    #[test]
    fn values_are_trimmed_on_save() {
        let conn = conn();
        create_clip(&conn, &input("  padded  ", "\n  hello world\n")).unwrap();
        let stored = list_clips(&conn).unwrap();
        assert_eq!(stored[0].clip.name, "padded");
        assert_eq!(stored[0].clip.content, "hello world");
        // A whitespace-only name is still untitled.
        create_clip(&conn, &input("   ", "real text")).unwrap();
        let stored = list_clips(&conn).unwrap();
        assert_eq!(stored[1].clip.name, "");
    }

    #[test]
    fn delete_compacts_positions() {
        let conn = conn();
        create_clip(&conn, &input("A", "a")).unwrap();
        create_clip(&conn, &input("B", "b")).unwrap();
        create_clip(&conn, &input("C", "c")).unwrap();
        delete_clip(&conn, 1).unwrap();
        let list = list_clips(&conn).unwrap();
        assert_eq!(list.iter().map(|c| c.id).collect::<Vec<_>>(), vec![2, 3]);
        // A new clip lands at the end of the compacted list.
        create_clip(&conn, &input("D", "d")).unwrap();
        let list = list_clips(&conn).unwrap();
        assert_eq!(
            list.iter().map(|c| c.id).collect::<Vec<_>>(),
            vec![2, 3, 4]
        );
        // Deleting an unknown id is a no-op.
        delete_clip(&conn, 999).unwrap();
        assert_eq!(list_clips(&conn).unwrap().len(), 3);
    }

    #[test]
    fn move_swaps_reorders_and_clamps() {
        let conn = conn();
        for (name, text) in [("A", "a"), ("B", "b"), ("C", "c"), ("D", "d")] {
            create_clip(&conn, &input(name, text)).unwrap();
        }
        let ids = |conn: &Connection| {
            list_clips(conn)
                .unwrap()
                .into_iter()
                .map(|c| c.id)
                .collect::<Vec<_>>()
        };
        // Swap two neighbours: moving B down exchanges it with C.
        move_clip(&conn, 2, 2).unwrap();
        assert_eq!(ids(&conn), vec![1, 3, 2, 4]);
        // Move the last clip to the front.
        move_clip(&conn, 4, 0).unwrap();
        assert_eq!(ids(&conn), vec![4, 1, 3, 2]);
        // Move the first clip to the end.
        move_clip(&conn, 4, 99).unwrap();
        assert_eq!(ids(&conn), vec![1, 3, 2, 4]);
        // Out-of-range targets clamp.
        move_clip(&conn, 2, -5).unwrap();
        assert_eq!(ids(&conn), vec![2, 1, 3, 4]);
        // Same position is a no-op.
        move_clip(&conn, 3, 2).unwrap();
        assert_eq!(ids(&conn), vec![2, 1, 3, 4]);
        // Unknown id leaves the list untouched.
        move_clip(&conn, 999, 0).unwrap();
        assert_eq!(ids(&conn), vec![2, 1, 3, 4]);
    }
}
