//! ADR-0029 keeps process lifetime and shell invocation knowledge with their owners.
mod process;
mod shell;

pub(crate) use process::{capture_hidden, capture_powershell, kill_tree, powershell_argv,
    powershell_output, run_timed_process, run_timed_process_in, spawn_action,
    spawn_action_stop, spawn_user_command, ProcessRun};
pub use shell::{launch_elevated, open_external};
