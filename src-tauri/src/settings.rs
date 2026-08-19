//! App settings (ticket 09): the knobs the Settings screen persists and the
//! run pipeline honors.
//!
//! Four values live in the `meta` table, so no schema change is needed:
//!
//! - `default_timeout_minutes` — the timeout pre-filled for new Requirements
//!   in the preset composer (the fixed 10-minute default it replaced);
//! - `log_retention_days` — how old a per-run log folder may get before runs
//!   prune it. History rows are kept forever; only raw log files expire;
//! - `theme` — the app-wide look: "system" follows the OS, "light"/"dark"
//!   pin it. Saved on its own the moment it is picked (ticket 31);
//! - `install_dir` — the machine-local default install directory (ticket 34):
//!   empty means winget's own default, otherwise every winget install/upgrade
//!   carries `--location`. It is a Settings value, never part of a Product,
//!   Requirement, or Preset — exports and preset files never contain it
//!   (ADR-0009);
//! - `launch_concurrency` — the Quick Launch cap (ticket 37/42): how many
//!   Launch entries may be in flight at once; the rest queue until a slot
//!   frees.

use rusqlite::{params, Connection, Result};

use serde::{Deserialize, Serialize};

/// The timeout new Requirements are pre-filled with, in minutes.
pub const DEFAULT_TIMEOUT_MINUTES: u32 = 10;
/// How many days a finished run's log folder is kept before pruning.
pub const DEFAULT_RETENTION_DAYS: u32 = 30;
/// The app-wide theme: "system" follows the OS, "light"/"dark" pin it.
pub const DEFAULT_THEME: &str = "system";
/// How many Quick Launch entries may launch at once before the rest queue.
pub const DEFAULT_LAUNCH_CONCURRENCY: u32 = 8;

const KEY_TIMEOUT: &str = "settings.timeout_minutes";
const KEY_RETENTION: &str = "settings.log_retention_days";
const KEY_THEME: &str = "settings.theme";
const KEY_INSTALL_DIR: &str = "settings.install_dir";
const KEY_LAUNCH_CONCURRENCY: &str = "launch.concurrency";

/// The persisted knobs. `u32` fields keep the frontend's number inputs safe;
/// validation lives in [`Settings::validate`]. `install_dir` is empty when
/// winget should use its own default directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub default_timeout_minutes: u32,
    pub log_retention_days: u32,
    pub theme: String,
    /// Machine-local default install directory (ADR-0009): empty = winget's
    /// default; non-empty = every winget step carries `--location`.
    pub install_dir: String,
    /// The Quick Launch concurrency cap (ticket 37/42): entries beyond it
    /// queue until a slot frees.
    pub launch_concurrency: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            default_timeout_minutes: DEFAULT_TIMEOUT_MINUTES,
            log_retention_days: DEFAULT_RETENTION_DAYS,
            theme: DEFAULT_THEME.to_string(),
            install_dir: String::new(),
            launch_concurrency: DEFAULT_LAUNCH_CONCURRENCY,
        }
    }
}

/// Accepts only the three modes the Settings screen offers.
pub fn validate_theme(theme: &str) -> std::result::Result<(), String> {
    match theme {
        "system" | "light" | "dark" => Ok(()),
        _ => Err("Theme must be \"system\", \"light\", or \"dark\"".into()),
    }
}

/// Accepts an empty value (winget's default directory) and any absolute
/// Windows path — drive-rooted (`D:\Apps`) or UNC (`\\server\share`). A
/// relative path would silently mean different things per machine, so it is
/// rejected.
pub fn validate_install_dir(install_dir: &str) -> std::result::Result<(), String> {
    if install_dir.trim().is_empty() {
        return Ok(());
    }
    let path = std::path::Path::new(install_dir.trim());
    if path.is_absolute() {
        Ok(())
    } else {
        Err(format!(
            "'{install_dir}' is not an absolute path — the install directory must be a full path like D:\\Apps"
        ))
    }
}

impl Settings {
    /// Rejects values that would break a run or empty the log archive.
    /// Timeouts must be at least 1 minute and at most a day; retention at
    /// least 1 day and at most 10 years; the install directory, when set,
    /// must be an absolute path.
    pub fn validate(&self) -> std::result::Result<(), String> {
        if !(1..=1440).contains(&self.default_timeout_minutes) {
            return Err("Default timeout must be between 1 and 1440 minutes (24 h)".into());
        }
        if !(1..=3650).contains(&self.log_retention_days) {
            return Err("Log retention must be between 1 and 3650 days (10 years)".into());
        }
        validate_theme(&self.theme)?;
        validate_install_dir(&self.install_dir)?;
        if !(1..=50).contains(&self.launch_concurrency) {
            return Err("Launch concurrency must be between 1 and 50".into());
        }
        Ok(())
    }
}

/// Reads the settings, falling back to the defaults for keys that were never
/// written (fresh installs, pre-09 databases).
pub fn load(conn: &Connection) -> Settings {
    let get = |key: &str| -> Option<u32> {
        conn.query_row(
            "SELECT value FROM meta WHERE key = ?1",
            params![key],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|v| v.parse().ok())
    };
    let theme = conn
        .query_row(
            "SELECT value FROM meta WHERE key = ?1",
            params![KEY_THEME],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .filter(|v| validate_theme(v).is_ok());
    let install_dir = conn
        .query_row(
            "SELECT value FROM meta WHERE key = ?1",
            params![KEY_INSTALL_DIR],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .filter(|v| validate_install_dir(v).is_ok())
        .unwrap_or_default();
    let launch_concurrency = get(KEY_LAUNCH_CONCURRENCY)
        .filter(|value| (1..=50).contains(value))
        .unwrap_or(DEFAULT_LAUNCH_CONCURRENCY);
    Settings {
        default_timeout_minutes: get(KEY_TIMEOUT).unwrap_or(DEFAULT_TIMEOUT_MINUTES),
        log_retention_days: get(KEY_RETENTION).unwrap_or(DEFAULT_RETENTION_DAYS),
        theme: theme.unwrap_or_else(|| DEFAULT_THEME.to_string()),
        install_dir,
        launch_concurrency,
    }
}

/// Persists the settings, validated first — a broken value must never reach
/// the run pipeline.
pub fn save(conn: &Connection, settings: &Settings) -> std::result::Result<(), String> {
    settings.validate()?;
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| e.to_string())?;
    upsert(&tx, KEY_TIMEOUT, &settings.default_timeout_minutes.to_string()).map_err(|e| e.to_string())?;
    upsert(&tx, KEY_RETENTION, &settings.log_retention_days.to_string()).map_err(|e| e.to_string())?;
    upsert(&tx, KEY_THEME, &settings.theme).map_err(|e| e.to_string())?;
    upsert(&tx, KEY_INSTALL_DIR, &settings.install_dir).map_err(|e| e.to_string())?;
    upsert(&tx, KEY_LAUNCH_CONCURRENCY, &settings.launch_concurrency.to_string())
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

/// Persists only the theme — the Settings screen applies it the moment it is
/// selected, without touching the other knobs (ticket 31).
pub fn save_theme(conn: &Connection, theme: &str) -> std::result::Result<(), String> {
    validate_theme(theme)?;
    upsert(conn, KEY_THEME, theme).map_err(|e| e.to_string())
}

fn upsert(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir() -> std::path::PathBuf {
        // Unique per call — libtest reuses worker threads across tests, so
        // pid+thread-id dirs collide on re-runs (tempfile is the dev
        // dependency the other suites use for the same reason).
        tempfile::tempdir().unwrap().into_path()
    }

    /// Removes leftovers from a previous test run — pids and thread ids get
    /// reused, and a stale database would break the default-value assertions.
    fn clean_dir() -> std::path::PathBuf {
        let dir = test_dir();
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn conn() -> Connection {
        crate::db::init_at(&clean_dir()).unwrap()
    }

    #[test]
    fn fresh_databases_load_the_defaults() {
        let conn = conn();
        assert_eq!(load(&conn), Settings::default());
        assert_eq!(load(&conn).default_timeout_minutes, 10);
        assert_eq!(load(&conn).log_retention_days, 30);
        assert_eq!(load(&conn).theme, DEFAULT_THEME);
        assert_eq!(load(&conn).install_dir, "");
    }

    #[test]
    fn settings_roundtrip_across_connections() {
        let dir = clean_dir();
        let custom = Settings {
            default_timeout_minutes: 25,
            log_retention_days: 90,
            theme: "dark".to_string(),
            install_dir: r"D:\Apps".to_string(),
            launch_concurrency: 12,
        };
        {
            let conn = crate::db::init_at(&dir).unwrap();
            save(&conn, &custom).unwrap();
            assert_eq!(load(&conn), custom);
        }
        // Re-open: the values survive the connection.
        let conn = crate::db::init_at(&dir).unwrap();
        assert_eq!(load(&conn), custom);
    }

    #[test]
    fn validation_rejects_breakable_values() {
        let mut s = Settings::default();
        s.default_timeout_minutes = 0;
        assert!(s.validate().is_err());
        s.default_timeout_minutes = 1441;
        assert!(s.validate().is_err());
        s.default_timeout_minutes = 1;
        s.log_retention_days = 0;
        assert!(s.validate().is_err());
        s.log_retention_days = 3651;
        assert!(s.validate().is_err());
        s.log_retention_days = 3650;
        assert!(s.validate().is_ok());
        s.theme = "sepia".to_string();
        assert!(s.validate().is_err());
        s.theme = DEFAULT_THEME.to_string();
        assert!(s.validate().is_ok());
        s.launch_concurrency = 0;
        assert!(s.validate().is_err());
        s.launch_concurrency = 51;
        assert!(s.validate().is_err());
        s.launch_concurrency = 50;
        assert!(s.validate().is_ok());
    }

    #[test]
    fn out_of_range_stored_concurrency_falls_back_to_default() {
        let conn = conn();
        // A broken value written by an older build must never reach the
        // launch pipeline — it reads back as the default.
        upsert(&conn, KEY_LAUNCH_CONCURRENCY, "0").unwrap();
        assert_eq!(load(&conn).launch_concurrency, DEFAULT_LAUNCH_CONCURRENCY);
        upsert(&conn, KEY_LAUNCH_CONCURRENCY, "99").unwrap();
        assert_eq!(load(&conn).launch_concurrency, DEFAULT_LAUNCH_CONCURRENCY);
    }

    #[test]
    fn install_dir_accepts_empty_and_absolute_paths_only() {
        // Empty means "winget's own default".
        assert!(validate_install_dir("").is_ok());
        assert!(validate_install_dir("   ").is_ok());
        // Drive-rooted and UNC paths are absolute.
        assert!(validate_install_dir(r"D:\Apps").is_ok());
        assert!(validate_install_dir(r"C:\Program Files").is_ok());
        assert!(validate_install_dir(r"\\server\share\apps").is_ok());
        // A relative path is not a directory on this machine.
        assert!(validate_install_dir("Apps").is_err());
        assert!(validate_install_dir(r"..\Apps").is_err());
        assert!(validate_install_dir(r"App:\relative").is_err());
        // The full Settings validation honors the same rule.
        let mut s = Settings::default();
        s.install_dir = "Apps".into();
        assert!(s.validate().is_err());
        s.install_dir = r"D:\Apps".into();
        assert!(s.validate().is_ok());
    }

    #[test]
    fn install_dir_roundtrips_and_can_be_cleared() {
        let dir = clean_dir();
        {
            let conn = crate::db::init_at(&dir).unwrap();
            let mut s = Settings::default();
            s.install_dir = r"D:\Apps".into();
            save(&conn, &s).unwrap();
            assert_eq!(load(&conn).install_dir, r"D:\Apps");
            // Clearing the value is an explicit empty save, not a deletion.
            s.install_dir = String::new();
            save(&conn, &s).unwrap();
            assert_eq!(load(&conn).install_dir, "");
        }
        // Re-open: the cleared value survives the connection.
        let conn = crate::db::init_at(&dir).unwrap();
        assert_eq!(load(&conn).install_dir, "");
    }

    #[test]
    fn save_rejects_a_relative_install_dir_and_keeps_the_old_one() {
        let dir = clean_dir();
        {
            let conn = crate::db::init_at(&dir).unwrap();
            let mut s = Settings::default();
            s.install_dir = r"D:\Apps".into();
            save(&conn, &s).unwrap();
            // A broken value must never reach the run pipeline.
            let bad = Settings {
                install_dir: "Apps".into(),
                ..s.clone()
            };
            assert!(save(&conn, &bad).is_err());
            assert_eq!(load(&conn).install_dir, r"D:\Apps");
        }
    }

    #[test]
    fn theme_roundtrips_on_its_own() {
        let dir = clean_dir();
        {
            let conn = crate::db::init_at(&dir).unwrap();
            save_theme(&conn, "dark").unwrap();
            assert_eq!(load(&conn).theme, "dark");
        }
        // Re-open: the theme survives the connection.
        let conn = crate::db::init_at(&dir).unwrap();
        assert_eq!(load(&conn).theme, "dark");
    }

    #[test]
    fn save_theme_rejects_unknown_values_and_keeps_the_old_one() {
        let conn = conn();
        assert!(save_theme(&conn, "sepia").is_err());
        // Nothing was persisted.
        assert_eq!(load(&conn).theme, DEFAULT_THEME);
    }

    #[test]
    fn save_rejects_invalid_values_and_keeps_the_old_ones() {
        let conn = conn();
        let bad = Settings {
            default_timeout_minutes: 0,
            log_retention_days: 7,
            theme: "dark".to_string(),
            install_dir: String::new(),
            launch_concurrency: 8,
        };
        assert!(save(&conn, &bad).is_err());
        // Nothing was persisted.
        assert_eq!(load(&conn), Settings::default());
    }
}