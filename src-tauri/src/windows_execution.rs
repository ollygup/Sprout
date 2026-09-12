//! ADR-0029 keeps process lifetime and shell invocation knowledge with their owners.
mod files;
mod process;
mod shell;

pub(crate) use files::{
    cleanup_staged_dir, contains_files_placeholder, expand_files_dir, release_staged_dir,
    stage_action_files, sweep_stale_staged_dirs,
};

pub(crate) use process::{
    action_argv, available_disk_bytes, capture_hidden, capture_powershell, extract_zip_hidden,
    kill_tree, powershell_argv, powershell_output, run_timed_process, run_timed_process_in,
    spawn_action, spawn_action_stop, spawn_owned_hidden, spawn_user_command, system_memory_mb,
    OwnedProcess, ProcessRun,
};
pub use shell::{launch_elevated, open_external};
