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
//!   frees;
//! - `dock_mode` — the Quick Launch dock's visibility mode (tickets 49/50):
//!   "auto-hide" slides to a sliver when not hovered (default), "fixed" keeps
//!   the strip permanently reserved;
//! - `dock_edge` — the screen edge the Quick Launch dock attaches to by
//!   default: "left" or "right" (the window's own live controls override it
//!   per monitor);
//! - `dock_state` — the Quick Launch window's dock/undock state (ticket 57):
//!   "floating" (default) or "docked", persisted so the window reopens in the
//!   state it was left in, and written back by the in-window dock controls;
//! - `autostart` — whether Sprout registers itself to start with Windows
//!   (ADR-0013, ticket 75): "on" (default) or "off". The registration itself
//!   is reconciled with this value by the autostart module at startup and on
//!   every toggle;
//! - `launch_groups` / `action_groups` / `clip_groups` — whether each list
//!   page offers its Groups feature (ticket 89): "off" (default) per
//!   collection. Off is fully dormant — the lists render flat and no group
//!   affordance appears anywhere (main app, window, dock) — while stored
//!   groups and memberships survive untouched, so re-enabling restores them.

use rusqlite::{params, Connection};

use serde::{Deserialize, Serialize};

use crate::db::upsert_meta;

/// The timeout new Requirements are pre-filled with, in minutes.
pub const DEFAULT_TIMEOUT_MINUTES: u32 = 10;
/// How many days a finished run's log folder is kept before pruning.
pub const DEFAULT_RETENTION_DAYS: u32 = 30;
/// The app-wide theme: "system" follows the OS, "light"/"dark" pin it.
pub const DEFAULT_THEME: &str = "system";
/// How many Quick Launch entries may launch at once before the rest queue.
pub const DEFAULT_LAUNCH_CONCURRENCY: u32 = 8;
/// The dock's default visibility mode: auto-hide slides to a sliver when not
/// hovered; "fixed" keeps the strip permanently reserved (ADR-0011).
pub const DEFAULT_DOCK_MODE: &str = "auto-hide";
/// The dock's default screen edge.
pub const DEFAULT_DOCK_EDGE: &str = "left";
/// The dock's default state: floating — the window only docks when the user
/// docks it (or sets the state to "docked" in Settings).
pub const DEFAULT_DOCK_STATE: &str = "floating";
/// Auto-start is on by default in installed builds (ADR-0013): the user opts
/// out with the Settings toggle, not in.
pub const DEFAULT_AUTOSTART: &str = "on";
/// Every collection's Groups feature is opt-in (ticket 89, research 0006
/// patterns 2–3): the lists stay flat until the user turns grouping on.
pub const DEFAULT_GROUPS_FEATURE: &str = "off";
/// Reveal gate tuning (ticket 113): dwell and sensitivity default to the
/// shipped gate constants (single size source via `constants::window`).
pub const DEFAULT_REVEAL_DWELL_MS: u64 = crate::constants::window::REVEAL_DWELL_MS;
pub const DEFAULT_REVEAL_SENSITIVITY_PX: i32 = crate::constants::window::REVEAL_SENSITIVITY_PX;
/// Sane ranges for the reveal knobs (ticket 113): dwell 0–1000 ms (snappy to
/// deliberate hold), sensitivity 0–50 px (immediate to demanding push).
pub const REVEAL_DWELL_MIN_MS: u64 = 0;
pub const REVEAL_DWELL_MAX_MS: u64 = 1000;
pub const REVEAL_SENSITIVITY_MIN_PX: i32 = 0;
pub const REVEAL_SENSITIVITY_MAX_PX: i32 = 50;
/// Companion pane (ticket 125): height ratio 25–60% — how much of the docked
/// window's height the embedded web view occupies (bottom strip). Default
/// 40% matches the ticket's 0.40 and sits comfortably at any dock height
/// from 1080p to 4K (research 0012).
pub const DEFAULT_COMPANION_HEIGHT_RATIO: f64 = 0.40;
pub const COMPANION_HEIGHT_RATIO_MIN: f64 = 0.25;
pub const COMPANION_HEIGHT_RATIO_MAX: f64 = 0.60;
/// Companion audio (mute-only): the dock toolbar's mute toggle persists here
/// so silence survives restarts and WebView recreations. Default unmuted —
/// a fresh install never starts silent.
pub const DEFAULT_COMPANION_MUTED: bool = false;

const KEY_TIMEOUT: &str = "settings.timeout_minutes";
const KEY_RETENTION: &str = "settings.log_retention_days";
const KEY_THEME: &str = "settings.theme";
const KEY_INSTALL_DIR: &str = "settings.install_dir";
const KEY_LAUNCH_CONCURRENCY: &str = "launch.concurrency";
const KEY_LAUNCH_GROUPS: &str = "launch.groups";
const KEY_ACTION_GROUPS: &str = "actions.groups";
const KEY_CLIP_GROUPS: &str = "clips.groups";
const KEY_DOCK_MODE: &str = "dock.mode";
const KEY_DOCK_EDGE: &str = "dock.edge";
const KEY_DOCK_STATE: &str = "dock.state";
const KEY_AUTOSTART: &str = "settings.autostart";
const KEY_REVEAL_DWELL_MS: &str = "dock.reveal_dwell_ms";
const KEY_REVEAL_SENSITIVITY_PX: &str = "dock.reveal_sensitivity_px";
const KEY_COMPANION_URL: &str = "settings.companion_url";
const KEY_COMPANION_HEIGHT_RATIO: &str = "settings.companion_height_ratio";
const KEY_COMPANION_URL_LIST: &str = "settings.companion_url_list";
const KEY_COMPANION_MUTED: &str = "settings.companion_muted";

/// The persisted knobs. `u32` fields keep the frontend's number inputs safe;
/// validation lives in [`Settings::validate`]. `install_dir` is empty when
/// winget should use its own default directory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// The Quick Launch dock's visibility mode (tickets 49/50): "auto-hide"
    /// or "fixed".
    pub dock_mode: String,
    /// The screen edge the Quick Launch dock attaches to by default (tickets
    /// 49/50): "left" or "right".
    pub dock_edge: String,
    /// The Quick Launch window's dock state (ticket 57): "floating" or
    /// "docked" — what the window reopens as, and what the in-window dock
    /// toggle writes back.
    pub dock_state: String,
    /// Whether Sprout starts with Windows (ADR-0013, ticket 75): "on" or
    /// "off". The Run-key registration is reconciled with this value by the
    /// autostart module; the setting itself only records the preference.
    pub autostart: String,
    /// Whether the Quick Launch page offers its Groups feature (ticket 89):
    /// "on" or "off". Off is fully dormant — a flat list, no group
    /// affordances — while stored groups and memberships survive untouched.
    pub launch_groups: String,
    /// Whether the Quick Actions page offers its Groups feature (ticket 89).
    pub action_groups: String,
    /// Whether the Quick Clips page offers its Groups feature (ticket 89).
    pub clip_groups: String,
    /// Reveal dwell in milliseconds (ticket 113): how long the cursor must
    /// stay inside the sliver band after accumulating sufficient toward-edge
    /// travel before the dock reveals. 0 is immediate.
    pub reveal_dwell_ms: u64,
    /// Reveal sensitivity threshold in physical pixels (ticket 113):
    /// accumulated toward-edge motion inside the sliver must exceed this before
    /// the dwell starts. 0 needs no push.
    pub reveal_sensitivity_px: i32,
    /// Companion active URL (ticket 125): the https URL rendered as a single
    /// mobile web view in the bottom ~40% of the dock. `None` means the feature
    /// is off — no pane, no splitter, no chrome (research 0004 rule 2 / 0006
    /// pattern 11, content-gated).
    pub companion_url: Option<String>,
    /// Companion height ratio (ticket 125): fraction of dock height devoted to
    /// the web view, clamped 0.25–0.60 (default 0.40). Live-draggable via the
    /// horizontal splitter and persisted per monitor (falls back to this).
    pub companion_height_ratio: f64,
    /// Companion saved URL list (ticket 125): the URLs the main app's companion
    /// manager edits; deduped trimmed case-insensitive on host+path, ordered by
    /// the user. Machine-local, never in Preset exports/backups beyond the
    /// settings row (ADR-0009 spirit).
    pub companion_url_list: Vec<String>,
    /// Companion mute (global, persisted): whether the docked pane's audio is
    /// silenced. Applied to the live WebView on every read and on every
    /// WebView creation, so a recreated pane never comes back loud.
    pub companion_muted: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            default_timeout_minutes: DEFAULT_TIMEOUT_MINUTES,
            log_retention_days: DEFAULT_RETENTION_DAYS,
            theme: DEFAULT_THEME.to_string(),
            install_dir: String::new(),
            launch_concurrency: DEFAULT_LAUNCH_CONCURRENCY,
            dock_mode: DEFAULT_DOCK_MODE.to_string(),
            dock_edge: DEFAULT_DOCK_EDGE.to_string(),
            dock_state: DEFAULT_DOCK_STATE.to_string(),
            autostart: DEFAULT_AUTOSTART.to_string(),
            launch_groups: DEFAULT_GROUPS_FEATURE.to_string(),
            action_groups: DEFAULT_GROUPS_FEATURE.to_string(),
            clip_groups: DEFAULT_GROUPS_FEATURE.to_string(),
            reveal_dwell_ms: DEFAULT_REVEAL_DWELL_MS,
            reveal_sensitivity_px: DEFAULT_REVEAL_SENSITIVITY_PX,
            companion_url: None,
            companion_height_ratio: DEFAULT_COMPANION_HEIGHT_RATIO,
            companion_url_list: Vec::new(),
            companion_muted: DEFAULT_COMPANION_MUTED,
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

/// Accepts only the two dock visibility modes the dock offers (ADR-0011).
pub fn validate_dock_mode(mode: &str) -> std::result::Result<(), String> {
    match mode {
        "auto-hide" | "fixed" => Ok(()),
        _ => Err("Dock mode must be \"auto-hide\" or \"fixed\"".into()),
    }
}

/// Accepts only the two screen edges the dock attaches to.
pub fn validate_dock_edge(edge: &str) -> std::result::Result<(), String> {
    match edge {
        "left" | "right" => Ok(()),
        _ => Err("Dock edge must be \"left\" or \"right\"".into()),
    }
}

/// Accepts only the two dock states the Quick Launch window can be in.
pub fn validate_dock_state(state: &str) -> std::result::Result<(), String> {
    match state {
        "floating" | "docked" => Ok(()),
        _ => Err("Dock state must be \"floating\" or \"docked\"".into()),
    }
}

/// Accepts only the two auto-start states the Settings toggle writes
/// (ADR-0013).
pub fn validate_autostart(value: &str) -> std::result::Result<(), String> {
    match value {
        "on" | "off" => Ok(()),
        _ => Err("Auto-start must be \"on\" or \"off\"".into()),
    }
}

/// Accepts only the two Groups-feature states every list page's toggle
/// writes (ticket 89).
pub fn validate_groups_feature(value: &str) -> std::result::Result<(), String> {
    match value {
        "on" | "off" => Ok(()),
        _ => Err("Groups must be \"on\" or \"off\"".into()),
    }
}

/// Validates the reveal dwell (ticket 113): 0–1000 ms inclusive.
pub fn validate_reveal_dwell_ms(value: u64) -> std::result::Result<(), String> {
    if (REVEAL_DWELL_MIN_MS..=REVEAL_DWELL_MAX_MS).contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "Reveal delay must be between {REVEAL_DWELL_MIN_MS} and {REVEAL_DWELL_MAX_MS} ms"
        ))
    }
}

/// Validates the reveal sensitivity threshold (ticket 113): 0–50 px inclusive.
pub fn validate_reveal_sensitivity_px(value: i32) -> std::result::Result<(), String> {
    if (REVEAL_SENSITIVITY_MIN_PX..=REVEAL_SENSITIVITY_MAX_PX).contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "Reveal sensitivity must be between {REVEAL_SENSITIVITY_MIN_PX} and {REVEAL_SENSITIVITY_MAX_PX} px"
        ))
    }
}

/// Clamps a stored dwell value to the sane range (ticket 113): broken values
/// become the default — the same shape `load` uses — while valid values
/// pass through unchanged. Used by the frontend's explicit clamp before save.
#[allow(dead_code)]
pub fn clamp_reveal_dwell_ms(value: u64) -> u64 {
    value.clamp(REVEAL_DWELL_MIN_MS, REVEAL_DWELL_MAX_MS)
}

/// Clamps a stored sensitivity value to the sane range (ticket 113).
#[allow(dead_code)]
pub fn clamp_reveal_sensitivity_px(value: i32) -> i32 {
    value.clamp(REVEAL_SENSITIVITY_MIN_PX, REVEAL_SENSITIVITY_MAX_PX)
}

/// Validates the companion active URL (ticket 125): None = off, Some must be
/// an https:// URL. Empty/whitespace-only is treated as None. Rejects
/// non-https and malformed URLs.
pub fn validate_companion_url(url: Option<&str>) -> std::result::Result<(), String> {
    if let Some(raw) = url {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        if !trimmed.to_ascii_lowercase().starts_with("https://") {
            return Err("Companion URL must be an https:// URL".into());
        }
        // Minimal structural check: host part must exist
        let after = trimmed[8..].trim();
        if after.is_empty() || after.contains(' ') {
            return Err("Companion URL must be a valid https:// URL".into());
        }
        // Use url crate-style check without adding dependency: ensure at least one dot or localhost token?
        // Keep permissive: any non-empty host is accepted as long as it is https.
    }
    Ok(())
}

/// Normalizes a companion URL: trimmed, empty => None.
pub fn normalize_companion_url(url: Option<&str>) -> Option<String> {
    match url {
        Some(raw) => {
            let trimmed = raw.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }
        None => None,
    }
}

/// Validates the companion height ratio (ticket 125): 0.25–0.60 inclusive.
pub fn validate_companion_height_ratio(value: f64) -> std::result::Result<(), String> {
    if !(COMPANION_HEIGHT_RATIO_MIN..=COMPANION_HEIGHT_RATIO_MAX).contains(&value) {
        return Err(format!(
            "Companion height ratio must be between {COMPANION_HEIGHT_RATIO_MIN:.2} and {COMPANION_HEIGHT_RATIO_MAX:.2}"
        ));
    }
    if !value.is_finite() {
        return Err("Companion height ratio must be a finite number".into());
    }
    Ok(())
}

/// Clamps a stored companion height ratio to the sane range (ticket 125).
pub fn clamp_companion_height_ratio(value: f64) -> f64 {
    if !value.is_finite() {
        return DEFAULT_COMPANION_HEIGHT_RATIO;
    }
    value.clamp(COMPANION_HEIGHT_RATIO_MIN, COMPANION_HEIGHT_RATIO_MAX)
}

/// Dedups companion URL list (ticket 125): trimmed, case-insensitive on host+path,
/// preserving first occurrence order. Empty entries are dropped. Only https URLs survive.
/// Trailing slashes are ignored for dedup (https://a.com ↔ https://a.com/ are the same).
pub fn dedup_companion_url_list(list: &[String]) -> Vec<String> {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out = Vec::new();
    for raw in list {
        let trimmed = raw.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        if validate_companion_url(Some(&trimmed)).is_err() {
            continue;
        }
        // Dedup key: lowercased trimmed URL without trailing slash (host+path case-insensitive).
        let key = trimmed.to_ascii_lowercase().trim_end_matches('/').to_string();
        if seen.contains(&key) {
            continue;
        }
        seen.insert(key);
        out.push(trimmed);
    }
    out
}

/// Validates companion URL list (ticket 125): each entry must be https or empty list.
pub fn validate_companion_url_list(list: &[String]) -> std::result::Result<(), String> {
    for url in list {
        validate_companion_url(Some(url))?;
    }
    Ok(())
}

impl Settings {
    /// Rejects values that would break a run or empty the log archive.
    /// Timeouts must be at least 1 minute and at most a day; retention at
    /// least 1 day and at most 10 years; the install directory, when set,
    /// must be an absolute path; the dock mode, edge, state, and auto-start
    /// must be one of the offered choices.
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
        validate_dock_mode(&self.dock_mode)?;
        validate_dock_edge(&self.dock_edge)?;
        validate_dock_state(&self.dock_state)?;
        validate_autostart(&self.autostart)?;
        validate_groups_feature(&self.launch_groups)?;
        validate_groups_feature(&self.action_groups)?;
        validate_groups_feature(&self.clip_groups)?;
        validate_reveal_dwell_ms(self.reveal_dwell_ms)?;
        validate_reveal_sensitivity_px(self.reveal_sensitivity_px)?;
        validate_companion_url(self.companion_url.as_deref())?;
        validate_companion_height_ratio(self.companion_height_ratio)?;
        validate_companion_url_list(&self.companion_url_list)?;
        Ok(())
    }
}

/// Reads the settings, falling back to the defaults for keys that were never
/// written (fresh installs, pre-09 databases). Every knob follows the same
/// query → validate → default shape: a missing, unparseable, or broken value
/// (a leftover from an older build) reads back as the default.
pub fn load(conn: &Connection) -> Settings {
    fn raw(conn: &Connection, key: &str) -> Option<String> {
        conn.query_row(
            "SELECT value FROM meta WHERE key = ?1",
            params![key],
            |row| row.get::<_, String>(0),
        )
        .ok()
    }
    fn number(conn: &Connection, key: &str) -> Option<u32> {
        raw(conn, key).and_then(|value| value.parse().ok())
    }
    fn number_u64(conn: &Connection, key: &str) -> Option<u64> {
        raw(conn, key).and_then(|value| value.parse().ok())
    }
    fn number_i32(conn: &Connection, key: &str) -> Option<i32> {
        raw(conn, key).and_then(|value| value.parse().ok())
    }
    fn validated(
        conn: &Connection,
        key: &str,
        check: fn(&str) -> std::result::Result<(), String>,
    ) -> Option<String> {
        raw(conn, key).filter(|value| check(value).is_ok())
    }

    // Companion fields (ticket 125): tolerant parsing — broken values fall back
    let companion_url = raw(conn, KEY_COMPANION_URL)
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .filter(|v| validate_companion_url(Some(v)).is_ok());
    let companion_height_ratio = raw(conn, KEY_COMPANION_HEIGHT_RATIO)
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| validate_companion_height_ratio(*v).is_ok())
        .unwrap_or(DEFAULT_COMPANION_HEIGHT_RATIO);
    let companion_url_list = raw(conn, KEY_COMPANION_URL_LIST)
        .and_then(|v| serde_json::from_str::<Vec<String>>(&v).ok())
        .map(|list| dedup_companion_url_list(&list))
        .unwrap_or_default();
    // WHY stored as "1"/"0": the meta table holds strings, and an explicit
    // pair keeps a broken value readable as unmuted instead of erroring.
    let companion_muted = raw(conn, KEY_COMPANION_MUTED)
        .map(|v| matches!(v.trim(), "1" | "true" | "on"))
        .unwrap_or(DEFAULT_COMPANION_MUTED);

    Settings {
        default_timeout_minutes: number(conn, KEY_TIMEOUT).unwrap_or(DEFAULT_TIMEOUT_MINUTES),
        log_retention_days: number(conn, KEY_RETENTION).unwrap_or(DEFAULT_RETENTION_DAYS),
        theme: validated(conn, KEY_THEME, validate_theme)
            .unwrap_or_else(|| DEFAULT_THEME.to_string()),
        install_dir: validated(conn, KEY_INSTALL_DIR, validate_install_dir).unwrap_or_default(),
        launch_concurrency: number(conn, KEY_LAUNCH_CONCURRENCY)
            .filter(|value| (1..=50).contains(value))
            .unwrap_or(DEFAULT_LAUNCH_CONCURRENCY),
        dock_mode: validated(conn, KEY_DOCK_MODE, validate_dock_mode)
            .unwrap_or_else(|| DEFAULT_DOCK_MODE.to_string()),
        dock_edge: validated(conn, KEY_DOCK_EDGE, validate_dock_edge)
            .unwrap_or_else(|| DEFAULT_DOCK_EDGE.to_string()),
        dock_state: validated(conn, KEY_DOCK_STATE, validate_dock_state)
            .unwrap_or_else(|| DEFAULT_DOCK_STATE.to_string()),
        autostart: validated(conn, KEY_AUTOSTART, validate_autostart)
            .unwrap_or_else(|| DEFAULT_AUTOSTART.to_string()),
        launch_groups: validated(conn, KEY_LAUNCH_GROUPS, validate_groups_feature)
            .unwrap_or_else(|| DEFAULT_GROUPS_FEATURE.to_string()),
        action_groups: validated(conn, KEY_ACTION_GROUPS, validate_groups_feature)
            .unwrap_or_else(|| DEFAULT_GROUPS_FEATURE.to_string()),
        clip_groups: validated(conn, KEY_CLIP_GROUPS, validate_groups_feature)
            .unwrap_or_else(|| DEFAULT_GROUPS_FEATURE.to_string()),
        reveal_dwell_ms: number_u64(conn, KEY_REVEAL_DWELL_MS)
            .filter(|v| (REVEAL_DWELL_MIN_MS..=REVEAL_DWELL_MAX_MS).contains(v))
            .unwrap_or(DEFAULT_REVEAL_DWELL_MS),
        reveal_sensitivity_px: number_i32(conn, KEY_REVEAL_SENSITIVITY_PX)
            .filter(|v| (REVEAL_SENSITIVITY_MIN_PX..=REVEAL_SENSITIVITY_MAX_PX).contains(v))
            .unwrap_or(DEFAULT_REVEAL_SENSITIVITY_PX),
        companion_url,
        companion_height_ratio,
        companion_url_list,
        companion_muted,
    }
}

/// Persists the settings, validated first — a broken value must never reach
/// the run pipeline.
pub fn save(conn: &Connection, settings: &Settings) -> std::result::Result<(), String> {
    settings.validate()?;
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| e.to_string())?;
    upsert_meta(&tx, KEY_TIMEOUT, &settings.default_timeout_minutes.to_string()).map_err(|e| e.to_string())?;
    upsert_meta(&tx, KEY_RETENTION, &settings.log_retention_days.to_string()).map_err(|e| e.to_string())?;
    upsert_meta(&tx, KEY_THEME, &settings.theme).map_err(|e| e.to_string())?;
    upsert_meta(&tx, KEY_INSTALL_DIR, &settings.install_dir).map_err(|e| e.to_string())?;
    upsert_meta(&tx, KEY_LAUNCH_CONCURRENCY, &settings.launch_concurrency.to_string())
        .map_err(|e| e.to_string())?;
    upsert_meta(&tx, KEY_DOCK_MODE, &settings.dock_mode).map_err(|e| e.to_string())?;
    upsert_meta(&tx, KEY_DOCK_EDGE, &settings.dock_edge).map_err(|e| e.to_string())?;
    upsert_meta(&tx, KEY_DOCK_STATE, &settings.dock_state).map_err(|e| e.to_string())?;
    upsert_meta(&tx, KEY_AUTOSTART, &settings.autostart).map_err(|e| e.to_string())?;
    upsert_meta(&tx, KEY_LAUNCH_GROUPS, &settings.launch_groups).map_err(|e| e.to_string())?;
    upsert_meta(&tx, KEY_ACTION_GROUPS, &settings.action_groups).map_err(|e| e.to_string())?;
    upsert_meta(&tx, KEY_CLIP_GROUPS, &settings.clip_groups).map_err(|e| e.to_string())?;
    upsert_meta(&tx, KEY_REVEAL_DWELL_MS, &settings.reveal_dwell_ms.to_string()).map_err(|e| e.to_string())?;
    upsert_meta(&tx, KEY_REVEAL_SENSITIVITY_PX, &settings.reveal_sensitivity_px.to_string())
        .map_err(|e| e.to_string())?;
    // Companion (ticket 125): active URL stored as plain string (empty = null fallback),
    // height ratio as f64 string, URL list as JSON array string.
    if let Some(url) = &settings.companion_url {
        upsert_meta(&tx, KEY_COMPANION_URL, url).map_err(|e| e.to_string())?;
    } else {
        // Remove key vs storing empty — store empty so load fallback works; also delete semantics via empty.
        // Keep empty string as null-equivalent for backwards compat.
        upsert_meta(&tx, KEY_COMPANION_URL, "").map_err(|e| e.to_string())?;
    }
    upsert_meta(&tx, KEY_COMPANION_HEIGHT_RATIO, &settings.companion_height_ratio.to_string())
        .map_err(|e| e.to_string())?;
    let list_json = serde_json::to_string(&settings.companion_url_list).map_err(|e| e.to_string())?;
    upsert_meta(&tx, KEY_COMPANION_URL_LIST, &list_json).map_err(|e| e.to_string())?;
    upsert_meta(&tx, KEY_COMPANION_MUTED, if settings.companion_muted { "1" } else { "0" })
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

/// Persists only the theme — the Settings screen applies it the moment it is
/// selected, without touching the other knobs (ticket 31).
pub fn save_theme(conn: &Connection, theme: &str) -> std::result::Result<(), String> {
    validate_theme(theme)?;
    upsert_meta(conn, KEY_THEME, theme).map_err(|e| e.to_string())
}

/// Persists only the dock state (ticket 57) — the in-window dock/undock
/// toggle writes back "floating"/"docked" without touching the other knobs,
/// so the Settings screen and the window never diverge.
pub fn save_dock_state(conn: &Connection, state: &str) -> std::result::Result<(), String> {
    validate_dock_state(state)?;
    upsert_meta(conn, KEY_DOCK_STATE, state).map_err(|e| e.to_string())
}

/// Persists only the dock edge (ticket 57) — the in-window edge-switch
/// arrows write back "left"/"right" without touching the other knobs, so the
/// Settings screen's default edge stays aligned with where the dock lives.
pub fn save_dock_edge(conn: &Connection, edge: &str) -> std::result::Result<(), String> {
    validate_dock_edge(edge)?;
    upsert_meta(conn, KEY_DOCK_EDGE, edge).map_err(|e| e.to_string())
}

/// Persists only the auto-start preference (ADR-0013, ticket 75) — the
/// Settings toggle writes it without touching the other knobs; the Run-key
/// registration is reconciled beside the save by the caller.
pub fn save_autostart(conn: &Connection, value: &str) -> std::result::Result<(), String> {
    validate_autostart(value)?;
    upsert_meta(conn, KEY_AUTOSTART, value).map_err(|e| e.to_string())
}

/// Persists one collection's Groups toggle (ticket 89) — the list page's
/// toolbar-row switch writes its own collection's key without touching the
/// other knobs or collections.
pub fn save_groups_feature(
    conn: &Connection,
    collection: crate::groups::Collection,
    value: &str,
) -> std::result::Result<(), String> {
    validate_groups_feature(value)?;
    let key = match collection {
        crate::groups::Collection::Launch => KEY_LAUNCH_GROUPS,
        crate::groups::Collection::Action => KEY_ACTION_GROUPS,
        crate::groups::Collection::Clip => KEY_CLIP_GROUPS,
    };
    upsert_meta(conn, key, value).map_err(|e| e.to_string())
}

/// Persists only the companion active URL (ticket 125) — null = empty string.
pub fn save_companion_url(conn: &Connection, url: Option<&str>) -> std::result::Result<(), String> {
    validate_companion_url(url)?;
    let normalized = normalize_companion_url(url);
    let stored = normalized.as_deref().unwrap_or("");
    upsert_meta(conn, KEY_COMPANION_URL, stored).map_err(|e| e.to_string())
}

/// Persists only the companion height ratio (ticket 125) — clamped on read but
/// rejected on save when out of range (explicit validation).
pub fn save_companion_height_ratio(conn: &Connection, ratio: f64) -> std::result::Result<(), String> {
    validate_companion_height_ratio(ratio)?;
    upsert_meta(conn, KEY_COMPANION_HEIGHT_RATIO, &ratio.to_string()).map_err(|e| e.to_string())
}

/// Persists only the companion URL list (ticket 125) — deduped first, then stored as JSON.
pub fn save_companion_url_list(conn: &Connection, list: &[String]) -> std::result::Result<(), String> {
    validate_companion_url_list(list)?;
    let deduped = dedup_companion_url_list(list);
    let json = serde_json::to_string(&deduped).map_err(|e| e.to_string())?;
    upsert_meta(conn, KEY_COMPANION_URL_LIST, &json).map_err(|e| e.to_string())
}

/// Persists only the companion mute — the dock toolbar's toggle writes here
/// without touching the other knobs, so Settings and the toolbar never
/// diverge. A bool needs no validation; any stored string outside "1" reads
/// back as unmuted.
pub fn save_companion_muted(conn: &Connection, muted: bool) -> std::result::Result<(), String> {
    upsert_meta(conn, KEY_COMPANION_MUTED, if muted { "1" } else { "0" })
        .map_err(|e| e.to_string())
}

/// Reads only the companion mute — the audio-state query's persisted half.
/// Missing or broken values read back as unmuted.
pub fn load_companion_muted(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT value FROM meta WHERE key = ?1",
        rusqlite::params![KEY_COMPANION_MUTED],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .map(|v| matches!(v.trim(), "1" | "true" | "on"))
    .unwrap_or(DEFAULT_COMPANION_MUTED)
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
        assert_eq!(load(&conn).dock_mode, DEFAULT_DOCK_MODE);
        assert_eq!(load(&conn).dock_edge, DEFAULT_DOCK_EDGE);
        assert_eq!(load(&conn).dock_state, DEFAULT_DOCK_STATE);
        assert_eq!(load(&conn).autostart, DEFAULT_AUTOSTART);
        assert_eq!(load(&conn).launch_groups, DEFAULT_GROUPS_FEATURE);
        assert_eq!(load(&conn).action_groups, DEFAULT_GROUPS_FEATURE);
        assert_eq!(load(&conn).clip_groups, DEFAULT_GROUPS_FEATURE);
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
            dock_mode: "fixed".to_string(),
            dock_edge: "right".to_string(),
            dock_state: "docked".to_string(),
            autostart: "off".to_string(),
            launch_groups: "on".to_string(),
            action_groups: "off".to_string(),
            clip_groups: "on".to_string(),
            reveal_dwell_ms: 350,
            reveal_sensitivity_px: 20,
            companion_url: Some("https://music.youtube.com".to_string()),
            companion_height_ratio: 0.55,
            companion_url_list: vec!["https://music.youtube.com".to_string(), "https://open.spotify.com".to_string()],
            companion_muted: true,
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
        s.dock_mode = "overlay".to_string();
        assert!(s.validate().is_err());
        s.dock_mode = DEFAULT_DOCK_MODE.to_string();
        assert!(s.validate().is_ok());
        s.dock_edge = "top".to_string();
        assert!(s.validate().is_err());
        s.dock_edge = DEFAULT_DOCK_EDGE.to_string();
        assert!(s.validate().is_ok());
        s.dock_state = "minimized".to_string();
        assert!(s.validate().is_err());
        s.dock_state = DEFAULT_DOCK_STATE.to_string();
        assert!(s.validate().is_ok());
        s.dock_state = "docked".to_string();
        assert!(s.validate().is_ok());
        s.autostart = "maybe".to_string();
        assert!(s.validate().is_err());
        s.autostart = DEFAULT_AUTOSTART.to_string();
        assert!(s.validate().is_ok());
        s.autostart = "off".to_string();
        assert!(s.validate().is_ok());
        s.launch_groups = "maybe".to_string();
        assert!(s.validate().is_err());
        s.launch_groups = "on".to_string();
        assert!(s.validate().is_ok());
        s.action_groups = "maybe".to_string();
        assert!(s.validate().is_err());
        s.action_groups = "off".to_string();
        assert!(s.validate().is_ok());
        s.clip_groups = "maybe".to_string();
        assert!(s.validate().is_err());
        s.clip_groups = "on".to_string();
        assert!(s.validate().is_ok());
    }

    #[test]
    fn invalid_stored_groups_features_fall_back_to_default() {
        let conn = conn();
        // Broken values written by an older build must never surface the
        // feature — each collection reads back as dormant (off).
        upsert_meta(&conn, KEY_LAUNCH_GROUPS, "maybe").unwrap();
        upsert_meta(&conn, KEY_ACTION_GROUPS, "").unwrap();
        upsert_meta(&conn, KEY_CLIP_GROUPS, "1").unwrap();
        let loaded = load(&conn);
        assert_eq!(loaded.launch_groups, DEFAULT_GROUPS_FEATURE);
        assert_eq!(loaded.action_groups, DEFAULT_GROUPS_FEATURE);
        assert_eq!(loaded.clip_groups, DEFAULT_GROUPS_FEATURE);
    }

    #[test]
    fn groups_feature_saver_targets_one_collection_only() {
        let conn = conn();
        save_groups_feature(&conn, crate::groups::Collection::Action, "on").unwrap();
        let loaded = load(&conn);
        assert_eq!(loaded.action_groups, "on");
        assert_eq!(loaded.launch_groups, DEFAULT_GROUPS_FEATURE);
        assert_eq!(loaded.clip_groups, DEFAULT_GROUPS_FEATURE);

        save_groups_feature(&conn, crate::groups::Collection::Clip, "off").unwrap();
        assert_eq!(load(&conn).clip_groups, "off");
        assert!(save_groups_feature(&conn, crate::groups::Collection::Launch, "maybe").is_err());
    }

    #[test]
    fn out_of_range_stored_concurrency_falls_back_to_default() {
        let conn = conn();
        // A broken value written by an older build must never reach the
        // launch pipeline — it reads back as the default.
        upsert_meta(&conn, KEY_LAUNCH_CONCURRENCY, "0").unwrap();
        assert_eq!(load(&conn).launch_concurrency, DEFAULT_LAUNCH_CONCURRENCY);
        upsert_meta(&conn, KEY_LAUNCH_CONCURRENCY, "99").unwrap();
        assert_eq!(load(&conn).launch_concurrency, DEFAULT_LAUNCH_CONCURRENCY);
    }

    #[test]
    fn invalid_stored_dock_values_fall_back_to_defaults() {
        let conn = conn();
        // Broken values written by an older build must never reach the dock —
        // they read back as the defaults.
        upsert_meta(&conn, KEY_DOCK_MODE, "overlay").unwrap();
        assert_eq!(load(&conn).dock_mode, DEFAULT_DOCK_MODE);
        upsert_meta(&conn, KEY_DOCK_EDGE, "top").unwrap();
        assert_eq!(load(&conn).dock_edge, DEFAULT_DOCK_EDGE);
        upsert_meta(&conn, KEY_DOCK_STATE, "minimized").unwrap();
        assert_eq!(load(&conn).dock_state, DEFAULT_DOCK_STATE);
    }

    #[test]
    fn invalid_stored_autostart_falls_back_to_default() {
        let conn = conn();
        // A broken value written by an older build must never decide the
        // registration — it reads back as the default (on).
        upsert_meta(&conn, KEY_AUTOSTART, "maybe").unwrap();
        assert_eq!(load(&conn).autostart, DEFAULT_AUTOSTART);
    }

    #[test]
    fn the_retired_desktop_assignments_key_is_never_read() {
        let conn = conn();
        // ADR-0015: the master switch is gone — assignments are always live
        // where the OS supports them. A value written by an older build stays
        // in the meta table untouched (no migration), but nothing reads it:
        // settings still equal the defaults with even an "on" row present.
        upsert_meta(&conn, "launch.desktop_assignments", "on").unwrap();
        assert_eq!(load(&conn), Settings::default());
    }

    #[test]
    fn dock_state_roundtrips_on_its_own() {
        let dir = clean_dir();
        {
            let conn = crate::db::init_at(&dir).unwrap();
            save_dock_state(&conn, "docked").unwrap();
            assert_eq!(load(&conn).dock_state, "docked");
        }
        // Re-open: the dock state survives the connection.
        let conn = crate::db::init_at(&dir).unwrap();
        assert_eq!(load(&conn).dock_state, "docked");
        // And the in-window toggle's edge write-back persists on its own too.
        save_dock_edge(&conn, "right").unwrap();
        assert_eq!(load(&conn).dock_edge, "right");
    }

    #[test]
    fn save_dock_state_rejects_unknown_values_and_keeps_the_old_one() {
        let conn = conn();
        assert!(save_dock_state(&conn, "minimized").is_err());
        // Nothing was persisted.
        assert_eq!(load(&conn).dock_state, DEFAULT_DOCK_STATE);
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
    fn autostart_roundtrips_on_its_own() {
        let dir = clean_dir();
        {
            let conn = crate::db::init_at(&dir).unwrap();
            save_autostart(&conn, "off").unwrap();
            assert_eq!(load(&conn).autostart, "off");
        }
        // Re-open: the preference survives the connection.
        let conn = crate::db::init_at(&dir).unwrap();
        assert_eq!(load(&conn).autostart, "off");
        // And toggling back on persists on its own too.
        save_autostart(&conn, "on").unwrap();
        assert_eq!(load(&conn).autostart, "on");
    }

    #[test]
    fn save_autostart_rejects_unknown_values_and_keeps_the_old_one() {
        let conn = conn();
        assert!(save_autostart(&conn, "maybe").is_err());
        // Nothing was persisted.
        assert_eq!(load(&conn).autostart, DEFAULT_AUTOSTART);
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
            dock_mode: DEFAULT_DOCK_MODE.to_string(),
            dock_edge: DEFAULT_DOCK_EDGE.to_string(),
            dock_state: DEFAULT_DOCK_STATE.to_string(),
            autostart: DEFAULT_AUTOSTART.to_string(),
            launch_groups: DEFAULT_GROUPS_FEATURE.to_string(),
            action_groups: DEFAULT_GROUPS_FEATURE.to_string(),
            clip_groups: DEFAULT_GROUPS_FEATURE.to_string(),
            reveal_dwell_ms: DEFAULT_REVEAL_DWELL_MS,
            reveal_sensitivity_px: DEFAULT_REVEAL_SENSITIVITY_PX,
            companion_url: None,
            companion_height_ratio: DEFAULT_COMPANION_HEIGHT_RATIO,
            companion_url_list: Vec::new(),
            companion_muted: false,
        };
        assert!(save(&conn, &bad).is_err());
        // Nothing was persisted.
        assert_eq!(load(&conn), Settings::default());
    }

    #[test]
    fn reveal_defaults_equal_shipped_gate_constants() {
        // Ticket 113: defaults must equal the shipped gate constants (single
        // size source); changing a constant updates the default automatically.
        assert_eq!(DEFAULT_REVEAL_DWELL_MS, crate::constants::window::REVEAL_DWELL_MS);
        assert_eq!(
            DEFAULT_REVEAL_SENSITIVITY_PX,
            crate::constants::window::REVEAL_SENSITIVITY_PX
        );
        let d = Settings::default();
        assert_eq!(d.reveal_dwell_ms, DEFAULT_REVEAL_DWELL_MS);
        assert_eq!(d.reveal_sensitivity_px, DEFAULT_REVEAL_SENSITIVITY_PX);
    }

    #[test]
    fn reveal_validation_and_clamp_cover_sane_ranges() {
        // Valid boundaries inclusive
        assert!(validate_reveal_dwell_ms(REVEAL_DWELL_MIN_MS).is_ok());
        assert!(validate_reveal_dwell_ms(REVEAL_DWELL_MAX_MS).is_ok());
        assert!(validate_reveal_dwell_ms(200).is_ok());
        assert!(validate_reveal_dwell_ms(REVEAL_DWELL_MAX_MS + 1).is_err());
        assert!(validate_reveal_sensitivity_px(REVEAL_SENSITIVITY_MIN_PX).is_ok());
        assert!(validate_reveal_sensitivity_px(REVEAL_SENSITIVITY_MAX_PX).is_ok());
        assert!(validate_reveal_sensitivity_px(12).is_ok());
        assert!(validate_reveal_sensitivity_px(REVEAL_SENSITIVITY_MAX_PX + 1).is_err());
        assert!(validate_reveal_sensitivity_px(REVEAL_SENSITIVITY_MIN_PX - 1).is_err());

        // Clamp mirrors validation range
        assert_eq!(clamp_reveal_dwell_ms(0), 0);
        assert_eq!(clamp_reveal_dwell_ms(500), 500);
        assert_eq!(clamp_reveal_dwell_ms(2000), REVEAL_DWELL_MAX_MS);
        assert_eq!(clamp_reveal_dwell_ms(u64::MAX), REVEAL_DWELL_MAX_MS);
        assert_eq!(clamp_reveal_sensitivity_px(-5), REVEAL_SENSITIVITY_MIN_PX);
        assert_eq!(clamp_reveal_sensitivity_px(50), 50);
        assert_eq!(clamp_reveal_sensitivity_px(99), REVEAL_SENSITIVITY_MAX_PX);

        // Settings::validate honors the same ranges
        let mut s = Settings::default();
        s.reveal_dwell_ms = REVEAL_DWELL_MAX_MS + 1;
        assert!(s.validate().is_err());
        s.reveal_dwell_ms = REVEAL_DWELL_MAX_MS;
        s.reveal_sensitivity_px = REVEAL_SENSITIVITY_MAX_PX + 1;
        assert!(s.validate().is_err());
        s.reveal_sensitivity_px = REVEAL_SENSITIVITY_MIN_PX;
        assert!(s.validate().is_ok());
    }

    #[test]
    fn invalid_stored_reveal_values_fall_back_to_defaults() {
        let conn = conn();
        // Broken values written by an older build or manual edit must never
        // reach the driver — they read back as the defaults.
        upsert_meta(&conn, KEY_REVEAL_DWELL_MS, "9999").unwrap();
        upsert_meta(&conn, KEY_REVEAL_SENSITIVITY_PX, "-5").unwrap();
        let loaded = load(&conn);
        assert_eq!(loaded.reveal_dwell_ms, DEFAULT_REVEAL_DWELL_MS);
        assert_eq!(loaded.reveal_sensitivity_px, DEFAULT_REVEAL_SENSITIVITY_PX);

        // Unparseable also falls back
        upsert_meta(&conn, KEY_REVEAL_DWELL_MS, "fast").unwrap();
        upsert_meta(&conn, KEY_REVEAL_SENSITIVITY_PX, "high").unwrap();
        let loaded = load(&conn);
        assert_eq!(loaded.reveal_dwell_ms, DEFAULT_REVEAL_DWELL_MS);
        assert_eq!(loaded.reveal_sensitivity_px, DEFAULT_REVEAL_SENSITIVITY_PX);
    }

    #[test]
    fn reveal_settings_roundtrip() {
        let dir = clean_dir();
        let mut s = Settings::default();
        s.reveal_dwell_ms = 450;
        s.reveal_sensitivity_px = 25;
        {
            let conn = crate::db::init_at(&dir).unwrap();
            save(&conn, &s).unwrap();
            assert_eq!(load(&conn).reveal_dwell_ms, 450);
            assert_eq!(load(&conn).reveal_sensitivity_px, 25);
        }
        let conn = crate::db::init_at(&dir).unwrap();
        assert_eq!(load(&conn).reveal_dwell_ms, 450);
        assert_eq!(load(&conn).reveal_sensitivity_px, 25);
    }

    #[test]
    fn companion_validation_and_clamp_cover_sane_ranges() {
        assert!(validate_companion_url(None).is_ok());
        assert!(validate_companion_url(Some("")).is_ok());
        assert!(validate_companion_url(Some("   ")).is_ok());
        assert!(validate_companion_url(Some("https://music.youtube.com")).is_ok());
        assert!(validate_companion_url(Some("https://open.spotify.com")).is_ok());
        assert!(validate_companion_url(Some("http://music.youtube.com")).is_err());
        assert!(validate_companion_url(Some("music.youtube.com")).is_err());
        assert!(validate_companion_url(Some("https://")).is_err());
        assert!(validate_companion_height_ratio(0.25).is_ok());
        assert!(validate_companion_height_ratio(0.40).is_ok());
        assert!(validate_companion_height_ratio(0.60).is_ok());
        assert!(validate_companion_height_ratio(0.24).is_err());
        assert!(validate_companion_height_ratio(0.61).is_err());
        assert!(validate_companion_height_ratio(f64::NAN).is_err());
        assert_eq!(clamp_companion_height_ratio(0.10), COMPANION_HEIGHT_RATIO_MIN);
        assert_eq!(clamp_companion_height_ratio(0.55), 0.55);
        assert_eq!(clamp_companion_height_ratio(0.90), COMPANION_HEIGHT_RATIO_MAX);
        assert_eq!(clamp_companion_height_ratio(f64::NAN), DEFAULT_COMPANION_HEIGHT_RATIO);
        let mut s = Settings::default();
        s.companion_url = Some("http://bad".to_string());
        assert!(s.validate().is_err());
        s.companion_url = Some("https://ok.example.com".to_string());
        s.companion_height_ratio = 0.90;
        assert!(s.validate().is_err());
        s.companion_height_ratio = 0.40;
        s.companion_url_list = vec!["https://ok.example.com".to_string(), "http://bad".to_string()];
        assert!(s.validate().is_err());
        s.companion_url_list = vec!["https://ok.example.com".to_string()];
        assert!(s.validate().is_ok());
    }

    #[test]
    fn companion_url_list_dedup_is_case_insensitive_and_trimmed() {
        let list = vec![
            " https://Music.Youtube.com ".to_string(),
            "https://music.youtube.com".to_string(),
            "https://open.spotify.com".to_string(),
            "HTTPS://OPEN.SPOTIFY.COM/ ".to_string(),
            "http://bad.example.com".to_string(),
            "  ".to_string(),
        ];
        let deduped = dedup_companion_url_list(&list);
        assert_eq!(deduped, vec!["https://Music.Youtube.com".to_string(), "https://open.spotify.com".to_string()]);
    }

    #[test]
    fn companion_settings_roundtrip() {
        let dir = clean_dir();
        let mut s = Settings::default();
        s.companion_url = Some("https://music.youtube.com".to_string());
        s.companion_height_ratio = 0.55;
        s.companion_url_list = vec!["https://music.youtube.com".to_string(), "https://open.spotify.com".to_string()];
        {
            let conn = crate::db::init_at(&dir).unwrap();
            save(&conn, &s).unwrap();
            let loaded = load(&conn);
            assert_eq!(loaded.companion_url, Some("https://music.youtube.com".to_string()));
            assert!((loaded.companion_height_ratio - 0.55).abs() < 1e-9);
            assert_eq!(loaded.companion_url_list, vec!["https://music.youtube.com".to_string(), "https://open.spotify.com".to_string()]);
        }
        let conn = crate::db::init_at(&dir).unwrap();
        let loaded = load(&conn);
        assert_eq!(loaded.companion_url, Some("https://music.youtube.com".to_string()));
        assert!((loaded.companion_height_ratio - 0.55).abs() < 1e-9);
    }

    #[test]
    fn invalid_stored_companion_values_fall_back_to_defaults() {
        let conn = conn();
        upsert_meta(&conn, KEY_COMPANION_URL, "http://bad").unwrap();
        upsert_meta(&conn, KEY_COMPANION_HEIGHT_RATIO, "0.90").unwrap();
        upsert_meta(&conn, KEY_COMPANION_URL_LIST, "not json").unwrap();
        let loaded = load(&conn);
        assert_eq!(loaded.companion_url, None);
        assert!((loaded.companion_height_ratio - DEFAULT_COMPANION_HEIGHT_RATIO).abs() < 1e-9);
        assert!(loaded.companion_url_list.is_empty());
        upsert_meta(&conn, KEY_COMPANION_HEIGHT_RATIO, "fast").unwrap();
        let loaded = load(&conn);
        assert!((loaded.companion_height_ratio - DEFAULT_COMPANION_HEIGHT_RATIO).abs() < 1e-9);
    }

    #[test]
    fn companion_url_saver_targets_one_knob_only() {
        let conn = conn();
        save_companion_url(&conn, Some("https://music.youtube.com")).unwrap();
        assert_eq!(load(&conn).companion_url, Some("https://music.youtube.com".to_string()));
        assert!((load(&conn).companion_height_ratio - DEFAULT_COMPANION_HEIGHT_RATIO).abs() < 1e-9);
        save_companion_url(&conn, None).unwrap();
        assert_eq!(load(&conn).companion_url, None);
        assert!(save_companion_url(&conn, Some("http://bad")).is_err());
        save_companion_height_ratio(&conn, 0.55).unwrap();
        assert!((load(&conn).companion_height_ratio - 0.55).abs() < 1e-9);
        assert!(save_companion_height_ratio(&conn, 0.90).is_err());
        let list = vec!["https://a.example.com".to_string(), " https://A.example.com ".to_string(), "https://b.example.com".to_string()];
        save_companion_url_list(&conn, &list).unwrap();
        // deduped on save load cycle
        assert_eq!(load(&conn).companion_url_list, vec!["https://a.example.com".to_string(), "https://b.example.com".to_string()]);
    }

    #[test]
    fn companion_mute_roundtrip_and_default() {
        // WHY the default matters: a fresh install must never start silent —
        // muting is always an explicit toolbar action.
        let conn = conn();
        assert_eq!(load(&conn).companion_muted, DEFAULT_COMPANION_MUTED);
        assert!(!DEFAULT_COMPANION_MUTED);
        save_companion_muted(&conn, true).unwrap();
        assert!(load(&conn).companion_muted);
        assert!(load_companion_muted(&conn));
        save_companion_muted(&conn, false).unwrap();
        assert!(!load(&conn).companion_muted);
        // Full-settings save carries the knob too.
        let mut s = Settings::default();
        s.companion_muted = true;
        save(&conn, &s).unwrap();
        assert!(load(&conn).companion_muted);
        // A broken stored value falls back to unmuted, never to silent.
        upsert_meta(&conn, KEY_COMPANION_MUTED, "loud").unwrap();
        assert!(!load(&conn).companion_muted);
        assert!(!load_companion_muted(&conn));
    }
}