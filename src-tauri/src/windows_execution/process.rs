use std::{fs::File, io::{self, BufRead, BufReader, Read}, os::windows::process::CommandExt, process::{Child, Command, Output, Stdio}, sync::{Arc, Mutex}, time::{Duration, Instant}};
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub(crate) fn capture_hidden(exe: &str, args: &[&str]) -> io::Result<Output> {
    hidden(Command::new(exe)).args(args).output()
}

pub(crate) fn capture_powershell(script: &str) -> io::Result<Output> {
    let (exe, args) = powershell_argv(script);
    hidden(Command::new(exe)).args(args).output()
}

pub(crate) fn spawn_user_command(exe: &str, args: &[String], show_window: bool) -> Result<Child, String> {
    let command = Command::new(exe);
    let mut command = if show_window { command } else { hidden(command) };
    command.args(args).spawn().map_err(|e| format!("failed to start '{exe}': {e}"))
}

enum LogAttachment { Required, BestEffort }

pub(crate) fn spawn_action(script: &str, cwd: Option<&str>, output: Option<&File>) -> Result<Child, String> {
    spawn_script(script, cwd, output, LogAttachment::Required)
}

pub(crate) fn spawn_action_stop(script: &str, cwd: Option<&str>, output: Option<&File>) -> Result<(), String> {
    spawn_script(script, cwd.map(str::trim).filter(|c| !c.is_empty()), output, LogAttachment::BestEffort).map(|_| ())
}

fn spawn_script(script: &str, cwd: Option<&str>, output: Option<&File>, policy: LogAttachment) -> Result<Child, String> {
    let (exe, args) = powershell_argv(script);
    let mut command = hidden(Command::new(&exe));
    command.args(args);
    if let Some(cwd) = cwd { command.current_dir(cwd); }
    if let Some(output) = output {
        let (stdout, stderr) = clone_log_handles(|| output.try_clone(), policy)?;
        if let Some(stdout) = stdout { command.stdout(Stdio::from(stdout)); }
        if let Some(stderr) = stderr { command.stderr(Stdio::from(stderr)); }
    }
    command.spawn().map_err(|e| format!("failed to start '{exe}': {e}"))
}

fn clone_log_handles<T>(mut clone: impl FnMut() -> io::Result<T>, policy: LogAttachment) -> Result<(Option<T>, Option<T>), String> {
    let mut attach = || match clone() {
        Ok(handle) => Ok(Some(handle)),
        Err(_) if matches!(policy, LogAttachment::BestEffort) => Ok(None),
        Err(e) => Err(format!("cannot attach the run log: {e}")),
    };
    Ok((attach()?, attach()?))
}

/// Applies `CREATE_NO_WINDOW` to a Command builder before it is spawned.
/// Shared with the Quick Actions runner (ticket 50) — every subprocess in the
/// app carries the flag, so a run never flashes a console window.
fn hidden(mut command: Command) -> Command {
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

/// Builds the argv for PowerShell's non-interactive one-liner convention —
/// the shape every scripted command in the app runs under: launch pipeline
/// command entries (ticket 42), Quick Actions and their Test button (tickets
/// 50 & 62), and the engine's own PowerShell calls (bootstrap, verify).
pub(crate) fn powershell_argv(command: &str) -> (String, Vec<String>) {
    (
        "powershell".into(),
        vec![
            "-NoProfile".into(),
            "-NonInteractive".into(),
            "-Command".into(),
            command.into(),
        ],
    )
}

/// Runs one PowerShell one-liner under a timebox and returns its stdout.
/// Non-zero exits fail loudly with the raw output attached.
pub(crate) fn powershell_output(script: &str, timeout: Duration) -> Result<String, String> {
    let (exe, args) = powershell_argv(script);
    let run = run_timed_process(&exe, &args, timeout);
    if run.timed_out {
        return Err("PowerShell did not finish in time — its processes were killed".to_string());
    }
    match run.exit_code {
        Some(0) => Ok(run.output),
        Some(code) => Err(format!(
            "PowerShell exited {code}: {}",
            run.output.trim()
        )),
        None => Err(format!("PowerShell failed to start: {}", run.output.trim())),
    }
}

/// Result of a timeboxed external step (the port of the legacy
/// `Start-TimedProcess`). stdout and stderr are merged, stderr lines prefixed
/// with `ERR ` exactly like the legacy per-entry logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessRun {
    pub timed_out: bool,
    pub exit_code: Option<i32>,
    pub output: String,
}

/// Runs `exe` with `args` under a per-Requirement timebox. If the process
/// outlives the box its whole tree is killed via `taskkill /T /F` (the legacy
/// runner's behavior) and the run is recorded as timed out — a hung installer
/// must never wedge the machine.
pub fn run_timed_process(exe: &str, args: &[String], timeout: Duration) -> ProcessRun {
    run_timed_process_in(None, exe, args, timeout)
}

/// The same as [`run_timed_process`] with an explicit working directory —
/// `cwd` `None` inherits the caller's. Shared with the Quick Actions Test
/// (ticket 50), whose commands honor their configured directory.
pub fn run_timed_process_in(
    cwd: Option<&str>,
    exe: &str,
    args: &[String],
    timeout: Duration,
) -> ProcessRun {
    let mut command = hidden(Command::new(exe));
    command.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            return ProcessRun {
                timed_out: false,
                exit_code: None,
                output: format!("failed to start: {e}"),
            }
        }
    };

    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");
    let output = Arc::new(Mutex::new(String::new()));

    let out = Arc::clone(&output);
    let reader = std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        let mut reader = stdout;
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => out
                    .lock()
                    .expect("output lock")
                    .push_str(&String::from_utf8_lossy(&buf[..n])),
            }
        }
    });
    let out = Arc::clone(&output);
    let err_reader = std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            out.lock()
                .expect("output lock")
                .push_str(&format!("ERR {line}\n"));
        }
    });

    let deadline = Instant::now() + timeout;
    let mut timed_out = false;
    let exit_code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) => {}
            Err(_) => break None,
        }
        if Instant::now() >= deadline {
            timed_out = true;
            kill_tree(child.id());
            let _ = child.wait();
            output
                .lock()
                .expect("output lock")
                .push_str(&format!(
                    "\n[TIMED OUT after {}s - killed]\n",
                    timeout.as_secs()
                ));
            break None;
        }
        std::thread::sleep(Duration::from_millis(200));
    };

    // Drain the reader threads before reading the merged output.
    let _ = reader.join();
    let _ = err_reader.join();
    let output = Arc::into_inner(output)
        .expect("reader threads joined")
        .into_inner()
        .expect("output lock");

    ProcessRun {
        timed_out,
        exit_code,
        output,
    }
}

/// Kills a process and its whole tree (`taskkill /T`), as the legacy runner
/// did on timebox expiry. Shared with the Quick Action Stop (ticket 62),
/// whose no-stop-command actions die the same way.
pub(crate) fn kill_tree(pid: u32) {
    let _ = hidden(Command::new("taskkill"))
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status();
}

#[cfg(test)]
mod tests {use super::*;
#[test]
fn log_clone_failure_aborts_run_but_stop_attempts_both_streams() {
    let mut calls = 0;
    let mut clone = || -> io::Result<()> { calls += 1; Err(io::Error::other("unavailable")) };
    assert!(clone_log_handles(&mut clone, LogAttachment::Required).unwrap_err().contains("cannot attach the run log"));
    assert_eq!(calls, 1);
    let mut calls = 0;
    let handles = clone_log_handles(|| { calls += 1; if calls == 1 { Err(io::Error::other("stdout")) } else { Ok(2) } }, LogAttachment::BestEffort).unwrap();
    assert_eq!(handles, (None, Some(2)));
    assert_eq!(calls, 2);
}
#[test]
fn timebox_kills_a_runaway_process() {
    let run = run_timed_process(
        "powershell",
        &[
            "-NoProfile".to_string(),
            "-Command".to_string(),
            "Start-Sleep -Seconds 30".to_string(),
        ],
        Duration::from_secs(2),
    );
    assert!(run.timed_out);
    assert_eq!(run.exit_code, None);
    assert!(run.output.contains("TIMED OUT"), "{}", run.output);
}

#[test]
fn completed_process_reports_exit_code_and_output() {
    let run = run_timed_process(
        "cmd",
        &["/c".to_string(), "echo".to_string(), "hello-sprout".to_string()],
        Duration::from_secs(30),
    );
    assert!(!run.timed_out);
    assert_eq!(run.exit_code, Some(0));
    assert!(run.output.contains("hello-sprout"), "{}", run.output);
}

#[test]
fn missing_executable_is_a_clean_failure() {
    let run = run_timed_process(
        "no-such-binary-sprout-test",
        &[],
        Duration::from_secs(5),
    );
    assert!(!run.timed_out);
    assert_eq!(run.exit_code, None);
    assert!(run.output.contains("failed to start"));
}
}
