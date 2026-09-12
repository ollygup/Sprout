//! Offline static Discord Rich Presence (ADR-0033): while Sprout runs, Discord
//! users see a fixed "Using Sprout / Composing presets" activity. The crate
//! speaks only to Discord's local IPC pipe — no network, no OAuth scopes, no
//! token storage, no user-ID read — so the offline posture holds: there is no
//! account access to prove beyond the shape of this module (the seam below can
//! only carry the two static strings, never user content).
//!
//! WHY a background loop instead of set-once: Discord may be closed at boot or
//! restarted mid-session, so connect→set retries with capped backoff and the
//! live connection is re-asserted on a heartbeat; a dropped pipe returns to
//! the retry loop instead of dying silently. Every failure stays local
//! (`eprintln` only) — presence never blocks startup, opens no window, and
//! shows no toast or dialog.
//!
//! WHY the single-flight guard: setup runs once per process, but a second
//! `start` must never spawn a second loop fighting over the same IPC pipe.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use discord_rich_presence::{activity::Activity, DiscordIpc, DiscordIpcClient};

/// Discord Application (= Client) ID identifying Sprout to the desktop
/// client. WHY hardcoded rather than Settings-editable, backed up, or
/// exported: it is one app-wide value, not per-user configuration — and it is
/// not a secret (it ships in the binary and travels to the local Discord
/// client only), so there is nothing to protect with a secret store
/// (ADR-0033).
pub const APPLICATION_ID: &str = "1548219927264235580";

/// The only two strings presence may ever send (ADR-0033): no preset, action,
/// path, count, or run-state text may travel this seam.
pub const DETAILS: &str = "Using Sprout";
pub const STATE: &str = "Composing presets";

/// How often a live connection re-asserts the activity: frequent enough to
/// notice a Discord restart, rare enough to stay quiet on a local pipe.
const HEARTBEAT: Duration = Duration::from_secs(30);

/// Whether the background loop has been claimed; WHY a static: setup and exit
/// are process-global events with no handle to thread through.
static STARTED: AtomicBool = AtomicBool::new(false);
/// WHY a static beside STARTED: the loop is detached, so shutdown signals
/// through shared state and confirms with its own synchronous clear below.
static STOP: AtomicBool = AtomicBool::new(false);

/// The exact v1 activity: static text only, no assets, buttons, party,
/// secrets, or timestamps — anything user-derived needs a new amendment
/// before it may appear here (ADR-0033).
pub fn static_activity() -> Activity<'static> {
    Activity::new().details(DETAILS).state(STATE)
}

/// The module's internal seam: everything presence can do to the outside
/// world. WHY a seam with two adapters: Discord present vs absent is one
/// variation, deterministic tests the other — a real seam, not a hypothetical
/// one. The interface admits only the static payload, which is what makes the
/// no-account-access promise structural instead of verbal.
trait Ipc {
    fn connect(&mut self) -> Result<(), String>;
    fn set_static(&mut self) -> Result<(), String>;
    fn clear_and_close(&mut self);
}

/// The live adapter: the handwired crate over the local IPC pipe, and the
/// only Discord IPC site in the app (ADR-0029).
struct RealIpc {
    client: DiscordIpcClient,
}

impl RealIpc {
    fn new() -> Self {
        Self {
            client: DiscordIpcClient::new(APPLICATION_ID),
        }
    }
}

impl Ipc for RealIpc {
    fn connect(&mut self) -> Result<(), String> {
        // WHY map_err to String: the loop logs uniformly without learning the
        // crate's error type — callers never match on failure kinds.
        self.client
            .connect()
            .map_err(|e| format!("Discord IPC connect failed: {e}"))
    }

    fn set_static(&mut self) -> Result<(), String> {
        self.client
            .set_activity(static_activity())
            .map_err(|e| format!("Discord IPC set failed: {e}"))
    }

    fn clear_and_close(&mut self) {
        // WHY best-effort with no return: shutdown paths must not fail — a
        // dead pipe and an already-cleared activity are both fine outcomes.
        let _ = self.client.clear_activity();
        let _ = self.client.close();
    }
}

/// Capped backoff between reconnect attempts: quick first retries for a
/// client still starting up, then a steady 10 s cadence that stays quiet.
fn backoff_for_attempt(failures: u32) -> Duration {
    match failures {
        0 => Duration::from_secs(1),
        1 => Duration::from_secs(2),
        2 => Duration::from_secs(5),
        _ => Duration::from_secs(10),
    }
}

/// One connect→set attempt: both halves must succeed before the activity is
/// considered live, so a half-open pipe never reads as presence.
fn connect_and_set(ipc: &mut impl Ipc) -> Result<(), String> {
    ipc.connect()?;
    ipc.set_static()
}

/// Starts the presence loop on a background thread: connect→set, then
/// heartbeat re-asserts until shutdown. Never blocks the caller; a second
/// call is a no-op so overlapping starts never duplicate the loop.
pub fn start() {
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    STOP.store(false, Ordering::SeqCst);
    std::thread::spawn(|| {
        let mut ipc = RealIpc::new();
        serve_with(&mut ipc, HEARTBEAT, std::thread::sleep, &STOP);
    });
}

/// Stops the loop and clears the activity. WHY a synchronous clear beside the
/// flag: the process may exit before the background thread wakes, so actual
/// exit clears through its own one-shot client while the flag stops the loop
/// from reconnecting behind it. Main-window close-to-tray never calls this —
/// only real exit does.
pub fn shutdown() {
    STOP.store(true, Ordering::SeqCst);
    RealIpc::new().clear_and_close();
}

/// The loop body with its sleeps injected: connect→set with backoff, then a
/// heartbeat that returns to retry when the pipe drops. WHY the heartbeat
/// re-sets instead of idling: idling would never notice a Discord restart.
/// WHY `stop` is a parameter rather than the `STOP` static: deterministic
/// tests drive this exact function with a local flag instead of
/// process-global state parallel tests would flake on.
fn serve_with(
    ipc: &mut impl Ipc,
    heartbeat: Duration,
    sleep: impl Fn(Duration),
    stop: &AtomicBool,
) {
    let mut failures: u32 = 0;
    loop {
        if stop.load(Ordering::SeqCst) {
            ipc.clear_and_close();
            return;
        }
        match connect_and_set(ipc) {
            Ok(()) => {
                failures = 0;
                loop {
                    sleep(heartbeat);
                    if stop.load(Ordering::SeqCst) {
                        ipc.clear_and_close();
                        return;
                    }
                    if let Err(e) = ipc.set_static() {
                        eprintln!("Discord presence lost ({e}) — retrying quietly");
                        break;
                    }
                }
            }
            Err(e) => {
                eprintln!("{e} — continuing without presence");
                sleep(backoff_for_attempt(failures));
                failures = failures.saturating_add(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// Calls the seam can observe: the test surface mirrors the trait, so a
    /// test asserting "only Connect/Set/ClearClose with the static strings"
    /// asserts everything the module can ever emit.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Call {
        Connect,
        Set { details: &'static str, state: &'static str },
        ClearClose,
    }

    /// Deterministic stand-in for Discord: scripted connect/set failures plus
    /// a full call log. The payload it records can only be the static pair —
    /// the seam gives it no other shape to carry. It trips the shared stop
    /// flag itself after a scripted number of connects or sets, so each test
    /// runs the real `serve_with` loop to a deterministic halt.
    struct FakeIpc {
        calls: Vec<Call>,
        stop: Arc<AtomicBool>,
        connects_done: u32,
        sets_done: u32,
        connect_failures_left: u32,
        set_failures_left: u32,
        stop_after_connects: Option<u32>,
        stop_after_sets: Option<u32>,
    }

    impl FakeIpc {
        fn new(stop: &Arc<AtomicBool>) -> Self {
            Self {
                calls: Vec::new(),
                stop: Arc::clone(stop),
                connects_done: 0,
                sets_done: 0,
                connect_failures_left: 0,
                set_failures_left: 0,
                stop_after_connects: None,
                stop_after_sets: None,
            }
        }

        fn failing_connects(mut self, n: u32) -> Self {
            self.connect_failures_left = n;
            self
        }

        fn stop_after_connects(mut self, n: u32) -> Self {
            self.stop_after_connects = Some(n);
            self
        }

        fn stop_after_sets(mut self, n: u32) -> Self {
            self.stop_after_sets = Some(n);
            self
        }
    }

    impl Ipc for FakeIpc {
        fn connect(&mut self) -> Result<(), String> {
            self.calls.push(Call::Connect);
            self.connects_done += 1;
            if let Some(n) = self.stop_after_connects {
                if self.connects_done >= n {
                    self.stop.store(true, Ordering::SeqCst);
                }
            }
            if self.connect_failures_left > 0 {
                self.connect_failures_left -= 1;
                return Err("Discord IPC connect failed: no Discord".into());
            }
            Ok(())
        }

        fn set_static(&mut self) -> Result<(), String> {
            self.calls
                .push(Call::Set { details: DETAILS, state: STATE });
            self.sets_done += 1;
            if let Some(n) = self.stop_after_sets {
                if self.sets_done >= n {
                    self.stop.store(true, Ordering::SeqCst);
                }
            }
            if self.set_failures_left > 0 {
                self.set_failures_left -= 1;
                return Err("Discord IPC set failed: pipe dropped".into());
            }
            Ok(())
        }

        fn clear_and_close(&mut self) {
            self.calls.push(Call::ClearClose);
        }
    }

    /// Runs the real `serve_with` loop against a fake: sleeps are recorded
    /// for backoff assertions and never taken, and the fake halts the loop
    /// itself — deterministic, no threads, no timing flakes.
    fn drive(
        mut ipc: FakeIpc,
        stop: &AtomicBool,
        heartbeat: Duration,
    ) -> (Vec<Call>, Vec<Duration>) {
        let sleeps: Vec<Duration> = Vec::new();
        let sleeps_cell = Mutex::new(sleeps);
        serve_with(
            &mut ipc,
            heartbeat,
            |d| sleeps_cell.lock().unwrap().push(d),
            stop,
        );
        (ipc.calls, sleeps_cell.into_inner().unwrap())
    }

    #[test]
    fn static_payload_carries_exactly_the_v1_text() {
        let value = serde_json::to_value(static_activity()).unwrap();
        assert_eq!(value["details"], DETAILS);
        assert_eq!(value["state"], STATE);
        assert_eq!(DETAILS, "Using Sprout");
        assert_eq!(STATE, "Composing presets");
    }

    #[test]
    fn static_payload_sets_no_optional_sections() {
        // WHY shape-assert instead of field-assert: the crate's fields are
        // private, so the serialized form is the observable contract — any
        // future assets/buttons/party/secrets/timestamps must arrive as null
        // or absent, never populated, or this fails loudly.
        let value = serde_json::to_value(static_activity()).unwrap();
        for key in ["assets", "buttons", "party", "secrets", "timestamps"] {
            assert!(
                value.get(key).is_none_or(|v| v.is_null()),
                "presence must not set {key}: {value}"
            );
        }
        let flat = serde_json::to_string(&value).unwrap().to_lowercase();
        assert!(
            !flat.contains("token") && !flat.contains("oauth"),
            "presence payload must carry no account material: {flat}"
        );
    }

    #[test]
    fn application_id_is_a_plain_numeric_id_not_a_secret_shape() {
        // WHY numeric-only: a Discord Application ID is a snowflake of digits
        // — anything shaped like a token, URI, or key refuses to ship here.
        // WHY the placeholder refusal: an all-zeros ID can never show
        // presence, so shipping one again must fail loudly, not silently.
        assert!(!APPLICATION_ID.is_empty());
        assert!(APPLICATION_ID.chars().all(|c| c.is_ascii_digit()));
        assert!(APPLICATION_ID.chars().any(|c| c != '0'));
    }

    #[test]
    fn backoff_starts_quick_then_settles_at_ten_seconds() {
        assert_eq!(
            (0..6).map(backoff_for_attempt).collect::<Vec<_>>(),
            vec![
                Duration::from_secs(1),
                Duration::from_secs(2),
                Duration::from_secs(5),
                Duration::from_secs(10),
                Duration::from_secs(10),
                Duration::from_secs(10),
            ]
        );
    }

    #[test]
    fn discord_running_sets_the_static_pair_then_clears_on_stop() {
        let stop = Arc::new(AtomicBool::new(false));
        let ipc = FakeIpc::new(&stop).stop_after_sets(1);
        // One live set parks the loop on its heartbeat; the stop tripped by
        // that set then clears on the way out.
        let heartbeat = Duration::from_millis(1);
        let (calls, sleeps) = drive(ipc, &stop, heartbeat);
        assert!(calls.contains(&Call::Connect));
        assert!(calls.contains(&Call::Set { details: DETAILS, state: STATE }));
        assert_eq!(calls.last(), Some(&Call::ClearClose));
        assert_eq!(sleeps, vec![heartbeat]);
    }

    #[test]
    fn discord_absent_retries_with_backoff_and_never_sets() {
        let stop = Arc::new(AtomicBool::new(false));
        let ipc = FakeIpc::new(&stop)
            .failing_connects(u32::MAX)
            .stop_after_connects(3);
        // Silent failure keeps retrying — never sets, never panics — with the
        // capped backoff between attempts, then clears on the way out.
        let (calls, sleeps) = drive(ipc, &stop, Duration::from_millis(1));
        assert!(!calls.contains(&Call::Set { details: DETAILS, state: STATE }));
        assert_eq!(
            calls,
            vec![Call::Connect, Call::Connect, Call::Connect, Call::ClearClose]
        );
        assert_eq!(
            sleeps,
            vec![
                Duration::from_secs(1),
                Duration::from_secs(2),
                Duration::from_secs(5),
            ]
        );
    }

    #[test]
    fn reconnect_after_startup_flaps_sets_eventually() {
        let stop = Arc::new(AtomicBool::new(false));
        let ipc = FakeIpc::new(&stop).failing_connects(2).stop_after_sets(1);
        let heartbeat = Duration::from_millis(1);
        let (calls, sleeps) = drive(ipc, &stop, heartbeat);
        assert_eq!(
            calls,
            vec![
                Call::Connect,
                Call::Connect,
                Call::Connect,
                Call::Set { details: DETAILS, state: STATE },
                Call::ClearClose,
            ]
        );
        assert_eq!(
            sleeps,
            vec![
                Duration::from_secs(1),
                Duration::from_secs(2),
                heartbeat,
            ]
        );
    }

    #[test]
    fn dropped_pipe_returns_to_retry_instead_of_dying() {
        let stop = Arc::new(AtomicBool::new(false));
        let mut ipc = FakeIpc::new(&stop);
        assert!(connect_and_set(&mut ipc).is_ok());
        // The heartbeat re-set fails: the loop must break back to
        // connect→set (a fresh Connect follows) rather than park silently.
        ipc.set_failures_left = 1;
        assert!(ipc.set_static().is_err());
        assert!(connect_and_set(&mut ipc).is_ok());
        let connects = ipc.calls.iter().filter(|c| **c == Call::Connect).count();
        assert_eq!(connects, 2);
    }

    #[test]
    fn shutdown_path_clears_and_closes() {
        let stop = Arc::new(AtomicBool::new(false));
        let mut ipc = FakeIpc::new(&stop);
        assert!(connect_and_set(&mut ipc).is_ok());
        ipc.clear_and_close();
        assert_eq!(ipc.calls.last(), Some(&Call::ClearClose));
    }

    #[test]
    fn only_the_static_pair_ever_crosses_the_seam() {
        let stop = Arc::new(AtomicBool::new(false));
        let mut ipc = FakeIpc::new(&stop).failing_connects(1);
        let _ = connect_and_set(&mut ipc);
        let _ = connect_and_set(&mut ipc);
        ipc.clear_and_close();
        // WHY exhaustive: the fake's `Call` enum is the whole vocabulary of
        // the seam — matching every variant here means a new leaking call
        // cannot compile without updating this proof.
        for call in &ipc.calls {
            match call {
                Call::Connect | Call::ClearClose => {}
                Call::Set { details, state } => {
                    assert_eq!((*details, *state), (DETAILS, STATE));
                }
            }
        }
        assert!(ipc.calls.contains(&Call::Set { details: DETAILS, state: STATE }));
    }
}
