// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

 mod appbar;
 mod autostart;
 mod backup;
  mod clips;
  mod companion_audio;
  mod constants;
mod db;
mod domain;
mod engine;
mod external;
mod groups;
mod icons;
mod import_export;
 mod launch;
 mod logs;
 mod ordered_list;
 mod plan;
mod quick_actions;
mod quick_window;
mod run;
mod settings;
mod store;
mod tray;
mod update;
mod walker;
mod winget;
mod worker;

pub use worker::run_worker;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use tauri::{AppHandle, Emitter, Manager, State};

use domain::{Preset, PresetRecord, Product, ProductRecord, Requirement};
use engine::{windows::WindowsWingetEngine, DesktopInfo, LauncherEngine, PlatformEngine};
use backup::{BackupCounts, ImportSummary};
use import_export::ImportResult;
use logs::LogLocations;
use plan::Composition;
use run::{ProgressEvent, RunRecord, RunSummary};
use settings::Settings;
use winget::{WingetMatch, WingetShow};
use worker::{ActiveRunInfo, DoneInfo};

/// Managed app state: the Library connection (mutexed, rusqlite's Connection
/// is not Sync), the platform engine behind the strategy seam, the launcher
/// engine behind the Quick Launch seam (ticket 42), the single-flight guard
/// that keeps two Quick Launch runs from stacking, the `.sprout.json`
/// path the app was launched with (double-click), consumed by the frontend
/// on first load, and the Quick Action run registry (ticket 62).
pub struct AppState {
    pub db: Mutex<Connection>,
    pub engine: Arc<dyn PlatformEngine>,
    pub launcher: Arc<dyn LauncherEngine>,
    pub launch_in_progress: Arc<AtomicBool>,
    pub pending_import: Mutex<Option<String>>,
    /// The Quick Launch window's live dock state (ticket 53): `Some` while the
    /// window is docked as a Win32 AppBar, cleared on undock/close/quit.
    pub dock: Mutex<Option<quick_window::DockState>>,
    /// The Quick Action run registry (ticket 62): action id -> the tracked
    /// run, for every action whose spawned process is still alive. Per-session
    /// only — the entries die with the boot, so nothing persists.
    pub running_actions: Mutex<HashMap<i64, quick_actions::RunningQuickAction>>,
    /// Ticket 116: whether Settings is dirty (unsaved changes). Frontend syncs
    /// via `set_settings_dirty`; the main-window close handler gates on it.
    pub settings_dirty: Mutex<bool>,
    /// Ticket 123: timestamp of last main window close (destroy) — used to
    /// detect the close→reopen race where `get_webview_window("main")` still
    /// returns a zombie handle for a few ms after `destroy()`. `open_main_window`
    /// checks this and sleeps if the close was very recent.
    pub main_close_time: Mutex<Option<std::time::Instant>>,
    /// WHY the open path single-flights off the event thread: `open_main_window`
    /// sleeps synchronously (close grace + zombie retries) while the queued
    /// `destroy()` it waits on needs that same thread to run — direct calls
    /// self-deadlock into an invisible frame with a dead X (ADR-0013 keeps the
    /// size source; this flag only owns dispatch, no geometry).
    pub main_window_opening: AtomicBool,
    /// A newly-created main window is native-visible but fully transparent so
    /// WebView2 can load without showing its blank startup surface. The main
    /// layout clears this only after its first render is mounted.
    pub main_window_loading: AtomicBool,
}

fn lock<'a>(state: &'a State<'a, AppState>) -> Result<std::sync::MutexGuard<'a, Connection>, String> {
    state.db.lock().map_err(|e| e.to_string())
}

/// Emits `quick-launch-changed` only to the Quick Launch window when its HWND
/// is still valid — avoids `PostMessage failed ; Invalid window handle` spam
/// when the main window has just been destroyed (close→reopen race, vite HMR
/// reload). `app.emit` would broadcast to the destroyed main webview as well.
///
/// The host resolves as a native Window: a WebviewWindow lookup rejects a
/// window hosting a differently labeled child WebView, so with Companion up
/// it yields None and silently drops the event the dock's entries, actions,
/// clips, and Companion state all refresh from.
fn emit_quick_launch_changed(app: &AppHandle) {
    if let Some(window) = quick_window::quick_launch_window(&app) {
        let valid = match window.hwnd() {
            Ok(hwnd) => unsafe { windows_sys::Win32::UI::WindowsAndMessaging::IsWindow(hwnd.0) != 0 },
            Err(_) => false,
        };
        if valid {
            let _ = window.emit("quick-launch-changed", ());
        }
    }
}

fn emit_valid<T: serde::Serialize + Clone>(app: &AppHandle, event: &str, payload: &T) {
    use tauri::Manager;
    for label in [quick_window::QUICK_LAUNCH_WINDOW, "main"] {
        if let Some(window) = app.get_window(label) {
            let valid = match window.hwnd() {
                Ok(hwnd) => unsafe { windows_sys::Win32::UI::WindowsAndMessaging::IsWindow(hwnd.0) != 0 },
                Err(_) => false,
            };
            if valid {
                let _ = window.emit(event, payload.clone());
            }
        }
    }
}

/// Lists Library Products, optionally filtered by a search query matched
/// against name and winget ID. Records carry the Library-only create/update
/// times.
#[tauri::command]
fn list_products(
    state: State<'_, AppState>,
    query: Option<String>,
) -> Result<Vec<ProductRecord>, String> {
    let conn = lock(&state)?;
    db::list_products(&conn, query.as_deref()).map_err(|e| e.to_string())
}

/// Adds a Product to the Library.
#[tauri::command]
fn create_product(state: State<'_, AppState>, product: Product) -> Result<(), String> {
    db::validate_product(&product)?;
    let conn = lock(&state)?;
    db::create_product(&conn, &product).map_err(|e| e.to_string())
}

/// Updates a Product in place (same id, new metadata/env wiring).
#[tauri::command]
fn update_product(state: State<'_, AppState>, product: Product) -> Result<(), String> {
    db::validate_product(&product)?;
    let conn = lock(&state)?;
    db::update_product(&conn, &product).map_err(|e| e.to_string())
}

/// The delete prompt's impact: how many local Presets reference a Product.
#[derive(serde::Serialize)]
pub struct ProductPresetImpact {
    pub preset_count: usize,
}

/// Removes a Product from the Library, dropping the Requirements that
/// reference it from local Presets (their live link is gone — ADR-0007).
/// Imported Presets keep their embedded snapshot; run history is untouched.
#[tauri::command]
fn delete_product(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let conn = lock(&state)?;
    db::delete_product(&conn, &id).map_err(|e| e.to_string())
}

/// The count behind the delete prompt: local Presets that reference the
/// Product and will lose its Requirement ("It will also be removed from N
/// preset(s) that contain it"). Imported Presets are snapshots and never
/// count.
#[tauri::command]
fn product_presets_impact(
    state: State<'_, AppState>,
    id: String,
) -> Result<ProductPresetImpact, String> {
    let conn = lock(&state)?;
    Ok(ProductPresetImpact {
        preset_count: db::count_presets_using_product(&conn, &id).map_err(|e| e.to_string())?,
    })
}

/// Live winget registry search for the product dialog: real matches
/// (name · id · version · source) picked from the winget source. Timeboxed
/// on the backend, "Searching…" on the frontend — never a hang.
#[tauri::command]
fn search_winget(query: String) -> Result<Vec<WingetMatch>, String> {
    winget::search(query.trim())
}

/// One package's `winget show` details, enriching a match the dialog picked.
#[tauri::command]
fn show_winget(id: String) -> Result<WingetShow, String> {
    winget::show(id.trim())
}

/// Lists all Presets in the Library.
#[tauri::command]
fn list_presets(state: State<'_, AppState>) -> Result<Vec<PresetRecord>, String> {
    let conn = lock(&state)?;
    db::list_presets(&conn).map_err(|e| e.to_string())
}

/// Adds a Preset to the Library, validated first. Locally authored presets
/// are never marked imported.
#[tauri::command]
fn create_preset(state: State<'_, AppState>, preset: PresetRecord) -> Result<(), String> {
    preset.preset.validate()?;
    let mut preset = preset;
    preset.imported = false;
    let conn = lock(&state)?;
    db::create_preset(&conn, &preset).map_err(|e| match e {
        rusqlite::Error::SqliteFailure(err, _)
            if err.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            format!(
                "A preset named '{}' already exists — rename it or fork it instead",
                preset.preset.name
            )
        }
        other => other.to_string(),
    })
}

/// Replaces a Preset's payload in place (same id), validated first.
#[tauri::command]
fn update_preset(state: State<'_, AppState>, preset: PresetRecord) -> Result<(), String> {
    preset.preset.validate()?;
    let conn = lock(&state)?;
    db::update_preset(&conn, &preset).map_err(|e| e.to_string())
}

/// Removes a Preset from the Library.
#[tauri::command]
fn delete_preset(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let conn = lock(&state)?;
    db::delete_preset(&conn, &id).map_err(|e| e.to_string())
}

/// Writes a Library Preset to `path` as a single self-contained `.sprout.json`.
#[tauri::command]
fn export_preset(state: State<'_, AppState>, path: String, preset_id: String) -> Result<(), String> {
    let conn = lock(&state)?;
    import_export::export_preset(&conn, &path, &preset_id)
}

/// Reads `path`, validates it, and stores the Preset in the Library
/// immutably (fork is required to edit). Returns the stored record plus any
/// non-fatal warning, e.g. a wrong-platform file.
#[tauri::command]
fn import_preset(state: State<'_, AppState>, path: String) -> Result<ImportResult, String> {
    let conn = lock(&state)?;
    import_export::import_preset_file(&conn, &path)
}

/// Writes one backup (Settings → Backup) to `path`, limited to the selected
/// collections — unchecked ones are empty arrays in the same kind-tagged
/// JSON document (ADR-0014). Machine-scoped state (runs history, logs,
/// settings knobs, dock memory) never travels. Returns the per-collection
/// counts for the success notice.
#[tauri::command]
fn export_backup(
    state: State<'_, AppState>,
    path: String,
    selection: backup::BackupSelection,
) -> Result<BackupCounts, String> {
    let conn = lock(&state)?;
    backup::export_backup(&conn, &path, &selection)
}

/// Reads a whole-app backup file and reports what a restore would write —
/// the parsed counts behind the confirmation dialog. Nothing is written.
#[tauri::command]
fn inspect_backup(path: String) -> Result<BackupCounts, String> {
    backup::inspect_backup(&path)
}

/// Restores a whole-app backup: parse → validate → transactional merge that
/// skips identities which already exist (never overwrites). Returns
/// {inserted, skipped} per collection for the summary notice.
#[tauri::command]
fn import_backup(state: State<'_, AppState>, path: String) -> Result<ImportSummary, String> {
    let conn = lock(&state)?;
    backup::import_backup(&conn, &path)
}

/// Returns the `.sprout.json` path the app was launched with, once; `None`
/// when there is none or it was already consumed.
#[tauri::command]
fn take_pending_import(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let mut pending = state.pending_import.lock().map_err(|e| e.to_string())?;
    Ok(pending.take())
}

/// Computes the read-only Plan for the selected Presets (spec decision 5):
/// detection via the engine (winget list + uninstall registry — no
/// elevation, nothing written), expected per-Requirement actions, and
/// explicit conflicts for overlapping Products. Nothing runs from here.
/// Requirements whose live reference is dangling (ADR-0007) are flagged in
/// the Plan and never detected.
#[tauri::command]
fn compute_plan(state: State<'_, AppState>, preset_ids: Vec<String>) -> Result<Composition, String> {
    let conn = lock(&state)?;
    let mut presets = Vec::new();
    for id in &preset_ids {
        let record = db::get_preset(&conn, id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| {
                format!("Preset '{id}' is no longer in the library — refresh and try again")
            })?;
        presets.push(record.preset);
    }

    let requirements: Vec<&Requirement> = presets
        .iter()
        .flat_map(|preset| preset.requirements.iter())
        .filter(|req| !req.unresolved)
        .collect();
    let detections = state.engine.detect_many(&requirements);
    plan::compose(&presets, &detections)
}

/// The Plan half of quick install (ticket 21): what the Plan page shows when
/// "Install now" is chosen from a product's menu. The default Requirement is
/// synthesized from the Product (latest policy, its winget step, its default
/// env wiring) and composed as a single-entry Plan labeled
/// "Quick install — {product}" — the same grouped, auto-validated rendering
/// as any preset selection, and the label rides into History when the run
/// starts through the standard `start_run` path. Nothing runs from here. A
/// Product without a usable step is a clear error, never a silent success.
#[tauri::command]
fn quick_install_plan(state: State<'_, AppState>, product_id: String) -> Result<Composition, String> {
    let conn = lock(&state)?;
    let product = db::get_product(&conn, &product_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            format!("Product '{product_id}' is no longer in the library — refresh and try again")
        })?;
    drop(conn);
    let requirement = run::synthesize_quick_requirement(&product.product)?;
    let detections = state.engine.detect_many(&[&requirement]);
    let preset = Preset {
        schema_version: 1,
        platform: "windows".into(),
        name: format!("Quick install — {}", product.product.name),
        description: String::new(),
        author: String::new(),
        version: "1".into(),
        requirements: vec![requirement],
    };
    plan::compose(&[preset], &detections)
}

/// Starts the real run path (ADR-0003, ticket 06): the Plan is written to the
/// per-run working directory, this exe relaunches itself as `--worker` under
/// a single UAC prompt, and the worker executes the Plan — the main process
/// never elevates. Returns the run id, which the UI tails via
/// `read_run_progress`. The worker reuses the exact `run::execute_run_observed`
/// pipeline from ticket 05.
#[tauri::command]
fn start_run(
    preset_names: Vec<String>,
    requirements: Vec<Requirement>,
) -> Result<StartRun, String> {
    launch_run(preset_names, requirements)
}

/// Quick install (ticket 17): installing a single Library Product without
/// composing a Preset. The default Requirement is synthesized from the
/// Product (latest policy, its winget step, its default env wiring) and the
/// Run starts through the exact same elevated path as a preset run — History
/// labels it "Quick install — {product}" and it renders through the same
/// outcome tiers. A Product without a usable step is a clear error, never a
/// silent success. The frontend entry point lands in ticket 21.
#[tauri::command]
fn quick_install(state: State<'_, AppState>, product_id: String) -> Result<StartRun, String> {
    let conn = lock(&state)?;
    let product = db::get_product(&conn, &product_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            format!("Product '{product_id}' is no longer in the library — refresh and try again")
        })?;
    drop(conn);
    let requirement = run::synthesize_quick_requirement(&product.product)?;
    launch_run(
        vec![format!("Quick install — {}", product.product.name)],
        vec![requirement],
    )
}

/// The shared run-launch body behind `start_run` and `quick_install`: writes
/// the Plan to the per-run working directory and relaunches this exe as the
/// elevated worker (ADR-0003, ticket 06).
fn launch_run(preset_names: Vec<String>, requirements: Vec<Requirement>) -> Result<StartRun, String> {
    let run_id = run::new_run_id();
    let dir = worker::run_dir(&run_id);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("cannot create the run directory: {e}"))?;
    // The Plan never contains dangling references (ADR-0007) — the Plan
    // screen excludes them; this guard keeps a stale request from ever
    // executing a requirement whose product left the library.
    let requirements: Vec<Requirement> = requirements
        .into_iter()
        .filter(|req| !req.unresolved)
        .collect();
    let request = worker::RequestPayload {
        preset_names,
        requirements,
    };
    std::fs::write(dir.join("request.json"), serde_json::to_vec(&request).map_err(|e| e.to_string())?)
        .map_err(|e| format!("cannot write the run request: {e}"))?;

    let exe = std::env::current_exe().map_err(|e| format!("cannot locate Sprout.exe: {e}"))?;
    worker::launch_elevated(&exe, &["--worker", "--run", run_id.as_str()]).map_err(|e| {
        format!(
            "Sprout could not start the elevated worker: {e}. If you declined the UAC prompt, click Run again."
        )
    })?;
    Ok(StartRun { run_id })
}

/// One chunk of live progress for a running (or finished) Run: the events
/// appended since `offset`, the offset to resume from, and the completion
/// marker once the worker has written it.
#[derive(serde::Serialize)]
pub struct ProgressChunk {
    pub events: Vec<ProgressEvent>,
    pub offset: usize,
    pub done: Option<DoneInfo>,
}

/// The response to a started Run: the id the UI polls with.
#[derive(serde::Serialize)]
pub struct StartRun {
    pub run_id: String,
}

/// Tails the worker's JSON-lines status file: returns every complete event
/// appended since `offset` (a partial trailing line is left for the next
/// read) plus the worker's completion marker, when there is one.
#[tauri::command]
fn read_run_progress(run_id: String, offset: usize) -> Result<ProgressChunk, String> {
    let dir = worker::run_dir(&run_id);
    let (events, offset) = worker::read_status_events(&dir, offset);
    Ok(ProgressChunk {
        events,
        offset,
        done: worker::read_done(&dir),
    })
}

/// Requests a stop of the running Plan: touches the worker's cancel marker.
/// The worker finishes the in-flight Requirement (its timebox still guards a
/// hung installer), then stops and records the Run as cancelled.
#[tauri::command]
fn cancel_run(run_id: String) -> Result<(), String> {
    let path = worker::run_dir(&run_id).join("cancel");
    std::fs::write(&path, b"").map_err(|e| format!("cannot request the cancel: {e}"))
}

/// The run-active query (ticket 18): whether a run is in progress right now —
/// and which one — from anywhere. Backed entirely by the per-run folders on
/// disk (the worker's status/done markers), so the answer survives navigation
/// and even an app restart while the worker kept installing. When a run just
/// finished, its outcome rides along once, so the UI can announce it; a run
/// whose worker died goes stale and stops being "active".
#[tauri::command]
fn get_active_run() -> Result<Option<ActiveRunInfo>, String> {
    Ok(worker::active_run(&crate::db::logs_dir().join("runs")))
}

/// Loads one persisted Run with its per-Requirement results — how the summary
/// screen reads back what the worker persisted.
#[tauri::command]
fn get_run(state: State<'_, AppState>, run_id: String) -> Result<Option<RunRecord>, String> {
    let conn = lock(&state)?;
    db::get_run(&conn, &run_id).map_err(|e| e.to_string())
}

/// Lists every Run's summary row, newest first — the History screen (ticket
/// 09). Per-Requirement results load on demand via `get_run`.
#[tauri::command]
fn list_runs(state: State<'_, AppState>) -> Result<Vec<RunSummary>, String> {
    let conn = lock(&state)?;
    db::list_runs(&conn).map_err(|e| e.to_string())
}

/// Loads the persisted knobs (default timeout, log retention) with their
/// built-in defaults when they were never written.
#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    let conn = lock(&state)?;
    Ok(settings::load(&conn))
}

/// Persists the Settings screen's knobs, validated first. Ticket 57: dock
/// changes apply to a live Quick Launch window right away (state change →
/// dock/undock, edge change → reposition, mode change → re-apply auto-hide),
/// and the window is told via `quick-launch-changed` so its chrome re-reads
/// the truth. A live dock failure is logged, never a save failure — the
/// settings are persisted regardless.
#[tauri::command]
fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<(), String> {
    let conn = lock(&state)?;
    settings::save(&conn, &settings)?;
    drop(conn);
    if let Err(e) = quick_window::apply_settings(&app, &settings) {
        eprintln!("Could not apply dock settings to the live window: {e}");
    }
    emit_quick_launch_changed(&app);
    Ok(())
}

/// Persists the theme on its own — the Settings screen applies it the moment
/// it is selected, before the rest of the form is saved (ticket 31). The
/// Quick Launch window is told via `quick-launch-changed` (ticket 57) so it
/// re-applies the theme without reopening.
#[tauri::command]
fn update_theme(
    app: AppHandle,
    state: State<'_, AppState>,
    theme: String,
) -> Result<(), String> {
    let conn = lock(&state)?;
    settings::save_theme(&conn, &theme)?;
    drop(conn);
    emit_quick_launch_changed(&app);
    Ok(())
}

/// The auto-start toggle (ADR-0013, ticket 75): persists only the
/// `autostart` preference, then reconciles the HKCU Run registration right
/// beside the save — turning it on or off takes effect immediately, without
/// a restart. Debug builds skip the registry write inside the sync (logged),
/// so dev sessions never touch the boot path.
#[tauri::command]
fn update_autostart(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    let conn = lock(&state)?;
    settings::save_autostart(&conn, if enabled { "on" } else { "off" })?;
    drop(conn);
    autostart::sync_registration(&app, enabled)
}

/// Ticket 116: the frontend's dirty flag for the Settings guard. While dirty,
/// the main window's close request is held and surfaced as
/// `settings-dirty-close-requested` instead of destroying the window.
#[tauri::command]
fn set_settings_dirty(state: State<'_, AppState>, dirty: bool) -> Result<(), String> {
    let mut flag = state.settings_dirty.lock().map_err(|e| e.to_string())?;
    *flag = dirty;
    Ok(())
}

/// Ticket 116: destroy the main window after a dirty-guard Save/Discard
/// confirmation — bypasses the close-requested guard (the caller already
/// resolved dirtiness).
#[tauri::command]
fn destroy_main_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        if let Some(state) = app.try_state::<AppState>() {
            if let Ok(mut t) = state.main_close_time.lock() {
                *t = Some(std::time::Instant::now());
            }
        }
        window.destroy().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// The Logs screen's picture of where logs live and how big they are — no
/// content, ever.
#[tauri::command]
fn list_logs() -> Result<LogLocations, String> {
    Ok(logs::list_log_locations())
}

/// The answer to a self-update check (ADR-0012, ticket 73): the running
/// build's version plus the newer release when one exists, or `None` —
/// which is also what offline, private-repo 403/404, and malformed payloads
/// all look like. The silent-failure contract means this never errors.
#[derive(serde::Serialize)]
pub struct UpdateCheck {
    pub current_version: String,
    pub update: Option<update::AvailableUpdate>,
}

/// Checks GitHub Releases for a newer Sprout (ADR-0012). Runs on the
/// blocking pool so the network round-trip never touches the main thread;
/// every failure resolves to "up to date" rather than an error surface.
#[tauri::command]
async fn check_for_update() -> Result<UpdateCheck, String> {
    let update = tauri::async_runtime::spawn_blocking(update::check_for_update_silent)
        .await
        .unwrap_or(None);
    Ok(UpdateCheck {
        current_version: update::current_version().to_string(),
        update,
    })
}

/// The user-confirmed apply step (ADR-0012): downloads the setup exe to
/// %TEMP%, spawns it detached with `/UPDATE /P /R`, and exits shortly after
/// so NSIS can replace the running exe and relaunch (`/R`). Runs on the
/// blocking pool; failures are reported — this action was explicit.
#[tauri::command]
async fn install_update(app: AppHandle, url: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || update::apply_update(&app, &url))
        .await
        .map_err(|e| format!("the update could not be applied: {e}"))?
}

/// The Logs screen's open-folder action: reveals `path` in Explorer.
#[tauri::command]
fn open_folder(path: String) -> Result<(), String> {
    logs::open_folder(&path)
}

/// Lists every Launch entry in the Quick Launch list, in order (ticket 38).
#[tauri::command]
fn list_launch_entries(state: State<'_, AppState>) -> Result<Vec<launch::LaunchEntry>, String> {
    let conn = lock(&state)?;
    launch::list_launch_entries(&conn).map_err(|e| e.to_string())
}

/// Appends a Launch entry at the end of the Quick Launch list (ticket 38).
/// The Quick Launch window is told via `quick-launch-changed` (ticket 57) so
/// a new entry appears without reopening it.
#[tauri::command]
fn create_launch_entry(
    app: AppHandle,
    state: State<'_, AppState>,
    entry: launch::LaunchEntryInput,
) -> Result<launch::LaunchEntry, String> {
    launch::validate_launch_entry(&entry)?;
    let conn = lock(&state)?;
    if let Some(existing) = launch::colliding_entry(&conn, &entry, None).map_err(|e| e.to_string())? {
        return Err(format!("\"{existing}\" is already in Quick Launch with this target."));
    }
    let created = launch::create_launch_entry(&conn, &entry).map_err(|e| e.to_string())?;
    drop(conn);
    emit_quick_launch_changed(&app);
    Ok(created)
}

/// Replaces a Launch entry's metadata in place; position is untouched
/// (ticket 38).
#[tauri::command]
fn update_launch_entry(
    app: AppHandle,
    state: State<'_, AppState>,
    entry: launch::LaunchEntry,
) -> Result<(), String> {
    launch::validate_launch_entry(&entry.entry)?;
    let conn = lock(&state)?;
    if let Some(existing) =
        launch::colliding_entry(&conn, &entry.entry, Some(entry.id)).map_err(|e| e.to_string())?
    {
        return Err(format!("\"{existing}\" is already in Quick Launch with this target."));
    }
    launch::update_launch_entry(&conn, &entry).map_err(|e| e.to_string())?;
    drop(conn);
    emit_quick_launch_changed(&app);
    Ok(())
}

/// Removes a Launch entry and compacts the list (ticket 38).
#[tauri::command]
fn delete_launch_entry(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
) -> Result<(), String> {
    let conn = lock(&state)?;
    launch::delete_launch_entry(&conn, id).map_err(|e| e.to_string())?;
    drop(conn);
    emit_quick_launch_changed(&app);
    Ok(())
}

/// Moves a Launch entry to another position in the list (ticket 38).
#[tauri::command]
fn move_launch_entry(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    to_position: i64,
) -> Result<(), String> {
    let conn = lock(&state)?;
    launch::move_launch_entry(&conn, id, to_position).map_err(|e| e.to_string())?;
    drop(conn);
    emit_quick_launch_changed(&app);
    Ok(())
}

/// One Test click in the add-command dialog (ticket 41): runs the command
/// entry under its shell, timeboxed, and reports exit code + captured
/// output. A command that outlives the box comes back timed out — honestly
/// not headless-verifiable, never passed.
#[tauri::command]
fn test_launch_command(
    shell: launch::LaunchShell,
    target: String,
) -> Result<launch::TestResult, String> {
    if target.trim().is_empty() {
        return Err("The command is empty — nothing to test.".into());
    }
    Ok(launch::test_launch_command(shell, &target))
}

/// Starts the whole Quick Launch list through the capped, queued pipeline
/// (ticket 42) — the launch trigger for both the Quick Launch window's Start
/// button and the Quick Launch page's Start button (ticket 54). The cap is
/// read from Settings at click time; the orchestrator runs on a background
/// thread so the UI never blocks; a second click while one run is in flight
/// is rejected — never stacked. When the run finishes, the summary lands as
/// a system notification and a `launch-run-done` event the page listens for.
#[tauri::command]
fn start_quick_launch(state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    let conn = lock(&state)?;
    let entries = launch::list_launch_entries(&conn).map_err(|e| e.to_string())?;
    drop(conn);
    if entries.is_empty() {
        return Err("Quick Launch list is empty — add entries first.".into());
    }
    launch_entries(&app, &state, entries)
}

/// Starts one Launch entry through the same capped, queued pipeline as Start
/// all (ticket 93) — the Quick Launch window's clickable entry rows. Ticket
/// 121: a single tap on a running `App` on its target desktop foregrounds its
/// window (restores if minimized) instead of spawning a second instance and
/// reports `skipped: foregrounded`; batch `Start all` keeps the skip without
/// foreground. The single-flight guard applies equally: a row click while a
/// run is in flight is rejected, never stacked, and the summary notification
/// and `launch-run-done` event report the single-entry outcome like any run.
#[tauri::command]
fn start_launch_entry(state: State<'_, AppState>, app: AppHandle, id: i64) -> Result<(), String> {
    let conn = lock(&state)?;
    let entry = launch::list_launch_entries(&conn)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| "That entry is gone from the Quick Launch list — try again.".to_string())?;
    drop(conn);
    launch_entries_single(&app, &state, vec![entry])
}

/// The shared launch-run body behind the Quick Launch window's and the
/// page's Start buttons (tickets 42 & 54): the single-flight guard, the
/// background thread running the capped, queued pipeline, the per-run log
/// folder (ticket 77), the `launch-run-done` event the page listens for,
/// and the summary notification. A second trigger while a run is in flight
/// is rejected — never stacked.
fn launch_entries(
    app: &AppHandle,
    state: &AppState,
    entries: Vec<launch::LaunchEntry>,
) -> Result<(), String> {
    launch_entries_inner(app, state, entries, false)
}

/// Ticket 121 single-tap variant: the same single-flight, log, event and
/// notification path as [`launch_entries`] but the queue foregrounds a running
/// `App` on its target desktop instead of spawning a second instance.
fn launch_entries_single(
    app: &AppHandle,
    state: &AppState,
    entries: Vec<launch::LaunchEntry>,
) -> Result<(), String> {
    launch_entries_inner(app, state, entries, true)
}

fn launch_entries_inner(
    app: &AppHandle,
    state: &AppState,
    entries: Vec<launch::LaunchEntry>,
    is_single: bool,
) -> Result<(), String> {
    if state
        .launch_in_progress
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("A Quick Launch run is already in progress — wait for it to finish.".into());
    }
    let cap = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        settings::load(&conn).launch_concurrency as usize
    };
    let engine = Arc::clone(&state.launcher);
    let running = Arc::clone(&state.launch_in_progress);
    let app = app.clone();
    std::thread::spawn(move || {
        // The run's own log (ticket 77): folder + header before the queue so
        // even a wedged run leaves its start behind, the report's story and
        // verdict after. Best-effort on both ends — a logging failure never
        // fails the run, its event, or its notification.
        let log_path = launch::new_launch_run_log_path(&crate::db::logs_dir());
        if let Some(path) = &log_path {
            launch::write_launch_run_header(path, entries.len(), cap);
        }
        // Stored assignments are always honored (ADR-0015): there is no
        // master switch anymore, and below the 24H2 gate the engine's empty
        // desktop list makes every entry behave as unassigned. Ticket 121:
        // single-tap foregrounds on hit, batch skips without foreground.
        let report = if is_single {
            launch::run_single_launch_queue(engine.as_ref(), &entries, cap)
        } else {
            launch::run_launch_queue(engine.as_ref(), &entries, cap)
        };
        if let Some(path) = &log_path {
            launch::write_launch_run_summary(path, &report);
        }
        emit_valid(&app, "launch-run-done", &report);
        let _ = notify_launch_summary(&app, &report);
        running.store(false, Ordering::SeqCst);
    });
    Ok(())
}

/// The end-of-run system notification (ticket 42): started / skipped /
/// failed counts, with the names of the failed. Emitted from the background
/// thread; a failure to notify is never a failure of the run.
fn notify_launch_summary(app: &AppHandle, report: &launch::LaunchReport) -> Result<(), String> {
    use tauri_plugin_notification::NotificationExt;
    app.notification()
        .builder()
        .title("Quick Launch done")
        .body(launch::launch_summary_body(report))
        .show()
        .map_err(|e| e.to_string())
}

/// Pure helper for ticket 123 white-screen: whether the window handle should be
/// treated as a zombie that will paint white. `is_valid_hwnd` is the `IsWindow`
/// result; `elapsed_since_close` is `main_close_time.elapsed()` when a close
/// happened. A recently-closed window ( <800 ms ) is a zombie even when HWND
/// still reports valid — WebView2 is destroyed but the manager still returns the
/// handle, and focusing it yields a full-white, unclosable window. The 800 ms
/// grace covers the async `destroy()` + manager cleanup + WebView2 COM teardown
/// on slower machines; 500 ms was insufficient for the close→reopen race
/// (click while shown → close → click again). Pure — the regression test seam.
pub(crate) fn is_zombie_window(
    is_valid_hwnd: bool,
    elapsed_since_close: Option<std::time::Duration>,
) -> bool {
    if !is_valid_hwnd {
        return true;
    }
    if let Some(elapsed) = elapsed_since_close {
        if elapsed < std::time::Duration::from_millis(800) {
            return true;
        }
    }
    false
}

#[derive(Debug, PartialEq, Eq)]
enum ExistingMainWindowAction {
    Rebuild,
    WaitForFrontendReady,
    Reveal,
}

fn existing_main_window_action(
    is_valid_hwnd: bool,
    elapsed_since_close: Option<std::time::Duration>,
    is_loading: bool,
) -> ExistingMainWindowAction {
    if is_zombie_window(is_valid_hwnd, elapsed_since_close) {
        ExistingMainWindowAction::Rebuild
    } else if is_loading {
        ExistingMainWindowAction::WaitForFrontendReady
    } else {
        ExistingMainWindowAction::Reveal
    }
}

#[cfg(test)]
mod white_screen_tests {
    use super::{existing_main_window_action, is_zombie_window, ExistingMainWindowAction};
    use std::time::Duration;

    #[test]
    fn valid_hwnd_but_recent_close_is_zombie() {
        // The exact repro: valid HWND but close 100ms ago -> must be zombie (white)
        assert!(is_zombie_window(true, Some(Duration::from_millis(100))));
        assert!(is_zombie_window(true, Some(Duration::from_millis(0))));
        assert!(is_zombie_window(true, Some(Duration::from_millis(799))));
    }

    #[test]
    fn valid_hwnd_with_old_close_is_not_zombie() {
        // Outside grace: focusing existing window is safe (idempotent focus)
        assert!(!is_zombie_window(true, Some(Duration::from_millis(800))));
        assert!(!is_zombie_window(true, Some(Duration::from_millis(2000))));
        assert!(!is_zombie_window(true, None));
    }

    #[test]
    fn invalid_hwnd_is_always_zombie() {
        assert!(is_zombie_window(false, None));
        assert!(is_zombie_window(false, Some(Duration::from_millis(100))));
        assert!(is_zombie_window(false, Some(Duration::from_millis(2000))));
    }

    #[test]
    fn close_reopen_race_needs_grace() {
        // Minimised repro sequence: shown -> click (focus) -> close -> click (reopen) within 800ms
        // The second click sees valid HWND but elapsed 100ms -> zombie
        let elapsed_after_close = Some(Duration::from_millis(100));
        let is_valid = true; // IsWindow still true because destroy() async
        // Legacy logic (only !is_valid) would be false -> bug (returns white)
        let legacy = !is_valid;
        assert!(!legacy, "legacy would think not zombie");
        // Fixed logic must be true -> rebuild
        assert!(is_zombie_window(is_valid, elapsed_after_close));
    }

    #[test]
    fn valid_loading_window_waits_for_frontend_ready() {
        assert_eq!(
            existing_main_window_action(true, None, true),
            ExistingMainWindowAction::WaitForFrontendReady
        );
    }

    #[test]
    fn valid_ready_window_is_revealed() {
        assert_eq!(
            existing_main_window_action(true, None, false),
            ExistingMainWindowAction::Reveal
        );
    }

    #[test]
    fn zombie_window_is_rebuilt_even_while_loading() {
        assert_eq!(
            existing_main_window_action(true, Some(Duration::from_millis(100)), true),
            ExistingMainWindowAction::Rebuild
        );
    }
}

fn set_main_window_alpha(window: &tauri::WebviewWindow, alpha: u8) -> tauri::Result<()> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetLayeredWindowAttributes, SetWindowLongPtrW, GWL_EXSTYLE, LWA_ALPHA,
        WS_EX_LAYERED,
    };

    let hwnd = window.hwnd()?;
    unsafe {
        let style = GetWindowLongPtrW(hwnd.0, GWL_EXSTYLE);
        SetWindowLongPtrW(
            hwnd.0,
            GWL_EXSTYLE,
            style | WS_EX_LAYERED as isize,
        );
        if SetLayeredWindowAttributes(hwnd.0, 0, alpha, LWA_ALPHA) == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        if alpha == u8::MAX {
            SetWindowLongPtrW(
                hwnd.0,
                GWL_EXSTYLE,
                (style | WS_EX_LAYERED as isize) & !(WS_EX_LAYERED as isize),
            );
        }
    }
    Ok(())
}

/// Opens the main window: focuses the existing one, or recreates it when it
/// was destroyed by closing it (ticket 43). Runs on its caller's thread and
/// sleeps through the close grace — the boot path calls it directly, while
/// every post-start caller goes through `request_open_main_window`. The
/// recreated window keeps the configured size and minimums from
/// `constants::window` — the single size source since the conf file stopped
/// declaring windows (ticket 76, ADR-0013).
pub(crate) fn open_main_window(app: &AppHandle) -> tauri::Result<tauri::WebviewWindow> {
    use std::time::Duration;
    use tauri::Manager;
    // Ticket 123 white-screen (close → reopen via dock mark showed blank):
    // `window.destroy()` on close is async — the label "main" lingers in the
    // manager until the event loop processes it. A immediate rebuild then sees
    // "already exists" and the old (destroyed) handle still answers
    // `get_webview_window`, leading to focusing a zombie white webview. Retry
    // with short sleeps so the manager can settle. A live, ready window is
    // shown/unminimized/focused; a transparent loading one waits for the
    // frontend's mounted signal.
    // Also respect `main_close_time` — if the user just closed (≤800 ms ago),
    // the manager is guaranteed to still hold the zombie, so wait first.
    // Lean destroy-on-close (least memory) requires this, not hidden-window.
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(t) = state.main_close_time.lock() {
            if let Some(at) = *t {
                let elapsed = at.elapsed();
                if elapsed < Duration::from_millis(800) {
                    std::thread::sleep(Duration::from_millis(800) - elapsed);
                }
            }
        }
    }
    for _ in 0..7 {
        if let Some(window) = app.get_webview_window("main") {
            // Zombie check: `destroy()` on close is async — the manager may still
            // return a handle whose HWND is already invalid (white/blank). `IsWindow`
            // fails for a destroyed HWND, so we drop it and retry the build instead
            // of focusing a dead webview (close→reopen white screen).
            let is_valid = match window.hwnd() {
                Ok(hwnd) => unsafe { windows_sys::Win32::UI::WindowsAndMessaging::IsWindow(hwnd.0) != 0 },
                Err(_) => false,
            };
            let elapsed_opt = {
                if let Some(state) = app.try_state::<AppState>() {
                    if let Ok(g) = state.main_close_time.lock() {
                        g.map(|at| at.elapsed())
                    } else {
                        None
                    }
                } else {
                    None
                }
            };
            // Ticket 123 fix: a recently-closed window ( <800ms ) is a zombie even when
            // HWND still reports valid — focusing it paints white. This is the repro:
            // shown → click (focus) → close (destroy async) → click (reopen) within 800ms
            // still sees the old handle with valid HWND. Legacy checked only !is_valid.
            let is_loading = app
                .try_state::<AppState>()
                .map(|state| state.main_window_loading.load(Ordering::SeqCst))
                .unwrap_or(false);
            match existing_main_window_action(is_valid, elapsed_opt, is_loading) {
                ExistingMainWindowAction::Rebuild => {
                    if let Some(state) = app.try_state::<AppState>() {
                        state.main_window_loading.store(false, Ordering::SeqCst);
                    }
                    let _ = window.destroy();
                    std::thread::sleep(Duration::from_millis(120));
                    continue;
                }
                ExistingMainWindowAction::WaitForFrontendReady => return Ok(window),
                ExistingMainWindowAction::Reveal => {}
            }
            // A ready live window takes the idempotent focus path. Loading is
            // deliberately left transparent until the frontend acknowledges mount.
            let ok = (|| -> tauri::Result<()> {
                window.show()?;
                window.unminimize()?;
                window.set_focus()?;
                Ok(())
            })()
            .is_ok();
            if ok {
                if let Some(state) = app.try_state::<AppState>() {
                    if let Ok(mut t) = state.main_close_time.lock() {
                        *t = None;
                    }
                }
                return Ok(window);
            }
            let _ = window.destroy();
            std::thread::sleep(Duration::from_millis(120));
            continue;
        }
        if let Some(state) = app.try_state::<AppState>() {
            state.main_window_loading.store(true, Ordering::SeqCst);
        }
        let build = tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::default())
            .title("Sprout")
            .visible(true)
            .transparent(true)
            .background_color(tauri::window::Color(0, 0, 0, 0))
            .skip_taskbar(true)
            .inner_size(
                constants::window::MAIN_WINDOW_WIDTH,
                constants::window::MAIN_WINDOW_HEIGHT,
            )
            .min_inner_size(
                constants::window::MAIN_WINDOW_MIN_WIDTH,
                constants::window::MAIN_WINDOW_MIN_HEIGHT,
            )
            .build();
        match build {
            Ok(window) => {
                // Ticket 111: WM_DISPLAYCHANGE for the main window so Settings' per-
                // monitor list re-renders when screens move while the Quick Launch
                // window is closed (tray-only). Reuses the Quick Launch subclass
                // proc — both windows share the same cache invalidation.
                if let Ok(hwnd) = window.hwnd() {
                    quick_window::install_display_change_hook(app, hwnd.0);
                }
                let prepare = window.center();
                if let Err(e) = prepare {
                    if let Some(state) = app.try_state::<AppState>() {
                        state.main_window_loading.store(false, Ordering::SeqCst);
                    }
                    let _ = window.destroy();
                    return Err(e);
                }
                if let Some(state) = app.try_state::<AppState>() {
                    if let Ok(mut t) = state.main_close_time.lock() {
                        *t = None;
                    }
                }
                return Ok(window);
            }
            Err(e) if e.to_string().to_lowercase().contains("already exists") => {
                std::thread::sleep(Duration::from_millis(120));
                continue;
            }
            Err(e) => {
                if let Some(state) = app.try_state::<AppState>() {
                    state.main_window_loading.store(false, Ordering::SeqCst);
                }
                return Err(e);
            },
        }
    }
    Err(tauri::Error::AssetNotFound(
        "main window failed to open after retries".into(),
    ))
}

/// The single post-start entry point for opening the main window: the dock
/// mark command, the tray menu, and the single-instance hook all enqueue here
/// instead of calling `open_main_window` directly.
///
/// WHY the indirection exists: `open_main_window` sleeps on its caller while
/// the queued `destroy()` it waits for needs the event thread to run — every
/// one of those callers runs on that thread, so a direct call self-deadlocks
/// (invisible frame, dead X). The sleeps run on the blocking pool while the
/// event thread stays free to settle the destroy. Open is idempotent, so a
/// request arriving mid-rebuild is already satisfied by it and coalesces; the
/// flag clears when the worker finishes, so a failed attempt stays retryable.
/// Boot keeps the direct sync call — no close race exists there (ADR-0013).
pub(crate) fn request_open_main_window(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        if state.main_window_opening.swap(true, Ordering::SeqCst) {
            return;
        }
    }
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _ = crate::open_main_window(&handle);
        if let Some(state) = handle.try_state::<AppState>() {
            state.main_window_opening.store(false, Ordering::SeqCst);
        }
    });
}

/// The fresh installed-app snapshot behind the Quick Launch search
/// (ticket 39): Start Menu shortcuts + uninstall-registry entries, re-walked
/// on every call — no cache. Runs on the blocking pool so the walk (hundreds
/// of IShellLink resolutions) never touches the UI thread; the frontend
/// filters the returned list locally as the user types.
#[tauri::command]
async fn list_launch_candidates() -> Result<Vec<walker::Candidate>, String> {
    tauri::async_runtime::spawn_blocking(walker::snapshot)
        .await
        .map_err(|e| format!("installed-app search failed: {e}"))
}

/// The icon for one search candidate, as a PNG data URL (ticket 40). Fetched
/// lazily per visible row and held in memory — never cached to disk. `None`
/// when the target no longer exists or has no icon.
#[tauri::command]
async fn candidate_icon(target: String) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || icons::candidate_icon(&target))
        .await
        .map_err(|e| format!("icon extraction failed: {e}"))
}

/// The virtual-desktop assignment surface (ticket 44): every desktop with
/// its label, in Task View order, plus the gate. `supported` is false below
/// Windows 11 24H2 (and on any winvd failure), which hides the whole
/// assignment surface — the page's grouping, labels, and assignments. Ids
/// are GUIDs and stay stable across Task View reorder; labels are the
/// Windows name when a desktop has one, "Desktop N" otherwise.
#[tauri::command]
fn list_virtual_desktops(state: State<'_, AppState>) -> Result<VirtualDesktops, String> {
    let desktops = state.launcher.desktops();
    Ok(VirtualDesktops {
        supported: !desktops.is_empty(),
        desktops,
    })
}

/// The gate + list answer of `list_virtual_desktops`. Windows always has at
/// least one desktop, so an empty list means the surface is unavailable —
/// below 24H2 or winvd failed — and the frontend hides everything.
#[derive(serde::Serialize)]
pub struct VirtualDesktops {
    pub supported: bool,
    pub desktops: Vec<DesktopInfo>,
}

/// Creates a virtual desktop on the user's behalf (ticket 44) and returns
/// its id. `None` below the 24H2 gate or when the OS refused.
#[tauri::command]
fn create_virtual_desktop(state: State<'_, AppState>) -> Result<Option<String>, String> {
    Ok(state.launcher.create_desktop())
}

/// Lists every Quick Action in list order (ticket 50).
#[tauri::command]
fn list_quick_actions(
    state: State<'_, AppState>,
) -> Result<Vec<quick_actions::QuickAction>, String> {
    let conn = lock(&state)?;
    quick_actions::list_quick_actions(&conn).map_err(|e| e.to_string())
}

/// Appends a Quick Action at the end of the list, validated first — a blank
/// name or command, or a relative working directory, never reaches the list
/// (ticket 50). The Quick Launch window is told via `quick-launch-changed`
/// (ticket 57) so a new action appears without reopening it.
#[tauri::command]
fn create_quick_action(
    app: AppHandle,
    state: State<'_, AppState>,
    action: quick_actions::QuickActionInput,
) -> Result<quick_actions::QuickAction, String> {
    quick_actions::validate_quick_action(&action)?;
    let conn = lock(&state)?;
    if let Some(existing) =
        quick_actions::colliding_action(&conn, &action, None).map_err(|e| e.to_string())?
    {
        return Err(format!("\"{existing}\" already runs this exact command."));
    }
    let created = quick_actions::create_quick_action(&conn, &action).map_err(|e| e.to_string())?;
    drop(conn);
    emit_quick_launch_changed(&app);
    Ok(created)
}

/// Replaces a Quick Action's command and metadata in place, validated first;
/// position is untouched — reorders go through `move_quick_action` (ticket
/// 50).
#[tauri::command]
fn update_quick_action(
    app: AppHandle,
    state: State<'_, AppState>,
    action: quick_actions::QuickAction,
) -> Result<(), String> {
    quick_actions::validate_quick_action(&action.action)?;
    let conn = lock(&state)?;
    if let Some(existing) =
        quick_actions::colliding_action(&conn, &action.action, Some(action.id))
            .map_err(|e| e.to_string())?
    {
        return Err(format!("\"{existing}\" already runs this exact command."));
    }
    quick_actions::update_quick_action(&conn, &action).map_err(|e| e.to_string())?;
    drop(conn);
    emit_quick_launch_changed(&app);
    Ok(())
}

/// Removes a Quick Action and compacts the list (ticket 50).
#[tauri::command]
fn delete_quick_action(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
) -> Result<(), String> {
    let conn = lock(&state)?;
    quick_actions::delete_quick_action(&conn, id).map_err(|e| e.to_string())?;
    drop(conn);
    emit_quick_launch_changed(&app);
    Ok(())
}

/// Moves a Quick Action to another position in the list, clamped (ticket 50).
#[tauri::command]
fn move_quick_action(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    to_position: i64,
) -> Result<(), String> {
    let conn = lock(&state)?;
    quick_actions::move_quick_action(&conn, id, to_position).map_err(|e| e.to_string())?;
    drop(conn);
    emit_quick_launch_changed(&app);
    Ok(())
}

// ------------------- Quick Clips (ticket 78) ------------------------------

/// Lists every Clip in list order (ticket 78).
#[tauri::command]
fn list_clips(state: State<'_, AppState>) -> Result<Vec<clips::Clip>, String> {
    let conn = lock(&state)?;
    clips::list_clips(&conn).map_err(|e| e.to_string())
}

/// Appends a Clip at the end of the list, validated first — blank text never
/// reaches the list (ticket 78). The Quick Launch window is told via
/// `quick-launch-changed` so its conditional Quick Clips tab appears (or
/// updates) without reopening it.
#[tauri::command]
fn create_clip(
    app: AppHandle,
    state: State<'_, AppState>,
    clip: clips::ClipInput,
) -> Result<clips::Clip, String> {
    clips::validate_clip(&clip)?;
    let conn = lock(&state)?;
    if let Some(existing) = clips::colliding_clip(&conn, &clip.content, None).map_err(|e| e.to_string())? {
        return Err(if existing.is_empty() {
            "A clip with this text already exists.".into()
        } else {
            format!("This text is already saved as \"{existing}\".")
        });
    }
    let created = clips::create_clip(&conn, &clip).map_err(|e| e.to_string())?;
    drop(conn);
    emit_quick_launch_changed(&app);
    Ok(created)
}

/// Replaces a Clip's name and text in place, validated first; position is
/// untouched — reorders go through `move_clip` (ticket 78).
#[tauri::command]
fn update_clip(
    app: AppHandle,
    state: State<'_, AppState>,
    clip: clips::Clip,
) -> Result<(), String> {
    clips::validate_clip(&clip.clip)?;
    let conn = lock(&state)?;
    if let Some(existing) =
        clips::colliding_clip(&conn, &clip.clip.content, Some(clip.id)).map_err(|e| e.to_string())?
    {
        return Err(if existing.is_empty() {
            "A clip with this text already exists.".into()
        } else {
            format!("This text is already saved as \"{existing}\".")
        });
    }
    clips::update_clip(&conn, &clip).map_err(|e| e.to_string())?;
    drop(conn);
    emit_quick_launch_changed(&app);
    Ok(())
}

/// Removes a Clip and compacts the list (ticket 78). Deleting the last clip
/// removes the window's third tab again via `quick-launch-changed`.
#[tauri::command]
fn delete_clip(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
) -> Result<(), String> {
    let conn = lock(&state)?;
    clips::delete_clip(&conn, id).map_err(|e| e.to_string())?;
    drop(conn);
    emit_quick_launch_changed(&app);
    Ok(())
}

/// Moves a Clip to another position in the list, clamped (ticket 78).
#[tauri::command]
fn move_clip(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    to_position: i64,
) -> Result<(), String> {
    let conn = lock(&state)?;
    clips::move_clip(&conn, id, to_position).map_err(|e| e.to_string())?;
    drop(conn);
    emit_quick_launch_changed(&app);
    Ok(())
}

/// Puts one stored Clip's content back on the clipboard (ticket 78), through
/// the clipboard-manager plugin on the Rust side. Returns success only after
/// the write landed, so surfaces can flash their "Copied" feedback honestly.
#[tauri::command]
fn copy_clip(app: AppHandle, state: State<'_, AppState>, id: i64) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    let conn = lock(&state)?;
    let clip = clips::get_clip(&conn, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "This clip no longer exists — refresh and try again".to_string())?;
    drop(conn);
    app.clipboard()
        .write_text(clip.clip.content)
        .map_err(|e| format!("Could not reach the clipboard: {e}"))
}

// ------------------- Groups (ticket 89) ------------------------------------

/// Lists one collection's Groups in user order (ticket 89).
#[tauri::command]
fn list_groups(
    state: State<'_, AppState>,
    collection: groups::Collection,
) -> Result<Vec<groups::Group>, String> {
    let conn = lock(&state)?;
    groups::list_groups(&conn, collection).map_err(|e| e.to_string())
}

/// Appends a Group at the end of its collection's order, name validated
/// first (ticket 89). The Quick Launch window is told via
/// `quick-launch-changed` so its lists re-render without reopening.
#[tauri::command]
fn create_group(
    app: AppHandle,
    state: State<'_, AppState>,
    collection: groups::Collection,
    name: String,
) -> Result<groups::Group, String> {
    let conn = lock(&state)?;
    let created = groups::create_group(&conn, collection, &name)?;
    drop(conn);
    emit_quick_launch_changed(&app);
    Ok(created)
}

/// Renames a Group in place; order and membership are untouched (ticket 89).
#[tauri::command]
fn rename_group(app: AppHandle, state: State<'_, AppState>, id: i64, name: String) -> Result<(), String> {
    let conn = lock(&state)?;
    groups::rename_group(&conn, id, &name)?;
    drop(conn);
    emit_quick_launch_changed(&app);
    Ok(())
}

/// Removes a Group: members return to ungrouped, never deleted (ticket 89).
#[tauri::command]
fn delete_group(app: AppHandle, state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = lock(&state)?;
    groups::delete_group(&conn, id).map_err(|e| e.to_string())?;
    drop(conn);
    emit_quick_launch_changed(&app);
    Ok(())
}

/// Moves a Group to another position within its collection's order, clamped
/// (ticket 89).
#[tauri::command]
fn move_group(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    to_position: i64,
) -> Result<(), String> {
    let conn = lock(&state)?;
    groups::move_group(&conn, id, to_position).map_err(|e| e.to_string())?;
    drop(conn);
    emit_quick_launch_changed(&app);
    Ok(())
}

/// Puts one item into one Group of its own collection — the item's single
/// group reference; any previous membership is replaced (ticket 89). An item
/// offered to another collection's group is refused at the data layer.
#[tauri::command]
fn assign_to_group(
    app: AppHandle,
    state: State<'_, AppState>,
    collection: groups::Collection,
    item_id: i64,
    group_id: i64,
) -> Result<(), String> {
    let conn = lock(&state)?;
    groups::assign_item(&conn, collection, item_id, group_id).map_err(|e| e.to_string())?;
    drop(conn);
    emit_quick_launch_changed(&app);
    Ok(())
}

/// Clears an item's group membership — back to ungrouped (ticket 89).
#[tauri::command]
fn unassign_from_group(
    app: AppHandle,
    state: State<'_, AppState>,
    collection: groups::Collection,
    item_id: i64,
) -> Result<(), String> {
    let conn = lock(&state)?;
    groups::unassign_item(&conn, collection, item_id).map_err(|e| e.to_string())?;
    drop(conn);
    emit_quick_launch_changed(&app);
    Ok(())
}

/// One collection's Groups toggle (ticket 89): persists only that
/// collection's knob. Off stays fully dormant — surfaces render flat while
/// stored groups and memberships survive untouched — so a live Quick Launch
/// window is told via `quick-launch-changed`.
#[tauri::command]
fn update_groups_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    collection: groups::Collection,
    enabled: bool,
) -> Result<(), String> {
    let conn = lock(&state)?;
    settings::save_groups_feature(&conn, collection, if enabled { "on" } else { "off" })?;
    drop(conn);
    emit_quick_launch_changed(&app);
    Ok(())
}

/// Runs one stored Quick Action (tickets 50 & 62): the action's PowerShell
/// command, hidden (`CREATE_NO_WINDOW`), working directory honored when set,
/// current user, no elevation, no status UI, no notification. The spawned
/// process is tracked in the per-session registry for its lifetime — a reaper
/// thread waits on it and emits `quick-action-run-state-changed` on exit, so
/// the Quick Launch window flips Run ↔ Stop with no polling. A stoppable
/// action that is already running is rejected — stop it first (the window
/// shows Stop, not Run, while tracked).
#[tauri::command]
fn run_quick_action(app: AppHandle, state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = lock(&state)?;
    let action = quick_actions::get_quick_action(&conn, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            "This quick action is no longer in the list — refresh and try again".to_string()
        })?;
    drop(conn);
    if action.action.stoppable
        && state
            .running_actions
            .lock()
            .map_err(|e| e.to_string())?
            .contains_key(&id)
    {
        return Err(format!(
            "'{}' is already running — stop it first.",
            action.action.name
        ));
    }
    let log_path = quick_actions::new_run_log_path(&crate::db::logs_dir(), &action.action.name);
    let log_file = log_path.as_ref().and_then(|p| quick_actions::open_run_log(p));
    let child = match quick_actions::spawn_quick_action(&action.action, log_file.as_ref()) {
        Ok(child) => child,
        Err(e) => {
            // The failure is the run's only record — land it in the folder
            // when there is one (ticket 64), then fail loudly.
            if let Some(p) = &log_path {
                quick_actions::append_log_line(
                    p,
                    &format!("{} start failed: {e}", quick_actions::log_stamp()),
                );
            }
            return Err(e);
        }
    };
    let pid = child.id();
    if let Some(p) = &log_path {
        quick_actions::write_run_log_header(p, &action.action, id, pid);
    }
    let exited = quick_actions::ExitSignal::new();
    state
        .running_actions
        .lock()
        .map_err(|e| e.to_string())?
        .insert(
            id,
            quick_actions::RunningQuickAction {
                pid,
                log_path: log_path.clone(),
                exited: exited.clone(),
            },
        );
    let _ = app.emit(
        "quick-action-run-state-changed",
        quick_actions::QuickActionRunState { id, running: true },
    );
    // The reaper owns the Child: it waits for the exit, records the exit
    // code in the run's output.log (ticket 64), marks the exit signal so a
    // Stop's watchdog stands down (ticket 92), drops the registry entry
    // (only if this run is still the tracked one — a Stop already removed
    // it), and tells the window. PIDs die with the boot anyway, so the
    // registry stays per-session.
    std::thread::spawn(move || {
        let mut child = child;
        let status = child.wait().ok();
        if let Some(p) = &log_path {
            quick_actions::write_run_log_exit(p, status.and_then(|s| s.code()));
        }
        exited.signal();
        if let Some(state) = app.try_state::<AppState>() {
            if let Ok(mut registry) = state.running_actions.lock() {
                if registry.get(&id).map(|r| r.pid) == Some(pid) {
                    registry.remove(&id);
                }
            }
        }
        let _ = app.emit(
            "quick-action-run-state-changed",
            quick_actions::QuickActionRunState { id, running: false },
        );
    });
    Ok(())
}

/// Stops a running Quick Action (ticket 62): runs the action's own stop
/// command when it has one (same hidden PowerShell spawn path, the action's
/// working directory honored), otherwise kills the tracked process tree
/// (`taskkill /T /F`). The registry entry is removed here; the reaper notices
/// the death and emits the not-running event. A configured stop command is
/// watched (ticket 92): when it has not finished the process inside
/// `quick_actions::STOP_WATCHDOG`, the tree is force-killed so a hung stop
/// can never wedge the control. Both the stop line and — when a stop command
/// ran — its output land in the run's `output.log` (ticket 64). Stopping an
/// action that is not running is a clear error, never a silent success.
#[tauri::command]
fn stop_quick_action(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let tracked = state
        .running_actions
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&id)
        .ok_or_else(|| "This quick action is not running.".to_string())?;
    let conn = lock(&state)?;
    let action = quick_actions::get_quick_action(&conn, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            "This quick action is no longer in the list — refresh and try again".to_string()
        })?;
    drop(conn);
    match quick_actions::normalized_stop_command(&action.action) {
        Some(stop_command) => {
            if let Some(p) = &tracked.log_path {
                quick_actions::write_run_log_stop(
                    p,
                    &format!("stop command: {stop_command}"),
                );
            }
            let log_file = tracked
                .log_path
                .as_ref()
                .and_then(|p| quick_actions::open_run_log(p));
            quick_actions::spawn_stop_command(
                &stop_command,
                action.action.cwd.as_deref(),
                log_file.as_ref(),
            )?;
            // The watchdog (ticket 92): a stop command that never finishes
            // the process gets its tree force-killed at STOP_WATCHDOG; an
            // early exit stands it down through the reaper's signal.
            let exited = tracked.exited.clone();
            let pid = tracked.pid;
            let log_path = tracked.log_path.clone();
            std::thread::spawn(move || {
                quick_actions::enforce_stop_watchdog(pid, log_path.as_deref(), &exited);
            });
            Ok(())
        }
        None => {
            if let Some(p) = &tracked.log_path {
                quick_actions::write_run_log_stop(p, "tree kill (taskkill /T /F)");
            }
            crate::engine::windows::kill_tree(tracked.pid);
            Ok(())
        }
    }
}

/// The ids of every Quick Action whose tracked process is still alive
/// (ticket 62) — the Quick Launch window's starting picture when it opens;
/// from then on the run-state events keep it current.
#[tauri::command]
fn list_running_quick_actions(state: State<'_, AppState>) -> Result<Vec<i64>, String> {
    let registry = state.running_actions.lock().map_err(|e| e.to_string())?;
    Ok(registry.keys().copied().collect())
}

/// One Test click in the Quick Actions editor (ticket 50, prior art: the
/// Launch entry Test button, ticket 41): runs the command under PowerShell,
/// timeboxed, and reports exit code + captured output. A command that
/// outlives the box comes back timed out — honestly not headless-verifiable,
/// never passed.
#[tauri::command]
fn test_quick_action(command: String, cwd: Option<String>) -> Result<launch::TestResult, String> {
    if command.trim().is_empty() {
        return Err("The command is empty — nothing to test.".into());
    }
    quick_actions::validate_cwd(cwd.as_deref())?;
    Ok(quick_actions::test_quick_action(&command, cwd.as_deref()))
}

/// The Quick Launch window's × button (tickets 52, 53 & 56): destroys the
/// window — the only way the floating palette closes, since blur is a no-op
/// (ticket 56) — and the tray's left-click reopens it at its fixed centered
/// size (it never remembers geometry). When the window is docked (ticket 53),
/// the AppBar is released first so the edge is never left occupied.
#[tauri::command]
fn close_quick_launch_window(app: AppHandle) -> Result<(), String> {
    quick_window::close(&app).map_err(|e| e.to_string())
}

/// The frontend's view of the live dock state (tickets 53 & 59): the edge and
/// visibility mode — the values the window is docked with, or, while it
/// floats, the target values the toggle would dock to — plus `docked`, which
/// tells the two apart, and the transient blocked reason (ticket 63) when
/// auto-hide is refused by the shell. The header's dock/undock toggle renders
/// the target edge's icon from it, so the chrome always tells the truth.
#[derive(serde::Serialize)]
pub struct DockStateView {
    pub edge: String,
    pub mode: String,
    pub docked: bool,
    pub blocked: Option<String>,
    pub left_eligible: bool,
    pub right_eligible: bool,
}

/// The dock/undock toggle (ticket 53): docks the window to its current
/// monitor's remembered (or Settings-default) edge, or undocks back to the
/// floating window when already docked. Ticket 57: the outcome is written
/// back to Settings (`dock.state`, and `dock.edge` when docking) so the
/// Settings screen and the window never diverge — the window reopens in the
/// state it was left in.
#[tauri::command]
fn toggle_quick_launch_dock(app: AppHandle) -> Result<(), String> {
    if quick_window::is_docked(&app) {
        quick_window::undock(&app)?;
        persist_dock_setting(&app, "dock.state", "floating")?;
    } else {
        quick_window::dock(&app, None)?;
        persist_dock_setting(&app, "dock.state", "docked")?;
        let edge = quick_window::docked_state(&app).map(|d| d.edge);
        if let Some(edge) = edge {
            persist_dock_setting(&app, "dock.edge", &edge)?;
        }
    }
    Ok(())
}

/// The left↔right edge-switch arrows (ticket 53): moves the docked window to
/// the given edge without unregistering the AppBar. Ticket 57: the outcome is
/// written back to Settings (`dock.edge`) so the Settings screen's default
/// edge stays aligned with the window.
#[tauri::command]
fn switch_quick_launch_dock_edge(app: AppHandle, edge: String) -> Result<(), String> {
    quick_window::dock(&app, Some(&edge))?;
    persist_dock_setting(&app, "dock.edge", &edge)?;
    persist_dock_setting(&app, "dock.state", "docked")?;
    Ok(())
}

/// Writes one dock knob back to Settings (ticket 57) — the in-window dock
/// controls persist their outcome so the two surfaces never diverge. The
/// targeted writers never touch the other knobs.
fn persist_dock_setting(app: &AppHandle, key: &str, value: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    match key {
        "dock.state" => settings::save_dock_state(&conn, value),
        "dock.edge" => settings::save_dock_edge(&conn, value),
        _ => Err(format!("unknown dock setting '{key}'")),
    }
}

/// The per-monitor display enumeration plus per-display dock memory (ticket
/// 111): label, resolution, identity, and wall eligibility via the single
/// geometry source.
#[tauri::command]
fn list_displays() -> Result<Vec<appbar::DisplayInfo>, String> {
    Ok(appbar::cached_displays())
}

#[tauri::command]
fn get_display_dock_edge(
    state: State<'_, AppState>,
    display: String,
) -> Result<Option<String>, String> {
    let conn = lock(&state)?;
    // Resolve identity for the device so the fallback (ticket 110) is honored:
    // read tries identity then legacy device-name.
    let displays = appbar::cached_displays();
    let (device_name, identity) = resolve_display_keys(&display, &displays);
    Ok(db::load_dock_edge_identified(&conn, identity.as_deref(), &device_name))
}

#[tauri::command]
fn set_display_dock_edge(
    state: State<'_, AppState>,
    display: String,
    edge: String,
) -> Result<(), String> {
    settings::validate_dock_edge(&edge)?;
    let displays = appbar::cached_displays();
    let (device_name, identity) = resolve_display_keys(&display, &displays);
    // Seam check (ticket 111 Study A) — refused with the identical inline
    // string Settings shows.
    match appbar::edge_eligible_for_device(&device_name, &edge) {
        Ok(false) => return Err(appbar::SEAM_REASON.to_string()),
        Ok(true) => {}
        Err(e) => return Err(e),
    }
    let conn = lock(&state)?;
    // Write through the EDID-keyed helper (ticket 110) — virtual displays fall
    // back to the device name with no visible difference.
    let key = per_display_key(identity.as_deref(), &device_name);
    db::save_dock_edge(&conn, &key, &edge).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_display_dock_mode(
    state: State<'_, AppState>,
    display: String,
) -> Result<Option<String>, String> {
    let conn = lock(&state)?;
    let displays = appbar::cached_displays();
    let (device_name, identity) = resolve_display_keys(&display, &displays);
    Ok(db::load_dock_mode_identified(&conn, identity.as_deref(), &device_name))
}

#[tauri::command]
fn set_display_dock_mode(
    state: State<'_, AppState>,
    display: String,
    mode: String,
) -> Result<(), String> {
    settings::validate_dock_mode(&mode)?;
    let conn = lock(&state)?;
    let displays = appbar::cached_displays();
    let (device_name, identity) = resolve_display_keys(&display, &displays);
    let key = per_display_key(identity.as_deref(), &device_name);
    db::save_dock_mode(&conn, &key, &mode).map_err(|e| e.to_string())
}

/// The remembered dock width % for one display (ticket 128): the per-monitor
/// override the Settings slider edits, falling back to the global default
/// when absent. `None` means no override — the caller uses the global.
#[tauri::command]
fn get_display_dock_width_pct(
    state: State<'_, AppState>,
    display: String,
) -> Result<Option<u32>, String> {
    let conn = lock(&state)?;
    let displays = appbar::cached_displays();
    let (device_name, identity) = resolve_display_keys(&display, &displays);
    Ok(db::load_dock_width_pct_identified(&conn, identity.as_deref(), &device_name))
}

/// Persists one display's dock width % (ticket 128): 10–30, validated first.
#[tauri::command]
fn set_display_dock_width_pct(
    state: State<'_, AppState>,
    display: String,
    pct: u32,
) -> Result<(), String> {
    settings::validate_dock_width_pct(pct)?;
    let conn = lock(&state)?;
    let displays = appbar::cached_displays();
    let (device_name, identity) = resolve_display_keys(&display, &displays);
    let key = per_display_key(identity.as_deref(), &device_name);
    db::save_dock_width_pct(&conn, &key, pct).map_err(|e| e.to_string())
}

/// Applies the final saved global + per-monitor picture after the Settings
/// screen finishes its batch writes, then publishes one definitive refresh.
#[tauri::command]
fn reconcile_quick_launch_settings(app: AppHandle) -> Result<(), String> {
    quick_window::reconcile_saved_settings(&app)?;
    emit_quick_launch_changed(&app);
    Ok(())
}

fn resolve_display_keys(
    display: &str,
    displays: &[appbar::DisplayInfo],
) -> (String, Option<String>) {
    // `display` may be the storage `id` (edid-... or device_name) or the raw
    // device name — match either.
    for d in displays {
        if d.device_name.eq_ignore_ascii_case(display) || d.id.eq_ignore_ascii_case(display) {
            return (d.device_name.clone(), d.identity.clone());
        }
    }
    // Unknown display (e.g. stored key for a now-unplugged monitor) — treat
    // the string as both device and identity attempt; the eligibility check
    // will treat it as eligible so the save still lands.
    let as_identity = if display.starts_with("edid-") {
        Some(display.to_string())
    } else {
        None
    };
    if as_identity.is_some() {
        (display.to_string(), as_identity)
    } else {
        (display.to_string(), None)
    }
}

fn per_display_key<'a>(identity: Option<&'a str>, device: &'a str) -> String {
    match identity {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => device.to_string(),
    }
}

// ------------------- Companion (ticket 125) -------------------------------

/// Sets the companion active URL (ticket 125): https:// or null (off).
/// Validates, persists, and notifies the Quick Launch window so the pane
/// appears/disappears without reopening (content-gated).
#[tauri::command]
fn set_companion_url(
    app: AppHandle,
    state: State<'_, AppState>,
    url: Option<String>,
) -> Result<(), String> {
    let normalized = settings::normalize_companion_url(url.as_deref());
    settings::validate_companion_url(normalized.as_deref())?;
    let conn = lock(&state)?;
    settings::save_companion_url(&conn, normalized.as_deref())?;
    drop(conn);
    emit_quick_launch_changed(&app);
    Ok(())
}

#[tauri::command]
fn open_companion_external(url: String) -> Result<(), String> {
    settings::validate_companion_url(Some(&url))?;
    external::open(url.trim(), "the companion URL")
}

/// The toolbar's volume-mixer shortcut: opens the per-app Volume mixer page
/// directly, so loud/soft never needs hunting. Fixed target — no input to
/// validate, nothing to persist.
#[tauri::command]
fn open_volume_mixer() -> Result<(), String> {
    // WHY this URI: the modern per-app sliders page, and the same page the
    // taskbar's own "Open volume mixer" opens — OS vocabulary, zero learning.
    external::open("ms-settings:apps-volume", "the volume mixer")
}

/// Sets the global companion height ratio (ticket 125): 0.25–0.60.
#[tauri::command]
fn set_companion_height_ratio(
    app: AppHandle,
    state: State<'_, AppState>,
    ratio: f64,
) -> Result<(), String> {
    settings::validate_companion_height_ratio(ratio)?;
    let conn = lock(&state)?;
    settings::save_companion_height_ratio(&conn, ratio)?;
    drop(conn);
    emit_quick_launch_changed(&app);
    Ok(())
}

/// Sets the companion saved URL list (ticket 125): deduped host+path case-insensitive.
#[tauri::command]
fn set_companion_url_list(
    app: AppHandle,
    state: State<'_, AppState>,
    urls: Vec<String>,
) -> Result<(), String> {
    settings::validate_companion_url_list(&urls)?;
    let conn = lock(&state)?;
    settings::save_companion_url_list(&conn, &urls)?;
    drop(conn);
    emit_quick_launch_changed(&app);
    Ok(())
}

/// Per-monitor companion height ratio read (ticket 125): identity wins, falls
/// back to device name, `None` means use the global settings ratio.
#[tauri::command]
fn get_companion_height_ratio(
    state: State<'_, AppState>,
    display: String,
) -> Result<Option<f64>, String> {
    let conn = lock(&state)?;
    let displays = appbar::cached_displays();
    let (device_name, identity) = resolve_display_keys(&display, &displays);
    Ok(db::load_companion_height_ratio_identified(&conn, identity.as_deref(), &device_name))
}

/// Per-monitor companion height ratio write (ticket 125): validated, clamped
/// on read, persisted per monitor (falls back to global settings).
#[tauri::command]
fn set_companion_height_ratio_for_display(
    state: State<'_, AppState>,
    display: String,
    ratio: f64,
) -> Result<(), String> {
    settings::validate_companion_height_ratio(ratio)?;
    let conn = lock(&state)?;
    let displays = appbar::cached_displays();
    let (device_name, identity) = resolve_display_keys(&display, &displays);
    let key = per_display_key(identity.as_deref(), &device_name);
    db::save_companion_height_ratio(&conn, &key, ratio).map_err(|e| e.to_string())
}

/// The dock toolbar's audio picture: persisted mute plus live playback.
/// Healing a drifted WebView toward the persisted mute happens inside, so a
/// fresh pane never stays loud after silence was asked for.
#[tauri::command]
fn get_companion_audio_state(app: AppHandle) -> Result<companion_audio::CompanionAudioState, String> {
    let persisted = companion_audio::persisted_muted(&app);
    Ok(companion_audio::current_state(&app, persisted))
}

/// The dock toolbar's mute toggle: persists the choice, pushes it into the
/// live WebView, and fans the new state out so the indicator follows without
/// waiting for the next poll.
#[tauri::command]
fn set_companion_muted(
    app: AppHandle,
    state: State<'_, AppState>,
    muted: bool,
) -> Result<companion_audio::CompanionAudioState, String> {
    {
        let conn = lock(&state)?;
        settings::save_companion_muted(&conn, muted)?;
    }
    companion_audio::apply_muted(&app, muted);
    let live = companion_audio::current_state(&app, muted);
    // WHY emit instead of relying on the poll: the toggle must read back
    // instantly or silence feels broken.
    let _ = app.emit("companion-audio-changed", &live);
    Ok(live)
}

/// Opens (or focuses) the main window (ticket 123): the dock header's mark
/// click and any other surface that needs a non-tray entry point. Enqueues
/// through the off-thread single-flight seam — never the blocking call, which
/// would hang this command's event thread (see `request_open_main_window`).
#[tauri::command]
fn open_main_window_cmd(app: AppHandle) -> Result<(), String> {
    crate::request_open_main_window(&app);
    Ok(())
}

#[tauri::command]
fn main_window_ready(app: AppHandle) -> Result<(), String> {
    let Some(state) = app.try_state::<AppState>() else {
        return Err("application state is unavailable".into());
    };
    if !state.main_window_loading.load(Ordering::SeqCst) {
        return Ok(());
    }
    let Some(window) = app.get_webview_window("main") else {
        state.main_window_loading.store(false, Ordering::SeqCst);
        return Err("main window is unavailable".into());
    };
    let reveal = set_main_window_alpha(&window, u8::MAX)
        .and_then(|_| window.set_skip_taskbar(false))
        .and_then(|_| window.show())
        .and_then(|_| window.unminimize())
        .and_then(|_| window.set_focus());
    state.main_window_loading.store(false, Ordering::SeqCst);
    if let Err(e) = reveal {
        let _ = window.destroy();
        return Err(e.to_string());
    }
    Ok(())
}

#[tauri::command]
fn open_sprout_cmd(app: AppHandle) -> Result<(), String> {
    crate::tray::open_sprout(&app);
    Ok(())
}

/// The dock chrome's state query (tickets 53 & 59): the current edge and mode
/// when docked, or — while the window floats — the target edge/mode the
/// toggle would dock to; `docked` tells the two apart. The header renders its
/// controls from this.
#[tauri::command]
fn get_quick_launch_dock_state(app: AppHandle) -> Result<DockStateView, String> {
    Ok(match quick_window::docked_state(&app) {
        Some(d) => {
            // Eligibility for the live monitor so the header arrows can share
            // the same wall rule as Settings (ticket 111).
            let displays = appbar::cached_displays();
            let all_rects: Vec<windows_sys::Win32::Foundation::RECT> = displays
                .iter()
                .map(|di| windows_sys::Win32::Foundation::RECT {
                    left: di.x,
                    top: di.y,
                    right: di.x + di.width,
                    bottom: di.y + di.height,
                })
                .collect();
            let (left_eligible, right_eligible) = {
                // Find current monitor's rect.
                let mut found: Option<windows_sys::Win32::Foundation::RECT> = None;
                for di in &displays {
                    if di.device_name == d.monitor {
                        found = Some(windows_sys::Win32::Foundation::RECT {
                            left: di.x,
                            top: di.y,
                            right: di.x + di.width,
                            bottom: di.y + di.height,
                        });
                        break;
                    }
                }
                if let Some(rect) = found {
                    (appbar::is_edge_eligible(rect, &all_rects, "left"), appbar::is_edge_eligible(rect, &all_rects, "right"))
                } else {
                    (true, true)
                }
            };
            DockStateView {
                edge: d.edge,
                mode: d.mode,
                docked: true,
                blocked: d.blocked,
                left_eligible,
                right_eligible,
            }
        },
        None => {
            let (edge, mode) = quick_window::pending_dock(&app)?;
            // While floating, report eligibility for the monitor the window
            // sits on (or would sit on) so the toggle icon/arrow disable
            // matches the docked case. Best-effort: if pending_dock cannot
            // resolve a monitor, assume eligible.
            let (left_eligible, right_eligible) = pending_eligibility(&app).unwrap_or((true, true));
            DockStateView {
                edge,
                mode,
                docked: false,
                // A block is a property of a live dock only — floating has
                // nothing to be blocked (ticket 63).
                blocked: None,
                left_eligible,
                right_eligible,
            }
        }
    })
}

fn pending_eligibility(app: &AppHandle) -> Result<(bool, bool), String> {
    let window = quick_window::quick_launch_window(&app)
        .ok_or_else(|| "Quick Launch window is not open".to_string())?;
    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    let device = appbar::monitor_key(hwnd.0)
        .ok_or_else(|| "cannot identify the current monitor".to_string())?;
    let displays = appbar::cached_displays();
    if displays.is_empty() {
        return Ok((true, true));
    }
    let all_rects: Vec<windows_sys::Win32::Foundation::RECT> = displays
        .iter()
        .map(|di| windows_sys::Win32::Foundation::RECT {
            left: di.x,
            top: di.y,
            right: di.x + di.width,
            bottom: di.y + di.height,
        })
        .collect();
    for (idx, di) in displays.iter().enumerate() {
        if di.device_name.eq_ignore_ascii_case(&device) {
            let rect = all_rects[idx];
            return Ok((
                appbar::is_edge_eligible(rect, &all_rects, "left"),
                appbar::is_edge_eligible(rect, &all_rects, "right"),
            ));
        }
    }
    Ok((true, true))
}

/// [DEBUG-66] Temporary stress driver for ticket 66's repro loop: rapid
/// fixed↔auto-hide mode switches against a live docked Quick Launch window —
/// the exact user flow that aborts the process. Debug builds only, and only
/// when `SPROUT_DOCK_STRESS=1`; writes a marker file (env
/// `SPROUT_DOCK_STRESS_RESULT`, default `%TEMP%\sprout-stress-66.json`)
/// containing "PASS iters=N" on clean completion, then exits. The harness in
/// `tools/repro-dock-mode-stress.ps1` asserts on that marker plus the
/// process exit code. Restores the captured settings/dock memory afterwards.
#[cfg(debug_assertions)]
fn debug66_dock_mode_stress(app: AppHandle) {
    use std::time::Duration;

    struct Snapshot {
        settings: Settings,
        monitor: Option<String>,
        monitor_edge: Option<String>,
        monitor_mode: Option<String>,
    }

    fn restore(app: &AppHandle, snap: &Snapshot) {
        let state = app.state::<AppState>();
        let Ok(conn) = state.db.lock() else {
            return;
        };
        let _ = settings::save(&conn, &snap.settings);
        if let Some(m) = &snap.monitor {
            if let Some(e) = &snap.monitor_edge {
                let _ = db::save_dock_edge(&conn, m, e);
            }
            if let Some(mo) = &snap.monitor_mode {
                let _ = db::save_dock_mode(&conn, m, mo);
            }
        }
    }

    std::thread::spawn(move || {
        let marker = std::env::var("SPROUT_DOCK_STRESS_RESULT").unwrap_or_else(|_| {
            std::env::temp_dir()
                .join("sprout-stress-66.json")
                .display()
                .to_string()
        });
        let iterations: u32 = std::env::var("SPROUT_DOCK_STRESS_ITERS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);
        let interval_ms: u64 = std::env::var("SPROUT_DOCK_STRESS_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(80);
        eprintln!("[stress-66] begin iters={iterations} interval_ms={interval_ms}");
        // Deterministic start: wipe leftover dock state (a crashed earlier run
        // persists mode=auto-hide / state=docked, which changes the next
        // launch's open() behavior and poisons the scenario).
        {
            let state = app.state::<AppState>();
            let Ok(conn) = state.db.lock() else {
                eprintln!("[stress-66] could not lock db for the state reset");
                return;
            };
            let _ = crate::db::upsert_meta(&conn, "dock.state", "floating");
            let _ = conn.execute(
                "DELETE FROM meta WHERE key LIKE 'quicklaunch.dock.%'",
                [],
            );
        }
        let fail = |marker: &str, reason: String| {
            eprintln!("[stress-66] FAIL {reason}");
            let _ = std::fs::write(marker, format!("FAIL {reason}"));
        };
        // Snapshot for restore while the window is still floating.
        let snapshot = {
            let state = app.state::<AppState>();
            let conn = match state.db.lock() {
                Ok(conn) => conn,
                Err(e) => {
                    fail(&marker, format!("snapshot {e}"));
                    app.exit(3);
                    return;
                }
            };
            let s = settings::load(&conn);
            let monitor = quick_window::quick_launch_window(&app)
                .and_then(|w| w.hwnd().ok())
                .and_then(|h| appbar::monitor_key(h.0));
            let (edge, mode) = match &monitor {
                Some(m) => (db::load_dock_edge(&conn, m), db::load_dock_mode(&conn, m)),
                None => (None, None),
            };
            Snapshot {
                settings: s,
                monitor,
                monitor_edge: edge,
                monitor_mode: mode,
            }
        };
        if let Err(e) = quick_window::open(&app) {
            restore(&app, &snapshot);
            fail(&marker, format!("open {e}"));
            app.exit(3);
            return;
        }
        std::thread::sleep(Duration::from_millis(800));
        if let Err(e) = quick_window::dock(&app, None) {
            let _ = quick_window::undock(&app);
            restore(&app, &snapshot);
            fail(&marker, format!("dock {e}"));
            app.exit(3);
            return;
        }
        // Normalize to fixed so iteration 0's flip is always fixed→auto-hide.
        if let Err(e) = quick_window::set_dock_mode(&app, "fixed") {
            eprintln!("[stress-66] normalize to fixed failed: {e}");
        }
        std::thread::sleep(Duration::from_millis(300));
        for i in 0..iterations {
            let mode = if i % 2 == 0 { "auto-hide" } else { "fixed" };
            eprintln!("[stress-66] iter={i} -> {mode}");
            if let Err(e) = quick_window::set_dock_mode(&app, mode) {
                eprintln!("[stress-66] iter={i} set_dock_mode({mode}) errored: {e}");
            }
            std::thread::sleep(Duration::from_millis(interval_ms));
        }
        std::thread::sleep(Duration::from_millis(500));
        let _ = quick_window::undock(&app);
        let _ = quick_window::close(&app);
        restore(&app, &snapshot);
        eprintln!("[stress-66] PASS iters={iterations}");
        let _ = std::fs::write(&marker, format!("PASS iters={iterations}"));
        app.exit(0);
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Lazy init: %LOCALAPPDATA%\Sprout\sprout.db + logs\ are created here on
    // first launch (ADR-0006) — nothing exists on disk before this point.
    let conn = db::init().expect("failed to initialize Sprout data directory");
    // Retention is honored at app start, not only after a run completes
    // (ticket 09): expired run log folders are pruned on every launch.
    let _ = logs::prune_run_logs(&conn);
    let pending_import = parse_pending_import_arg();
    // The auto-start login launches with the Run key's `--autostart`
    // argument (ADR-0013): such a boot brings up backend + tray only.
    let autostart_boot = autostart::is_autostart_launch(
        &std::env::args().skip(1).collect::<Vec<_>>(),
    );

    tauri::Builder::default()
        // Registered before any other plugin: a second launch (e.g. a
        // double-clicked .sprout.json while Sprout is already open) is
        // intercepted here — its file argument is handed to the running
        // instance instead of opening a second window. The elevated worker
        // never reaches this point (main.rs routes `--worker` before Tauri).
        // When the window was destroyed by closing it (ticket 43), the hook
        // recreates it instead of assuming it exists.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            use tauri::{Emitter, Manager};
            if let Some(path) = parse_pending_import(&argv) {
                if let Some(state) = app.try_state::<AppState>() {
                    if let Ok(mut slot) = state.pending_import.lock() {
                        *slot = Some(path.clone());
                    }
                }
                let _ = app.emit("pending-import", path);
            }
            // Off-thread seam: this hook runs on the event thread, which the
            // blocking open would hang (see `request_open_main_window`).
            crate::request_open_main_window(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        // Quick Clips' clipboard writes (ticket 78): the plugin is driven
        // from Rust commands only — no JS-side plugin surface, so no
        // capability grants beyond the defaults are needed.
        .plugin(tauri_plugin_clipboard_manager::init())
        // Auto-start (ADR-0013, ticket 75): the HKCU Run entry carries the
        // `--autostart` launcher argument, which ticket 76's boot path
        // consumes to start tray-only. The registration itself is synced by
        // `autostart::sync_registration`, never here.
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .on_window_event(|window, event| {
            // Ticket 43: closing the main window (× or Alt+F4) destroys it —
            // the webview goes away and the lean Rust backend stays resident
            // in the tray. Open Sprout (or a second launch) recreates the
            // window; Quit lives in the tray menu.
            //
            // Ticket 56: the Quick Launch window is a persistent palette —
            // blur does nothing (the floating window stays open until closed,
            // and the docked bar's visibility is Sprout's own driver,
            // ticket 63), and its × button /
            // Alt+F4 destroy it. The tray's left-click reopens it at its
            // fixed centered size (it never remembers geometry).
            if window.label() == quick_window::QUICK_LAUNCH_WINDOW {
                match event {
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let _ = quick_window::close(window.app_handle());
                    }
                    _ => {}
                }
                return;
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Ticket 116: while Settings is dirty the main window's close
                // is held and the frontend shows the save/discard/keep dialog.
                let dirty = window
                    .app_handle()
                    .try_state::<AppState>()
                    .and_then(|s| s.settings_dirty.lock().ok().map(|g| *g))
                    .unwrap_or(false);
                if dirty {
                    api.prevent_close();
                    let _ = window.app_handle().emit("settings-dirty-close-requested", ());
                } else {
                    api.prevent_close();
                    // Ticket 123: remember close time for the close→reopen white-screen
                    // race — `destroy()` is async and the label lingers.
                    if let Some(state) = window.app_handle().try_state::<AppState>() {
                        if let Ok(mut t) = state.main_close_time.lock() {
                            *t = Some(std::time::Instant::now());
                        }
                    }
                    let _ = window.destroy();
                }
            }
        })
        .setup(move |app| {
            // The tray icon is the resident surface (ticket 43): created at
            // startup, left-click opens the Quick Launch window, right-click
            // menu is Open Sprout / Quit (ticket 54).
            tray::init(app.handle())?;
            // The dock drift watchdog (ticket 61): one background thread that
            // re-docks the bar when its window drifts from its edge.
            quick_window::start_drift_guard(app.handle().clone());
            // The auto-hide motion driver (ticket 63): ~16 ms cursor polling
            // that slides the docked strip to its sliver and back — Sprout
            // owns the motion; the OS never moves an appbar.
            quick_window::start_autohide_driver(app.handle().clone());
            // The boot path (ADR-0013, ticket 76): the conf file declares no
            // windows — manual launches build the main window here through
            // the same open/recreate seam the tray and single-instance hook
            // use, while an `--autostart` login keeps the desktop clear. The
            // Quick Launch window materializes under one rule either way:
            // remembered "docked" → opened (its open path applies the
            // edge/mode memory and docks immediately); floating or a fresh
            // install → tray-only until the first click.
            if let Err(e) = quick_window::open_if_docked(app.handle()) {
                // A failed restore leaves the tray resident; the left-click
                // or Open Sprout retries the same seam.
                eprintln!("Quick Launch dock restore failed: {e}");
            }
            if !autostart_boot {
                if let Err(e) = open_main_window(app.handle()) {
                    eprintln!("Could not open the main window: {e}");
                }
            }
            // The once-per-launch self-update check (ADR-0012, ticket 73):
            // background thread, single `update-available` event on a newer
            // release, silent on every failure.
            update::start_background_check(app.handle().clone());
            // The auto-start reconciliation (ADR-0013, ticket 75): one sync
            // per launch on a background thread — the Run key ends up
            // matching the persisted preference (default: registered).
            {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    let desired_on = match handle.try_state::<AppState>() {
                        Some(state) => {
                            let Ok(conn) = state.db.lock() else {
                                eprintln!("Auto-start: could not lock the db for the startup sync");
                                return;
                            };
                            settings::load(&conn).autostart == "on"
                        }
                        None => return,
                    };
                    if let Err(e) = autostart::sync_registration(&handle, desired_on) {
                        eprintln!("Auto-start: {e}");
                    }
                });
            }
            // [DEBUG-66] ticket 66 repro loop (debug builds + opt-in env only).
            #[cfg(debug_assertions)]
            if std::env::var("SPROUT_DOCK_STRESS").as_deref() == Ok("1") {
                debug66_dock_mode_stress(app.handle().clone());
            }
            Ok(())
        })
        .manage(AppState {
            db: Mutex::new(conn),
            engine: Arc::new(WindowsWingetEngine),
            launcher: Arc::new(engine::windows::WindowsLauncherEngine),
            launch_in_progress: Arc::new(AtomicBool::new(false)),
            pending_import: Mutex::new(pending_import),
            dock: Mutex::new(None),
            running_actions: Mutex::new(HashMap::new()),
            settings_dirty: Mutex::new(false),
            main_close_time: Mutex::new(None),
            main_window_opening: AtomicBool::new(false),
            main_window_loading: AtomicBool::new(false),
        })
        .invoke_handler(tauri::generate_handler![
            list_products,
            create_product,
            update_product,
            delete_product,
            product_presets_impact,
            search_winget,
            show_winget,
            list_presets,
            create_preset,
            update_preset,
            delete_preset,
            export_preset,
            import_preset,
            export_backup,
            inspect_backup,
            import_backup,
            take_pending_import,
            compute_plan,
            quick_install_plan,
            start_run,
            quick_install,
            read_run_progress,
            cancel_run,
            get_active_run,
            get_run,
            list_runs,
            get_settings,
            update_settings,
            update_theme,
            update_autostart,
            update_groups_enabled,
            check_for_update,
            install_update,
            list_logs,
            open_folder,
            list_launch_entries,
            create_launch_entry,
            update_launch_entry,
            delete_launch_entry,
            move_launch_entry,
            test_launch_command,
            start_quick_launch,
            start_launch_entry,
            list_launch_candidates,
            candidate_icon,
            list_virtual_desktops,
            create_virtual_desktop,
            list_quick_actions,
            create_quick_action,
            update_quick_action,
            delete_quick_action,
            move_quick_action,
            run_quick_action,
            stop_quick_action,
            list_running_quick_actions,
            test_quick_action,
            list_clips,
            create_clip,
            update_clip,
            delete_clip,
            move_clip,
            copy_clip,
            list_groups,
            create_group,
            rename_group,
            delete_group,
            move_group,
            assign_to_group,
            unassign_from_group,
            update_groups_enabled,
            close_quick_launch_window,
            open_main_window_cmd,
            main_window_ready,
            open_sprout_cmd,
            toggle_quick_launch_dock,
            switch_quick_launch_dock_edge,
            get_quick_launch_dock_state,
            list_displays,
            get_display_dock_edge,
            set_display_dock_edge,
            get_display_dock_mode,
            set_display_dock_mode,
            get_display_dock_width_pct,
            set_display_dock_width_pct,
            reconcile_quick_launch_settings,
            set_companion_url,
            open_companion_external,
            open_volume_mixer,
            set_companion_height_ratio,
            set_companion_url_list,
            get_companion_height_ratio,
            set_companion_height_ratio_for_display,
            get_companion_audio_state,
            set_companion_muted,
            set_settings_dirty,
            destroy_main_window
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        // The `run` callback is the only place `RunEvent` surfaces (the
        // builder's `.run` shortcut hides it). The window was destroyed, not
        // the app: with the tray resident, an exit requested by user
        // interaction (the last window closing) is suppressed — the backend
        // keeps running with zero windows (ticket 43). A programmatic exit
        // (`app.exit(0)` from the tray's Quit) carries a code and is never
        // suppressed.
        .run(|app, event| {
            match event {
                // The window was destroyed, not the app: with the tray
                // resident, an exit requested by user interaction (the last
                // window closing) is suppressed — the backend keeps running
                // with zero windows (ticket 43). A programmatic exit
                // (`app.exit(0)` from the tray's Quit) carries a code and is
                // never suppressed.
                tauri::RunEvent::ExitRequested { code: None, api, .. } => {
                    if app.tray_by_id(tray::TRAY_ID).is_some() {
                        api.prevent_exit();
                    }
                }
                // Ticket 53: the docked AppBar is unregistered on quit so the
                // screen edge is never left occupied after the process dies.
                tauri::RunEvent::Exit => {
                    let _ = quick_window::release_dock(app);
                }
                _ => {}
            }
        });
}

/// Picks a `.sprout.json` from the command line: either a bare file argument
/// or an explicit `--import <path>` pair — what the installer's file
/// association hands over when a preset is double-clicked (ticket 10).
/// Shared by first-launch parsing and the single-instance forwarding hook.
fn parse_pending_import_arg() -> Option<String> {
    parse_pending_import(&std::env::args().skip(1).collect::<Vec<_>>())
}

/// The shared scan: `args` excludes the executable name. A bare
/// `.sprout.json` argument or a `--import <path>` pair both count.
fn parse_pending_import(args: &[String]) -> Option<String> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--import" {
            return iter.next().cloned();
        }
        if arg.ends_with(".sprout.json") {
            return Some(arg.clone());
        }
    }
    None
}
