use std::{
    fs::File,
    io::{self, BufRead, BufReader, Read},
    os::windows::process::CommandExt,
    path::Path,
    process::{Child, Command, Output, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
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

pub(crate) struct OwnedProcess {
    child: Child,
}

impl OwnedProcess {
    pub(crate) fn exited(&mut self) -> Result<Option<i32>, String> {
        self.child
            .try_wait()
            .map(|status| status.and_then(|value| value.code()))
            .map_err(|error| format!("could not inspect the owned process: {error}"))
    }

    pub(crate) fn stop(&mut self) {
        kill_tree(self.child.id());
        let _ = self.child.wait();
    }
}

pub(crate) fn spawn_owned_hidden(exe: &Path, args: &[String]) -> Result<OwnedProcess, String> {
    let mut command = hidden(Command::new(exe));
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
        .spawn()
        .map(|child| OwnedProcess { child })
        .map_err(|error| format!("failed to start '{}': {error}", exe.display()))
}

fn powershell_literal(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "''"))
}

pub(crate) fn extract_zip_hidden(archive: &Path, destination: &Path) -> Result<(), String> {
    let script = format!(
        "$ErrorActionPreference='Stop'; Expand-Archive -LiteralPath {} -DestinationPath {} -Force",
        powershell_literal(archive),
        powershell_literal(destination)
    );
    powershell_output(&script, Duration::from_secs(120)).map(|_| ())
}

pub(crate) fn available_disk_bytes(path: &Path) -> Result<u64, String> {
    let script = format!(
        "$p={}; $d=Get-Item -LiteralPath $p; [uint64]$d.PSDrive.Free",
        powershell_literal(path)
    );
    powershell_output(&script, Duration::from_secs(15))?
        .trim()
        .parse::<u64>()
        .map_err(|_| "Windows did not report available disk space as a byte count".into())
}

pub(crate) fn system_memory_mb() -> Result<u64, String> {
    powershell_output(
        "[uint64]((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory / 1MB)",
        Duration::from_secs(15),
    )?
    .trim()
    .parse::<u64>()
    .map_err(|_| "Windows did not report physical memory as a megabyte count".into())
}

enum LogAttachment { Required, BestEffort }

pub(crate) fn spawn_action(
    shell: &str,
    script: &str,
    cwd: Option<&str>,
    output: Option<&File>,
) -> Result<Child, String> {
    spawn_script(shell, script, cwd, output, LogAttachment::Required)
}

pub(crate) fn spawn_action_stop(
    shell: &str,
    script: &str,
    cwd: Option<&str>,
    output: Option<&File>,
) -> Result<(), String> {
    spawn_script(
        shell,
        script,
        cwd.map(str::trim).filter(|c| !c.is_empty()),
        output,
        LogAttachment::BestEffort,
    )
    .map(|_| ())
}

fn spawn_script(
    shell: &str,
    script: &str,
    cwd: Option<&str>,
    output: Option<&File>,
    policy: LogAttachment,
) -> Result<Child, String> {
    let (exe, args) = action_argv(shell, script)?;
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
/// command entries, Quick Actions and their Test button, and the engine's own
/// PowerShell calls (bootstrap, verify).
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

/// Builds the argv for CMD's one-liner convention: the same hidden,
/// no-window policy as PowerShell, only the shell differs (ADR-0017 shell
/// extension keeps one execution owner rather than a second runner).
pub(crate) fn cmd_argv(command: &str) -> (String, Vec<String>) {
    ("cmd".into(), vec!["/c".into(), command.into()])
}

/// Resolves a Quick Action shell name to its argv through the single execution
/// owner (ADR-0029): callers pass domain intent (`powershell`/`cmd`), never a
/// reconstructed Windows call. Unknown values fail honestly instead of
/// falling back to PowerShell, so a corrupt record can never run under the
/// wrong shell (ADR-0017 shell extension).
pub(crate) fn action_argv(shell: &str, command: &str) -> Result<(String, Vec<String>), String> {
    match shell {
        "powershell" => Ok(powershell_argv(command)),
        "cmd" => Ok(cmd_argv(command)),
        other => Err(format!(
            "'{other}' is not a supported Quick Action shell — expected 'powershell' or 'cmd'"
        )),
    }
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

#[test]
fn shell_argv_routes_through_the_single_owner() {
    assert_eq!(
        action_argv("powershell", "Write-Output hi").unwrap(),
        (
            "powershell".into(),
            vec![
                "-NoProfile".into(),
                "-NonInteractive".into(),
                "-Command".into(),
                "Write-Output hi".into()
            ]
        )
    );
    assert_eq!(
        action_argv("cmd", "echo hi").unwrap(),
        ("cmd".into(), vec!["/c".into(), "echo hi".into()])
    );
    assert_eq!(cmd_argv("echo hi"), ("cmd".into(), vec!["/c".into(), "echo hi".into()]));
    let err = action_argv("none", "echo hi").unwrap_err();
    assert!(err.contains("not a supported Quick Action shell"), "{err}");
    let err = action_argv("PowerShell", "echo hi").unwrap_err();
    assert!(err.contains("not a supported Quick Action shell"), "{err}");
}
}
