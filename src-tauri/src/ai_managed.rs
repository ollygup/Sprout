//! Optional app-owned local inference behind the shared draft-provider seam.
//!
//! The public interface stays deliberately small: report the bundled catalog,
//! install one qualified selection after an explicit request, borrow one
//! request-scoped provider, cancel it, reap an idle runtime, or shut down.
//! Artifact, process, transport, and clock adapters are private seams used by
//! the production implementation and deterministic lifecycle tests.

use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, Weak,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{
    ai_assist::{DraftPrompt, DraftProvider, ProviderError},
    windows_execution,
};

const CATALOG_JSON: &str = include_str!("../resources/ai-model-recommendations.json");
const INSTALL_MANIFEST: &str = "managed-install.json";
const IDLE_INTERVAL: Duration = Duration::from_secs(5 * 60);
const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
const IO_POLL: Duration = Duration::from_millis(200);
const MAX_HTTP_RESPONSE: usize = 65_536;

#[derive(Clone, Deserialize)]
struct Catalog {
    schema_version: u32,
    note: String,
    managed_runtime: RuntimeCatalog,
    tiers: Vec<ModelCatalog>,
}

#[derive(Clone, Deserialize)]
struct RuntimeCatalog {
    name: String,
    candidate_version: String,
    windows_cpu_x64_artifact: String,
    source: String,
    license: String,
    status: String,
    verified: RuntimeVerification,
    blocker: String,
    #[serde(default)]
    download_url: Option<String>,
    #[serde(default)]
    sha256: Option<String>,
    #[serde(default)]
    size_bytes: Option<u64>,
    #[serde(default)]
    executable: Option<String>,
    #[serde(default)]
    launch_args: Option<Vec<String>>,
}

#[derive(Clone, Deserialize)]
struct RuntimeVerification {
    download: bool,
    per_user_launch: bool,
    health: bool,
    cancellation: bool,
    model_release: bool,
    process_ownership: bool,
    exit_behavior: bool,
}

impl RuntimeVerification {
    fn complete(&self) -> bool {
        self.download
            && self.per_user_launch
            && self.health
            && self.cancellation
            && self.model_release
            && self.process_ownership
            && self.exit_behavior
    }
}

#[derive(Clone, Deserialize)]
struct ModelCatalog {
    id: String,
    intent: String,
    candidate_artifact: String,
    artifact_source: String,
    revision: Option<String>,
    quantization: Option<String>,
    download_hash: Option<String>,
    download_size_bytes: Option<u64>,
    license: String,
    license_source: String,
    status: String,
    blocker: String,
    context_limit_tokens: Option<u64>,
    template_requirements: Option<String>,
    memory_needs_mb: Option<u64>,
    minimum_runtime_version: Option<String>,
    #[serde(default)]
    download_url: Option<String>,
    #[serde(default)]
    model_name: Option<String>,
}

#[derive(Clone)]
struct QualifiedSelection {
    runtime: RuntimeCatalog,
    model: ModelCatalog,
}

impl Catalog {
    fn parse(source: &str) -> Result<Self, String> {
        let catalog: Catalog = serde_json::from_str(source)
            .map_err(|error| format!("The bundled AI recommendation catalog is invalid: {error}"))?;
        if catalog.schema_version != 1 {
            return Err(format!(
                "This build cannot read AI recommendation catalog schema {}.",
                catalog.schema_version
            ));
        }
        Ok(catalog)
    }

    fn qualified(&self, id: &str) -> Result<QualifiedSelection, String> {
        let model = self
            .tiers
            .iter()
            .find(|entry| entry.id == id)
            .cloned()
            .ok_or_else(|| "That managed model is not in this build's bundled catalog.".to_string())?;
        if self.managed_runtime.status != "qualified" || !self.managed_runtime.verified.complete() {
            return Err(format!(
                "Managed installation is unavailable: {}",
                self.managed_runtime.blocker
            ));
        }
        if model.status != "qualified" {
            return Err(format!("This recommendation is unavailable: {}", model.blocker));
        }
        let missing_model = model.revision.is_none()
            || model.download_hash.is_none()
            || model.download_size_bytes.is_none()
            || model.context_limit_tokens.is_none()
            || model.template_requirements.is_none()
            || model.memory_needs_mb.is_none()
            || model.minimum_runtime_version.is_none()
            || model.download_url.is_none()
            || model.model_name.is_none();
        let missing_runtime = self.managed_runtime.download_url.is_none()
            || self.managed_runtime.sha256.is_none()
            || self.managed_runtime.size_bytes.is_none()
            || self.managed_runtime.executable.is_none()
            || self.managed_runtime.launch_args.is_none();
        if missing_model || missing_runtime {
            return Err(
                "This catalog entry is marked qualified but lacks required artifact, context, memory, or runtime evidence. Update Sprout before installing."
                    .into(),
            );
        }
        if model.minimum_runtime_version.as_deref()
            != Some(self.managed_runtime.candidate_version.as_str())
        {
            return Err(
                "The recommended model does not match this build's qualified managed runtime version. Update Sprout before installing."
                    .into(),
            );
        }
        Ok(QualifiedSelection {
            runtime: self.managed_runtime.clone(),
            model,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ManagedRuntimeView {
    pub name: String,
    pub version: String,
    pub artifact: String,
    pub source: String,
    pub license: String,
    pub status: String,
    pub blocker: String,
    pub qualified: bool,
    pub download_size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ManagedModelView {
    pub id: String,
    pub intent: String,
    pub artifact: String,
    pub source: String,
    pub revision: Option<String>,
    pub quantization: Option<String>,
    pub sha256: Option<String>,
    pub download_size_bytes: Option<u64>,
    pub license: String,
    pub license_source: String,
    pub status: String,
    pub blocker: String,
    pub context_limit_tokens: Option<u64>,
    pub template_requirements: Option<String>,
    pub memory_needs_mb: Option<u64>,
    pub minimum_runtime_version: Option<String>,
    pub installable: bool,
    pub installed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ManagedCatalogView {
    pub schema_version: u32,
    pub note: String,
    pub runtime: ManagedRuntimeView,
    pub models: Vec<ManagedModelView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ManagedInstallResult {
    pub model_id: String,
    pub installed: bool,
    pub message: String,
}

#[derive(Clone, Deserialize, Serialize)]
struct InstalledManifest {
    schema_version: u32,
    model_id: String,
    revision: String,
    model_name: String,
    model_file: String,
    model_sha256: String,
    runtime_version: String,
    runtime_executable: String,
    runtime_args: Vec<String>,
    runtime_sha256: String,
    context_limit_tokens: u64,
    template_requirements: String,
    memory_needs_mb: u64,
    idle_seconds: u64,
}

trait Downloader: Send + Sync {
    fn fetch(
        &self,
        url: &str,
        target: &Path,
        expected_bytes: u64,
        cancelled: &AtomicBool,
    ) -> Result<(), String>;
}

struct HttpDownloader;

impl Downloader for HttpDownloader {
    fn fetch(
        &self,
        url: &str,
        target: &Path,
        expected_bytes: u64,
        cancelled: &AtomicBool,
    ) -> Result<(), String> {
        if !url.starts_with("https://") {
            return Err("Managed artifacts must use an HTTPS download URL.".into());
        }
        if cancelled.load(Ordering::SeqCst) {
            return Err("Managed installation cancelled; no staged artifact was activated.".into());
        }
        let response = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(20))
            .timeout_read(Duration::from_secs(30))
            .redirects(0)
            .build()
            .get(url)
            .call()
            .map_err(|error| format!("Managed artifact download failed: {error}"))?;
        if response.status() != 200 {
            return Err(format!("Managed artifact download returned HTTP {}.", response.status()));
        }
        let mut reader = response.into_reader();
        let mut file = File::create(target)
            .map_err(|error| format!("Could not stage a managed artifact: {error}"))?;
        let mut total = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            if cancelled.load(Ordering::SeqCst) {
                return Err("Managed installation cancelled; no staged artifact was activated.".into());
            }
            let count = reader
                .read(&mut buffer)
                .map_err(|error| format!("Managed artifact download ended early: {error}"))?;
            if count == 0 {
                break;
            }
            total = total.saturating_add(count as u64);
            if total > expected_bytes {
                return Err("Managed artifact was larger than the qualified byte size.".into());
            }
            file.write_all(&buffer[..count])
                .map_err(|error| format!("Could not finish staging a managed artifact: {error}"))?;
        }
        file.sync_all()
            .map_err(|error| format!("Could not flush a staged managed artifact: {error}"))?;
        if total != expected_bytes {
            return Err(format!(
                "Managed artifact download was incomplete: expected {expected_bytes} bytes, received {total}."
            ));
        }
        Ok(())
    }
}

trait HostAdapter: Send + Sync {
    fn disk_bytes(&self, path: &Path) -> Result<u64, String>;
    fn memory_mb(&self) -> Result<u64, String>;
    fn extract_runtime(&self, archive: &Path, destination: &Path) -> Result<(), String>;
    fn reserve_port(&self) -> Result<u16, String>;
    fn spawn(&self, executable: &Path, args: &[String]) -> Result<Box<dyn RuntimeProcess>, String>;
}

trait RuntimeProcess: Send {
    fn exited(&mut self) -> Result<Option<i32>, String>;
    fn stop(&mut self);
}

struct WindowsHost;

struct WindowsRuntimeProcess(windows_execution::OwnedProcess);

impl RuntimeProcess for WindowsRuntimeProcess {
    fn exited(&mut self) -> Result<Option<i32>, String> {
        self.0.exited()
    }

    fn stop(&mut self) {
        self.0.stop();
    }
}

impl HostAdapter for WindowsHost {
    fn disk_bytes(&self, path: &Path) -> Result<u64, String> {
        windows_execution::available_disk_bytes(path)
    }

    fn memory_mb(&self) -> Result<u64, String> {
        windows_execution::system_memory_mb()
    }

    fn extract_runtime(&self, archive: &Path, destination: &Path) -> Result<(), String> {
        windows_execution::extract_zip_hidden(archive, destination)
    }

    fn reserve_port(&self) -> Result<u16, String> {
        TcpListener::bind(("127.0.0.1", 0))
            .and_then(|listener| listener.local_addr())
            .map(|address| address.port())
            .map_err(|error| format!("Could not reserve a loopback endpoint for managed AI: {error}"))
    }

    fn spawn(&self, executable: &Path, args: &[String]) -> Result<Box<dyn RuntimeProcess>, String> {
        windows_execution::spawn_owned_hidden(executable, args)
            .map(|process| Box::new(WindowsRuntimeProcess(process)) as Box<dyn RuntimeProcess>)
    }
}

trait Clock: Send + Sync {
    fn now_ms(&self) -> u64;
    fn sleep(&self, duration: Duration);
}

struct SystemClock {
    origin: Instant,
}

impl SystemClock {
    fn new() -> Self {
        Self { origin: Instant::now() }
    }
}

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        self.origin.elapsed().as_millis() as u64
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

trait LocalTransport: Send + Sync {
    fn healthy(&self, endpoint: SocketAddr, cancelled: &AtomicBool) -> Result<bool, String>;
    fn generate(
        &self,
        endpoint: SocketAddr,
        prompt: &DraftPrompt,
        model: &str,
        cancelled: &AtomicBool,
    ) -> Result<String, ProviderError>;
}

struct LoopbackTransport;

impl LocalTransport for LoopbackTransport {
    fn healthy(&self, endpoint: SocketAddr, cancelled: &AtomicBool) -> Result<bool, String> {
        match http_exchange(endpoint, "GET", "/health", None, cancelled, Duration::from_secs(2)) {
            Ok((200, _)) => Ok(true),
            Ok(_) => Ok(false),
            Err(ProviderError::Cancelled) => Err("Managed generation cancelled before startup.".into()),
            Err(_) => Ok(false),
        }
    }

    fn generate(
        &self,
        endpoint: SocketAddr,
        prompt: &DraftPrompt,
        model: &str,
        cancelled: &AtomicBool,
    ) -> Result<String, ProviderError> {
        let body = serde_json::json!({
            "model": model,
            "stream": false,
            "messages": [
                { "role": "system", "content": prompt.system },
                { "role": "user", "content": prompt.user },
            ],
        })
        .to_string();
        let (status, body) = http_exchange(
            endpoint,
            "POST",
            "/v1/chat/completions",
            Some(&body),
            cancelled,
            crate::ai_assist::GENERATION_TIMEOUT,
        )?;
        if status != 200 {
            return Err(ProviderError::Http(status));
        }
        let parsed: serde_json::Value =
            serde_json::from_str(&body).map_err(|_| ProviderError::Malformed)?;
        parsed
            .get("choices")
            .and_then(|choices| choices.as_array())
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(|content| content.as_str())
            .filter(|content| !content.trim().is_empty())
            .map(str::to_string)
            .ok_or_else(|| ProviderError::Unsupported("no usable message content".into()))
    }
}

fn http_exchange(
    endpoint: SocketAddr,
    method: &str,
    path: &str,
    body: Option<&str>,
    cancelled: &AtomicBool,
    timeout: Duration,
) -> Result<(u16, String), ProviderError> {
    if cancelled.load(Ordering::SeqCst) {
        return Err(ProviderError::Cancelled);
    }
    let mut stream = TcpStream::connect_timeout(&endpoint, Duration::from_secs(2))
        .map_err(|error| ProviderError::Unavailable(error.to_string()))?;
    stream
        .set_read_timeout(Some(IO_POLL))
        .map_err(|error| ProviderError::Unavailable(error.to_string()))?;
    stream
        .set_write_timeout(Some(IO_POLL))
        .map_err(|error| ProviderError::Unavailable(error.to_string()))?;
    let payload = body.unwrap_or_default();
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        endpoint.port(),
        payload.len()
    );
    for chunk in request.as_bytes().chunks(4096) {
        if cancelled.load(Ordering::SeqCst) {
            return Err(ProviderError::Cancelled);
        }
        stream
            .write_all(chunk)
            .map_err(|error| ProviderError::Unavailable(error.to_string()))?;
    }
    let started = Instant::now();
    let mut response = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        if cancelled.load(Ordering::SeqCst) {
            return Err(ProviderError::Cancelled);
        }
        if started.elapsed() >= timeout {
            return Err(ProviderError::Timeout);
        }
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                response.extend_from_slice(&buffer[..count]);
                if response.len() > MAX_HTTP_RESPONSE {
                    return Err(ProviderError::Oversized);
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(error) => return Err(ProviderError::Unavailable(error.to_string())),
        }
    }
    let split = response
        .windows(4)
        .position(|part| part == b"\r\n\r\n")
        .ok_or(ProviderError::Malformed)?;
    let head = std::str::from_utf8(&response[..split]).map_err(|_| ProviderError::Malformed)?;
    if head.to_ascii_lowercase().contains("transfer-encoding: chunked") {
        return Err(ProviderError::Unsupported(
            "chunked managed responses are outside the qualified contract".into(),
        ));
    }
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or(ProviderError::Malformed)?;
    if (300..400).contains(&status) {
        return Err(ProviderError::Redirected);
    }
    let body = String::from_utf8(response[(split + 4)..].to_vec())
        .map_err(|_| ProviderError::Malformed)?;
    Ok((status, body))
}

struct RunningRuntime {
    process: Box<dyn RuntimeProcess>,
    endpoint: SocketAddr,
    model_id: String,
}

#[derive(Default)]
struct Lifecycle {
    runtime: Option<RunningRuntime>,
    active_requests: usize,
    last_activity_ms: u64,
}

pub struct ManagedAi {
    root: PathBuf,
    catalog_json: &'static str,
    downloader: Arc<dyn Downloader>,
    host: Arc<dyn HostAdapter>,
    transport: Arc<dyn LocalTransport>,
    clock: Arc<dyn Clock>,
    lifecycle: Mutex<Lifecycle>,
    cancellations: Mutex<HashMap<String, Arc<AtomicBool>>>,
    installing: AtomicBool,
    install_cancelled: AtomicBool,
    exiting: AtomicBool,
}

impl ManagedAi {
    pub fn new(root: PathBuf) -> Self {
        Self::with_adapters(
            root,
            CATALOG_JSON,
            Arc::new(HttpDownloader),
            Arc::new(WindowsHost),
            Arc::new(LoopbackTransport),
            Arc::new(SystemClock::new()),
        )
    }

    fn with_adapters(
        root: PathBuf,
        catalog_json: &'static str,
        downloader: Arc<dyn Downloader>,
        host: Arc<dyn HostAdapter>,
        transport: Arc<dyn LocalTransport>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            root,
            catalog_json,
            downloader,
            host,
            transport,
            clock,
            lifecycle: Mutex::new(Lifecycle::default()),
            cancellations: Mutex::new(HashMap::new()),
            installing: AtomicBool::new(false),
            install_cancelled: AtomicBool::new(false),
            exiting: AtomicBool::new(false),
        }
    }

    pub fn start_idle_guard(this: &Arc<Self>) {
        let weak = Arc::downgrade(this);
        std::thread::spawn(move || idle_guard(weak));
    }

    pub fn catalog_status(&self) -> Result<ManagedCatalogView, String> {
        let catalog = Catalog::parse(self.catalog_json)?;
        let runtime_qualified = catalog.managed_runtime.status == "qualified"
            && catalog.managed_runtime.verified.complete();
        let models = catalog
            .tiers
            .iter()
            .map(|model| {
                let installable = runtime_qualified && catalog.qualified(&model.id).is_ok();
                ManagedModelView {
                    id: model.id.clone(),
                    intent: model.intent.clone(),
                    artifact: model.candidate_artifact.clone(),
                    source: model.artifact_source.clone(),
                    revision: model.revision.clone(),
                    quantization: model.quantization.clone(),
                    sha256: model.download_hash.clone(),
                    download_size_bytes: model.download_size_bytes,
                    license: model.license.clone(),
                    license_source: model.license_source.clone(),
                    status: model.status.clone(),
                    blocker: model.blocker.clone(),
                    context_limit_tokens: model.context_limit_tokens,
                    template_requirements: model.template_requirements.clone(),
                    memory_needs_mb: model.memory_needs_mb,
                    minimum_runtime_version: model.minimum_runtime_version.clone(),
                    installable,
                    installed: self.read_installed(&model.id).is_ok(),
                }
            })
            .collect();
        Ok(ManagedCatalogView {
            schema_version: catalog.schema_version,
            note: catalog.note,
            runtime: ManagedRuntimeView {
                name: catalog.managed_runtime.name.clone(),
                version: catalog.managed_runtime.candidate_version.clone(),
                artifact: catalog.managed_runtime.windows_cpu_x64_artifact.clone(),
                source: catalog.managed_runtime.source.clone(),
                license: catalog.managed_runtime.license.clone(),
                status: catalog.managed_runtime.status.clone(),
                blocker: catalog.managed_runtime.blocker.clone(),
                qualified: runtime_qualified,
                download_size_bytes: catalog.managed_runtime.size_bytes,
            },
            models,
        })
    }

    pub fn install(&self, id: &str) -> Result<ManagedInstallResult, String> {
        if self
            .installing
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err("A managed installation is already in progress.".into());
        }
        self.install_cancelled.store(false, Ordering::SeqCst);
        let result = self.install_inner(id);
        self.installing.store(false, Ordering::SeqCst);
        result
    }

    pub fn cancel_install(&self) -> bool {
        if self.installing.load(Ordering::SeqCst) {
            self.install_cancelled.store(true, Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    fn install_inner(&self, id: &str) -> Result<ManagedInstallResult, String> {
        let selection = Catalog::parse(self.catalog_json)?.qualified(id)?;
        if self.read_installed(id).is_ok() {
            return Ok(ManagedInstallResult {
                model_id: id.into(),
                installed: true,
                message: "The verified managed model is already installed for this user.".into(),
            });
        }
        fs::create_dir_all(&self.root)
            .map_err(|error| format!("Could not create the per-user managed AI folder: {error}"))?;
        let runtime_bytes = selection.runtime.size_bytes.unwrap_or_default();
        let model_bytes = selection.model.download_size_bytes.unwrap_or_default();
        let required_disk = runtime_bytes
            .saturating_add(model_bytes)
            .saturating_mul(2);
        let available = self.host.disk_bytes(&self.root)?;
        if available < required_disk {
            return Err(format!(
                "Managed setup needs {} bytes free to stage and activate verified artifacts; only {} bytes are available.",
                required_disk, available
            ));
        }
        let required_memory = selection.model.memory_needs_mb.unwrap_or_default();
        let available_memory = self.host.memory_mb()?;
        if available_memory < required_memory {
            return Err(format!(
                "This qualified model needs {} MB of working RAM; this PC reports {} MB. Download size is not a RAM requirement.",
                required_memory, available_memory
            ));
        }
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let staging = self.root.join("staging").join(format!("{}-{nonce}", safe_segment(id)));
        fs::create_dir_all(&staging)
            .map_err(|error| format!("Could not create a managed staging folder: {error}"))?;
        let mut cleanup = StagingCleanup(Some(staging.clone()));
        let runtime_archive = staging.join(&selection.runtime.windows_cpu_x64_artifact);
        let model_file_name = file_name_from_url(
            selection.model.download_url.as_deref().unwrap_or_default(),
            "model.gguf",
        );
        let staged_model = staging.join(&model_file_name);
        self.downloader.fetch(
            selection.runtime.download_url.as_deref().unwrap_or_default(),
            &runtime_archive,
            runtime_bytes,
            &self.install_cancelled,
        )?;
        verify_file(
            &runtime_archive,
            runtime_bytes,
            selection.runtime.sha256.as_deref().unwrap_or_default(),
        )?;
        self.downloader.fetch(
            selection.model.download_url.as_deref().unwrap_or_default(),
            &staged_model,
            model_bytes,
            &self.install_cancelled,
        )?;
        verify_file(
            &staged_model,
            model_bytes,
            selection.model.download_hash.as_deref().unwrap_or_default(),
        )?;
        if self.install_cancelled.load(Ordering::SeqCst) {
            return Err("Managed installation cancelled; no staged artifact was activated.".into());
        }
        let runtime_dir = staging.join("runtime");
        fs::create_dir_all(&runtime_dir)
            .map_err(|error| format!("Could not stage the managed runtime: {error}"))?;
        self.host.extract_runtime(&runtime_archive, &runtime_dir)?;
        let executable = selection.runtime.executable.as_deref().unwrap_or_default();
        if !safe_relative(executable) || !runtime_dir.join(executable).is_file() {
            return Err("The verified runtime archive does not contain its qualified executable.".into());
        }
        let revision = selection.model.revision.clone().unwrap_or_default();
        let final_dir = self
            .root
            .join("installations")
            .join(safe_segment(id))
            .join(safe_segment(&revision));
        if final_dir.exists() {
            return Err(
                "An incomplete or incompatible installation already occupies this model revision. Nothing was replaced; update Sprout or remove that app-owned selection explicitly."
                    .into(),
            );
        }
        let manifest = InstalledManifest {
            schema_version: 1,
            model_id: id.into(),
            revision,
            model_name: selection.model.model_name.unwrap_or_default(),
            model_file: model_file_name,
            model_sha256: selection.model.download_hash.unwrap_or_default(),
            runtime_version: selection.runtime.candidate_version,
            runtime_executable: format!("runtime/{executable}"),
            runtime_args: selection.runtime.launch_args.unwrap_or_default(),
            runtime_sha256: selection.runtime.sha256.unwrap_or_default(),
            context_limit_tokens: selection.model.context_limit_tokens.unwrap_or_default(),
            template_requirements: selection.model.template_requirements.unwrap_or_default(),
            memory_needs_mb: required_memory,
            idle_seconds: IDLE_INTERVAL.as_secs(),
        };
        fs::write(
            staging.join(INSTALL_MANIFEST),
            serde_json::to_vec_pretty(&manifest)
                .map_err(|error| format!("Could not record the managed installation: {error}"))?,
        )
        .map_err(|error| format!("Could not record the managed installation: {error}"))?;
        fs::create_dir_all(final_dir.parent().unwrap_or(&self.root))
            .map_err(|error| format!("Could not activate the managed installation: {error}"))?;
        fs::rename(&staging, &final_dir)
            .map_err(|error| format!("Could not activate the verified managed installation: {error}"))?;
        cleanup.0 = None;
        Ok(ManagedInstallResult {
            model_id: id.into(),
            installed: true,
            message: "Installed verified runtime and model artifacts for this user. The runtime stays stopped until Generate is used.".into(),
        })
    }

    fn read_installed(&self, id: &str) -> Result<(InstalledManifest, PathBuf), String> {
        let model_root = self.root.join("installations").join(safe_segment(id));
        let entries = fs::read_dir(&model_root)
            .map_err(|_| "Install this qualified managed model before generating.".to_string())?;
        for entry in entries.flatten() {
            let path = entry.path();
            let manifest_path = path.join(INSTALL_MANIFEST);
            let Ok(bytes) = fs::read(&manifest_path) else {
                continue;
            };
            let Ok(manifest) = serde_json::from_slice::<InstalledManifest>(&bytes) else {
                continue;
            };
            if manifest.schema_version == 1
                && manifest.model_id == id
                && safe_relative(&manifest.model_file)
                && safe_relative(&manifest.runtime_executable)
                && path.join(&manifest.model_file).is_file()
                && path.join(&manifest.runtime_executable).is_file()
            {
                return Ok((manifest, path));
            }
        }
        Err("Install this qualified managed model before generating.".into())
    }

    pub fn begin_request(self: &Arc<Self>, request_id: &str, model_id: &str) -> Result<ManagedRequest, String> {
        if request_id.trim().is_empty() {
            return Err("Managed generation needs a request id for cancellation.".into());
        }
        if self.exiting.load(Ordering::SeqCst) {
            return Err("Sprout is exiting; managed generation did not start.".into());
        }
        let (manifest, install_dir) = self.read_installed(model_id)?;
        let cancelled = Arc::new(AtomicBool::new(false));
        {
            let mut cancellations = self
                .cancellations
                .lock()
                .map_err(|_| "Managed AI cancellation state is unavailable.".to_string())?;
            if cancellations.contains_key(request_id) {
                return Err("That managed generation request is already active.".into());
            }
            cancellations.insert(request_id.into(), Arc::clone(&cancelled));
        }
        let endpoint = {
            let mut state = match self.lifecycle.lock() {
                Ok(state) => state,
                Err(_) => {
                    self.remove_cancellation(request_id);
                    return Err("Managed AI state is unavailable.".into());
                }
            };
            if let Err(error) = ensure_runtime(
                &mut state,
                model_id,
                &manifest,
                &install_dir,
                self.host.as_ref(),
                self.transport.as_ref(),
                self.clock.as_ref(),
                &cancelled,
            ) {
                self.remove_cancellation(request_id);
                return Err(error);
            }
            state.active_requests += 1;
            match state.runtime.as_ref() {
                Some(runtime) => runtime.endpoint,
                None => {
                    state.active_requests = state.active_requests.saturating_sub(1);
                    self.remove_cancellation(request_id);
                    return Err("Managed runtime did not start.".into());
                }
            }
        };
        Ok(ManagedRequest {
            owner: Arc::clone(self),
            request_id: request_id.into(),
            endpoint,
            model_name: manifest.model_name,
            cancelled,
        })
    }

    pub fn cancel_request(&self, request_id: &str) -> bool {
        let Ok(cancellations) = self.cancellations.lock() else {
            return false;
        };
        let Some(cancelled) = cancellations.get(request_id) else {
            return false;
        };
        cancelled.store(true, Ordering::SeqCst);
        true
    }

    fn remove_cancellation(&self, request_id: &str) -> bool {
        self.cancellations
            .lock()
            .map(|mut cancellations| cancellations.remove(request_id).is_some())
            .unwrap_or(false)
    }

    fn finish_request(&self, request_id: &str) {
        if self.remove_cancellation(request_id) {
            if let Ok(mut state) = self.lifecycle.lock() {
                state.active_requests = state.active_requests.saturating_sub(1);
                state.last_activity_ms = self.clock.now_ms();
            }
        }
    }

    pub fn reap_idle(&self) -> bool {
        let Ok(mut state) = self.lifecycle.lock() else {
            return false;
        };
        if state.active_requests != 0
            || state.runtime.is_none()
            || self.clock.now_ms().saturating_sub(state.last_activity_ms)
                < IDLE_INTERVAL.as_millis() as u64
        {
            return false;
        }
        stop_runtime(&mut state);
        true
    }

    pub fn shutdown(&self) {
        self.exiting.store(true, Ordering::SeqCst);
        self.install_cancelled.store(true, Ordering::SeqCst);
        if let Ok(cancellations) = self.cancellations.lock() {
            for cancellation in cancellations.values() {
                cancellation.store(true, Ordering::SeqCst);
            }
        }
        if let Ok(mut state) = self.lifecycle.lock() {
            stop_runtime(&mut state);
        }
    }
}

fn idle_guard(owner: Weak<ManagedAi>) {
    loop {
        std::thread::sleep(Duration::from_secs(1));
        let Some(owner) = owner.upgrade() else {
            return;
        };
        if owner.exiting.load(Ordering::SeqCst) {
            return;
        }
        owner.reap_idle();
    }
}

fn ensure_runtime(
    state: &mut Lifecycle,
    model_id: &str,
    manifest: &InstalledManifest,
    install_dir: &Path,
    host: &dyn HostAdapter,
    transport: &dyn LocalTransport,
    clock: &dyn Clock,
    cancelled: &AtomicBool,
) -> Result<(), String> {
    if let Some(runtime) = state.runtime.as_mut() {
        match runtime.process.exited()? {
            None if runtime.model_id == model_id => return Ok(()),
            None if state.active_requests > 0 => {
                return Err("Another managed model is serving an active request; try again when it finishes.".into())
            }
            None => stop_runtime(state),
            Some(_) => state.runtime = None,
        }
    }
    let port = host.reserve_port()?;
    let endpoint = SocketAddr::from(([127, 0, 0, 1], port));
    let executable = install_dir.join(&manifest.runtime_executable);
    let model = install_dir.join(&manifest.model_file);
    let args = manifest
        .runtime_args
        .iter()
        .map(|arg| {
            arg.replace("{port}", &port.to_string())
                .replace("{model}", &model.to_string_lossy())
        })
        .collect::<Vec<_>>();
    let process = host.spawn(&executable, &args)?;
    state.runtime = Some(RunningRuntime {
        process,
        endpoint,
        model_id: model_id.into(),
    });
    let started = clock.now_ms();
    loop {
        if cancelled.load(Ordering::SeqCst) {
            stop_runtime(state);
            return Err("Managed generation cancelled during runtime startup.".into());
        }
        let runtime = state.runtime.as_mut().ok_or_else(|| "Managed runtime disappeared during startup.".to_string())?;
        if let Some(code) = runtime.process.exited()? {
            state.runtime = None;
            return Err(format!("Managed runtime exited during startup with code {code}. Try Generate again; no foreign process was touched."));
        }
        match transport.healthy(endpoint, cancelled) {
            Ok(true) => {
                state.last_activity_ms = clock.now_ms();
                return Ok(());
            }
            Ok(false) => {}
            Err(error) => {
                stop_runtime(state);
                return Err(error);
            }
        }
        if clock.now_ms().saturating_sub(started) >= STARTUP_TIMEOUT.as_millis() as u64 {
            stop_runtime(state);
            return Err("Managed runtime did not become healthy in 30 seconds. Try again; no foreign process was touched.".into());
        }
        clock.sleep(IO_POLL);
    }
}

fn stop_runtime(state: &mut Lifecycle) {
    if let Some(mut runtime) = state.runtime.take() {
        runtime.process.stop();
    }
}

pub struct ManagedRequest {
    owner: Arc<ManagedAi>,
    request_id: String,
    endpoint: SocketAddr,
    model_name: String,
    cancelled: Arc<AtomicBool>,
}

impl DraftProvider for ManagedRequest {
    fn generate(&self, prompt: &DraftPrompt, _model: &str) -> Result<String, ProviderError> {
        self.owner.transport.generate(
            self.endpoint,
            prompt,
            &self.model_name,
            &self.cancelled,
        )
    }
}

impl Drop for ManagedRequest {
    fn drop(&mut self) {
        self.owner.finish_request(&self.request_id);
    }
}

struct StagingCleanup(Option<PathBuf>);

impl Drop for StagingCleanup {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = fs::remove_dir_all(path);
        }
    }
}

fn safe_segment(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn safe_relative(value: &str) -> bool {
    let path = Path::new(value);
    !value.trim().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
}

fn file_name_from_url(url: &str, fallback: &str) -> String {
    url.rsplit('/')
        .next()
        .and_then(|part| part.split(['?', '#']).next())
        .filter(|part| !part.is_empty() && safe_relative(part))
        .unwrap_or(fallback)
        .to_string()
}

fn verify_file(path: &Path, expected_bytes: u64, expected_sha256: &str) -> Result<(), String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("Could not read a staged managed artifact: {error}"))?;
    if bytes.len() as u64 != expected_bytes {
        return Err("A staged managed artifact does not match its qualified byte size.".into());
    }
    let actual = sha256_hex(&bytes);
    if actual != expected_sha256.to_ascii_lowercase() {
        return Err(format!(
            "Managed artifact checksum mismatch; expected {}, received {}. Nothing was activated.",
            expected_sha256, actual
        ));
    }
    Ok(())
}

fn sha256_hex(input: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1,
        0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
        0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
        0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147,
        0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
        0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
        0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut state = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    let bit_len = (input.len() as u64).wrapping_mul(8);
    let mut padded = input.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in padded.chunks_exact(64) {
        let mut words = [0u32; 64];
        for (index, word) in words[..16].iter_mut().enumerate() {
            *word = u32::from_be_bytes(chunk[index * 4..index * 4 + 4].try_into().unwrap());
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }
        let mut work = state;
        for index in 0..64 {
            let sum1 = work[4].rotate_right(6)
                ^ work[4].rotate_right(11)
                ^ work[4].rotate_right(25);
            let choose = (work[4] & work[5]) ^ (!work[4] & work[6]);
            let temp1 = work[7]
                .wrapping_add(sum1)
                .wrapping_add(choose)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let sum0 = work[0].rotate_right(2)
                ^ work[0].rotate_right(13)
                ^ work[0].rotate_right(22);
            let majority = (work[0] & work[1]) ^ (work[0] & work[2]) ^ (work[1] & work[2]);
            let temp2 = sum0.wrapping_add(majority);
            work = [
                temp1.wrapping_add(temp2),
                work[0],
                work[1],
                work[2],
                work[3].wrapping_add(temp1),
                work[4],
                work[5],
                work[6],
            ];
        }
        for index in 0..8 {
            state[index] = state[index].wrapping_add(work[index]);
        }
    }
    state.iter().map(|word| format!("{word:08x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, AtomicUsize};

    const RUNTIME_BYTES: &[u8] = b"runtime archive";
    const MODEL_BYTES: &[u8] = b"model weights";

    struct FakeDownloader {
        corrupt_model: bool,
        cancelled: bool,
    }

    impl Downloader for FakeDownloader {
        fn fetch(&self, url: &str, target: &Path, _: u64, cancelled: &AtomicBool) -> Result<(), String> {
            if self.cancelled {
                cancelled.store(true, Ordering::SeqCst);
                return Err("Managed installation cancelled; no staged artifact was activated.".into());
            }
            let bytes = if url.ends_with("runtime.zip") {
                RUNTIME_BYTES
            } else if self.corrupt_model {
                b"corrupt"
            } else {
                MODEL_BYTES
            };
            fs::write(target, bytes).map_err(|error| error.to_string())
        }
    }

    struct FakeProcess {
        stops: Arc<AtomicUsize>,
        crashed: bool,
    }

    impl RuntimeProcess for FakeProcess {
        fn exited(&mut self) -> Result<Option<i32>, String> {
            Ok(self.crashed.then_some(9))
        }
        fn stop(&mut self) { self.stops.fetch_add(1, Ordering::SeqCst); }
    }

    struct FakeHost {
        spawns: Arc<AtomicUsize>,
        stops: Arc<AtomicUsize>,
        fail_port: bool,
        crash: bool,
    }

    impl HostAdapter for FakeHost {
        fn disk_bytes(&self, _: &Path) -> Result<u64, String> { Ok(u64::MAX) }
        fn memory_mb(&self) -> Result<u64, String> { Ok(u64::MAX) }
        fn extract_runtime(&self, _: &Path, destination: &Path) -> Result<(), String> {
            fs::write(destination.join("llama-server.exe"), b"exe").map_err(|error| error.to_string())
        }
        fn reserve_port(&self) -> Result<u16, String> {
            if self.fail_port { Err("occupied endpoint".into()) } else { Ok(32123) }
        }
        fn spawn(&self, _: &Path, _: &[String]) -> Result<Box<dyn RuntimeProcess>, String> {
            self.spawns.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(FakeProcess { stops: Arc::clone(&self.stops), crashed: self.crash }))
        }
    }

    struct FakeClock(AtomicU64);

    impl Clock for FakeClock {
        fn now_ms(&self) -> u64 { self.0.load(Ordering::SeqCst) }
        fn sleep(&self, duration: Duration) { self.0.fetch_add(duration.as_millis() as u64, Ordering::SeqCst); }
    }

    struct FakeTransport {
        healthy: bool,
    }

    impl LocalTransport for FakeTransport {
        fn healthy(&self, _: SocketAddr, cancelled: &AtomicBool) -> Result<bool, String> {
            if cancelled.load(Ordering::SeqCst) { return Err("cancelled".into()); }
            Ok(self.healthy)
        }
        fn generate(&self, _: SocketAddr, _: &DraftPrompt, _: &str, cancelled: &AtomicBool) -> Result<String, ProviderError> {
            if cancelled.load(Ordering::SeqCst) { Err(ProviderError::Cancelled) } else { Ok("{}".into()) }
        }
    }

    struct WaitForCancellationTransport {
        entered: Arc<AtomicBool>,
    }

    impl LocalTransport for WaitForCancellationTransport {
        fn healthy(&self, _: SocketAddr, cancelled: &AtomicBool) -> Result<bool, String> {
            self.entered.store(true, Ordering::SeqCst);
            while !cancelled.load(Ordering::SeqCst) {
                std::thread::yield_now();
            }
            Err("Managed generation cancelled during runtime startup.".into())
        }

        fn generate(&self, _: SocketAddr, _: &DraftPrompt, _: &str, _: &AtomicBool) -> Result<String, ProviderError> {
            unreachable!("startup cancellation never reaches generation")
        }
    }

    fn qualified_catalog() -> &'static str {
        Box::leak(format!(r#"{{
          "schema_version":1,"note":"qualified fixture",
          "managed_runtime":{{"name":"llama.cpp","candidate_version":"v1","windows_cpu_x64_artifact":"runtime.zip","source":"https://example/runtime","license":"MIT","status":"qualified","verified":{{"download":true,"per_user_launch":true,"health":true,"cancellation":true,"model_release":true,"process_ownership":true,"exit_behavior":true}},"blocker":"","download_url":"https://example/runtime.zip","sha256":"{}","size_bytes":{},"executable":"llama-server.exe","launch_args":["--host","127.0.0.1","--port","{{port}}","--model","{{model}}"]}},
          "tiers":[{{"id":"lite","intent":"fixture","candidate_artifact":"lite.gguf","artifact_source":"https://example/model","revision":"rev1","quantization":"Q4","download_hash":"{}","download_size_bytes":{},"license":"Apache-2.0","license_source":"https://example/license","status":"qualified","blocker":"","context_limit_tokens":4096,"template_requirements":"chatml","memory_needs_mb":2048,"minimum_runtime_version":"v1","download_url":"https://example/model.gguf","model_name":"lite"}}]
        }}"#, sha256_hex(RUNTIME_BYTES), RUNTIME_BYTES.len(), sha256_hex(MODEL_BYTES), MODEL_BYTES.len()).into_boxed_str())
    }

    fn fixture(root: PathBuf, downloader: Arc<dyn Downloader>, host: Arc<FakeHost>, clock: Arc<FakeClock>, transport: Arc<dyn LocalTransport>) -> Arc<ManagedAi> {
        Arc::new(ManagedAi::with_adapters(root, qualified_catalog(), downloader, host, transport, clock))
    }

    #[test]
    fn shipped_catalog_is_fail_closed_and_status_creates_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("managed");
        let manager = ManagedAi::new(root.clone());
        let status = manager.catalog_status().unwrap();
        assert!(!status.runtime.qualified);
        assert!(status.models.iter().all(|model| !model.installable));
        assert!(!root.exists());
    }

    #[test]
    fn sha256_matches_the_standard_vector() {
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn install_activates_only_after_both_artifacts_verify() {
        let dir = tempfile::tempdir().unwrap();
        let host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::new(AtomicUsize::new(0)), fail_port: false, crash: false });
        let manager = fixture(dir.path().join("managed"), Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }), host, Arc::new(FakeClock(AtomicU64::new(0))), Arc::new(FakeTransport { healthy: true }));
        assert!(manager.install("lite").unwrap().installed);
        let (manifest, path) = manager.read_installed("lite").unwrap();
        assert_eq!(manifest.runtime_version, "v1");
        assert!(path.join(INSTALL_MANIFEST).is_file());
        assert!(!manager.root.join("staging").read_dir().map(|mut entries| entries.next().is_some()).unwrap_or(false));
    }

    #[test]
    fn corrupt_or_cancelled_download_never_activates() {
        for downloader in [
            FakeDownloader { corrupt_model: true, cancelled: false },
            FakeDownloader { corrupt_model: false, cancelled: true },
        ] {
            let dir = tempfile::tempdir().unwrap();
            let host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::new(AtomicUsize::new(0)), fail_port: false, crash: false });
            let manager = fixture(dir.path().join("managed"), Arc::new(downloader), host, Arc::new(FakeClock(AtomicU64::new(0))), Arc::new(FakeTransport { healthy: true }));
            assert!(manager.install("lite").is_err());
            assert!(manager.read_installed("lite").is_err());
        }
    }

    #[test]
    fn overlapping_requests_share_one_owned_runtime_and_idle_reaps_it() {
        let dir = tempfile::tempdir().unwrap();
        let spawns = Arc::new(AtomicUsize::new(0));
        let stops = Arc::new(AtomicUsize::new(0));
        let host = Arc::new(FakeHost { spawns: Arc::clone(&spawns), stops: Arc::clone(&stops), fail_port: false, crash: false });
        let clock = Arc::new(FakeClock(AtomicU64::new(0)));
        let manager = fixture(dir.path().join("managed"), Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }), host, Arc::clone(&clock), Arc::new(FakeTransport { healthy: true }));
        manager.install("lite").unwrap();
        let first = manager.begin_request("first", "lite").unwrap();
        let second = manager.begin_request("second", "lite").unwrap();
        assert_eq!(spawns.load(Ordering::SeqCst), 1);
        assert!(manager.cancel_request("first"));
        let prompt = DraftPrompt { system: "system".into(), user: "user".into() };
        assert!(matches!(first.generate(&prompt, "lite"), Err(ProviderError::Cancelled)));
        assert!(!manager.reap_idle());
        drop(first);
        drop(second);
        clock.0.store(IDLE_INTERVAL.as_millis() as u64 + 1, Ordering::SeqCst);
        assert!(manager.reap_idle());
        assert_eq!(stops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn crashes_and_occupied_endpoints_fail_without_foreign_kills() {
        let dir = tempfile::tempdir().unwrap();
        let stops = Arc::new(AtomicUsize::new(0));
        let host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::clone(&stops), fail_port: true, crash: false });
        let manager = fixture(dir.path().join("managed"), Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }), host, Arc::new(FakeClock(AtomicU64::new(0))), Arc::new(FakeTransport { healthy: true }));
        manager.install("lite").unwrap();
        let error = manager
            .begin_request("occupied", "lite")
            .err()
            .expect("the occupied endpoint must fail before returning a request");
        assert!(error.contains("occupied endpoint"));
        assert_eq!(stops.load(Ordering::SeqCst), 0);

        let crash_dir = tempfile::tempdir().unwrap();
        let crash_stops = Arc::new(AtomicUsize::new(0));
        let crash_host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::clone(&crash_stops), fail_port: false, crash: true });
        let crash_manager = fixture(crash_dir.path().join("managed"), Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }), crash_host, Arc::new(FakeClock(AtomicU64::new(0))), Arc::new(FakeTransport { healthy: true }));
        crash_manager.install("lite").unwrap();
        let crash_error = crash_manager
            .begin_request("crashed", "lite")
            .err()
            .expect("an exited runtime must fail before returning a request");
        assert!(crash_error.contains("exited during startup"));
        assert_eq!(crash_stops.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn startup_timeout_and_cancellation_stop_the_owned_process() {
        let timeout_dir = tempfile::tempdir().unwrap();
        let timeout_stops = Arc::new(AtomicUsize::new(0));
        let timeout_host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::clone(&timeout_stops), fail_port: false, crash: false });
        let timeout_manager = fixture(timeout_dir.path().join("managed"), Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }), timeout_host, Arc::new(FakeClock(AtomicU64::new(0))), Arc::new(FakeTransport { healthy: false }));
        timeout_manager.install("lite").unwrap();
        let timeout_error = timeout_manager
            .begin_request("timeout", "lite")
            .err()
            .expect("an unhealthy runtime must time out");
        assert!(timeout_error.contains("did not become healthy"));
        assert_eq!(timeout_stops.load(Ordering::SeqCst), 1);

        let cancel_dir = tempfile::tempdir().unwrap();
        let cancel_stops = Arc::new(AtomicUsize::new(0));
        let cancel_host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::clone(&cancel_stops), fail_port: false, crash: false });
        let entered = Arc::new(AtomicBool::new(false));
        let cancel_manager = fixture(
            cancel_dir.path().join("managed"),
            Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }),
            cancel_host,
            Arc::new(FakeClock(AtomicU64::new(0))),
            Arc::new(WaitForCancellationTransport { entered: Arc::clone(&entered) }),
        );
        cancel_manager.install("lite").unwrap();
        let worker_manager = Arc::clone(&cancel_manager);
        let worker = std::thread::spawn(move || match worker_manager.begin_request("starting", "lite") {
            Ok(_) => panic!("startup cancellation unexpectedly returned a request"),
            Err(error) => error,
        });
        while !entered.load(Ordering::SeqCst) {
            std::thread::yield_now();
        }
        assert!(cancel_manager.cancel_request("starting"));
        let cancel_error = worker.join().unwrap();
        assert!(cancel_error.contains("cancelled during runtime startup"));
        assert_eq!(cancel_stops.load(Ordering::SeqCst), 1);
        assert!(!cancel_manager.cancel_request("starting"));
    }

    #[test]
    fn actual_shutdown_stops_only_the_owned_process() {
        let dir = tempfile::tempdir().unwrap();
        let stops = Arc::new(AtomicUsize::new(0));
        let host = Arc::new(FakeHost { spawns: Arc::new(AtomicUsize::new(0)), stops: Arc::clone(&stops), fail_port: false, crash: false });
        let manager = fixture(dir.path().join("managed"), Arc::new(FakeDownloader { corrupt_model: false, cancelled: false }), host, Arc::new(FakeClock(AtomicU64::new(0))), Arc::new(FakeTransport { healthy: true }));
        manager.install("lite").unwrap();
        let request = manager.begin_request("active", "lite").unwrap();
        manager.shutdown();
        assert_eq!(stops.load(Ordering::SeqCst), 1);
        drop(request);
    }
}
