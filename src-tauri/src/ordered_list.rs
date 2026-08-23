//! The ordered user-config lists (Launch entries, Quick Actions): one
//! implementation of the position discipline they share.
//!
//! Both entities persist the same four shapes — create at MAX(position)+1,
//! update in place without touching the position, delete with position
//! compaction, and move via read-all → remove/reinsert → renumber — differing
//! only in table name and row columns. This module owns those mechanics once;
//! `launch.rs` and `quick_actions.rs` are thin adapters over it.
//!
//! Positions stay gapless (0..n-1) through create/delete/move; the CRUD tests
//! on both entities are the guard. Table names are interpolated into SQL only
//! from the trusted internal constants below — never from user input.

use rusqlite::{params, Connection, OptionalExtension, Result, ToSql};

/// One ordered list's persistence seam: which table holds it.
pub(crate) struct OrderedList {
    table: &'static str,
}

impl OrderedList {
    pub(crate) const LAUNCH_ENTRIES: Self = Self {
        table: "launch_entries",
    };
    pub(crate) const QUICK_ACTIONS: Self = Self { table: "quick_actions" };

    /// Appends a row at MAX(position)+1 inside one transaction and returns its
    /// id. `insert_sql` is the entity's fixed INSERT with the position as the
    /// trailing placeholder (?N); `values` binds ?1..?N-1.
    pub(crate) fn create_at_end(
        &self,
        conn: &Connection,
        insert_sql: &str,
        values: &[&dyn ToSql],
    ) -> Result<i64> {
        let tx = conn.unchecked_transaction()?;
        let mut binds: Vec<&dyn ToSql> = values.to_vec();
        let position: i64 = tx.query_row(
            &format!(
                "SELECT COALESCE(MAX(position), -1) + 1 FROM {}",
                self.table
            ),
            [],
            |row| row.get(0),
        )?;
        binds.push(&position);
        tx.execute(insert_sql, binds.as_slice())?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(id)
    }

    /// Removes a row by id and compacts the positions so the list stays
    /// gapless. A missing id is a no-op.
    pub(crate) fn delete(&self, conn: &Connection, id: i64) -> Result<()> {
        let tx = conn.unchecked_transaction()?;
        let position: Option<i64> = tx
            .query_row(
                &format!("SELECT position FROM {} WHERE id = ?1", self.table),
                params![id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(position) = position {
            tx.execute(
                &format!("DELETE FROM {} WHERE id = ?1", self.table),
                params![id],
            )?;
            tx.execute(
                &format!(
                    "UPDATE {} SET position = position - 1 WHERE position > ?1",
                    self.table
                ),
                params![position],
            )?;
        }
        tx.commit()
    }

    /// Moves a row to `to_position` (clamped to the list), renumbering the
    /// rest. The lists are small (user config), so read-all-renumber-write in
    /// one transaction is the obviously-correct approach.
    pub(crate) fn move_to(&self, conn: &Connection, id: i64, to_position: i64) -> Result<()> {
        let tx = conn.unchecked_transaction()?;
        let mut ids: Vec<i64> = {
            let mut stmt = tx.prepare(&format!(
                "SELECT id FROM {} ORDER BY position, id",
                self.table
            ))?;
            let rows = stmt.query_map([], |row| row.get::<_, i64>(0))?;
            rows.collect::<Result<Vec<_>>>()?
        };
        let from = match ids.iter().position(|&candidate| candidate == id) {
            Some(index) => index,
            None => return Ok(()), // nothing to move
        };
        ids.remove(from);
        let clamped = to_position.clamp(0, ids.len() as i64) as usize;
        ids.insert(clamped, id);
        for (index, row_id) in ids.iter().enumerate() {
            tx.execute(
                &format!("UPDATE {} SET position = ?1 WHERE id = ?2", self.table),
                params![index as i64, row_id],
            )?;
        }
        tx.commit()
    }
}
