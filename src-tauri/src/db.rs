//! Lazy data initialization and persistence (ADR-0006).
//!
//! Everything lives under %LOCALAPPDATA%\Sprout, created on first launch —
//! never shipped by the installer. The Library (products + their default env
//! wiring), Presets, and Runs (with per-Requirement results) are stored in
//! `sprout.db`; the logs directory is created alongside. A fresh database
//! starts with zero Products (ADR-0008): entries exist only after the user
//! adds them from the live winget registry search.

use std::path::PathBuf;

use rusqlite::{params, Connection, OptionalExtension, Result};

use crate::domain::{EnvAction, EnvWiring, Preset, PresetRecord, Product, ProductRecord, Step};
use crate::run::{RequirementOutcome, RunOutcome, RunRecord, RunStatus, RunSummary};

/// Root data directory: %LOCALAPPDATA%\Sprout
pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(dirs::data_local_dir)
        .expect("LOCALAPPDATA is not defined on this system");
    base.join("Sprout")
}

pub fn logs_dir() -> PathBuf {
    data_dir().join("logs")
}

// The Logs tab (ticket 09) reports the database file's size.
pub fn db_path() -> PathBuf {
    data_dir().join("sprout.db")
}

/// Opens the database at `dir/sprout.db`, creating `dir` and `dir/logs` and
/// the schema on first use.
pub fn init() -> Result<Connection> {
    let dir = data_dir();
    let conn = init_at(&dir)?;
    Ok(conn)
}

/// Same as [`init`] but against an explicit directory (tests, future flags).
pub fn init_at(dir: &PathBuf) -> Result<Connection> {
    std::fs::create_dir_all(dir)
        .and_then(|_| std::fs::create_dir_all(dir.join("logs")))
        .map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(e))
        })?;

    let conn = Connection::open(dir.join("sprout.db"))?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS meta (
             key   TEXT PRIMARY KEY,
             value TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS products (
             id                     TEXT PRIMARY KEY,
             name                   TEXT NOT NULL,
             winget_id              TEXT,
             install_location_hint  TEXT,
             install_dir            TEXT,
             created_at             INTEGER NOT NULL DEFAULT 0,
             updated_at             INTEGER NOT NULL DEFAULT 0
         );
         CREATE TABLE IF NOT EXISTS product_env (
             product_id TEXT NOT NULL REFERENCES products(id) ON DELETE CASCADE,
             action     TEXT NOT NULL CHECK (action IN ('set', 'prepend')),
             name       TEXT NOT NULL,
             value      TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS presets (
             id          TEXT PRIMARY KEY,
             name        TEXT NOT NULL,
             description TEXT NOT NULL,
             version     TEXT NOT NULL,
             data        TEXT NOT NULL,
             imported    INTEGER NOT NULL DEFAULT 0
         );
         CREATE TABLE IF NOT EXISTS runs (
             id          TEXT PRIMARY KEY,
             started_at  INTEGER NOT NULL,
             finished_at INTEGER NOT NULL,
             presets     TEXT NOT NULL,
             outcome     TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS run_results (
             run_id          TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
             product_id      TEXT NOT NULL,
             product_name    TEXT NOT NULL,
             status          TEXT NOT NULL,
             detail          TEXT NOT NULL,
             reboot_required INTEGER NOT NULL DEFAULT 0,
             log_path        TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS groups (
             id         INTEGER PRIMARY KEY AUTOINCREMENT,
             collection TEXT NOT NULL CHECK (collection IN ('launch', 'action', 'clip')),
             name       TEXT NOT NULL,
             position   INTEGER NOT NULL DEFAULT 0
         );
         CREATE TABLE IF NOT EXISTS launch_entries (
             id          INTEGER PRIMARY KEY AUTOINCREMENT,
             name        TEXT NOT NULL,
             kind        TEXT NOT NULL CHECK (kind IN ('app', 'command')),
             target      TEXT NOT NULL,
             shell       TEXT CHECK (shell IN ('powershell', 'cmd', 'none')),
             show_window INTEGER NOT NULL DEFAULT 0,
             desktop_id  TEXT,
             position    INTEGER NOT NULL DEFAULT 0
         );
        CREATE TABLE IF NOT EXISTS quick_actions (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            name         TEXT NOT NULL,
            command      TEXT NOT NULL,
            cwd          TEXT,
            stoppable    INTEGER NOT NULL DEFAULT 0,
            stop_command TEXT,
            note         TEXT,
            notes        TEXT,
            position     INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS clips (
            id       INTEGER PRIMARY KEY AUTOINCREMENT,
            name     TEXT NOT NULL,
            content  TEXT NOT NULL,
            position INTEGER NOT NULL DEFAULT 0
        );",
    )?;
    ensure_preset_imported_column(conn)?;
    ensure_product_timestamps(conn)?;
    ensure_product_install_dir(conn)?;
    ensure_quick_action_stoppable(conn)?;
    ensure_quick_action_note(conn)?;
    ensure_item_group_columns(conn)
}

/// Upgrades databases created before Groups existed (any database from
/// tickets 01-88): creates the `groups` table (fresh databases already have
/// it) and adds the nullable `group_id` column to the three item tables.
/// Idempotent — re-runs change nothing.
fn ensure_item_group_columns(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS groups (
             id         INTEGER PRIMARY KEY AUTOINCREMENT,
             collection TEXT NOT NULL CHECK (collection IN ('launch', 'action', 'clip')),
             name       TEXT NOT NULL,
             position   INTEGER NOT NULL DEFAULT 0
         );",
    )?;
    for table in ["launch_entries", "quick_actions", "clips"] {
        let exists: bool = conn.query_row(
            &format!(
                "SELECT EXISTS(SELECT 1 FROM pragma_table_info('{table}') WHERE name = 'group_id')"
            ),
            [],
            |row| row.get(0),
        )?;
        if !exists {
            // ON DELETE SET NULL is the data-layer shape of ticket 89's
            // delete rule: removing a group returns its members to ungrouped.
            conn.execute_batch(&format!(
                "ALTER TABLE {table} ADD COLUMN group_id
                 REFERENCES groups(id) ON DELETE SET NULL"
            ))?;
        }
    }
    Ok(())
}

/// Upgrades databases created before the `imported` flag existed (dev
/// databases from tickets 01-02). Fresh databases already have the column.
fn ensure_preset_imported_column(conn: &Connection) -> Result<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('presets') WHERE name = 'imported')",
        [],
        |row| row.get(0),
    )?;
    if !exists {
        conn.execute_batch(
            "ALTER TABLE presets ADD COLUMN imported INTEGER NOT NULL DEFAULT 0",
        )?;
    }
    Ok(())
}

/// Upgrades databases created before the product timestamps existed (any
/// database from tickets 01-12): adds `created_at`/`updated_at` and backfills
/// pre-existing rows with the migration time so the More info surface never
/// shows an epoch. Fresh databases already have the columns.
fn ensure_product_timestamps(conn: &Connection) -> Result<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('products') WHERE name = 'created_at')",
        [],
        |row| row.get(0),
    )?;
    if !exists {
        conn.execute_batch(
            "ALTER TABLE products ADD COLUMN created_at INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE products ADD COLUMN updated_at INTEGER NOT NULL DEFAULT 0;
             UPDATE products SET created_at = strftime('%s','now'),
                                 updated_at = strftime('%s','now')
              WHERE created_at = 0",
        )?;
    }
    Ok(())
}

/// Upgrades databases created before the per-product install directory
/// existed (any database from tickets 01-35): adds the `install_dir` column.
/// Fresh databases already have it. Idempotent — re-runs change nothing.
fn ensure_product_install_dir(conn: &Connection) -> Result<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('products') WHERE name = 'install_dir')",
        [],
        |row| row.get(0),
    )?;
    if !exists {
        conn.execute_batch("ALTER TABLE products ADD COLUMN install_dir TEXT")?;
    }
    Ok(())
}

/// Upgrades databases created before the Quick Action run tracking existed
/// (any database from tickets 01-61): adds the `stoppable` flag and the
/// nullable `stop_command` to `quick_actions`. Fresh databases already have
/// both. Idempotent — re-runs change nothing.
fn ensure_quick_action_stoppable(conn: &Connection) -> Result<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('quick_actions') WHERE name = 'stoppable')",
        [],
        |row| row.get(0),
    )?;
    if !exists {
        conn.execute_batch(
            "ALTER TABLE quick_actions ADD COLUMN stoppable INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE quick_actions ADD COLUMN stop_command TEXT",
        )?;
    }
    Ok(())
}

/// Upgrades databases created before Quick Action notes existed (ticket 117):
/// adds the nullable `note` (and its alias `notes`) column to
/// `quick_actions`. Fresh databases already have both. Idempotent — re-runs
/// change nothing.
fn ensure_quick_action_note(conn: &Connection) -> Result<()> {
    let has_note: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('quick_actions') WHERE name = 'note')",
        [],
        |row| row.get(0),
    )?;
    if !has_note {
        conn.execute_batch("ALTER TABLE quick_actions ADD COLUMN note TEXT")?;
    }
    let has_notes: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('quick_actions') WHERE name = 'notes')",
        [],
        |row| row.get(0),
    )?;
    if !has_notes {
        conn.execute_batch("ALTER TABLE quick_actions ADD COLUMN notes TEXT")?;
    }
    // Keep the two names in sync for any pre-existing rows that had only one
    // of them populated (e.g. a DB migrated from a build that used a different
    // name). Best-effort — failures are ignored because the columns may still
    // be empty on a fresh DB.
    let _ = conn.execute(
        "UPDATE quick_actions SET notes = note WHERE notes IS NULL AND note IS NOT NULL",
        [],
    );
    let _ = conn.execute(
        "UPDATE quick_actions SET note = notes WHERE note IS NULL AND notes IS NOT NULL",
        [],
    );
    Ok(())
}

/// Unix seconds now — the one timestamp source for product create/update,
/// run start/finish, and log pruning.
pub(crate) fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// The install directory as stored: whitespace-trimmed, empty values become
/// `None` (winget's default), so only meaningful absolute paths persist.
fn normalized_install_dir(product: &Product) -> Option<String> {
    product
        .install_dir
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Inserts a Product row + its env wiring, stamping fresh create/update
/// times. No duplicate checks — callers own them (`create_product` for the
/// Library paths, the whole-app backup's merge for its transaction).
pub(crate) fn insert_product(conn: &Connection, product: &Product) -> Result<()> {
    let now = now_ts();
    conn.execute(
        "INSERT INTO products (id, name, winget_id, install_location_hint, install_dir,
                               created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            product.id,
            product.name,
            product.winget_id,
            product.install_location_hint,
            normalized_install_dir(product),
            now,
            now
        ],
    )?;
    insert_product_env(conn, &product.id, &product.default_env)
}

fn insert_product_env(conn: &Connection, product_id: &str, env: &[EnvWiring]) -> Result<()> {
    for item in env {
        conn.execute(
            "INSERT INTO product_env (product_id, action, name, value)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                product_id,
                match item.action {
                    EnvAction::Set => "set",
                    EnvAction::Prepend => "prepend",
                },
                item.name,
                item.value
            ],
        )?;
    }
    Ok(())
}

fn product_from_row(row: &rusqlite::Row) -> Result<ProductRecord> {
    Ok(ProductRecord {
        product: Product {
            id: row.get(0)?,
            name: row.get(1)?,
            winget_id: row.get(2)?,
            install_location_hint: row.get(3)?,
            install_dir: row.get(4)?,
            default_env: vec![],
        },
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn load_env(conn: &Connection, product_id: &str) -> Result<Vec<EnvWiring>> {
    let mut stmt = conn.prepare(
        "SELECT action, name, value FROM product_env
         WHERE product_id = ?1 ORDER BY rowid",
    )?;
    let rows = stmt.query_map(params![product_id], |row| {
        let action: String = row.get(0)?;
        Ok(EnvWiring {
            action: match action.as_str() {
                "prepend" => EnvAction::Prepend,
                _ => EnvAction::Set,
            },
            name: row.get(1)?,
            value: row.get(2)?,
        })
    })?;
    rows.collect()
}

/// Lists all Products, optionally filtered by a search query matched against
/// name and winget ID (case-insensitive, substring).
pub fn list_products(conn: &Connection, query: Option<&str>) -> Result<Vec<ProductRecord>> {
    let mut stmt = match query {
        Some(q) if !q.trim().is_empty() => conn.prepare(
            "SELECT id, name, winget_id, install_location_hint, install_dir, created_at, updated_at
             FROM products
             WHERE name LIKE '%' || ?1 || '%' OR winget_id LIKE '%' || ?1 || '%'
             ORDER BY name COLLATE NOCASE",
        )?,
        _ => conn.prepare(
            "SELECT id, name, winget_id, install_location_hint, install_dir, created_at, updated_at
             FROM products
             ORDER BY name COLLATE NOCASE",
        )?,
    };

    let rows = match query {
        Some(q) if !q.trim().is_empty() => {
            let like = q.trim();
            stmt.query_map(params![like], product_from_row)?
        }
        _ => stmt.query_map([], product_from_row)?,
    };

    let mut records = Vec::new();
    for row in rows {
        let mut record = row?;
        record.product.default_env = load_env(conn, &record.product.id)?;
        records.push(record);
    }
    Ok(records)
}

// Used by live-requirement resolution (ADR-0007): every local Preset read
// fills its Requirement references from the Library.
pub fn get_product(conn: &Connection, id: &str) -> Result<Option<ProductRecord>> {
    let row = conn
        .query_row(
            "SELECT id, name, winget_id, install_location_hint, install_dir, created_at, updated_at
             FROM products WHERE id = ?1",
            params![id],
            product_from_row,
        )
        .optional()?;
    match row {
        Some(mut record) => {
            record.product.default_env = load_env(conn, id)?;
            Ok(Some(record))
        }
        None => Ok(None),
    }
}

/// Why a Product write could not be applied (ticket 28): the id or name
/// collides with an existing Product (a friendly, user-facing message) or an
/// underlying storage error.
#[derive(Debug)]
pub enum ProductError {
    /// A Product with the same id or a case-insensitively matching name
    /// already exists in the Library.
    DuplicateName(String),
    Storage(rusqlite::Error),
}

impl std::fmt::Display for ProductError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProductError::DuplicateName(name) => {
                write!(f, "A product named '{name}' already exists.")
            }
            ProductError::Storage(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for ProductError {}

impl From<rusqlite::Error> for ProductError {
    fn from(err: rusqlite::Error) -> Self {
        ProductError::Storage(err)
    }
}

/// The name of an existing Product that collides with the proposed id or a
/// case-insensitively matching name — the duplicate pre-check for
/// create/update (ticket 28). `self_id` excludes the Product being updated,
/// so renaming a Product onto another's name fails while keeping its own
/// name passes.
fn colliding_product_name(
    conn: &Connection,
    id: &str,
    name: &str,
    self_id: Option<&str>,
) -> Result<Option<String>> {
    conn.query_row(
        "SELECT name FROM products
         WHERE (id = ?1 OR name = ?2 COLLATE NOCASE)
           AND (?3 IS NULL OR id != ?3)
         LIMIT 1",
        params![id.trim(), name.trim(), self_id],
        |row| row.get(0),
    )
    .optional()
}

pub fn create_product(conn: &Connection, product: &Product) -> std::result::Result<(), ProductError> {
    if let Some(name) = colliding_product_name(conn, &product.id, &product.name, None)? {
        return Err(ProductError::DuplicateName(name));
    }
    let tx = conn.unchecked_transaction().map_err(ProductError::Storage)?;
    insert_product(&tx, product).map_err(ProductError::Storage)?;
    tx.commit().map_err(ProductError::Storage)
}

pub fn update_product(conn: &Connection, product: &Product) -> std::result::Result<(), ProductError> {
    if let Some(name) =
        colliding_product_name(conn, &product.id, &product.name, Some(&product.id))?
    {
        return Err(ProductError::DuplicateName(name));
    }
    let tx = conn.unchecked_transaction().map_err(ProductError::Storage)?;
    let changed = tx
        .execute(
            "UPDATE products SET name = ?2, winget_id = ?3, install_location_hint = ?4,
                                 install_dir = ?5, updated_at = ?6
             WHERE id = ?1",
            params![
                product.id,
                product.name,
                product.winget_id,
                product.install_location_hint,
                normalized_install_dir(product),
                now_ts()
            ],
        )
        .map_err(ProductError::Storage)?;
    if changed == 0 {
        return Err(ProductError::Storage(rusqlite::Error::QueryReturnedNoRows));
    }
    tx.execute("DELETE FROM product_env WHERE product_id = ?1", params![product.id])
        .map_err(ProductError::Storage)?;
    insert_product_env(&tx, &product.id, &product.default_env).map_err(ProductError::Storage)?;
    tx.commit().map_err(ProductError::Storage)
}

pub fn delete_product(conn: &Connection, id: &str) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    let changed = tx.execute("DELETE FROM products WHERE id = ?1", params![id])?;
    if changed == 0 {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    drop_product_requirements(&tx, id)?;
    tx.commit()
}

/// How many local (non-imported) Presets reference a Product — the "N
/// preset(s)" behind the delete prompt (ADR-0007). Imported Presets are
/// snapshots, never count, and are never touched by a delete.
pub fn count_presets_using_product(conn: &Connection, product_id: &str) -> Result<usize> {
    let mut stmt = conn.prepare("SELECT id, data FROM presets WHERE imported = 0")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut count = 0;
    for row in rows {
        let (_, data) = row?;
        let preset: Preset = serde_json::from_str(&data)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        if preset
            .requirements
            .iter()
            .any(|req| req.product.id == product_id)
        {
            count += 1;
        }
    }
    Ok(count)
}

/// Deleting a Product drops the Requirements that reference it from local
/// Presets — their live link is gone (ADR-0007). Imported Presets keep their
/// embedded snapshot and are never touched; run history keeps its records.
fn drop_product_requirements(conn: &Connection, product_id: &str) -> Result<()> {
    let mut stmt = conn.prepare("SELECT id, data FROM presets WHERE imported = 0")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut affected = Vec::new();
    for row in rows {
        let (id, data) = row?;
        let mut preset: Preset = serde_json::from_str(&data)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let before = preset.requirements.len();
        preset.requirements.retain(|req| req.product.id != product_id);
        if preset.requirements.len() == before {
            continue;
        }
        let data = serde_json::to_string(&preset)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        affected.push((id, data));
    }
    drop(stmt);
    for (id, data) in affected {
        conn.execute("UPDATE presets SET data = ?2 WHERE id = ?1", params![id, data])?;
    }
    Ok(())
}

fn preset_from_row(row: &rusqlite::Row) -> Result<PresetRecord> {
    let id: String = row.get(0)?;
    let data: String = row.get(1)?;
    let imported: bool = row.get(2)?;
    let preset: Preset =
        serde_json::from_str(&data).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    Ok(PresetRecord { id, preset, imported })
}

/// The stored payload of a Preset (ADR-0007): a local Preset's Requirements
/// keep only their Product id as a live reference — the current name and
/// winget step are resolved from the Library on every read, so editing a
/// Product propagates without touching any Preset. Imported Presets are
/// snapshots and are stored exactly as authored, references included.
fn preset_to_row(record: &PresetRecord) -> Result<(String, String, i64)> {
    let mut preset = record.preset.clone();
    if !record.imported {
        for req in &mut preset.requirements {
            let id = req.product.id.clone();
            req.product = Product {
                id,
                name: String::new(),
                winget_id: None,
                install_location_hint: None,
                install_dir: None,
                default_env: vec![],
            };
            req.unresolved = false;
        }
    }
    let data = serde_json::to_string(&preset)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    Ok((record.id.clone(), data, i64::from(record.imported)))
}

/// Resolves a Preset's live references against the Library (ADR-0007):
/// local Presets' Requirements point at Library Products by id, and every
/// read fills in the current name, winget step, and install-location hint —
/// an edit to a Product propagates to every Preset that references it.
/// Requirements whose Product is gone are flagged `unresolved`, which the
/// composer shows as "product removed from library" and the plan excludes
/// from runs until the Product is re-added or the requirement is re-linked.
/// Imported Presets are snapshots and never resolve.
pub fn resolve_preset(conn: &Connection, record: &PresetRecord) -> Result<PresetRecord> {
    if record.imported {
        return Ok(record.clone());
    }
    let mut record = record.clone();
    for req in &mut record.preset.requirements {
        match get_product(conn, &req.product.id)? {
            Some(live) => {
                req.product.name = live.product.name.clone();
                req.product.winget_id = live.product.winget_id.clone();
                req.product.install_location_hint = live.product.install_location_hint.clone();
                req.product.install_dir = live.product.install_dir.clone();
                if let Step::Winget { id, .. } = &mut req.step {
                    if let Some(winget_id) = &live.product.winget_id {
                        *id = winget_id.clone();
                    }
                }
                req.unresolved = false;
            }
            None => req.unresolved = true,
        }
    }
    Ok(record)
}

/// Lists all Presets in the Library, ordered by name, with local Presets'
/// live references resolved (ADR-0007).
pub fn list_presets(conn: &Connection) -> Result<Vec<PresetRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, data, imported FROM presets ORDER BY name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], preset_from_row)?;
    let mut records = Vec::new();
    for row in rows {
        records.push(resolve_preset(conn, &row?)?);
    }
    Ok(records)
}

/// Fetches one Preset by its Library id, with local Presets' live references
/// resolved (ADR-0007).
pub fn get_preset(conn: &Connection, id: &str) -> Result<Option<PresetRecord>> {
    let record = conn
        .query_row(
            "SELECT id, data, imported FROM presets WHERE id = ?1",
            params![id],
            preset_from_row,
        )
        .optional()?;
    match record {
        Some(record) => Ok(Some(resolve_preset(conn, &record)?)),
        None => Ok(None),
    }
}

/// Inserts a Preset into the Library. The id is the Library key; the stored
/// payload is the preset in file shape (spec decision 11).
pub fn create_preset(conn: &Connection, record: &PresetRecord) -> Result<()> {
    let (id, data, imported) = preset_to_row(record)?;
    conn.execute(
        "INSERT INTO presets (id, name, description, version, data, imported)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            id,
            record.preset.name,
            record.preset.description,
            record.preset.version,
            data,
            imported
        ],
    )?;
    Ok(())
}

/// Replaces a Preset's payload in place (same id).
pub fn update_preset(conn: &Connection, record: &PresetRecord) -> Result<()> {
    let (id, data, imported) = preset_to_row(record)?;
    let changed = conn.execute(
        "UPDATE presets SET name = ?2, description = ?3, version = ?4, data = ?5, imported = ?6
         WHERE id = ?1",
        params![
            id,
            record.preset.name,
            record.preset.description,
            record.preset.version,
            data,
            imported
        ],
    )?;
    if changed == 0 {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    Ok(())
}

pub fn delete_preset(conn: &Connection, id: &str) -> Result<()> {
    let changed = conn.execute("DELETE FROM presets WHERE id = ?1", params![id])?;
    if changed == 0 {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    Ok(())
}

fn run_outcome_str(outcome: RunOutcome) -> &'static str {
    match outcome {
        RunOutcome::Ok => "ok",
        RunOutcome::WithNotes => "with_notes",
        RunOutcome::Failed => "failed",
        RunOutcome::Cancelled => "cancelled",
    }
}

fn run_outcome_from_str(value: &str) -> RunOutcome {
    match value {
        "failed" => RunOutcome::Failed,
        "cancelled" => RunOutcome::Cancelled,
        "with_notes" => RunOutcome::WithNotes,
        _ => RunOutcome::Ok,
    }
}

fn run_status_str(status: &RunStatus) -> &'static str {
    match status {
        RunStatus::Installed => "installed",
        RunStatus::Upgraded => "upgraded",
        RunStatus::AlreadyOk => "already_ok",
        RunStatus::SatisfiedByNewer => "satisfied_by_newer",
        RunStatus::SkippedUnmanaged => "skipped_unmanaged",
        RunStatus::Failed => "failed",
        RunStatus::TimedOut => "timed_out",
    }
}

fn run_status_from_str(value: &str) -> RunStatus {
    match value {
        "upgraded" => RunStatus::Upgraded,
        "already_ok" => RunStatus::AlreadyOk,
        "satisfied_by_newer" => RunStatus::SatisfiedByNewer,
        "skipped_unmanaged" => RunStatus::SkippedUnmanaged,
        "failed" => RunStatus::Failed,
        "timed_out" => RunStatus::TimedOut,
        _ => RunStatus::Installed,
    }
}

/// Persists a completed Run with its per-Requirement results.
pub fn create_run(conn: &Connection, record: &RunRecord) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO runs (id, started_at, finished_at, presets, outcome)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            record.id,
            record.started_at,
            record.finished_at,
            serde_json::to_string(&record.preset_names)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
            run_outcome_str(record.outcome)
        ],
    )?;
    for result in &record.results {
        tx.execute(
            "INSERT INTO run_results (run_id, product_id, product_name, status, detail,
                                      reboot_required, log_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                record.id,
                result.product_id,
                result.product_name,
                run_status_str(&result.status),
                result.detail,
                result.reboot_required,
                result.log_path
            ],
        )?;
    }
    tx.commit()
}

/// Lists every Run's summary row, newest first — the History screen (ticket
/// 09). The per-Requirement results stay out of the list and load on demand.
pub fn list_runs(conn: &Connection) -> Result<Vec<RunSummary>> {
    let mut stmt = conn.prepare(
        "SELECT id, started_at, finished_at, presets, outcome
         FROM runs ORDER BY started_at DESC, id DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let presets: String = row.get(3)?;
        let outcome: String = row.get(4)?;
        Ok(RunSummary {
            id,
            started_at: row.get(1)?,
            finished_at: row.get(2)?,
            preset_names: serde_json::from_str(&presets).unwrap_or_default(),
            outcome: run_outcome_from_str(&outcome),
        })
    })?;
    rows.collect()
}

/// Loads one Run with its per-Requirement results (the summary screen and the
/// worker's results on the main process side).
pub fn get_run(conn: &Connection, id: &str) -> Result<Option<RunRecord>> {
    let Some((started_at, finished_at, presets, outcome)) = conn
        .query_row(
            "SELECT started_at, finished_at, presets, outcome FROM runs WHERE id = ?1",
            params![id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?
    else {
        return Ok(None);
    };
    let preset_names: Vec<String> =
        serde_json::from_str(&presets).unwrap_or_default();
    let mut stmt = conn.prepare(
        "SELECT product_id, product_name, status, detail, reboot_required, log_path
         FROM run_results WHERE run_id = ?1 ORDER BY rowid",
    )?;
    let results = stmt
        .query_map(params![id], |row| {
            Ok(RequirementOutcome {
                product_id: row.get(0)?,
                product_name: row.get(1)?,
                status: run_status_from_str(&row.get::<_, String>(2)?),
                detail: row.get(3)?,
                reboot_required: row.get(4)?,
                log_path: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>>>()?;
    Ok(Some(RunRecord {
        id: id.to_string(),
        started_at,
        finished_at,
        preset_names,
        outcome: run_outcome_from_str(&outcome),
        results,
    }))
}

/// The Quick Launch dock's per-monitor memory (ticket 53): which screen edge
/// and visibility mode the dock last used on each monitor. Stored in the
/// `meta` table keyed by the monitor's device name (e.g. `\\.\DISPLAY1`), so
/// each monitor remembers its own dock — re-docking elsewhere uses that
/// monitor's remembered edge/mode, falling back to the Settings defaults the
/// first time.
const KEY_DOCK_EDGE_PREFIX: &str = "quicklaunch.dock.edge.";
const KEY_DOCK_MODE_PREFIX: &str = "quicklaunch.dock.mode.";
/// Dock width % per monitor (ticket 128): the Settings slider's per-display
/// override, stored per monitor so each screen remembers its own width —
/// falls back to the global `dock.width_pct`.
const KEY_DOCK_WIDTH_PCT_PREFIX: &str = "quicklaunch.dock.width_pct.";
/// Companion height ratio per monitor (ticket 125): the splitter position that
/// dock bottom 25–60% occupies, stored per monitor so each screen remembers its
/// own divider — falls back to the global settings ratio.
const KEY_COMPANION_HEIGHT_RATIO_PREFIX: &str = "quicklaunch.companion.height_ratio.";

/// The monitor-scoped meta key for a dock property.
fn dock_key(prefix: &str, monitor: &str) -> String {
    format!("{prefix}{monitor}")
}

/// Persists which edge the dock last used on `monitor`. The caller validates
/// the edge (the Settings validators); a broken value must never reach the
/// dock, and `load` guards reads regardless.
pub fn save_dock_edge(conn: &Connection, monitor: &str, edge: &str) -> Result<()> {
    upsert_meta(conn, &dock_key(KEY_DOCK_EDGE_PREFIX, monitor), edge)
}

/// Persists which visibility mode the dock last used on `monitor` (ticket 53).
pub fn save_dock_mode(conn: &Connection, monitor: &str, mode: &str) -> Result<()> {
    upsert_meta(conn, &dock_key(KEY_DOCK_MODE_PREFIX, monitor), mode)
}

/// The remembered edge for `monitor`, when a valid one is stored — broken
/// values (a leftover from a buggy build) read back as `None`, so the dock
/// falls back to the Settings default.
pub fn load_dock_edge(conn: &Connection, monitor: &str) -> Option<String> {
    read_meta(conn, &dock_key(KEY_DOCK_EDGE_PREFIX, monitor))
        .filter(|v| crate::settings::validate_dock_edge(v).is_ok())
}

/// The remembered visibility mode for `monitor`, when a valid one is stored.
pub fn load_dock_mode(conn: &Connection, monitor: &str) -> Option<String> {
    read_meta(conn, &dock_key(KEY_DOCK_MODE_PREFIX, monitor))
        .filter(|v| crate::settings::validate_dock_mode(v).is_ok())
}

/// Persists the dock width % for `monitor` (ticket 128). The caller validates
/// the %; a broken value must never reach the dock, and `load` guards reads
/// regardless.
pub fn save_dock_width_pct(conn: &Connection, monitor: &str, pct: u32) -> Result<()> {
    upsert_meta(conn, &dock_key(KEY_DOCK_WIDTH_PCT_PREFIX, monitor), &pct.to_string())
}

/// The remembered width % for `monitor`, when a valid one is stored — broken
/// values read back as `None`, so the dock falls back to the Settings default.
pub fn load_dock_width_pct(conn: &Connection, monitor: &str) -> Option<u32> {
    read_meta(conn, &dock_key(KEY_DOCK_WIDTH_PCT_PREFIX, monitor))
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|v| crate::settings::validate_dock_width_pct(*v).is_ok())
}

/// The remembered edge for a display, preferring its hardware-identity row
/// over the legacy device-name row (ticket 110): identity-keyed values win,
/// and a display with no resolvable identity — or no row under its identity —
/// reads the device-name key, so pre-upgrade memories survive.
pub fn load_dock_edge_identified(
    conn: &Connection,
    identity: Option<&str>,
    device_name: &str,
) -> Option<String> {
    identity
        .and_then(|id| load_dock_edge(conn, id))
        .or_else(|| load_dock_edge(conn, device_name))
}

/// The identified-shape twin of [`load_dock_edge_identified`] for the
/// visibility mode.
pub fn load_dock_mode_identified(
    conn: &Connection,
    identity: Option<&str>,
    device_name: &str,
) -> Option<String> {
    identity
        .and_then(|id| load_dock_mode(conn, id))
        .or_else(|| load_dock_mode(conn, device_name))
}

/// The identified-shape twin for the dock width % (ticket 128):
/// identity-keyed values win, falling back to device-name.
pub fn load_dock_width_pct_identified(
    conn: &Connection,
    identity: Option<&str>,
    device_name: &str,
) -> Option<u32> {
    identity
        .and_then(|id| load_dock_width_pct(conn, id))
        .or_else(|| load_dock_width_pct(conn, device_name))
}

/// Persists the companion height ratio for `monitor` (ticket 125). The caller
/// validates the ratio; a broken value must never reach the dock.
pub fn save_companion_height_ratio(conn: &Connection, monitor: &str, ratio: f64) -> Result<()> {
    upsert_meta(conn, &dock_key(KEY_COMPANION_HEIGHT_RATIO_PREFIX, monitor), &ratio.to_string())
}

/// The remembered companion height ratio for `monitor`, when a valid one is
/// stored — broken values read back as `None`, so the dock falls back to the
/// Settings default.
pub fn load_companion_height_ratio(conn: &Connection, monitor: &str) -> Option<f64> {
    read_meta(conn, &dock_key(KEY_COMPANION_HEIGHT_RATIO_PREFIX, monitor))
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| crate::settings::validate_companion_height_ratio(*v).is_ok())
}

/// The identified-shape twin for the companion height ratio (ticket 125):
/// identity-keyed values win, falling back to device-name.
pub fn load_companion_height_ratio_identified(
    conn: &Connection,
    identity: Option<&str>,
    device_name: &str,
) -> Option<f64> {
    identity
        .and_then(|id| load_companion_height_ratio(conn, id))
        .or_else(|| load_companion_height_ratio(conn, device_name))
}

fn read_meta(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM meta WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    )
    .ok()
}

/// The one key-value upsert into `meta` (settings, dock memory): insert or
/// overwrite in place.
pub(crate) fn upsert_meta(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

/// Validates a Product before it is written to the Library.
pub fn validate_product(product: &Product) -> std::result::Result<(), String> {
    if product.id.trim().is_empty() {
        return Err("Product id must not be empty".into());
    }
    if product.name.trim().is_empty() {
        return Err("Product name must not be empty".into());
    }
    // The per-product install directory follows the same rule as the global
    // default (ticket 36, ADR-0009): empty or an absolute path, never a
    // relative one that would mean different things per machine.
    crate::settings::validate_install_dir(product.install_dir.as_deref().unwrap_or(""))?;
    for item in &product.default_env {
        if item.name.trim().is_empty() {
            return Err("Every env wiring entry needs a variable name".into());
        }
        if item.value.trim().is_empty() {
            return Err(format!(
                "Env wiring '{}' needs a value",
                item.name
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir() -> PathBuf {
        // A unique directory per call: libtest reuses worker threads across
        // tests, so pid+thread-id dirs collide and re-run tests fail on
        // stale files. tempfile guarantees uniqueness.
        tempfile::tempdir().unwrap().into_path()
    }

    /// The Library starts empty (ADR-0008); tests that need a Product create
    /// their own.
    fn make_product(
        conn: &Connection,
        id: &str,
        name: &str,
        winget_id: Option<&str>,
        install_location_hint: Option<&str>,
    ) {
        create_product(
            conn,
            &Product {
                id: id.into(),
                name: name.into(),
                winget_id: winget_id.map(str::to_string),
                install_location_hint: install_location_hint.map(str::to_string),
                install_dir: None,
                default_env: vec![],
            },
        )
        .unwrap();
    }

    #[test]
    fn fresh_database_starts_empty_and_stays_empty_across_reopen() {
        let dir = test_dir();
        let conn = init_at(&dir).unwrap();
        assert_eq!(list_products(&conn, None).unwrap().len(), 0);

        // Re-opening the database changes nothing.
        drop(conn);
        let conn = init_at(&dir).unwrap();
        assert_eq!(list_products(&conn, None).unwrap().len(), 0);
    }

    #[test]
    fn search_matches_name_and_winget_id() {
        let dir = test_dir();
        let conn = init_at(&dir).unwrap();
        make_product(
            &conn,
            "docker",
            "Docker Desktop",
            Some("Docker.DockerDesktop"),
            None,
        );
        make_product(
            &conn,
            "openjdk21",
            "Eclipse Temurin OpenJDK 21 (LTS)",
            Some("EclipseAdoptium.Temurin.21.JDK"),
            Some("Eclipse Temurin"),
        );

        let by_name = list_products(&conn, Some("docker")).unwrap();
        assert_eq!(by_name.len(), 1);
        assert_eq!(by_name[0].product.id, "docker");

        let by_winget = list_products(&conn, Some("adoptium")).unwrap();
        assert_eq!(by_winget.len(), 1);
        assert_eq!(by_winget[0].product.id, "openjdk21");

        let none = list_products(&conn, Some("zzz-no-such")).unwrap();
        assert!(none.is_empty());

        let all = list_products(&conn, Some("  ")).unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn crud_roundtrip_persists_across_reopen() {
        let dir = test_dir();
        {
            let conn = init_at(&dir).unwrap();
            let made = Product {
                id: "my-tool".into(),
                name: "My Tool".into(),
                winget_id: Some("Vendor.MyTool".into()),
                install_location_hint: Some("MyTool".into()),
                install_dir: Some(r"D:\Apps".into()),
                default_env: vec![EnvWiring {
                    action: EnvAction::Set,
                    name: "MY_TOOL_HOME".into(),
                    value: "<InstallLocation:MyTool>".into(),
                }],
            };
            create_product(&conn, &made).unwrap();
            assert_eq!(get_product(&conn, "my-tool").unwrap().unwrap().product.default_env.len(), 1);
            assert_eq!(
                get_product(&conn, "my-tool").unwrap().unwrap().product.install_dir.as_deref(),
                Some(r"D:\Apps"),
                "the per-product install directory persists through create"
            );

            // Duplicate id must fail.
            assert!(create_product(&conn, &made).is_err());

            let renamed = Product {
                name: "My Tool Pro".into(),
                ..made.clone()
            };
            update_product(&conn, &renamed).unwrap();
            assert_eq!(
                get_product(&conn, "my-tool").unwrap().unwrap().product.name,
                "My Tool Pro"
            );

            delete_product(&conn, "my-tool").unwrap();
            assert!(get_product(&conn, "my-tool").unwrap().is_none());
        }
        // Re-open: everything from the previous connection persists — and
        // nothing came back with the deletion.
        let conn = init_at(&dir).unwrap();
        assert_eq!(list_products(&conn, None).unwrap().len(), 0);
        assert!(get_product(&conn, "my-tool").unwrap().is_none());
    }

    #[test]
    fn duplicate_product_id_or_name_rejected_with_friendly_error() {
        let dir = test_dir();
        let conn = init_at(&dir).unwrap();
        make_product(
            &conn,
            "dbeaver",
            "DBeaver",
            Some("DBeaver.DBeaver.Community"),
            None,
        );

        // Same id and same name — the insert must be refused before it
        // happens, with a message the dialog can show as-is.
        let dup = Product {
            id: "dbeaver".into(),
            name: "DBeaver".into(),
            winget_id: Some("DBeaver.DBeaver.Community".into()),
            install_location_hint: None,
            install_dir: None,
            default_env: vec![],
        };
        let err = create_product(&conn, &dup).unwrap_err();
        assert!(
            err.to_string().contains("DBeaver"),
            "duplicate must carry the friendly message, got: {err}"
        );

        // Duplicate name in a different case, different id: still refused.
        let made = Product {
            id: "my-tool".into(),
            name: "My Tool".into(),
            winget_id: None,
            install_location_hint: None,
            install_dir: None,
            default_env: vec![],
        };
        create_product(&conn, &made).unwrap();
        let twist = Product {
            id: "my-tool-2".into(),
            name: "MY TOOL".into(),
            ..made.clone()
        };
        let err = create_product(&conn, &twist).unwrap_err();
        assert!(err.to_string().contains("already exists"), "got: {err}");
    }

    #[test]
    fn update_product_blocks_renaming_onto_another_name_but_keeps_its_own() {
        let dir = test_dir();
        let conn = init_at(&dir).unwrap();
        make_product(
            &conn,
            "dbeaver",
            "DBeaver",
            Some("DBeaver.DBeaver.Community"),
            None,
        );
        let made = Product {
            id: "my-tool".into(),
            name: "My Tool".into(),
            winget_id: None,
            install_location_hint: None,
            install_dir: None,
            default_env: vec![],
        };
        create_product(&conn, &made).unwrap();

        // Renaming onto another product's name must fail, leaving the row
        // untouched.
        let onto_dbeaver = Product {
            name: "DBeaver".into(),
            ..made.clone()
        };
        let err = update_product(&conn, &onto_dbeaver).unwrap_err();
        assert!(err.to_string().contains("already exists"), "got: {err}");
        assert_eq!(
            get_product(&conn, "my-tool").unwrap().unwrap().product.name,
            "My Tool"
        );

        // Keeping one's own name (same id) must pass.
        let same = Product {
            name: "My Tool".into(),
            ..made.clone()
        };
        update_product(&conn, &same).unwrap();
        assert_eq!(
            get_product(&conn, "my-tool").unwrap().unwrap().product.name,
            "My Tool"
        );
    }

    #[test]
    fn timestamps_stamp_on_create_and_update() {
        let dir = test_dir();
        let conn = init_at(&dir).unwrap();
        let made = Product {
            id: "stamped".into(),
            name: "Stamped".into(),
            winget_id: None,
            install_location_hint: None,
            install_dir: None,
            default_env: vec![],
        };
        create_product(&conn, &made).unwrap();
        let created = get_product(&conn, "stamped").unwrap().unwrap();
        let created_at = created.created_at.expect("created_at set on create");
        let updated_at = created.updated_at.expect("updated_at set on create");
        assert!(created_at > 0, "created_at must be a real unix time");
        assert_eq!(created_at, updated_at, "a fresh product is born updated");

        // An update refreshes updated_at but never touches created_at.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let renamed = Product {
            name: "Stamped Renamed".into(),
            ..made
        };
        update_product(&conn, &renamed).unwrap();
        let after = get_product(&conn, "stamped").unwrap().unwrap();
        assert_eq!(after.created_at, Some(created_at));
        assert_eq!(after.product.name, "Stamped Renamed");
        assert!(
            after.updated_at.unwrap() > created_at,
            "updated_at must advance on update"
        );
    }

    #[test]
    fn migrates_databases_created_before_product_timestamps() {
        // A database created before ticket 13 (old products table without
        // created_at/updated_at) must migrate and backfill its rows.
        let dir = test_dir();
        std::fs::create_dir_all(&dir).unwrap();
        {
            let conn = Connection::open(dir.join("sprout.db")).unwrap();
            conn.execute_batch(
                "CREATE TABLE products (
                     id                     TEXT PRIMARY KEY,
                     name                   TEXT NOT NULL,
                     winget_id              TEXT,
                     install_location_hint  TEXT
                 );
                 INSERT INTO products (id, name, winget_id, install_location_hint)
                 VALUES ('legacy-tool', 'Legacy Tool', NULL, NULL);",
            )
            .unwrap();
        }
        let conn = init_at(&dir).unwrap();
        let has_created: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('products') WHERE name = 'created_at'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let has_updated: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('products') WHERE name = 'updated_at'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(has_created, 1);
        assert_eq!(has_updated, 1);
        // The per-product install directory column (ticket 36) is added by
        // the same migration chain — the old table never had it.
        let has_install_dir: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('products') WHERE name = 'install_dir'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(has_install_dir, 1);

        // Pre-existing rows read back with a sane backfill, not an epoch.
        let legacy = get_product(&conn, "legacy-tool").unwrap().unwrap();
        assert_eq!(legacy.product.name, "Legacy Tool");
        let backfilled = legacy.created_at.expect("legacy row backfilled");
        assert!(backfilled > 0, "backfilled time must not be the epoch");

        // New writes after the migration stamp real times.
        let made = Product {
            id: "fresh".into(),
            name: "Fresh".into(),
            winget_id: None,
            install_location_hint: None,
            install_dir: None,
            default_env: vec![],
        };
        create_product(&conn, &made).unwrap();
        let fresh = get_product(&conn, "fresh").unwrap().unwrap();
        assert_eq!(fresh.created_at, fresh.updated_at);
        assert!(fresh.created_at.unwrap() >= backfilled);
    }

    #[test]
    fn run_roundtrips_with_results_across_reopen() {
        use crate::run::RequirementOutcome;
        use crate::run::RunStatus;
        let dir = test_dir();
        let record = RunRecord {
            id: "run-test-1".into(),
            started_at: 1000,
            finished_at: 2000,
            preset_names: vec!["Backend dev box".into()],
            outcome: RunOutcome::Failed,
            results: vec![
                RequirementOutcome {
                    product_id: "git".into(),
                    product_name: "Git".into(),
                    status: RunStatus::Installed,
                    detail: "installed".into(),
                    reboot_required: false,
                    log_path: r"C:\logs\git.log".into(),
                },
                RequirementOutcome {
                    product_id: "docker".into(),
                    product_name: "Docker Desktop".into(),
                    status: RunStatus::Failed,
                    detail: "install failed (exit 5) — not installed".into(),
                    reboot_required: false,
                    log_path: r"C:\logs\docker.log".into(),
                },
                RequirementOutcome {
                    product_id: "vscode".into(),
                    product_name: "Visual Studio Code".into(),
                    status: RunStatus::Upgraded,
                    detail: "installed - reboot required to finish".into(),
                    reboot_required: true,
                    log_path: r"C:\logs\vscode.log".into(),
                },
            ],
        };
        {
            let conn = init_at(&dir).unwrap();
            create_run(&conn, &record).unwrap();
            let loaded = get_run(&conn, "run-test-1").unwrap().unwrap();
            assert_eq!(loaded, record);
        }
        // Re-open: the run survives the connection.
        let conn = init_at(&dir).unwrap();
        let loaded = get_run(&conn, "run-test-1").unwrap().unwrap();
        assert_eq!(loaded, record);
        assert!(get_run(&conn, "no-such-run").unwrap().is_none());
    }

    #[test]
    fn cancelled_run_outcome_roundtrips() {
        let dir = test_dir();
        let conn = init_at(&dir).unwrap();
        let record = RunRecord {
            id: "run-cancelled".into(),
            started_at: 1000,
            finished_at: 1500,
            preset_names: vec![],
            outcome: RunOutcome::Cancelled,
            results: vec![],
        };
        create_run(&conn, &record).unwrap();
        let loaded = get_run(&conn, "run-cancelled").unwrap().unwrap();
        assert_eq!(loaded.outcome, RunOutcome::Cancelled);
        assert_eq!(loaded, record);
    }

    #[test]
    fn with_notes_run_outcome_roundtrips() {
        let dir = test_dir();
        let conn = init_at(&dir).unwrap();
        let record = RunRecord {
            id: "run-with-notes".into(),
            started_at: 1000,
            finished_at: 1500,
            preset_names: vec!["Backend dev box".into()],
            outcome: RunOutcome::WithNotes,
            results: vec![],
        };
        create_run(&conn, &record).unwrap();
        let loaded = get_run(&conn, "run-with-notes").unwrap().unwrap();
        assert_eq!(loaded.outcome, RunOutcome::WithNotes);
        assert_eq!(loaded, record);
    }

    #[test]
    fn run_list_is_newest_first_without_results() {
        use crate::run::RunSummary;
        let dir = test_dir();
        let conn = init_at(&dir).unwrap();
        let record = |id: &str, started: i64, outcome: RunOutcome| RunRecord {
            id: id.into(),
            started_at: started,
            finished_at: started + 500,
            preset_names: vec![format!("Preset of {id}")],
            outcome,
            results: vec![],
        };
        create_run(&conn, &record("run-old", 1000, RunOutcome::Ok)).unwrap();
        create_run(&conn, &record("run-new", 3000, RunOutcome::Failed)).unwrap();
        create_run(&conn, &record("run-mid", 2000, RunOutcome::Cancelled)).unwrap();

        let runs = list_runs(&conn).unwrap();
        assert_eq!(
            runs,
            vec![
                RunSummary {
                    id: "run-new".into(),
                    started_at: 3000,
                    finished_at: 3500,
                    preset_names: vec!["Preset of run-new".into()],
                    outcome: RunOutcome::Failed,
                },
                RunSummary {
                    id: "run-mid".into(),
                    started_at: 2000,
                    finished_at: 2500,
                    preset_names: vec!["Preset of run-mid".into()],
                    outcome: RunOutcome::Cancelled,
                },
                RunSummary {
                    id: "run-old".into(),
                    started_at: 1000,
                    finished_at: 1500,
                    preset_names: vec!["Preset of run-old".into()],
                    outcome: RunOutcome::Ok,
                },
            ]
        );
        // The list never carries the per-Requirement detail.
        assert!(get_run(&conn, "run-new").unwrap().is_some());
    }

    #[test]
    fn install_dir_roundtrips_updates_and_clears() {
        let dir = test_dir();
        let made = Product {
            id: "placed".into(),
            name: "Placed Tool".into(),
            winget_id: Some("Vendor.Placed".into()),
            install_location_hint: None,
            install_dir: Some(r"D:\Apps".into()),
            default_env: vec![],
        };
        {
            let conn = init_at(&dir).unwrap();
            create_product(&conn, &made).unwrap();
            // The override reads back as stored.
            assert_eq!(
                get_product(&conn, "placed").unwrap().unwrap().product.install_dir.as_deref(),
                Some(r"D:\Apps")
            );
            // An update moves the override; an empty value clears it.
            let moved = Product {
                install_dir: Some(r"E:\Tools".into()),
                ..made.clone()
            };
            update_product(&conn, &moved).unwrap();
            assert_eq!(
                get_product(&conn, "placed").unwrap().unwrap().product.install_dir.as_deref(),
                Some(r"E:\Tools")
            );
            let cleared = Product {
                install_dir: Some("   ".into()),
                ..made
            };
            update_product(&conn, &cleared).unwrap();
            assert_eq!(
                get_product(&conn, "placed").unwrap().unwrap().product.install_dir,
                None,
                "a whitespace-only value clears the override"
            );
        }
        // Re-open: the cleared override stays cleared, nothing is resurrected.
        let conn = init_at(&dir).unwrap();
        assert_eq!(get_product(&conn, "placed").unwrap().unwrap().product.install_dir, None);
    }

    #[test]
    fn validate_rejects_relative_install_dir() {
        let good = Product {
            id: "x".into(),
            name: "x".into(),
            winget_id: None,
            install_location_hint: None,
            install_dir: Some(r"D:\Apps".into()),
            default_env: vec![],
        };
        assert!(validate_product(&good).is_ok());

        let relative = Product {
            install_dir: Some("Apps".into()),
            ..good.clone()
        };
        let err = validate_product(&relative).unwrap_err();
        assert!(err.contains("absolute"), "got: {err}");

        let empty = Product {
            install_dir: Some(String::new()),
            ..good
        };
        assert!(validate_product(&empty).is_ok(), "empty means winget's default");
    }

    #[test]
    fn validate_rejects_blank_fields() {
        let bad = Product {
            id: "".into(),
            name: "x".into(),
            winget_id: None,
            install_location_hint: None,
            install_dir: None,
            default_env: vec![],
        };
        assert!(validate_product(&bad).is_err());

        let bad_env = Product {
            id: "x".into(),
            name: "x".into(),
            winget_id: None,
            install_location_hint: None,
            install_dir: None,
            default_env: vec![EnvWiring {
                action: EnvAction::Set,
                name: "".into(),
                value: "v".into(),
            }],
        };
        assert!(validate_product(&bad_env).is_err());

        let good = Product {
            id: "x".into(),
            name: "x".into(),
            winget_id: None,
            install_location_hint: None,
            install_dir: None,
            default_env: vec![],
        };
        assert!(validate_product(&good).is_ok());
    }

    fn test_preset(id: &str) -> PresetRecord {
        PresetRecord {
            id: id.into(),
            imported: false,
            preset: Preset {
                schema_version: 1,
                platform: "windows".into(),
                name: format!("Preset {id}"),
                description: "A test preset".into(),
                author: "Tester".into(),
                version: "1".into(),
                requirements: vec![crate::domain::Requirement {
                    product: Product {
                        id: "openjdk21".into(),
                        name: "Eclipse Temurin OpenJDK 21 (LTS)".into(),
                        winget_id: Some("EclipseAdoptium.Temurin.21.JDK".into()),
                        install_location_hint: None,
                        install_dir: None,
                        default_env: vec![],
                    },
                    step: crate::domain::Step::Winget {
                        id: "EclipseAdoptium.Temurin.21.JDK".into(),
                        scope: "machine".into(),
                    },
                    version_policy: crate::domain::VersionPolicy::Latest,
                    depends_on: vec![],
                    timeout_minutes: 10,
                    env: vec![],
                    verify: vec![],
                    unresolved: false,
                }],
            },
        }
    }

    #[test]
    fn preset_crud_roundtrips_persists_across_reopen() {
        let dir = test_dir();
        {
            let conn = init_at(&dir).unwrap();
            assert!(list_presets(&conn).unwrap().is_empty());
            // The live link resolves against a Library product the test
            // created itself — nothing is pre-seeded (ADR-0008).
            make_product(
                &conn,
                "openjdk21",
                "Eclipse Temurin OpenJDK 21 (LTS)",
                Some("EclipseAdoptium.Temurin.21.JDK"),
                Some("Eclipse Temurin"),
            );

            let made = test_preset("backend-dev-box");
            create_preset(&conn, &made).unwrap();
            assert_eq!(list_presets(&conn).unwrap().len(), 1);
            let loaded = get_preset(&conn, "backend-dev-box").unwrap().unwrap();
            assert!(!loaded.imported);
            assert_eq!(loaded.preset.requirements.len(), 1);
            assert_eq!(
                loaded.preset.requirements[0].version_policy,
                crate::domain::VersionPolicy::Latest
            );
            // The read is live-resolved (ADR-0007): the Library product's
            // current fields win over the embedded copy.
            let req = &loaded.preset.requirements[0];
            assert_eq!(req.product.id, "openjdk21");
            assert_eq!(req.product.name, "Eclipse Temurin OpenJDK 21 (LTS)");
            assert_eq!(req.product.winget_id.as_deref(), Some("EclipseAdoptium.Temurin.21.JDK"));
            assert_eq!(req.product.install_location_hint.as_deref(), Some("Eclipse Temurin"));
            assert!(!req.unresolved);

            // Duplicate id must fail.
            assert!(create_preset(&conn, &made).is_err());

            // Update in place keeps the id.
            let renamed = PresetRecord {
                id: "backend-dev-box".into(),
                imported: false,
                preset: Preset {
                    name: "Backend Dev Box Pro".into(),
                    version: "2".into(),
                    ..made.preset.clone()
                },
            };
            update_preset(&conn, &renamed).unwrap();
            let loaded = get_preset(&conn, "backend-dev-box").unwrap().unwrap();
            assert_eq!(loaded.preset.name, "Backend Dev Box Pro");
            assert_eq!(loaded.preset.version, "2");

            // Missing id on update/delete fails.
            assert!(update_preset(&conn, &test_preset("nope")).is_err());
            assert!(delete_preset(&conn, "nope").is_err());

            delete_preset(&conn, "backend-dev-box").unwrap();
            assert!(get_preset(&conn, "backend-dev-box").unwrap().is_none());
        }
        // Re-open: nothing from the previous connection is lost.
        let conn = init_at(&dir).unwrap();
        assert!(list_presets(&conn).unwrap().is_empty());
    }

    #[test]
    fn imported_flag_persists_and_old_databases_upgrade() {
        // A database created before the `imported` column existed (tickets
        // 01-02 dev databases) must migrate and keep its presets.
        let dir = test_dir();
        {
            let conn = init_at(&dir).unwrap();
            let made = PresetRecord {
                imported: true,
                ..test_preset("imported-flag")
            };
            create_preset(&conn, &made).unwrap();
        }
        let conn = init_at(&dir).unwrap();
        let loaded = get_preset(&conn, "imported-flag").unwrap().unwrap();
        assert!(loaded.imported);

        let has_column: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('presets') WHERE name = 'imported'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(has_column, 1);
    }

    #[test]
    fn migrates_databases_created_before_imported_column() {
        let dir = test_dir();
        std::fs::create_dir_all(&dir).unwrap();
        // Simulate a ticket-02 database: old presets table without the
        // `imported` column, plus one existing preset row.
        {
            let conn = Connection::open(dir.join("sprout.db")).unwrap();
            conn.execute_batch(
                "CREATE TABLE meta (
                     key   TEXT PRIMARY KEY,
                     value TEXT NOT NULL
                 );
                 CREATE TABLE products (
                     id                     TEXT PRIMARY KEY,
                     name                   TEXT NOT NULL,
                     winget_id              TEXT,
                     install_location_hint  TEXT
                 );
                 CREATE TABLE product_env (
                     product_id TEXT NOT NULL REFERENCES products(id) ON DELETE CASCADE,
                     action     TEXT NOT NULL CHECK (action IN ('set', 'prepend')),
                     name       TEXT NOT NULL,
                     value      TEXT NOT NULL
                 );
                 CREATE TABLE presets (
                     id          TEXT PRIMARY KEY,
                     name        TEXT NOT NULL,
                     description TEXT NOT NULL,
                     version     TEXT NOT NULL,
                     data        TEXT NOT NULL
                 );",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO presets (id, name, description, version, data)
                 VALUES ('old-box', 'Old Box', 'pre-03', '1',
                         '{\"schema_version\":1,\"platform\":\"windows\",\"name\":\"Old Box\",\"description\":\"pre-03\",\"author\":\"\",\"version\":\"1\",\"requirements\":[]}')",
                [],
            )
            .unwrap();
        }
        let conn = init_at(&dir).unwrap();
        let has_column: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('presets') WHERE name = 'imported'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(has_column, 1);
        // Pre-existing rows keep their payload and read back as local.
        let old = get_preset(&conn, "old-box").unwrap().unwrap();
        assert_eq!(old.preset.name, "Old Box");
        assert!(!old.imported);
    }

    #[test]
    fn migrates_databases_created_before_launch_entries() {
        let dir = test_dir();
        std::fs::create_dir_all(&dir).unwrap();
        // Simulate a ticket-36 database: the full pre-38 schema without the
        // launch_entries table, plus one existing product row.
        {
            let conn = Connection::open(dir.join("sprout.db")).unwrap();
            conn.execute_batch(
                "CREATE TABLE meta (
                     key   TEXT PRIMARY KEY,
                     value TEXT NOT NULL
                 );
                 CREATE TABLE products (
                     id                     TEXT PRIMARY KEY,
                     name                   TEXT NOT NULL,
                     winget_id              TEXT,
                     install_location_hint  TEXT,
                     install_dir            TEXT,
                     created_at             INTEGER NOT NULL DEFAULT 0,
                     updated_at             INTEGER NOT NULL DEFAULT 0
                 );
                 CREATE TABLE product_env (
                     product_id TEXT NOT NULL REFERENCES products(id) ON DELETE CASCADE,
                     action     TEXT NOT NULL CHECK (action IN ('set', 'prepend')),
                     name       TEXT NOT NULL,
                     value      TEXT NOT NULL
                 );
                 CREATE TABLE presets (
                     id          TEXT PRIMARY KEY,
                     name        TEXT NOT NULL,
                     description TEXT NOT NULL,
                     version     TEXT NOT NULL,
                     data        TEXT NOT NULL,
                     imported    INTEGER NOT NULL DEFAULT 0
                 );
                 CREATE TABLE runs (
                     id          TEXT PRIMARY KEY,
                     started_at  INTEGER NOT NULL,
                     finished_at INTEGER NOT NULL,
                     presets     TEXT NOT NULL,
                     outcome     TEXT NOT NULL
                 );
                 CREATE TABLE run_results (
                     run_id          TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
                     product_id      TEXT NOT NULL,
                     product_name    TEXT NOT NULL,
                     status          TEXT NOT NULL,
                     detail          TEXT NOT NULL,
                     reboot_required INTEGER NOT NULL DEFAULT 0,
                     log_path        TEXT NOT NULL
                 );",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO products (id, name, winget_id, created_at, updated_at)
                 VALUES ('vscode', 'Visual Studio Code', 'Microsoft.VisualStudioCode', 0, 0)",
                [],
            )
            .unwrap();
        }
        let conn = init_at(&dir).unwrap();
        let has_table: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'launch_entries'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(has_table, 1);
        // Pre-existing data survives, and the new table is usable.
        assert_eq!(get_product(&conn, "vscode").unwrap().unwrap().product.name, "Visual Studio Code");
        let entry = crate::launch::LaunchEntryInput {
            name: "Code".into(),
            kind: crate::launch::LaunchEntryKind::App,
            target: r"C:\Program Files\Microsoft VS Code\Code.exe".into(),
            shell: None,
            show_window: false,
            desktop_id: None,
        };
        crate::launch::create_launch_entry(&conn, &entry).unwrap();
        assert_eq!(crate::launch::list_launch_entries(&conn).unwrap().len(), 1);
    }

    #[test]
    fn migrates_databases_created_before_quick_actions() {
        let dir = test_dir();
        std::fs::create_dir_all(&dir).unwrap();
        // Simulate a pre-50 database: the full pre-38 schema without the
        // quick_actions table, plus one existing product row.
        {
            let conn = Connection::open(dir.join("sprout.db")).unwrap();
            conn.execute_batch(
                "CREATE TABLE meta (
                     key   TEXT PRIMARY KEY,
                     value TEXT NOT NULL
                 );
                 CREATE TABLE products (
                     id                     TEXT PRIMARY KEY,
                     name                   TEXT NOT NULL,
                     winget_id              TEXT,
                     install_location_hint  TEXT,
                     install_dir            TEXT,
                     created_at             INTEGER NOT NULL DEFAULT 0,
                     updated_at             INTEGER NOT NULL DEFAULT 0
                 );
                 CREATE TABLE product_env (
                     product_id TEXT NOT NULL REFERENCES products(id) ON DELETE CASCADE,
                     action     TEXT NOT NULL CHECK (action IN ('set', 'prepend')),
                     name       TEXT NOT NULL,
                     value      TEXT NOT NULL
                 );
                 CREATE TABLE presets (
                     id          TEXT PRIMARY KEY,
                     name        TEXT NOT NULL,
                     description TEXT NOT NULL,
                     version     TEXT NOT NULL,
                     data        TEXT NOT NULL,
                     imported    INTEGER NOT NULL DEFAULT 0
                 );
                 CREATE TABLE runs (
                     id          TEXT PRIMARY KEY,
                     started_at  INTEGER NOT NULL,
                     finished_at INTEGER NOT NULL,
                     presets     TEXT NOT NULL,
                     outcome     TEXT NOT NULL
                 );
                 CREATE TABLE run_results (
                     run_id          TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
                     product_id      TEXT NOT NULL,
                     product_name    TEXT NOT NULL,
                     status          TEXT NOT NULL,
                     detail          TEXT NOT NULL,
                     reboot_required INTEGER NOT NULL DEFAULT 0,
                     log_path        TEXT NOT NULL
                 );",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO products (id, name, winget_id, created_at, updated_at)
                 VALUES ('vscode', 'Visual Studio Code', 'Microsoft.VisualStudioCode', 0, 0)",
                [],
            )
            .unwrap();
        }
        let conn = init_at(&dir).unwrap();
        let has_table: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'quick_actions'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(has_table, 1);
        // Pre-existing data survives, and the new table is usable.
        assert_eq!(get_product(&conn, "vscode").unwrap().unwrap().product.name, "Visual Studio Code");
        let action = crate::quick_actions::QuickActionInput {
            name: "docker-start".into(),
            command: "docker compose up -d".into(),
            cwd: None,
            stoppable: false,
            stop_command: None,
            note: None,
        };
        crate::quick_actions::create_quick_action(&conn, &action).unwrap();
        assert_eq!(crate::quick_actions::list_quick_actions(&conn).unwrap().len(), 1);
    }

    #[test]
    fn migrates_quick_actions_created_before_run_tracking() {
        let dir = test_dir();
        std::fs::create_dir_all(&dir).unwrap();
        // Simulate a pre-62 database: the quick_actions table exists but has
        // neither `stoppable` nor `stop_command`, with one existing row.
        {
            let conn = Connection::open(dir.join("sprout.db")).unwrap();
            conn.execute_batch(
                "CREATE TABLE meta (
                     key   TEXT PRIMARY KEY,
                     value TEXT NOT NULL
                 );
                 CREATE TABLE quick_actions (
                     id       INTEGER PRIMARY KEY AUTOINCREMENT,
                     name     TEXT NOT NULL,
                     command  TEXT NOT NULL,
                     cwd      TEXT,
                     position INTEGER NOT NULL DEFAULT 0
                 );",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO quick_actions (name, command, cwd, position)
                 VALUES ('docker-start', 'docker compose up -d', NULL, 0)",
                [],
            )
            .unwrap();
        }
        let conn = init_at(&dir).unwrap();
        // Pre-existing data survives and reads back with the defaults: not
        // stoppable, no stop command.
        let list = crate::quick_actions::list_quick_actions(&conn).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].action.name, "docker-start");
        assert!(!list[0].action.stoppable);
        assert_eq!(list[0].action.stop_command, None);
        // The migrated table is fully usable for the new fields.
        let mut tracked = crate::quick_actions::QuickActionInput {
            name: "dev-services".into(),
            command: "docker compose up".into(),
            cwd: None,
            stoppable: true,
            stop_command: Some("docker compose stop".into()),
            note: None,
        };
        crate::quick_actions::create_quick_action(&conn, &tracked).unwrap();
        let list = crate::quick_actions::list_quick_actions(&conn).unwrap();
        assert!(list[1].action.stoppable);
        assert_eq!(
            list[1].action.stop_command.as_deref(),
            Some("docker compose stop")
        );
        // The migration is idempotent — re-running init changes nothing.
        drop(conn);
        let conn = init_at(&dir).unwrap();
        tracked.name = "renamed".into();
        crate::quick_actions::update_quick_action(
            &conn,
            &crate::quick_actions::QuickAction { id: list[1].id, action: tracked, group_id: None },
        )
        .unwrap();
        assert_eq!(
            crate::quick_actions::list_quick_actions(&conn).unwrap().len(),
            2
        );
    }

    #[test]
    fn preset_payload_is_the_file_shape() {
        let dir = test_dir();
        let conn = init_at(&dir).unwrap();
        let made = test_preset("file-shape");
        create_preset(&conn, &made).unwrap();
        let data: String = conn
            .query_row(
                "SELECT data FROM presets WHERE id = 'file-shape'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&data).unwrap();
        // The stored payload carries no Library id — it is export-ready.
        assert!(value.get("id").is_none());
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["name"], "Preset file-shape");
        assert_eq!(value["requirements"][0]["product"]["id"], "openjdk21");
        assert_eq!(value["requirements"][0]["version_policy"]["kind"], "latest");
        // The stored product is a live reference, not a snapshot (ADR-0007).
        assert_eq!(value["requirements"][0]["product"]["name"], "");
        assert!(value["requirements"][0]["unresolved"].is_null());
    }

    fn reference_req(product_id: &str, winget_id: &str) -> crate::domain::Requirement {
        crate::domain::Requirement {
            product: Product {
                id: product_id.into(),
                name: String::new(),
                winget_id: None,
                install_location_hint: None,
                install_dir: None,
                default_env: vec![],
            },
            step: crate::domain::Step::Winget {
                id: winget_id.into(),
                scope: "machine".into(),
            },
            version_policy: crate::domain::VersionPolicy::Latest,
            depends_on: vec![],
            timeout_minutes: 10,
            env: vec![],
            verify: vec![],
            unresolved: false,
        }
    }

    fn test_preset_with(id: &str, requirements: Vec<crate::domain::Requirement>) -> PresetRecord {
        PresetRecord {
            id: id.into(),
            imported: false,
            preset: Preset {
                schema_version: 1,
                platform: "windows".into(),
                name: format!("Preset {id}"),
                description: "A test preset".into(),
                author: "Tester".into(),
                version: "1".into(),
                requirements,
            },
        }
    }

    #[test]
    fn editing_a_product_propagates_to_referencing_local_presets() {
        let dir = test_dir();
        let conn = init_at(&dir).unwrap();
        create_product(
            &conn,
            &Product {
                id: "my-tool".into(),
                name: "My Tool".into(),
                winget_id: Some("Vendor.MyTool".into()),
                install_location_hint: None,
                install_dir: None,
                default_env: vec![],
            },
        )
        .unwrap();
        create_preset(
            &conn,
            &test_preset_with(
                "uses-my-tool",
                vec![crate::domain::Requirement {
                    product: Product {
                        id: "my-tool".into(),
                        name: "Stale Snapshot Name".into(),
                        winget_id: Some("Stale.Winget".into()),
                        install_location_hint: None,
                        install_dir: None,
                        default_env: vec![],
                    },
                    step: crate::domain::Step::Winget {
                        id: "Stale.Winget".into(),
                        scope: "machine".into(),
                    },
                    version_policy: crate::domain::VersionPolicy::Latest,
                    depends_on: vec![],
                    timeout_minutes: 10,
                    env: vec![],
                    verify: vec![],
                    unresolved: false,
                }],
            ),
        )
        .unwrap();

        // The read resolves the current name and winget step from the
        // Library — the embedded copy never reaches the wire.
        let loaded = get_preset(&conn, "uses-my-tool").unwrap().unwrap();
        assert_eq!(loaded.preset.requirements[0].product.name, "My Tool");
        assert_eq!(
            loaded.preset.requirements[0].step,
            crate::domain::Step::Winget {
                id: "Vendor.MyTool".into(),
                scope: "machine".into()
            }
        );
        assert!(!loaded.preset.requirements[0].unresolved);

        // Editing the Product propagates with no preset edit at all.
        update_product(
            &conn,
            &Product {
                id: "my-tool".into(),
                name: "My Tool Pro".into(),
                winget_id: Some("New.Vendor.MyTool".into()),
                install_location_hint: Some("MyTool Pro".into()),
                install_dir: None,
                default_env: vec![],
            },
        )
        .unwrap();
        let loaded = get_preset(&conn, "uses-my-tool").unwrap().unwrap();
        assert_eq!(loaded.preset.requirements[0].product.name, "My Tool Pro");
        assert_eq!(
            loaded.preset.requirements[0].step,
            crate::domain::Step::Winget {
                id: "New.Vendor.MyTool".into(),
                scope: "machine".into()
            }
        );

        // The stored payload keeps only the id reference.
        let data: String = conn
            .query_row(
                "SELECT data FROM presets WHERE id = 'uses-my-tool'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&data).unwrap();
        assert_eq!(value["requirements"][0]["product"]["id"], "my-tool");
        assert_eq!(value["requirements"][0]["product"]["name"], "");
    }

    #[test]
    fn imported_presets_keep_their_embedded_snapshot() {
        let dir = test_dir();
        let conn = init_at(&dir).unwrap();
        // The Library product exists (created by the test, not seeded) and
        // differs from the imported snapshot's embedded copy.
        make_product(
            &conn,
            "openjdk21",
            "Renamed JDK",
            Some("Vendor.Renamed"),
            None,
        );
        let imported = PresetRecord {
            imported: true,
            ..test_preset_with(
                "imported-snapshot",
                vec![crate::domain::Requirement {
                    product: Product {
                        id: "openjdk21".into(),
                        name: "Snapshot JDK".into(),
                        winget_id: Some("Snapshot.Winget".into()),
                        install_location_hint: Some("Snapshot".into()),
                        install_dir: None,
                        default_env: vec![],
                    },
                    step: crate::domain::Step::Winget {
                        id: "Snapshot.Winget".into(),
                        scope: "machine".into(),
                    },
                    version_policy: crate::domain::VersionPolicy::Latest,
                    depends_on: vec![],
                    timeout_minutes: 10,
                    env: vec![],
                    verify: vec![],
                    unresolved: false,
                }],
            )
        };
        create_preset(&conn, &imported).unwrap();

        // Imported presets never resolve: the snapshot reads back as authored
        // even though the Library product changed.
        let loaded = get_preset(&conn, "imported-snapshot").unwrap().unwrap();
        let req = &loaded.preset.requirements[0];
        assert_eq!(req.product.name, "Snapshot JDK");
        assert_eq!(req.product.winget_id.as_deref(), Some("Snapshot.Winget"));
        assert_eq!(
            req.step,
            crate::domain::Step::Winget {
                id: "Snapshot.Winget".into(),
                scope: "machine".into()
            }
        );
        assert!(!req.unresolved);

        // The stored payload is exactly as authored — never stripped.
        let data: String = conn
            .query_row(
                "SELECT data FROM presets WHERE id = 'imported-snapshot'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&data).unwrap();
        assert_eq!(value["requirements"][0]["product"]["name"], "Snapshot JDK");
    }

    #[test]
    fn deleting_a_product_drops_local_requirements_keeps_imported_and_history() {
        let dir = test_dir();
        let conn = init_at(&dir).unwrap();
        // Products the presets reference — created here, nothing is seeded
        // (ADR-0008).
        make_product(&conn, "git", "Git", Some("Git.Git"), None);
        make_product(
            &conn,
            "openjdk21",
            "Eclipse Temurin OpenJDK 21 (LTS)",
            Some("EclipseAdoptium.Temurin.21.JDK"),
            None,
        );
        // Local preset referencing git + openjdk21; imported preset with a git
        // snapshot; a run whose history mentions git.
        create_preset(
            &conn,
            &test_preset_with(
                "local-box",
                vec![reference_req("git", "Git.Git"), reference_req("openjdk21", "EclipseAdoptium.Temurin.21.JDK")],
            ),
        )
        .unwrap();
        create_preset(
            &conn,
            &PresetRecord {
                imported: true,
                ..test_preset_with("imported-box", vec![reference_req("git", "Git.Git")])
            },
        )
        .unwrap();
        let record = crate::run::RunRecord {
            id: "run-before-delete".into(),
            started_at: 1,
            finished_at: 2,
            preset_names: vec![],
            outcome: crate::run::RunOutcome::Ok,
            results: vec![crate::run::RequirementOutcome {
                product_id: "git".into(),
                product_name: "Git".into(),
                status: crate::run::RunStatus::Installed,
                detail: "installed".into(),
                reboot_required: false,
                log_path: String::new(),
            }],
        };
        create_run(&conn, &record).unwrap();

        // Only the local preset counts toward the prompt.
        assert_eq!(count_presets_using_product(&conn, "git").unwrap(), 1);
        assert_eq!(count_presets_using_product(&conn, "openjdk21").unwrap(), 1);

        delete_product(&conn, "git").unwrap();

        // The local preset's requirement is dropped…
        let local = get_preset(&conn, "local-box").unwrap().unwrap();
        assert_eq!(local.preset.requirements.len(), 1);
        assert_eq!(local.preset.requirements[0].product.id, "openjdk21");
        // …the imported preset keeps its snapshot…
        let imported = get_preset(&conn, "imported-box").unwrap().unwrap();
        assert_eq!(imported.preset.requirements.len(), 1);
        assert_eq!(imported.preset.requirements[0].product.id, "git");
        // …and run history keeps its records.
        assert_eq!(get_run(&conn, "run-before-delete").unwrap(), Some(record));
    }

    #[test]
    fn unresolvable_requirements_are_flagged_and_recover_when_re_added() {
        let dir = test_dir();
        let conn = init_at(&dir).unwrap();
        // A reference to a product that is not in the library (legacy data
        // from before live links, or a fork of a snapshot): reads back with
        // the flag the composer shows as "product removed from library".
        create_preset(
            &conn,
            &test_preset_with("dangling", vec![reference_req("ghost", "Vendor.Ghost")]),
        )
        .unwrap();
        let loaded = get_preset(&conn, "dangling").unwrap().unwrap();
        assert!(loaded.preset.requirements[0].unresolved);
        assert_eq!(loaded.preset.requirements[0].product.id, "ghost");

        // Re-adding the product with the same id re-links the requirement.
        create_product(
            &conn,
            &Product {
                id: "ghost".into(),
                name: "Ghost Tool".into(),
                winget_id: Some("Vendor.Ghost".into()),
                install_location_hint: None,
                install_dir: None,
                default_env: vec![],
            },
        )
        .unwrap();
        let loaded = get_preset(&conn, "dangling").unwrap().unwrap();
        assert!(!loaded.preset.requirements[0].unresolved);
        assert_eq!(loaded.preset.requirements[0].product.name, "Ghost Tool");
    }

    #[test]
    fn deleting_a_missing_product_still_fails() {
        let dir = test_dir();
        let conn = init_at(&dir).unwrap();
        assert!(delete_product(&conn, "no-such").is_err());
        assert_eq!(count_presets_using_product(&conn, "no-such").unwrap(), 0);
    }

    #[test]
    fn dock_state_roundtrips_across_reopen() {
        let dir = test_dir();
        let monitor = r"\\.\DISPLAY1";
        {
            let conn = init_at(&dir).unwrap();
            // Nothing is remembered on a fresh database.
            assert_eq!(load_dock_edge(&conn, monitor), None);
            assert_eq!(load_dock_mode(&conn, monitor), None);
            save_dock_edge(&conn, monitor, "right").unwrap();
            save_dock_mode(&conn, monitor, "fixed").unwrap();
            assert_eq!(load_dock_edge(&conn, monitor), Some("right".into()));
            assert_eq!(load_dock_mode(&conn, monitor), Some("fixed".into()));
        }
        // Re-open: the memory survives the connection.
        let conn = init_at(&dir).unwrap();
        assert_eq!(load_dock_edge(&conn, monitor), Some("right".into()));
        assert_eq!(load_dock_mode(&conn, monitor), Some("fixed".into()));
    }

    #[test]
    fn dock_state_updates_in_place() {
        let dir = test_dir();
        let conn = init_at(&dir).unwrap();
        let monitor = r"\\.\DISPLAY2";
        save_dock_edge(&conn, monitor, "left").unwrap();
        save_dock_edge(&conn, monitor, "right").unwrap();
        // The second save overwrites, never stacks.
        assert_eq!(load_dock_edge(&conn, monitor), Some("right".into()));
    }

    #[test]
    fn dock_memory_is_per_monitor() {
        let dir = test_dir();
        let conn = init_at(&dir).unwrap();
        save_dock_edge(&conn, r"\\.\DISPLAY1", "left").unwrap();
        save_dock_edge(&conn, r"\\.\DISPLAY2", "right").unwrap();
        // Each monitor remembers its own edge.
        assert_eq!(load_dock_edge(&conn, r"\\.\DISPLAY1"), Some("left".into()));
        assert_eq!(load_dock_edge(&conn, r"\\.\DISPLAY2"), Some("right".into()));
    }

    #[test]
    fn invalid_stored_dock_state_falls_back_to_none() {
        let dir = test_dir();
        let conn = init_at(&dir).unwrap();
        let monitor = r"\\.\DISPLAY1";
        // Broken values written by a buggy build must never reach the dock —
        // they read back as None so the dock falls back to the Settings.
        upsert_meta(&conn, &dock_key(KEY_DOCK_EDGE_PREFIX, monitor), "top").unwrap();
        assert_eq!(load_dock_edge(&conn, monitor), None);
        upsert_meta(&conn, &dock_key(KEY_DOCK_MODE_PREFIX, monitor), "overlay").unwrap();
        assert_eq!(load_dock_mode(&conn, monitor), None);
    }

    #[test]
    fn identified_reads_prefer_the_identity_row_over_the_legacy_row() {
        let dir = test_dir();
        let conn = init_at(&dir).unwrap();
        let identity = "edid-1234-5678";
        let device = r"\\.\DISPLAY1";
        save_dock_edge(&conn, identity, "left").unwrap();
        save_dock_mode(&conn, identity, "auto-hide").unwrap();
        save_dock_edge(&conn, device, "right").unwrap();
        save_dock_mode(&conn, device, "fixed").unwrap();
        // The identity row wins when one exists.
        assert_eq!(
            load_dock_edge_identified(&conn, Some(identity), device),
            Some("left".into())
        );
        assert_eq!(
            load_dock_mode_identified(&conn, Some(identity), device),
            Some("auto-hide".into())
        );
    }

    #[test]
    fn identified_reads_fall_back_to_the_legacy_device_name_row() {
        let dir = test_dir();
        let conn = init_at(&dir).unwrap();
        let device = r"\\.\DISPLAY1";
        // A memory saved before ticket 110 lives under the device name only.
        save_dock_edge(&conn, device, "right").unwrap();
        save_dock_mode(&conn, device, "fixed").unwrap();
        assert_eq!(
            load_dock_edge_identified(&conn, Some("edid-AAAA-0001"), device),
            Some("right".into())
        );
        assert_eq!(
            load_dock_mode_identified(&conn, Some("edid-AAAA-0001"), device),
            Some("fixed".into())
        );
        // No resolvable identity at all reads the legacy row directly.
        assert_eq!(load_dock_edge_identified(&conn, None, device), Some("right".into()));
        assert_eq!(load_dock_mode_identified(&conn, None, device), Some("fixed".into()));
    }

    #[test]
    fn identified_reads_return_none_when_neither_row_exists() {
        let dir = test_dir();
        let conn = init_at(&dir).unwrap();
        assert_eq!(
            load_dock_edge_identified(&conn, Some("edid-1234-5678"), r"\\.\DISPLAY9"),
            None
        );
        assert_eq!(
            load_dock_mode_identified(&conn, None, r"\\.\DISPLAY9"),
            None
        );
    }

    #[test]
    fn identified_reads_treat_a_broken_identity_row_as_absent_and_fall_back() {
        let dir = test_dir();
        let conn = init_at(&dir).unwrap();
        let identity = "edid-DEAD-BEEF";
        let device = r"\\.\DISPLAY1";
        // A corrupted row under the identity key must not shadow the valid
        // legacy value — the validation filter drops it and the read falls
        // through.
        upsert_meta(&conn, &dock_key(KEY_DOCK_EDGE_PREFIX, identity), "top").unwrap();
        save_dock_edge(&conn, device, "left").unwrap();
        assert_eq!(
            load_dock_edge_identified(&conn, Some(identity), device),
            Some("left".into())
        );
    }
}

#[cfg(test)]
mod groups_migration_tests {
    use super::*;

    #[test]
    fn migrates_databases_created_before_groups() {
        // A database from tickets 01-88 has the item tables but no `groups`
        // table and no `group_id` columns; init_at must add both and leave
        // the schema fully usable.
        let dir = tempfile::tempdir().unwrap().into_path();
        std::fs::create_dir_all(&dir).unwrap();
        {
            let conn = Connection::open(dir.join("sprout.db")).unwrap();
            conn.execute_batch(
                "CREATE TABLE launch_entries (
                     id          INTEGER PRIMARY KEY AUTOINCREMENT,
                     name        TEXT NOT NULL,
                     kind        TEXT NOT NULL CHECK (kind IN ('app', 'command')),
                     target      TEXT NOT NULL,
                     shell       TEXT CHECK (shell IN ('powershell', 'cmd', 'none')),
                     show_window INTEGER NOT NULL DEFAULT 0,
                     desktop_id  TEXT,
                     position    INTEGER NOT NULL DEFAULT 0
                 );
                 CREATE TABLE clips (
                     id       INTEGER PRIMARY KEY AUTOINCREMENT,
                     name     TEXT NOT NULL,
                     content  TEXT NOT NULL,
                     position INTEGER NOT NULL DEFAULT 0
                 );
                 INSERT INTO launch_entries (name, kind, target, shell, position)
                 VALUES ('legacy', 'command', 'echo hi', 'none', 0);",
            )
            .unwrap();
        }
        let conn = init_at(&dir).unwrap();

        // The migrated table reads back with the group reference unset, and
        // the new machinery works on it end to end.
        let entries = crate::launch::list_launch_entries(&conn).unwrap();
        assert_eq!(entries[0].group_id, None);
        let group = crate::groups::create_group(&conn, crate::groups::Collection::Launch, "Legacy").unwrap();
        crate::groups::assign_item(&conn, crate::groups::Collection::Launch, entries[0].id, group.id)
            .unwrap();
        let entries = crate::launch::list_launch_entries(&conn).unwrap();
        assert_eq!(entries[0].group_id, Some(group.id));

        // The migration is idempotent — re-running init changes nothing.
        drop(conn);
        let conn = init_at(&dir).unwrap();
        let entries = crate::launch::list_launch_entries(&conn).unwrap();
        assert_eq!(entries[0].group_id, Some(group.id));
    }
}
