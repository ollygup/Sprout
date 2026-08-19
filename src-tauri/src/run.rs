//! Run orchestration (tickets 05-06; spec decisions 6-7; CONTEXT.md "Run").
//!
//! One application of a Plan to this machine: Requirements execute in
//! dependency-first order, each under its per-Requirement timebox (killed and
//! recorded as timed-out if exceeded), winget install/upgrade outcomes are
//! classified with the engine's exit-code whitelist, and every Requirement's
//! outcome is persisted with the Run.
//!
//! The loop is engine-driven (never hardcodes winget): detection comes in as
//! data and every mutating call goes through `PlatformEngine`, so the
//! elevated worker (ticket 06) reuses this pipeline unchanged — the worker
//! only changes *who* calls it and how progress streams out. `execute_run` is
//! the plain entry point (tests, dev runs); `execute_run_observed` is the
//! same loop with a progress callback and a cancel check that the worker
//! drives through the per-run status file — the ordering, planning, step
//! execution, and persistence logic is one code path. Re-running the same
//! Plan re-detects first, so already-satisfied Requirements are recorded as
//! already OK and never re-executed.
//!
//! Before detection the engine gets a `prepare` call (ticket 08): the
//! Windows implementation bootstraps winget when it is missing and the run
//! needs it — an `Err` aborts the Run with its message, so an unsupported OS
//! build is a clear failure, never a cascade of per-Requirement errors.
//!
//! A successful step is followed by env wiring (User scope only, never
//! overwriting; applied notes and skip reasons go into the Requirement's
//! detail) and then the Requirement's verify commands — a non-zero exit or
//! non-matching output fails the Requirement loudly (ticket 07).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::domain::{default_timeout_minutes, Product, Requirement, Step, VersionPolicy};
use crate::engine::{Detection, PlatformEngine, StepOutcome};
use crate::plan::{plan_requirement, PlannedAction};

/// Overall outcome of a Run, derived from its per-Requirement results
/// (ticket 16): failed when any Requirement failed or timed out; with notes
/// when the run completed but something needed attention — an unmanaged
/// install was detected and skipped, and the run is never reported clean
/// while that stands; cancelled when the user aborted the Plan between
/// Requirements (the in-flight step always completes first — never killed
/// mid-install).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunOutcome {
    Ok,
    WithNotes,
    Failed,
    Cancelled,
}

/// The per-Requirement result recorded with the Run — the categories the
/// summary screen groups by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Installed,
    Upgraded,
    AlreadyOk,
    SatisfiedByNewer,
    SkippedUnmanaged,
    Failed,
    TimedOut,
}

/// One Requirement's outcome inside a Run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequirementOutcome {
    pub product_id: String,
    pub product_name: String,
    pub status: RunStatus,
    pub detail: String,
    /// The step succeeded but winget asked for a restart to finish.
    pub reboot_required: bool,
    /// Absolute path to the raw output log written for this Requirement.
    pub log_path: String,
}

/// One application of a Plan, persisted to the Library and returned to the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRecord {
    pub id: String,
    pub started_at: i64,
    pub finished_at: i64,
    pub preset_names: Vec<String>,
    pub outcome: RunOutcome,
    pub results: Vec<RequirementOutcome>,
}

/// One row of the Runs list (ticket 09): everything the History screen shows
/// without the per-Requirement detail, which loads on demand via `get_run`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunSummary {
    pub id: String,
    pub started_at: i64,
    pub finished_at: i64,
    pub preset_names: Vec<String>,
    pub outcome: RunOutcome,
}

/// A fresh, collision-resistant Run id (`run-<epoch millis>`).
pub fn new_run_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("run-{millis}")
}

/// Synthesizes the default Requirement for a Product (ticket 17, quick
/// install): latest version policy, the Product's winget step in machine
/// scope, and its default env wiring. A Product without a usable step — no
/// winget id — is a clear error, never a silent success (nothing to run).
pub fn synthesize_quick_requirement(product: &Product) -> Result<Requirement, String> {
    let Some(winget_id) = &product.winget_id else {
        return Err(format!(
            "Product '{}' has no installable step — it has no winget package id, so there is nothing to run. Add a winget id to the product, or install it through a preset instead",
            product.id
        ));
    };
    Ok(Requirement {
        product: product.clone(),
        step: Step::Winget {
            id: winget_id.clone(),
            scope: crate::domain::default_machine_scope(),
        },
        version_policy: VersionPolicy::Latest,
        depends_on: vec![],
        timeout_minutes: default_timeout_minutes(),
        env: product.default_env.clone(),
        verify: vec![],
        unresolved: false,
    })
}

/// One JSON-lines progress record the worker appends to the per-run status
/// file (ADR-0003): the main process tails it and the UI renders it live.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProgressEvent {
    /// A coarse phase, e.g. `detecting` before the machine snapshot.
    Phase { phase: String },
    /// A Requirement is about to execute (`index` is 0-based within `total`).
    RequirementStarted {
        index: usize,
        total: usize,
        product_id: String,
        product_name: String,
        action: String,
    },
    /// A Requirement finished; the outcome fields are the persisted ones.
    RequirementFinished(RequirementOutcome),
    /// The Run is over, with its overall outcome.
    RunFinished { outcome: RunOutcome },
}

/// Executes the Plan: fresh detection (so re-runs skip what is already
/// satisfied), dependency-first ordering, one timeboxed step per Requirement,
/// and a per-Requirement outcome. `logs_dir` is the per-run log directory the
/// raw outputs land in. Never touches the database — persistence is the
/// caller's job, so the worker (ticket 06) and the dev-mode command share it.
/// The worker calls [`execute_run_observed`] instead; this plain form remains
/// as the tests' entry point into the identical pipeline.
#[allow(dead_code)]
pub fn execute_run(
    engine: &dyn PlatformEngine,
    run_id: &str,
    preset_names: &[String],
    requirements: &[Requirement],
    logs_dir: &Path,
    install_dir: Option<&str>,
) -> Result<RunRecord, String> {
    execute_run_observed(
        engine,
        run_id,
        preset_names,
        requirements,
        logs_dir,
        install_dir,
        &mut |_| {},
        &mut || false,
    )
}

/// The same pipeline as [`execute_run`], with two hooks the worker (ticket
/// 06) needs: `on_progress` receives one event per Requirement (and one final
/// RunFinished), and `should_cancel` is checked before every Requirement — a
/// cancel request aborts the Plan between steps, never mid-install, and the
/// Run is recorded as cancelled with the results completed so far. There is
/// no second copy of the loop: this is the one code path.
pub fn execute_run_observed(
    engine: &dyn PlatformEngine,
    run_id: &str,
    preset_names: &[String],
    requirements: &[Requirement],
    logs_dir: &Path,
    install_dir: Option<&str>,
    on_progress: &mut dyn FnMut(ProgressEvent),
    should_cancel: &mut dyn FnMut() -> bool,
) -> Result<RunRecord, String> {
    let started_at = now_epoch();
    std::fs::create_dir_all(logs_dir)
        .map_err(|e| format!("cannot create the run log directory: {e}"))?;

    on_progress(ProgressEvent::Phase {
        phase: "preparing".into(),
    });
    let references: Vec<&Requirement> = requirements.iter().collect();
    engine.prepare(&references)?;

    on_progress(ProgressEvent::Phase {
        phase: "detecting".into(),
    });
    let detections = engine.detect_many(&references);

    let total = requirements.len();
    let mut results = Vec::new();
    let mut cancelled = false;
    for &index in &dependency_order(requirements) {
        if should_cancel() {
            cancelled = true;
            break;
        }
        let requirement = &requirements[index];
        let detection = detections
            .get(&requirement.product.id)
            .cloned()
            .unwrap_or_default();
        on_progress(ProgressEvent::RequirementStarted {
            index: results.len(),
            total,
            product_id: requirement.product.id.clone(),
            product_name: requirement.product.name.clone(),
            action: step_action_label(&detection, requirement),
        });
        let outcome = execute_one(engine, requirement, &detection, logs_dir, install_dir);
        on_progress(ProgressEvent::RequirementFinished(outcome.clone()));
        results.push(outcome);
    }

    let outcome = if cancelled {
        RunOutcome::Cancelled
    } else if results
        .iter()
        .any(|r| matches!(r.status, RunStatus::Failed | RunStatus::TimedOut))
    {
        RunOutcome::Failed
    } else if results
        .iter()
        .any(|r| matches!(r.status, RunStatus::SkippedUnmanaged))
    {
        RunOutcome::WithNotes
    } else {
        RunOutcome::Ok
    };
    on_progress(ProgressEvent::RunFinished { outcome });

    Ok(RunRecord {
        id: run_id.to_string(),
        started_at,
        finished_at: now_epoch(),
        preset_names: preset_names.to_vec(),
        outcome,
        results,
    })
}

/// What the step is about to do, for the live progress line — the same plan
/// logic `execute_one` will run, so the label never disagrees with the action.
fn step_action_label(detection: &Detection, requirement: &Requirement) -> String {
    match plan_requirement(detection, requirement).0 {
        PlannedAction::Install => "install".into(),
        PlannedAction::Upgrade { .. } => "upgrade".into(),
        PlannedAction::AlreadyOk => "already ok".into(),
        PlannedAction::SatisfiedByNewer { .. } => "satisfied by newer".into(),
        PlannedAction::UnmanagedSkip => "skip".into(),
    }
}

/// Executes one Requirement against its fresh Detection: the plan decides
/// what happens (never a downgrade, skips what is already satisfied), the
/// engine does it under the Requirement's timebox, and a successful step is
/// followed by env wiring and the verify commands. `install_dir` is the
/// machine-local default install directory the winget steps carry
/// `--location` for (ticket 34, ADR-0009); a Product's own install directory
/// (ticket 36) overrides it per Requirement.
fn execute_one(
    engine: &dyn PlatformEngine,
    requirement: &Requirement,
    detection: &Detection,
    logs_dir: &Path,
    install_dir: Option<&str>,
) -> RequirementOutcome {
    let product_id = requirement.product.id.clone();
    let product_name = requirement.product.name.clone();
    let log_path = logs_dir.join(format!("{product_id}.log"));
    let effective_dir = requirement.product.install_dir.as_deref().or(install_dir);

    let (action, plan_detail) = plan_requirement(detection, requirement);
    match action {
        PlannedAction::Install => {
            let outcome = engine.install(
                &requirement.step,
                requirement.timeout_minutes,
                effective_dir,
            );
            finish_step(
                engine,
                requirement,
                outcome,
                RunStatus::Installed,
                log_path,
                effective_dir,
            )
        }
        PlannedAction::Upgrade { .. } => {
            let outcome = engine.upgrade(
                &requirement.step,
                requirement.timeout_minutes,
                effective_dir,
            );
            finish_step(
                engine,
                requirement,
                outcome,
                RunStatus::Upgraded,
                log_path,
                effective_dir,
            )
        }
        PlannedAction::AlreadyOk => RequirementOutcome {
            product_id,
            product_name,
            status: RunStatus::AlreadyOk,
            detail: plan_detail,
            reboot_required: false,
            log_path: String::new(),
        },
        PlannedAction::SatisfiedByNewer { .. } => RequirementOutcome {
            product_id,
            product_name,
            status: RunStatus::SatisfiedByNewer,
            detail: plan_detail,
            reboot_required: false,
            log_path: String::new(),
        },
        PlannedAction::UnmanagedSkip => RequirementOutcome {
            product_id,
            product_name,
            status: RunStatus::SkippedUnmanaged,
            detail: plan_detail,
            reboot_required: false,
            log_path: String::new(),
        },
    }
}

/// A successful step is followed by the two post-install phases (ticket 07):
/// env wiring first (its notes — applied values and skip reasons — go into
/// the detail and never fail the Requirement), then the verify commands, one
/// of which failing fails the Requirement loudly. Nothing runs when the step
/// itself failed or timed out. When a directory was requested (ticket 34 +
/// per-product overrides from ticket 36, ADR-0009), a successful step also
/// reports where the product actually landed — an installer that ignored
/// `--location` is called out honestly, and nothing is fabricated when the
/// location cannot be resolved.
fn finish_step(
    engine: &dyn PlatformEngine,
    requirement: &Requirement,
    outcome: StepOutcome,
    success: RunStatus,
    log_path: PathBuf,
    install_dir: Option<&str>,
) -> RequirementOutcome {
    let mut record = finalize_step(
        &requirement.product.id,
        &requirement.product.name,
        outcome,
        success.clone(),
        log_path,
    );
    if record.status != success {
        return record;
    }

    if let Some(requested) = install_dir {
        if let Some(actual) = engine.actual_install_location(&requirement.product) {
            if !same_directory(&actual, requested) {
                record.detail.push_str(&format!(
                    "\ninstalled to {actual} (installer ignored the requested directory)"
                ));
            }
        }
    }

    for note in engine.apply_env_wiring(&requirement.product, &requirement.env) {
        record.detail.push('\n');
        record.detail.push_str(&note);
    }

    for check in &requirement.verify {
        let report = engine.verify(check);
        append_to_log(&record.log_path, &report.log);
        if report.ok {
            record.detail.push_str(&format!("\nverify: {}", report.detail));
        } else {
            record.status = RunStatus::Failed;
            record.detail = format!("verify failed: {}", report.detail);
            break;
        }
    }
    record
}

/// Appends a verify command's raw output to the Requirement's log, so a
/// failed verification is debuggable from the same file as the install.
fn append_to_log(log_path: &str, output: &str) {
    if log_path.is_empty() || output.is_empty() {
        return;
    }
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(log_path) {
        let _ = writeln!(file, "\n--- verify ---");
        let _ = write!(file, "{output}");
    }
}

/// Turns a step's outcome into a Requirement outcome: writes the raw output
/// to the per-run log, and maps timed out / failed / ok onto the RunStatuses
/// the summary groups by. Env wiring and verify commands are layered on top
/// of a successful outcome by [`finish_step`].
fn finalize_step(
    product_id: &str,
    product_name: &str,
    outcome: StepOutcome,
    success: RunStatus,
    log_path: PathBuf,
) -> RequirementOutcome {
    if !outcome.log.is_empty() {
        let _ = std::fs::write(&log_path, &outcome.log);
    }
    let (status, detail) = if outcome.timed_out {
        (RunStatus::TimedOut, outcome.detail)
    } else if outcome.ok {
        (success, outcome.detail)
    } else {
        (RunStatus::Failed, outcome.detail)
    };
    // A failed step's log must carry the honest verdict, not just the raw
    // output it ends with (ticket 16): the same reason the summary screen
    // shows travels with the log.
    if matches!(status, RunStatus::Failed | RunStatus::TimedOut) {
        append_summary_to_log(&log_path, &detail);
    }
    RequirementOutcome {
        product_id: product_id.to_string(),
        product_name: product_name.to_string(),
        status,
        detail,
        reboot_required: outcome.reboot_required,
        log_path: log_path.to_string_lossy().into_owned(),
    }
}

/// Appends the failure reason to a Requirement's raw log under a `--- sprout
/// ---` marker, so the file always ends by naming its verdict.
fn append_summary_to_log(log_path: &Path, summary: &str) {
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
    {
        let _ = writeln!(file, "\n--- sprout ---");
        let _ = writeln!(file, "{summary}");
    }
}

/// Dependencies first: a stable topological ordering of the Requirements by
/// `depends_on`, following the selection order for everything else. Cycle-safe
/// (a dependency that is already scheduled is left in place) and ignores
/// dependencies that are not part of this run (the user may have toggled one
/// out — the Requirement still runs).
pub fn dependency_order(requirements: &[Requirement]) -> Vec<usize> {
    let mut by_id: HashMap<&str, usize> = HashMap::new();
    for (i, requirement) in requirements.iter().enumerate() {
        by_id.insert(requirement.product.id.as_str(), i);
    }
    let mut ordered = Vec::new();
    let mut visited = HashSet::new();
    for i in 0..requirements.len() {
        visit_dependencies(i, requirements, &by_id, &mut visited, &mut ordered);
    }
    ordered
}

fn visit_dependencies(
    i: usize,
    requirements: &[Requirement],
    by_id: &HashMap<&str, usize>,
    visited: &mut HashSet<usize>,
    ordered: &mut Vec<usize>,
) {
    if !visited.insert(i) {
        return;
    }
    for dep in &requirements[i].depends_on {
        if let Some(&j) = by_id.get(dep.as_str()) {
            visit_dependencies(j, requirements, by_id, visited, ordered);
        }
    }
    ordered.push(i);
}

/// Do two directory paths name the same place? Case-insensitive and blind to
/// trailing separators (`D:\Apps` == `d:\apps\`), the way Windows treats
/// them. A drive root keeps its separator (`C:\` never collapses to `C:`).
fn same_directory(a: &str, b: &str) -> bool {
    fn normalize(path: &str) -> String {
        let trimmed = path.trim();
        let stripped = trimmed.trim_end_matches(['\\', '/']);
        if stripped.is_empty() || (stripped.len() == 2 && stripped.as_bytes()[1] == b':') {
            trimmed.to_string()
        } else {
            stripped.to_string()
        }
    }
    normalize(a).eq_ignore_ascii_case(&normalize(b))
}

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EnvAction, EnvWiring, Product, Step, VerifyCommand, VersionPolicy};
    use crate::engine::VerifyOutcome;

    struct FakeEngine {
        detections: Mutex<HashMap<String, Detection>>,
        installs: Mutex<Vec<(String, u32, Option<String>)>>,
        upgrades: Mutex<Vec<(String, u32, Option<String>)>>,
        install_result: StepOutcome,
        upgrade_result: StepOutcome,
        /// Every verify command invoked, in order (raw command text).
        verified: Mutex<Vec<String>>,
        /// Verify outcomes keyed by the raw command text; unknown commands
        /// pass.
        verify_results: Mutex<HashMap<String, VerifyOutcome>>,
        /// Every env wiring applied, as (product id, [wiring names]).
        env_wiring: Mutex<Vec<(String, Vec<String>)>>,
        env_notes: Vec<String>,
        /// The install location reported for a product, keyed by id (ticket
        /// 34's post-install honesty check); unknown products report none.
        actual_locations: Mutex<HashMap<String, String>>,
        /// `prepare` calls; `prepare_error` makes the next one fail.
        prepares: Mutex<u32>,
        prepare_error: Option<String>,
    }

    use std::sync::Mutex;

    impl FakeEngine {
        fn new() -> Self {
            FakeEngine {
                detections: Mutex::new(HashMap::new()),
                installs: Mutex::new(Vec::new()),
                upgrades: Mutex::new(Vec::new()),
                install_result: StepOutcome {
                    ok: true,
                    reboot_required: false,
                    timed_out: false,
                    detail: "installed".into(),
                    log: String::new(),
                },
                upgrade_result: StepOutcome {
                    ok: true,
                    reboot_required: false,
                    timed_out: false,
                    detail: "upgraded".into(),
                    log: String::new(),
                },
                verified: Mutex::new(Vec::new()),
                verify_results: Mutex::new(HashMap::new()),
                env_wiring: Mutex::new(Vec::new()),
                env_notes: Vec::new(),
                actual_locations: Mutex::new(HashMap::new()),
                prepares: Mutex::new(0),
                prepare_error: None,
            }
        }

        fn with_detection(mut self, product: &str, detection: Detection) -> Self {
            self.detections
                .get_mut()
                .unwrap()
                .insert(product.to_string(), detection);
            self
        }

        fn with_env_notes(mut self, notes: &[&str]) -> Self {
            self.env_notes = notes.iter().map(|s| s.to_string()).collect();
            self
        }

        fn with_prepare_error(mut self, message: &str) -> Self {
            self.prepare_error = Some(message.to_string());
            self
        }

        fn prepares(&self) -> u32 {
            *self.prepares.lock().unwrap()
        }

        fn with_verify_result(mut self, command: &str, outcome: VerifyOutcome) -> Self {
            self.verify_results
                .get_mut()
                .unwrap()
                .insert(command.to_string(), outcome);
            self
        }

        /// Where the product reports it landed, for the post-install honesty
        /// check (ticket 34).
        fn with_actual_location(mut self, product: &str, location: &str) -> Self {
            self.actual_locations
                .get_mut()
                .unwrap()
                .insert(product.to_string(), location.to_string());
            self
        }

        fn installs(&self) -> Vec<(String, u32, Option<String>)> {
            self.installs.lock().unwrap().clone()
        }

        fn upgrades(&self) -> Vec<(String, u32, Option<String>)> {
            self.upgrades.lock().unwrap().clone()
        }

        fn verified(&self) -> Vec<String> {
            self.verified.lock().unwrap().clone()
        }

        fn env_wiring(&self) -> Vec<(String, Vec<String>)> {
            self.env_wiring.lock().unwrap().clone()
        }
    }

    impl PlatformEngine for FakeEngine {
        fn prepare(&self, _requirements: &[&Requirement]) -> Result<(), String> {
            *self.prepares.lock().unwrap() += 1;
            match &self.prepare_error {
                Some(message) => Err(message.clone()),
                None => Ok(()),
            }
        }

        fn detect_many(&self, _requirements: &[&Requirement]) -> HashMap<String, Detection> {
            self.detections.lock().unwrap().clone()
        }

        fn install(
            &self,
            step: &Step,
            timeout_minutes: u32,
            install_dir: Option<&str>,
        ) -> StepOutcome {
            if let Step::Winget { id, .. } = step {
                self.installs
                    .lock()
                    .unwrap()
                    .push((id.clone(), timeout_minutes, install_dir.map(str::to_string)));
            }
            self.install_result.clone()
        }

        fn upgrade(
            &self,
            step: &Step,
            timeout_minutes: u32,
            install_dir: Option<&str>,
        ) -> StepOutcome {
            if let Step::Winget { id, .. } = step {
                self.upgrades
                    .lock()
                    .unwrap()
                    .push((id.clone(), timeout_minutes, install_dir.map(str::to_string)));
            }
            self.upgrade_result.clone()
        }

        fn actual_install_location(&self, product: &Product) -> Option<String> {
            self.actual_locations
                .lock()
                .unwrap()
                .get(&product.id)
                .cloned()
        }

        fn verify(&self, command: &VerifyCommand) -> VerifyOutcome {
            self.verified
                .lock()
                .unwrap()
                .push(command.command.clone());
            self.verify_results
                .lock()
                .unwrap()
                .get(&command.command)
                .cloned()
                .unwrap_or_else(|| VerifyOutcome::passed(format!("'{}' exited 0", command.command), String::new()))
        }

        fn apply_env_wiring(&self, product: &Product, env: &[EnvWiring]) -> Vec<String> {
            self.env_wiring.lock().unwrap().push((
                product.id.clone(),
                env.iter().map(|w| w.name.clone()).collect(),
            ));
            self.env_notes.clone()
        }
    }

    fn winget_req(id: &str, policy: VersionPolicy, timeout_minutes: u32) -> Requirement {
        Requirement {
            product: Product {
                id: id.into(),
                name: format!("Product {id}"),
                winget_id: Some(format!("Vendor.{id}")),
                install_location_hint: None,
                install_dir: None,
                default_env: vec![],
            },
            step: Step::Winget {
                id: format!("Vendor.{id}"),
                scope: "machine".into(),
            },
            version_policy: policy,
            depends_on: vec![],
            timeout_minutes,
            env: vec![],
            verify: vec![],
            unresolved: false,
        }
    }

    fn detection(installed: bool, managed: bool, installed_version: Option<&str>) -> Detection {
        Detection {
            installed,
            winget_managed: managed,
            installed_version: installed_version.map(str::to_string),
            available_version: None,
        }
    }

    fn installed_latest(_id: &str) -> Detection {
        detection(true, true, Some("1.0.0"))
    }

    fn run(engine: &FakeEngine, requirements: &[Requirement]) -> RunRecord {
        let dir = tempfile::tempdir().unwrap();
        execute_run(
            engine,
            "run-test",
            &["Preset A".into()],
            requirements,
            dir.path(),
            None,
        )
        .unwrap()
    }

    fn status_of<'a>(run: &'a RunRecord, id: &str) -> &'a RunStatus {
        &run.results
            .iter()
            .find(|r| r.product_id == id)
            .unwrap()
            .status
    }

    #[test]
    fn dependency_order_puts_dependencies_first() {
        let mut a = winget_req("a", VersionPolicy::Latest, 10);
        a.depends_on = vec!["b".into(), "c".into()];
        let mut b = winget_req("b", VersionPolicy::Latest, 10);
        b.depends_on = vec!["c".into()];
        let c = winget_req("c", VersionPolicy::Latest, 10);
        let requirements = vec![a.clone(), b.clone(), c.clone()];
        let order = dependency_order(&requirements);
        let ids: Vec<&str> = order
            .iter()
            .map(|&i| requirements[i].product.id.as_str())
            .collect();
        assert_eq!(ids, vec!["c", "b", "a"]);
    }

    #[test]
    fn dependency_order_is_stable_and_cycle_safe() {
        let mut a = winget_req("a", VersionPolicy::Latest, 10);
        a.depends_on = vec!["b".into()];
        let mut b = winget_req("b", VersionPolicy::Latest, 10);
        b.depends_on = vec!["a".into()]; // cycle — must still terminate
        let c = winget_req("c", VersionPolicy::Latest, 10);
        let requirements = vec![a.clone(), b.clone(), c.clone()];
        let order = dependency_order(&requirements);
        let ids: Vec<&str> = order
            .iter()
            .map(|&i| requirements[i].product.id.as_str())
            .collect();
        assert_eq!(ids, vec!["b", "a", "c"]);
    }

    #[test]
    fn dependency_order_ignores_dependencies_outside_the_run() {
        let mut a = winget_req("a", VersionPolicy::Latest, 10);
        a.depends_on = vec!["not-in-run".into()];
        let b = winget_req("b", VersionPolicy::Latest, 10);
        let requirements = vec![a.clone(), b.clone()];
        let order = dependency_order(&requirements);
        assert_eq!(order, vec![0, 1]);
    }

    #[test]
    fn installs_in_dependency_order() {
        let b = winget_req("b", VersionPolicy::Latest, 10);
        let mut a = winget_req("a", VersionPolicy::Latest, 10);
        a.depends_on = vec!["b".into()];
        let engine = FakeEngine::new();
        let record = run(&engine, &[a.clone(), b.clone()]);
        assert_eq!(record.outcome, RunOutcome::Ok);
        assert_eq!(status_of(&record, "a"), &RunStatus::Installed);
        assert_eq!(status_of(&record, "b"), &RunStatus::Installed);
        // Dependency executed first, each under its own timebox.
        assert_eq!(
            engine.installs(),
            vec![
                ("Vendor.b".to_string(), 10, None),
                ("Vendor.a".to_string(), 10, None)
            ]
        );
    }

    #[test]
    fn already_satisfied_requirements_are_skipped_on_rerun() {
        let engine = FakeEngine::new().with_detection("a", installed_latest("a"));
        let a = winget_req("a", VersionPolicy::Latest, 10);
        let record = run(&engine, &[a.clone()]);
        assert_eq!(record.outcome, RunOutcome::Ok);
        assert_eq!(status_of(&record, "a"), &RunStatus::AlreadyOk);
        assert!(engine.installs().is_empty(), "nothing should have installed");
    }

    #[test]
    fn present_and_newer_are_never_re_executed() {
        let engine = FakeEngine::new().with_detection(
            "pin",
            detection(true, true, Some("2.0.0")), // newer than pinned 1.0.0
        );
        let pin = winget_req("pin", VersionPolicy::Pinned { version: "1.0.0".into() }, 10);
        let record = run(&engine, &[pin.clone()]);
        assert_eq!(status_of(&record, "pin"), &RunStatus::SatisfiedByNewer);
        assert!(engine.installs().is_empty());
        assert!(engine.upgrades().is_empty());

        let engine = FakeEngine::new().with_detection("pres", installed_latest("pres"));
        let present = winget_req("pres", VersionPolicy::Present, 10);
        let record = run(&engine, &[present.clone()]);
        assert_eq!(status_of(&record, "pres"), &RunStatus::AlreadyOk);
    }

    #[test]
    fn unmanaged_installs_are_skipped_with_a_note() {
        let engine = FakeEngine::new().with_detection(
            "unmanaged",
            detection(true, false, None),
        );
        let requirement = winget_req("unmanaged", VersionPolicy::Latest, 10);
        let record = run(&engine, &[requirement.clone()]);
        assert_eq!(status_of(&record, "unmanaged"), &RunStatus::SkippedUnmanaged);
        assert_eq!(record.outcome, RunOutcome::WithNotes);
        assert!(engine.installs().is_empty());
        assert!(record.results[0].detail.contains("outside winget"));
    }

    #[test]
    fn run_outcome_derivation_covers_all_four_tiers() {
        // Applied — every requirement applied or was already satisfied.
        let engine = FakeEngine::new().with_detection("a", installed_latest("a"));
        let a = winget_req("a", VersionPolicy::Latest, 10);
        let record = run(&engine, &[a.clone()]);
        assert_eq!(record.outcome, RunOutcome::Ok);
        assert_eq!(status_of(&record, "a"), &RunStatus::AlreadyOk);

        // With notes — a success next to an unmanaged skip is never "clean":
        // the attention state must not hide behind the green.
        let engine = FakeEngine::new().with_detection(
            "u",
            detection(true, false, None),
        );
        let a = winget_req("a", VersionPolicy::Latest, 10);
        let u = winget_req("u", VersionPolicy::Latest, 10);
        let record = run(&engine, &[a.clone(), u.clone()]);
        assert_eq!(record.outcome, RunOutcome::WithNotes);
        assert_eq!(status_of(&record, "a"), &RunStatus::Installed);
        assert_eq!(status_of(&record, "u"), &RunStatus::SkippedUnmanaged);

        // Failed dominates — a failed requirement next to an unmanaged skip
        // must report failed, never the softer attention tier.
        let mut engine = FakeEngine::new().with_detection(
            "u",
            detection(true, false, None),
        );
        engine.install_result.ok = false;
        engine.install_result.detail = "install failed (exit 5) — not installed".into();
        let bad = winget_req("bad", VersionPolicy::Latest, 10);
        let u = winget_req("u", VersionPolicy::Latest, 10);
        let record = run(&engine, &[bad.clone(), u.clone()]);
        assert_eq!(record.outcome, RunOutcome::Failed);
        assert_eq!(status_of(&record, "bad"), &RunStatus::Failed);

        // Cancelled is covered by the dedicated cancel tests above.
    }

    #[test]
    fn upgrade_flow_calls_upgrade_and_records_reboot() {
        let mut engine = FakeEngine::new().with_detection(
            "old",
            Detection {
                installed: true,
                winget_managed: true,
                installed_version: Some("1.0.0".into()),
                available_version: Some("1.1.0".into()),
            },
        );
        engine.upgrade_result.reboot_required = true;
        let requirement = winget_req("old", VersionPolicy::Latest, 7);
        let record = run(&engine, &[requirement.clone()]);
        assert_eq!(status_of(&record, "old"), &RunStatus::Upgraded);
        assert_eq!(engine.upgrades(), vec![("Vendor.old".to_string(), 7, None)]);
        assert!(record.results[0].reboot_required);
    }

    #[test]
    fn failed_steps_fail_the_run_with_the_engine_detail() {
        let mut engine = FakeEngine::new().with_detection("bad", Detection::absent());
        engine.install_result.ok = false;
        engine.install_result.detail = "install failed (exit 5) — not installed".into();
        let requirement = winget_req("bad", VersionPolicy::Latest, 10);
        let record = run(&engine, &[requirement.clone()]);
        assert_eq!(status_of(&record, "bad"), &RunStatus::Failed);
        assert_eq!(record.outcome, RunOutcome::Failed);
        assert!(record.results[0].detail.contains("exit 5"));
    }

    #[test]
    fn timed_out_steps_are_recorded_and_fail_the_run() {
        let mut engine = FakeEngine::new().with_detection("hung", Detection::absent());
        engine.install_result.ok = false;
        engine.install_result.timed_out = true;
        engine.install_result.detail = "install did not finish in 10 min — its processes were killed".into();
        let requirement = winget_req("hung", VersionPolicy::Latest, 10);
        let record = run(&engine, &[requirement.clone()]);
        assert_eq!(status_of(&record, "hung"), &RunStatus::TimedOut);
        assert_eq!(record.outcome, RunOutcome::Failed);
    }

    #[test]
    fn raw_output_is_written_to_the_per_run_log() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = FakeEngine::new().with_detection("git", Detection::absent());
        engine.install_result.log = "downloading…\ndone".into();
        let requirement = winget_req("git", VersionPolicy::Latest, 10);
        let record = execute_run(
            &engine,
            "run-test",
            &[],
            &[requirement.clone()],
            dir.path(),
            None,
        )
        .unwrap();
        let log = std::fs::read_to_string(&record.results[0].log_path).unwrap();
        assert_eq!(log, "downloading…\ndone");
        // Already-OK Requirements write no log file.
        let engine = FakeEngine::new().with_detection("git", installed_latest("git"));
        let record = execute_run(
            &engine,
            "run-test",
            &[],
            &[requirement.clone()],
            dir.path(),
            None,
        )
        .unwrap();
        assert_eq!(record.results[0].log_path, "");
    }

    #[test]
    fn no_package_found_failures_carry_the_check_your_id_message() {
        let mut engine = FakeEngine::new().with_detection("ghost", Detection::absent());
        engine.install_result.ok = false;
        engine.install_result.detail =
            "can't find this app in the winget registry (exit 5) — check its ID is correct, then re-run this plan"
                .into();
        let requirement = winget_req("ghost", VersionPolicy::Latest, 10);
        let record = run(&engine, &[requirement.clone()]);
        assert_eq!(status_of(&record, "ghost"), &RunStatus::Failed);
        assert_eq!(record.outcome, RunOutcome::Failed);
        assert!(
            record.results[0].detail.contains("check its ID"),
            "{}",
            record.results[0].detail
        );
    }

    #[test]
    fn failed_step_reason_is_written_to_the_run_log() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = FakeEngine::new().with_detection("bad", Detection::absent());
        engine.install_result.ok = false;
        engine.install_result.detail = "install failed (exit 5) — not installed".into();
        engine.install_result.log = "downloading…\nfailed".into();
        let requirement = winget_req("bad", VersionPolicy::Latest, 10);
        let record = execute_run(
            &engine,
            "run-test",
            &[],
            &[requirement.clone()],
            dir.path(),
            None,
        )
        .unwrap();
        let log = std::fs::read_to_string(&record.results[0].log_path).unwrap();
        assert!(log.contains("downloading…"), "{log}");
        assert!(
            log.contains("--- sprout ---") && log.contains("install failed (exit 5)"),
            "{log}"
        );
    }

    #[test]
    fn run_record_carries_presets_and_timestamps() {
        let engine = FakeEngine::new();
        let requirement = winget_req("git", VersionPolicy::Latest, 10);
        let record = run(&engine, &[requirement.clone()]);
        assert_eq!(record.preset_names, vec!["Preset A"]);
        assert!(record.started_at > 0);
        assert!(record.finished_at >= record.started_at);
    }

    #[test]
    fn observed_run_streams_progress_in_execution_order() {
        let engine = FakeEngine::new().with_detection("b", installed_latest("b"));
        let a = winget_req("a", VersionPolicy::Latest, 10);
        let b = winget_req("b", VersionPolicy::Latest, 10);
        let dir = tempfile::tempdir().unwrap();
        let mut events = Vec::new();
        let record = execute_run_observed(
            &engine,
            "run-test",
            &["Preset A".into()],
            &[a.clone(), b.clone()],
            dir.path(),
            None,
            &mut |e| events.push(e),
            &mut || false,
        )
        .unwrap();

        let kinds: Vec<&str> = events.iter().map(|e| match e {
            ProgressEvent::Phase { .. } => "phase",
            ProgressEvent::RequirementStarted { .. } => "started",
            ProgressEvent::RequirementFinished(_) => "finished",
            ProgressEvent::RunFinished { .. } => "run_finished",
        }).collect();
        assert_eq!(kinds, vec!["phase", "phase", "started", "finished", "started", "finished", "run_finished"]);

        // The run prepares the engine before it detects.
        let ProgressEvent::Phase { phase } = &events[0] else {
            panic!("expected a phase event");
        };
        assert_eq!(phase, "preparing");
        let ProgressEvent::Phase { phase } = &events[1] else {
            panic!("expected a phase event");
        };
        assert_eq!(phase, "detecting");
        assert_eq!(engine.prepares(), 1);

        // Started events carry the run totals and the planned action.
        let ProgressEvent::RequirementStarted { index, total, product_id, product_name, action } = &events[2] else {
            panic!("expected a started event");
        };
        assert_eq!((*index, *total), (0, 2));
        assert_eq!(product_id, "a");
        assert_eq!(product_name, "Product a");
        assert_eq!(action, "install");

        // Finished events carry the persisted outcome.
        let ProgressEvent::RequirementFinished(outcome) = &events[3] else {
            panic!("expected a finished event");
        };
        assert_eq!(outcome.product_id, "a");
        assert_eq!(outcome.status, RunStatus::Installed);

        let ProgressEvent::RequirementFinished(outcome) = &events[5] else {
            panic!("expected a finished event");
        };
        assert_eq!(outcome.status, RunStatus::AlreadyOk);

        let ProgressEvent::RunFinished { outcome } = events.last().unwrap() else {
            panic!("expected a run_finished event");
        };
        assert_eq!(*outcome, record.outcome);
        assert_eq!(record.outcome, RunOutcome::Ok);
    }

    #[test]
    fn observed_run_reports_skip_and_upgrade_actions() {
        let engine = FakeEngine::new().with_detection(
            "old",
            Detection {
                installed: true,
                winget_managed: true,
                installed_version: Some("1.0.0".into()),
                available_version: Some("1.1.0".into()),
            },
        );
        let old = winget_req("old", VersionPolicy::Latest, 10);
        let dir = tempfile::tempdir().unwrap();
        let mut events = Vec::new();
        execute_run_observed(
            &engine,
            "run-test",
            &[],
            &[old],
            dir.path(),
            None,
            &mut |e| events.push(e),
            &mut || false,
        )
        .unwrap();
        let ProgressEvent::RequirementStarted { action, .. } = &events[2] else {
            panic!("expected a started event");
        };
        assert_eq!(action, "upgrade");
    }

    #[test]
    fn cancel_aborts_between_requirements_and_marks_the_run_cancelled() {
        let engine = FakeEngine::new();
        let a = winget_req("a", VersionPolicy::Latest, 10);
        let b = winget_req("b", VersionPolicy::Latest, 10);
        let dir = tempfile::tempdir().unwrap();
        let mut calls = 0;
        let record = execute_run_observed(
            &engine,
            "run-test",
            &[],
            &[a.clone(), b.clone()],
            dir.path(),
            None,
            &mut |_| {},
            &mut || {
                calls += 1;
                calls > 1 // cancel before the second requirement
            },
        )
        .unwrap();

        assert_eq!(record.outcome, RunOutcome::Cancelled);
        assert_eq!(record.results.len(), 1);
        assert_eq!(record.results[0].product_id, "a");
        // Only the first step ran — nothing is left half-installed.
        assert_eq!(engine.installs().len(), 1);
    }

    #[test]
    fn cancel_before_anything_runs_yields_no_results() {
        let engine = FakeEngine::new();
        let a = winget_req("a", VersionPolicy::Latest, 10);
        let dir = tempfile::tempdir().unwrap();
        let record = execute_run_observed(
            &engine,
            "run-test",
            &[],
            &[a.clone()],
            dir.path(),
            None,
            &mut |_| {},
            &mut || true,
        )
        .unwrap();
        assert_eq!(record.outcome, RunOutcome::Cancelled);
        assert!(record.results.is_empty());
        assert!(engine.installs().is_empty());
    }

    #[test]
    fn prepare_failure_aborts_the_run_with_the_engine_message() {
        let engine = FakeEngine::new().with_prepare_error(
            "winget is missing and this Windows build (0) is unsupported",
        );
        let a = winget_req("a", VersionPolicy::Latest, 10);
        let dir = tempfile::tempdir().unwrap();
        let err = execute_run_observed(
            &engine,
            "run-test",
            &[],
            &[a.clone()],
            dir.path(),
            None,
            &mut |_| {},
            &mut || false,
        )
        .unwrap_err();
        assert!(err.contains("unsupported"), "{err}");
        // Nothing detected, nothing installed — the run never started.
        assert_eq!(engine.prepares(), 1);
        assert!(engine.installs().is_empty());
    }

    #[test]
    fn observed_run_without_hooks_matches_plain_execute_run() {        let engine = FakeEngine::new().with_detection("a", installed_latest("a"));
        let a = winget_req("a", VersionPolicy::Latest, 10);
        let dir = tempfile::tempdir().unwrap();
        let plain = execute_run(&engine, "run-test", &[], &[a.clone()], dir.path(), None).unwrap();
        let mut events = Vec::new();
        let observed = execute_run_observed(
            &engine,
            "run-test",
            &[],
            &[a.clone()],
            dir.path(),
            None,
            &mut |e| events.push(e),
            &mut || false,
        )
        .unwrap();
        assert_eq!(observed, plain);
        assert!(matches!(
            events.last().unwrap(),
            ProgressEvent::RunFinished { outcome: RunOutcome::Ok }
        ));
    }

    fn with_env(req: &mut Requirement, wiring: EnvWiring) {
        req.env.push(wiring);
    }

    fn with_verify(req: &mut Requirement, command: &str) {
        req.verify.push(VerifyCommand {
            command: command.into(),
            args: vec![],
            match_text: None,
        });
    }

    /// A requirement whose product carries an install-location hint — the
    /// registry lookup a post-install honesty check needs (ticket 34).
    fn hinted_req(id: &str, hint: &str) -> Requirement {
        let mut req = winget_req(id, VersionPolicy::Latest, 10);
        req.product.install_location_hint = Some(hint.into());
        req
    }

    #[test]
    fn env_wiring_applies_after_a_successful_install() {
        let mut a = winget_req("a", VersionPolicy::Latest, 10);
        with_env(
            &mut a,
            EnvWiring {
                action: EnvAction::Set,
                name: "JAVA_HOME".into(),
                value: "C:\\jdk".into(),
            },
        );
        let engine = FakeEngine::new().with_env_notes(&["env: set JAVA_HOME = C:\\jdk (User)"]);
        let record = run(&engine, &[a.clone()]);
        assert_eq!(status_of(&record, "a"), &RunStatus::Installed);
        assert_eq!(
            engine.env_wiring(),
            vec![("a".to_string(), vec!["JAVA_HOME".to_string()])]
        );
        assert!(
            record.results[0]
                .detail
                .contains("env: set JAVA_HOME = C:\\jdk (User)"),
            "{}",
            record.results[0].detail
        );
    }

    #[test]
    fn env_wiring_and_verify_never_run_when_the_step_fails() {
        let mut engine = FakeEngine::new().with_detection("bad", Detection::absent());
        engine.install_result.ok = false;
        engine.install_result.detail = "install failed (exit 5) — not installed".into();
        let mut a = winget_req("a", VersionPolicy::Latest, 10);
        with_env(
            &mut a,
            EnvWiring {
                action: EnvAction::Set,
                name: "JAVA_HOME".into(),
                value: "C:\\jdk".into(),
            },
        );
        with_verify(&mut a, "java -version");
        let record = run(&engine, &[a.clone()]);
        assert_eq!(status_of(&record, "a"), &RunStatus::Failed);
        assert!(engine.env_wiring().is_empty(), "env must not apply");
        assert!(engine.verified().is_empty(), "verify must not run");
    }

    #[test]
    fn env_wiring_and_verify_do_not_run_for_already_ok() {
        let engine = FakeEngine::new().with_detection("a", installed_latest("a"));
        let mut a = winget_req("a", VersionPolicy::Latest, 10);
        with_env(
            &mut a,
            EnvWiring {
                action: EnvAction::Set,
                name: "JAVA_HOME".into(),
                value: "C:\\jdk".into(),
            },
        );
        with_verify(&mut a, "java -version");
        let record = run(&engine, &[a.clone()]);
        assert_eq!(status_of(&record, "a"), &RunStatus::AlreadyOk);
        assert!(engine.env_wiring().is_empty());
        assert!(engine.verified().is_empty());
    }

    #[test]
    fn verify_runs_after_a_successful_install_and_notes_its_pass() {
        let mut a = winget_req("a", VersionPolicy::Latest, 10);
        with_verify(&mut a, "java -version");
        let mut engine = FakeEngine::new().with_verify_result(
            "java -version",
            VerifyOutcome::passed("'java' exited 0 and reported '21'", "java version 21.0.5"),
        );
        // The install writes its own log first; the verify output is appended
        // to the same file.
        engine.install_result.log = "installed ok".into();
        let dir = tempfile::tempdir().unwrap();
        let record =
            execute_run(&engine, "run-test", &[], &[a.clone()], dir.path(), None).unwrap();
        assert_eq!(status_of(&record, "a"), &RunStatus::Installed);
        assert_eq!(engine.verified(), vec!["java -version"]);
        assert!(
            record.results[0].detail.contains("verify: 'java' exited 0 and reported '21'"),
            "{}",
            record.results[0].detail
        );
        // The verify output is appended to the Requirement's log.
        let log = std::fs::read_to_string(&record.results[0].log_path).unwrap();
        assert!(log.contains("--- verify ---") && log.contains("java version 21.0.5"), "{log}");
    }

    #[test]
    fn a_failed_verify_fails_the_requirement_loudly() {
        let mut a = winget_req("a", VersionPolicy::Latest, 10);
        with_verify(&mut a, "java -version");
        let engine = FakeEngine::new().with_verify_result(
            "java -version",
            VerifyOutcome::failed("'java' exited 1 — the product is not behaving as declared", "some error output"),
        );
        let record = run(&engine, &[a.clone()]);
        assert_eq!(record.outcome, RunOutcome::Failed);
        assert_eq!(status_of(&record, "a"), &RunStatus::Failed);
        assert_eq!(
            record.results[0].detail,
            "verify failed: 'java' exited 1 — the product is not behaving as declared"
        );
    }

    #[test]
    fn verify_commands_run_in_order_and_stop_at_the_first_failure() {
        let mut a = winget_req("a", VersionPolicy::Latest, 10);
        with_verify(&mut a, "first");
        with_verify(&mut a, "second");
        with_verify(&mut a, "third");
        // First check passes, second fails — the third must never run.
        let engine = FakeEngine::new()
            .with_verify_result("first", VerifyOutcome::passed("'first' exited 0", String::new()))
            .with_verify_result("second", VerifyOutcome::failed("'second' exited 2", String::new()));
        let record = run(&engine, &[a.clone()]);
        assert_eq!(status_of(&record, "a"), &RunStatus::Failed);
        assert_eq!(engine.verified(), vec!["first", "second"]);
    }

    #[test]
    fn verify_runs_after_an_upgrade_too() {
        let engine = FakeEngine::new().with_detection(
            "old",
            Detection {
                installed: true,
                winget_managed: true,
                installed_version: Some("1.0.0".into()),
                available_version: Some("1.1.0".into()),
            },
        );
        let mut old = winget_req("old", VersionPolicy::Latest, 10);
        with_verify(&mut old, "git --version");
        let record = run(&engine, &[old.clone()]);
        assert_eq!(status_of(&record, "old"), &RunStatus::Upgraded);
        assert_eq!(engine.verified(), vec!["git --version"]);
    }

    #[test]
    fn quick_requirement_synthesis_carries_product_defaults() {
        let product = Product {
            id: "openjdk21".into(),
            name: "Eclipse Temurin OpenJDK 21 (LTS)".into(),
            winget_id: Some("EclipseAdoptium.Temurin.21.JDK".into()),
            install_location_hint: Some("Eclipse Temurin".into()),
            install_dir: None,
            default_env: vec![
                EnvWiring {
                    action: EnvAction::Set,
                    name: "JAVA_HOME".into(),
                    value: "<InstallLocation:Eclipse Temurin>".into(),
                },
                EnvWiring {
                    action: EnvAction::Prepend,
                    name: "PATH".into(),
                    value: "<InstallLocation:Eclipse Temurin>\\bin".into(),
                },
            ],
        };
        let requirement = synthesize_quick_requirement(&product).unwrap();
        // Latest version policy and the Product's winget step in machine
        // scope — the installable identity comes from the Library row.
        assert_eq!(requirement.version_policy, VersionPolicy::Latest);
        assert_eq!(
            requirement.step,
            Step::Winget {
                id: "EclipseAdoptium.Temurin.21.JDK".into(),
                scope: "machine".into()
            }
        );
        // The Product's default env wiring rides along, with the shared
        // default timeout and no extras (no deps, no verify commands).
        assert_eq!(requirement.env, product.default_env);
        assert_eq!(requirement.timeout_minutes, 10);
        assert!(requirement.depends_on.is_empty());
        assert!(requirement.verify.is_empty());
        assert!(!requirement.unresolved);
        // The synthesized step is the winget id, not the Library id.
        assert_eq!(requirement.product.winget_id.as_deref(), Some("EclipseAdoptium.Temurin.21.JDK"));
    }

    #[test]
    fn quick_requirement_without_a_winget_step_is_a_clear_error() {
        let product = Product {
            id: "node-lts".into(),
            name: "Node.js LTS (via NVM)".into(),
            winget_id: None,
            install_location_hint: None,
            install_dir: None,
            default_env: vec![],
        };
        let err = synthesize_quick_requirement(&product).unwrap_err();
        assert!(err.contains("node-lts"), "{err}");
        assert!(err.contains("winget"), "{err}");
    }

    #[test]
    fn same_directory_ignores_case_and_trailing_separators() {
        assert!(same_directory(r"D:\Apps", r"D:\Apps"));
        assert!(same_directory(r"D:\Apps", r"d:\apps\"));
        assert!(same_directory(r"D:\Apps\", r"D:\Apps"));
        assert!(same_directory(r"C:\", r"c:\"));
        assert!(!same_directory(r"D:\Apps", r"D:\Tools"));
        assert!(!same_directory(r"D:\Apps", r""));
    }

    #[test]
    fn requested_directory_is_passed_to_install_and_upgrade() {
        let engine = FakeEngine::new().with_detection(
            "old",
            Detection {
                installed: true,
                winget_managed: true,
                installed_version: Some("1.0.0".into()),
                available_version: Some("1.1.0".into()),
            },
        );
        let old = hinted_req("old", "Git");
        let dir = tempfile::tempdir().unwrap();
        execute_run(
            &engine,
            "run-test",
            &[],
            &[old.clone()],
            dir.path(),
            Some(r"D:\Apps"),
        )
        .unwrap();
        // The upgrade carried the directory; the same requirement planned
        // fresh on an absent machine would install with it.
        assert_eq!(
            engine.upgrades(),
            vec![("Vendor.old".to_string(), 10, Some(r"D:\Apps".to_string()))]
        );
        let engine = FakeEngine::new().with_detection("new", Detection::absent());
        let new = hinted_req("new", "Git");
        let record = execute_run(
            &engine,
            "run-test",
            &[],
            &[new.clone()],
            dir.path(),
            Some(r"D:\Apps"),
        )
        .unwrap();
        assert_eq!(record.outcome, RunOutcome::Ok);
        assert_eq!(
            engine.installs(),
            vec![("Vendor.new".to_string(), 10, Some(r"D:\Apps".to_string()))]
        );
    }

    #[test]
    fn ignored_requested_directory_is_called_out_in_the_detail() {
        let engine = FakeEngine::new()
            .with_detection("git", Detection::absent())
            .with_actual_location("git", r"C:\Program Files\Git");
        let requirement = hinted_req("git", "Git");
        let dir = tempfile::tempdir().unwrap();
        let record = execute_run(
            &engine,
            "run-test",
            &[],
            &[requirement],
            dir.path(),
            Some(r"D:\Apps"),
        )
        .unwrap();
        assert_eq!(status_of(&record, "git"), &RunStatus::Installed);
        assert!(
            record.results[0]
                .detail
                .contains(r"installed to C:\Program Files\Git (installer ignored the requested directory)"),
            "{}",
            record.results[0].detail
        );
    }

    #[test]
    fn honored_requested_directory_stays_quiet() {
        // The product landed where it was asked to — case and a trailing
        // separator are the same place, so no fabricated note.
        let engine = FakeEngine::new()
            .with_detection("git", Detection::absent())
            .with_actual_location("git", r"d:\apps\");
        let requirement = hinted_req("git", "Git");
        let dir = tempfile::tempdir().unwrap();
        let record = execute_run(
            &engine,
            "run-test",
            &[],
            &[requirement],
            dir.path(),
            Some(r"D:\Apps"),
        )
        .unwrap();
        assert_eq!(record.results[0].detail, "installed");
    }

    #[test]
    fn product_install_dir_override_wins_over_the_global_default() {
        // One product carries its own directory, the other does not — the
        // run must honor the override for the first and fall back to the
        // global default for the second (ticket 36).
        let mut overridden = hinted_req("overridden", "Git");
        overridden.product.install_dir = Some(r"E:\Tools".into());
        let plain = hinted_req("plain", "Git");
        let engine = FakeEngine::new()
            .with_detection("overridden", Detection::absent())
            .with_detection("plain", Detection::absent());
        let dir = tempfile::tempdir().unwrap();
        execute_run(
            &engine,
            "run-test",
            &[],
            &[overridden, plain],
            dir.path(),
            Some(r"D:\Apps"),
        )
        .unwrap();
        assert_eq!(
            engine.installs(),
            vec![
                ("Vendor.overridden".to_string(), 10, Some(r"E:\Tools".to_string())),
                ("Vendor.plain".to_string(), 10, Some(r"D:\Apps".to_string())),
            ]
        );
    }

    #[test]
    fn product_override_applies_even_without_a_global_default() {
        let mut req = hinted_req("git", "Git");
        req.product.install_dir = Some(r"E:\Tools".into());
        let engine = FakeEngine::new().with_detection("git", Detection::absent());
        let dir = tempfile::tempdir().unwrap();
        execute_run(
            &engine,
            "run-test",
            &[],
            &[req],
            dir.path(),
            None,
        )
        .unwrap();
        assert_eq!(
            engine.installs(),
            vec![("Vendor.git".to_string(), 10, Some(r"E:\Tools".to_string()))]
        );
    }

    #[test]
    fn honesty_check_compares_against_the_product_override() {
        // The override asked for D:\Apps; the installer landed elsewhere —
        // the note must name the requested directory it was given.
        let mut req = hinted_req("git", "Git");
        req.product.install_dir = Some(r"D:\Apps".into());
        let engine = FakeEngine::new()
            .with_detection("git", Detection::absent())
            .with_actual_location("git", r"C:\Program Files\Git");
        let dir = tempfile::tempdir().unwrap();
        let record = execute_run(&engine, "run-test", &[], &[req], dir.path(), None).unwrap();
        assert!(
            record.results[0]
                .detail
                .contains(r"installed to C:\Program Files\Git (installer ignored the requested directory)"),
            "{}",
            record.results[0].detail
        );
    }

    #[test]
    fn unresolvable_actual_location_never_fabricates_a_note() {
        // The engine knows no location (no registry hint) — the detail stays
        // the bare verdict, whatever directory was requested.
        let engine = FakeEngine::new().with_detection("git", Detection::absent());
        let requirement = hinted_req("git", "Git");
        let dir = tempfile::tempdir().unwrap();
        let record = execute_run(
            &engine,
            "run-test",
            &[],
            &[requirement],
            dir.path(),
            Some(r"D:\Apps"),
        )
        .unwrap();
        assert_eq!(record.results[0].detail, "installed");
        // Same when the product has no hint at all.
        let requirement = winget_req("git", VersionPolicy::Latest, 10);
        let record = execute_run(
            &engine,
            "run-test",
            &[],
            &[requirement],
            dir.path(),
            Some(r"D:\Apps"),
        )
        .unwrap();
        assert_eq!(record.results[0].detail, "installed");
    }

    #[test]
    fn failed_steps_never_report_an_actual_location() {
        let mut engine = FakeEngine::new()
            .with_detection("git", Detection::absent())
            .with_actual_location("git", r"C:\Program Files\Git");
        engine.install_result.ok = false;
        engine.install_result.detail = "install failed (exit 5) — not installed".into();
        let requirement = hinted_req("git", "Git");
        let dir = tempfile::tempdir().unwrap();
        let record = execute_run(
            &engine,
            "run-test",
            &[],
            &[requirement],
            dir.path(),
            Some(r"D:\Apps"),
        )
        .unwrap();
        assert_eq!(status_of(&record, "git"), &RunStatus::Failed);
        assert_eq!(
            record.results[0].detail,
            "install failed (exit 5) — not installed"
        );
    }

    #[test]
    fn quick_install_run_carries_its_label_through_persistence() {
        // The quick-install command labels the Run "Quick install — {product}";
        // the label rides the same preset_names path as preset runs, so
        // History renders it through the same outcome tiers.
        let engine = FakeEngine::new().with_detection("git", installed_latest("git"));
        let product = Product {
            id: "git".into(),
            name: "Git".into(),
            winget_id: Some("Git.Git".into()),
            install_location_hint: None,
            install_dir: None,
            default_env: vec![],
        };
        let requirement = synthesize_quick_requirement(&product).unwrap();
        let label = vec![format!("Quick install — {}", product.name)];
        let dir = tempfile::tempdir().unwrap();
        let record =
            execute_run(&engine, "run-test", &label, &[requirement], dir.path(), None).unwrap();
        assert_eq!(record.preset_names, label);
        assert_eq!(record.outcome, RunOutcome::Ok);
        assert_eq!(status_of(&record, "git"), &RunStatus::AlreadyOk);

        // The persisted Run keeps the label and its outcome tier.
        let db_dir = tempfile::tempdir().unwrap();
        let conn = crate::db::init_at(&db_dir.path().to_path_buf()).unwrap();
        crate::db::create_run(&conn, &record).unwrap();
        let loaded = crate::db::get_run(&conn, "run-test").unwrap().unwrap();
        assert_eq!(loaded.preset_names, label);
        assert_eq!(loaded.outcome, RunOutcome::Ok);
        assert_eq!(loaded, record);
    }
}
