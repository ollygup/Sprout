// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // The run phase relaunches this exe as `--worker --run <id>` (ADR-0003):
    // the worker shows no window, executes the Plan from the per-run request
    // file, streams progress to the status file, and persists the Run — then
    // exits. Everything else takes the normal Tauri path.
    if std::env::args().any(|arg| arg == "--worker") {
        sprout_lib::run_worker();
        return;
    }
    sprout_lib::run()
}
