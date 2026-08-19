// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

mod db;
mod domain;
mod engine;
mod icons;
mod import_export;
mod launch;
mod logs;
mod plan;
mod quick_actions;
mod quick_window;
mod run;
mod settings;
mod tray;
mod walker;
mod winget;
mod worker;

pub use worker::run_worker;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use tauri::{AppHandle, Emitter, State};

use domain::{Preset, PresetRecord, Product, ProductRecord, Requirement};
use engine::{windows::WindowsWingetEngine, DesktopInfo, LauncherEngine, PlatformEngine};
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
/// that keeps two Quick Launch runs from stacking, and the `.sprout.json`
/// path the app was launched with (double-click), consumed by the frontend
/// on first load.
pub struct AppState {
    pub db: Mutex<Connection>,
    pub engine: Arc<dyn PlatformEngine>,
    pub launcher: Arc<dyn LauncherEngine>,
    pub launch_in_progress: Arc<AtomicBool>,
    pub pending_import: Mutex<Option<String>>,
}

fn lock<'a>(state: &'a State<'a, AppState>) -> Result<std::sync::MutexGuard<'a, Connection>, String> {
    state.db.lock().map_err(|e| e.to_string())
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

/// Persists the Settings screen's knobs, validated first.
#[tauri::command]
fn update_settings(state: State<'_, AppState>, settings: Settings) -> Result<(), String> {
    let conn = lock(&state)?;
    settings::save(&conn, &settings)
}

/// Persists the theme on its own — the Settings screen applies it the moment
/// it is selected, before the rest of the form is saved (ticket 31).
#[tauri::command]
fn update_theme(state: State<'_, AppState>, theme: String) -> Result<(), String> {
    let conn = lock(&state)?;
    settings::save_theme(&conn, &theme)
}

/// The Logs screen's picture of where logs live and how big they are — no
/// content, ever.
#[tauri::command]
fn list_logs() -> Result<LogLocations, String> {
    Ok(logs::list_log_locations())
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
#[tauri::command]
fn create_launch_entry(
    state: State<'_, AppState>,
    entry: launch::LaunchEntryInput,
) -> Result<launch::LaunchEntry, String> {
    launch::validate_launch_entry(&entry)?;
    let conn = lock(&state)?;
    launch::create_launch_entry(&conn, &entry).map_err(|e| e.to_string())
}

/// Replaces a Launch entry's metadata in place; position is untouched
/// (ticket 38).
#[tauri::command]
fn update_launch_entry(
    state: State<'_, AppState>,
    entry: launch::LaunchEntry,
) -> Result<(), String> {
    launch::validate_launch_entry(&entry.entry)?;
    let conn = lock(&state)?;
    launch::update_launch_entry(&conn, &entry).map_err(|e| e.to_string())
}

/// Removes a Launch entry and compacts the list (ticket 38).
#[tauri::command]
fn delete_launch_entry(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = lock(&state)?;
    launch::delete_launch_entry(&conn, id).map_err(|e| e.to_string())
}

/// Moves a Launch entry to another position in the list (ticket 38).
#[tauri::command]
fn move_launch_entry(
    state: State<'_, AppState>,
    id: i64,
    to_position: i64,
) -> Result<(), String> {
    let conn = lock(&state)?;
    launch::move_launch_entry(&conn, id, to_position).map_err(|e| e.to_string())
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

/// The shared launch-run body behind the Quick Launch window's and the
/// page's Start buttons (tickets 42 & 54): the single-flight guard, the
/// background thread running the capped, queued pipeline, the
/// `launch-run-done` event the page listens for, and the summary
/// notification. A second trigger while a run is in flight is rejected —
/// never stacked.
fn launch_entries(
    app: &AppHandle,
    state: &AppState,
    entries: Vec<launch::LaunchEntry>,
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
        let report = launch::run_launch_queue(engine.as_ref(), &entries, cap);
        let _ = app.emit("launch-run-done", &report);
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

/// Opens the main window: focuses the existing one, or recreates it when it
/// was destroyed by closing it (ticket 43). Shared by the tray's Open Sprout
/// and the single-instance focus hook. The recreated window keeps the
/// configured size and minimums (tauri.conf.json).
pub(crate) fn open_main_window(app: &AppHandle) -> tauri::Result<tauri::WebviewWindow> {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window("main") {
        window.set_focus()?;
        Ok(window)
    } else {
        tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::default())
            .title("Sprout")
            .inner_size(1200.0, 800.0)
            .min_inner_size(900.0, 620.0)
            .build()
    }
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
/// (ticket 50).
#[tauri::command]
fn create_quick_action(
    state: State<'_, AppState>,
    action: quick_actions::QuickActionInput,
) -> Result<quick_actions::QuickAction, String> {
    quick_actions::validate_quick_action(&action)?;
    let conn = lock(&state)?;
    quick_actions::create_quick_action(&conn, &action).map_err(|e| e.to_string())
}

/// Replaces a Quick Action's command and metadata in place, validated first;
/// position is untouched — reorders go through `move_quick_action` (ticket
/// 50).
#[tauri::command]
fn update_quick_action(
    state: State<'_, AppState>,
    action: quick_actions::QuickAction,
) -> Result<(), String> {
    quick_actions::validate_quick_action(&action.action)?;
    let conn = lock(&state)?;
    quick_actions::update_quick_action(&conn, &action).map_err(|e| e.to_string())
}

/// Removes a Quick Action and compacts the list (ticket 50).
#[tauri::command]
fn delete_quick_action(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = lock(&state)?;
    quick_actions::delete_quick_action(&conn, id).map_err(|e| e.to_string())
}

/// Moves a Quick Action to another position in the list, clamped (ticket 50).
#[tauri::command]
fn move_quick_action(
    state: State<'_, AppState>,
    id: i64,
    to_position: i64,
) -> Result<(), String> {
    let conn = lock(&state)?;
    quick_actions::move_quick_action(&conn, id, to_position).map_err(|e| e.to_string())
}

/// Runs one stored Quick Action fire-and-forget (ticket 50): the action's
/// PowerShell command, hidden (`CREATE_NO_WINDOW`), working directory honored
/// when set, spawned on a background thread so the UI never blocks. Current
/// user, no elevation, no status UI, no notification.
#[tauri::command]
fn run_quick_action(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = lock(&state)?;
    let action = quick_actions::get_quick_action(&conn, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            "This quick action is no longer in the list — refresh and try again".to_string()
        })?;
    drop(conn);
    std::thread::spawn(move || {
        let _ = quick_actions::spawn_quick_action(&action.action);
    });
    Ok(())
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

/// The Quick Launch window's × button (ticket 52): remembers the floating
/// window's size and position, then destroys it. Blur and close both hide
/// the window to the tray, and the tray's left-click reopens it.
#[tauri::command]
fn close_quick_launch_window(app: AppHandle) -> Result<(), String> {
    quick_window::close(&app).map_err(|e| e.to_string())
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
            let _ = open_main_window(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .on_window_event(|window, event| {
            // Ticket 43: closing the main window (× or Alt+F4) destroys it —
            // the webview goes away and the lean Rust backend stays resident
            // in the tray. Open Sprout (or a second launch) recreates the
            // window; Quit lives in the tray menu.
            //
            // Ticket 52: the Quick Launch window is a palette — blur hides it
            // (its geometry is remembered, then it is destroyed, keeping the
            // backend lean), and its × button / Alt+F4 take the same path.
            // The tray's left-click reopens it.
            if window.label() == quick_window::QUICK_LAUNCH_WINDOW {
                match event {
                    tauri::WindowEvent::Focused(false) => {
                        quick_window::save_geometry(window);
                        let _ = window.destroy();
                    }
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        quick_window::save_geometry(window);
                        let _ = window.destroy();
                    }
                    _ => {}
                }
                return;
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.destroy();
            }
        })
        .setup(|app| {
            // The tray icon is the resident surface (ticket 43): created at
            // startup, left-click opens the Quick Launch window, right-click
            // menu is Open Sprout / Quit (ticket 54).
            tray::init(app.handle())?;
            Ok(())
        })
        .manage(AppState {
            db: Mutex::new(conn),
            engine: Arc::new(WindowsWingetEngine),
            launcher: Arc::new(engine::windows::WindowsLauncherEngine),
            launch_in_progress: Arc::new(AtomicBool::new(false)),
            pending_import: Mutex::new(pending_import),
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
            list_logs,
            open_folder,
            list_launch_entries,
            create_launch_entry,
            update_launch_entry,
            delete_launch_entry,
            move_launch_entry,
            test_launch_command,
            start_quick_launch,
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
            test_quick_action,
            close_quick_launch_window
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
            if let tauri::RunEvent::ExitRequested { code: None, api, .. } = event {
                if app.tray_by_id(tray::TRAY_ID).is_some() {
                    api.prevent_exit();
                }
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
