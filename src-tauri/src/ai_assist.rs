//! AI-assisted Quick Action authoring over the user's own on-device service.
//!
//! The one deep authoring interface behind every generation request: it loads
//! the bundled skill pair, runs the request checks before inference, talks to
//! exactly one provider route, and runs the output checks before any candidate
//! becomes usable. Generation, validation, cancellation, and saving never
//! execute generated text — running stays with the existing manual controls,
//! which keep working when AI is off or unavailable (ADR-0030).
//!
//! Existing-local uses a user-managed loopback service; managed local borrows
//! an app-owned request-scoped provider through the same interface. Cloud
//! remains named but fails closed until its own slice lands (ADR-0031).
//! Sprout never launches, stops, unloads, reconfigures, or deletes a user's
//! existing-local service — it only sends that service prompts.
//!
//! Safety layering, stated plainly: request refusal, output rejection, and
//! the absence of execution authority are three distinct layers. The scans
//! below reduce risk; they never promise arbitrary-script safety and are
//! never described as a sandbox (ADR-0030). Effect and composition decide —
//! a benign first step never launders a destructive whole, and a model
//! rating its own output safe is never authorization.

use std::io::Read;
#[cfg(test)]
use std::collections::VecDeque;
#[cfg(test)]
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::quick_actions::QuickActionShell;

/// The plain product boundary shown on every refusal. Shared rules carry the
/// same sentence; the backend repeats it so a refusal is complete even if a
/// skill file ever fails to load (ADR-0030).
pub const REFUSAL_BOUNDARY: &str = "Destructive commands are outside AI assistance. You can write and manage those commands in the manual editor.";

/// Bounds are implementation defaults with boundary tests, not measured user
/// commitments — qualification recorded no latency or budget evidence, so
/// these stay conservative and documented here.
pub const MAX_REQUEST_CHARS: usize = 2000;
pub const MAX_CONTEXT_CHARS: usize = 2000;
pub const MAX_COMMAND_CHARS: usize = 8000;
pub const MAX_RESPONSE_BYTES: u64 = 65536;
pub const GENERATION_TIMEOUT: Duration = Duration::from_secs(90);
pub const MODELS_TIMEOUT: Duration = Duration::from_secs(15);

/// The bundled authoring instructions, embedded at compile time so the
/// installed app serves the exact pinned pair without a repository checkout
/// (ADR-0032).
const SHARED_RULES: &str = include_str!("../resources/ai-skills/shared-rules.md");
const CREATE_SKILL: &str = include_str!("../resources/ai-skills/create-quick-action.md");

/// The provider route for one request. One route per request, never a silent
/// fallback to another (ADR-0031).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiProvider {
    Off,
    ExistingLocal,
    Managed,
    Cloud,
}

impl AiProvider {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "off" => Some(AiProvider::Off),
            "existing-local" => Some(AiProvider::ExistingLocal),
            "managed" => Some(AiProvider::Managed),
            "cloud" => Some(AiProvider::Cloud),
            _ => None,
        }
    }
}

/// The validated shape of the Settings AI knobs, resolved before any request.
pub struct AiRoute {
    pub provider: AiProvider,
    pub root: String,
    pub model: String,
}

/// Validates the persisted AI knobs. Managed and cloud selections save fine —
/// setup stays discoverable — but generation through them fails closed until
/// their own slices land, so saving one never pretends it works (ADR-0031).
pub fn validate_ai_settings(provider: &str, base_url: &str, model: &str) -> Result<(), String> {
    let route = AiProvider::parse(provider)
        .ok_or_else(|| "AI provider must be \"off\", \"existing-local\", \"managed\", or \"cloud\"".to_string())?;
    if model.chars().count() > 200 {
        return Err("AI model name must be at most 200 characters".into());
    }
    match route {
        AiProvider::Off | AiProvider::Managed | AiProvider::Cloud => Ok(()),
        AiProvider::ExistingLocal => {
            classify_endpoint(base_url)?;
            if model.trim().is_empty() {
                return Err(
                    "Pick the model your local service exposes — Sprout never substitutes another one."
                        .into(),
                );
            }
            Ok(())
        }
    }
}

/// Resolves the saved knobs to the single route one generation request uses.
pub fn resolve_route(provider: &str, base_url: &str, model: &str) -> Result<AiRoute, String> {
    validate_ai_settings(provider, base_url, model)?;
    let provider = AiProvider::parse(provider).unwrap_or(AiProvider::Off);
    let root = match provider {
        AiProvider::ExistingLocal => classify_endpoint(base_url)?,
        AiProvider::Off | AiProvider::Managed | AiProvider::Cloud => String::new(),
    };
    Ok(AiRoute {
        provider,
        root,
        model: model.trim().to_string(),
    })
}

/// Classifies an existing-local base URL and returns its normalized root.
/// v1 speaks plain HTTP to loopback only: a non-loopback destination takes
/// the external disclosure path or is refused, never labeled private
/// on-device inference (ADR-0031). Anything outside the tested shape fails
/// with an actionable message instead of degrading silently.
pub fn classify_endpoint(base_url: &str) -> Result<String, String> {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return Err(
            "Enter your service's address, for example http://127.0.0.1:11434.".into(),
        );
    }
    if trimmed.contains(char::is_whitespace) {
        return Err("The service address must not contain spaces.".into());
    }
    let lower = trimmed.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("http://")
        .ok_or_else(|| {
            if lower.starts_with("https://") {
                "This build speaks plain HTTP to a local service only — use the http:// address your service prints on startup.".to_string()
            } else {
                "The service address must start with http://, for example http://127.0.0.1:11434.".to_string()
            }
        })?;
    if rest.contains('@') {
        return Err("The service address must not carry credentials.".into());
    }
    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    if authority.is_empty() {
        return Err(
            "Enter your service's address, for example http://127.0.0.1:11434.".into(),
        );
    }
    if rest.len() != authority.len() && rest[authority.len()..].contains(['?', '#']) {
        return Err("The service address is the server root — no query or fragment.".into());
    }
    let (host, port) = if let Some(after_bracket) = authority.strip_prefix('[') {
        let (host, rest) = after_bracket.split_once(']').unwrap_or((after_bracket, ""));
        (host, rest.strip_prefix(':'))
    } else {
        match authority.rsplit_once(':') {
            Some((host, port)) if !host.is_empty() && !port.contains(':') => (host, Some(port)),
            _ => (authority, None),
        }
    };
    if !is_loopback_host(host) {
        return Err(
            "That address is not this machine — v1 connects to a loopback service only (localhost, 127.0.0.1, or ::1).".into(),
        );
    }
    if let Some(port) = port {
        if port.is_empty() || port.parse::<u16>().is_err() {
            return Err("The service address carries an invalid port.".into());
        }
    }
    Ok(format!("http://{}", authority))
}

/// Loopback means this machine, not merely nearby: localhost, the 127/8
/// block, or ::1. Unspecified (0.0.0.0) and LAN names are refused — the
/// former names no single service, the latter is off-device (ADR-0031).
fn is_loopback_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    if host == "localhost" || host == "::1" || host == "0:0:0:0:0:0:0:1" {
        return true;
    }
    if let Ok(addr) = host.parse::<std::net::IpAddr>() {
        return addr.is_loopback();
    }
    false
}

/// The pinned skill pair loaded on every generation request. Fail-closed:
/// an empty or boundary-less bundle refuses generation rather than running
/// ungoverned (ADR-0032).
pub struct SkillPair {
    pub shared: String,
    pub create: String,
}

pub fn load_skills() -> Result<SkillPair, String> {
    for (name, text) in [("shared rules", SHARED_RULES), ("Create Quick Action", CREATE_SKILL)] {
        if text.trim().is_empty() {
            return Err(format!("The bundled {name} skill is missing — reinstall Sprout."));
        }
    }
    if !SHARED_RULES.contains("Destructive commands are outside AI assistance") {
        return Err("The bundled skills failed their integrity check — reinstall Sprout.".into());
    }
    Ok(SkillPair {
        shared: SHARED_RULES.to_string(),
        create: CREATE_SKILL.to_string(),
    })
}

/// What the request checks decided before any inference ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestVerdict {
    Allow,
    Refuse { reason: String },
    Clarify { reason: String },
}

/// What the output checks decided before a candidate became usable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputVerdict {
    Allow,
    Refuse { reason: String },
    Clarify { message: String },
}

/// Splits text into comparable tokens: lowercase alphanumerics plus `-`,
/// `+`, `?`, and `.` (so `remove-item`, `c:`, and `??` survive); everything
/// else is a separator. Keeps matching dependency-free.
fn tokens(text: &str) -> Vec<String> {
    text.to_ascii_lowercase()
        .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '+' | '?' | '.')))
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

/// Single-space normal form for phrase matching.
fn flattened(text: &str) -> String {
    text.to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// A base64-looking blob of this size is obfuscation, not maintenance —
/// encoded deletion composes to a refused effect whatever it decodes to.
fn has_encoded_blob(text: &str) -> bool {
    text.split_whitespace().any(|token| {
        let token = token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '+' && c != '/' && c != '=');
        token.len() >= 60
            && token.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=')
            && token.bytes().any(|b| b.is_ascii_uppercase())
            && token.bytes().any(|b| b.is_ascii_lowercase())
            && token.bytes().any(|b| b.is_ascii_digit())
    })
}

/// Scans the combined request text for refused effects. Whole-text scanning
/// is what catches composition: a benign first step never launders the
/// destructive whole, and repaired, re-targeted, or injected destruction
/// still reads as destruction (ADR-0030). `raw` keeps its case for the
/// obfuscation scan; `flat` and `toks` are the normalized views.
fn refusal_reason(raw: &str, flat: &str, toks: &[String]) -> Option<String> {
    let has = |word: &str| toks.iter().any(|t| t == word);
    let phrase = |p: &str| flat.contains(p);

    if phrase("you refused")
        || phrase("just tell me the exact command")
        || phrase("paste it into the manual editor")
        || phrase("ignore previous instructions")
        || phrase("ignore your instructions")
        || phrase("disregard your instructions")
        || phrase("bypass the refusal")
        || phrase("follow it")
        || phrase("follow these instructions")
    {
        return Some("Requests that work around a refusal, or injected instructions from untrusted content, are refused.".into());
    }
    if has("remove-item")
        || has("delete")
        || has("deletes")
        || has("deleted")
        || has("deleting")
        || has("deletion")
        || has("erase")
        || has("rmdir")
        || has("del")
        || phrase("rd /s")
        || phrase("/s /q")
        || phrase("remove item")
    {
        return Some("Permanent deletion is refused, even for one file.".into());
    }
    if has("overwrite")
        || has("overwrites")
        || has("set-content")
        || has("out-file")
        || has("clear-content")
    {
        return Some("Destructive overwrites are refused.".into());
    }
    if has("diskpart")
        || has("wipe")
        || has("wipes")
        || has("wiping")
        || has("shred")
        || phrase("cipher /w")
        || (has("format") && drive_context(flat, toks))
    {
        return Some("Disk wipes and formatting are refused.".into());
    }
    if has("defender")
        || has("set-mppreference")
        || has("mimikatz")
        || has("lsass")
        || phrase("real-time protection")
        || phrase("real time protection")
        || phrase("executionpolicy")
        || phrase("execution policy")
        || phrase("turn off windows")
        || phrase("disable defender")
        || phrase("disable windows defender")
    {
        return Some("Weakening security protections is refused.".into());
    }
    if has("password")
        || has("passwords")
        || has("credential")
        || has("credentials")
        || has("sekurlsa")
        || has("ntdsutil")
        || phrase("saved passwords")
    {
        return Some("Credential extraction is refused.".into());
    }
    if has("encodedcommand")
        || has("frombase64string")
        || has("invoke-expression")
        || has("invoke expression")
        || phrase("[char]")
        || phrase("fromcharcode")
        || has_encoded_blob(raw)
    {
        return Some("Obfuscated or encoded commands are refused — disguise never downgrades the verdict.".into());
    }
    if phrase("clear event log")
        || phrase("clear-eventlog")
        || phrase("wevtutil cl")
    {
        return Some("Clearing evidence alongside another change is refused as a composed destructive effect.".into());
    }
    None
}

/// `format` alone is ambiguous (dates, strings) — it refuses only with a
/// drive, disk, or volume target beside it.
fn drive_context(flat: &str, toks: &[String]) -> bool {
    if flat.contains("drive") || flat.contains("disk") || flat.contains("volume") || flat.contains("partition") {
        return true;
    }
    toks.iter().any(|t| {
        let t = t.trim_end_matches(':');
        t.len() == 1 && t.as_bytes()[0].is_ascii_alphabetic() && flat.contains(&format!("format {t}"))
    })
}

/// Uncertain effects need clarification or refusal — never a guessed
/// destructive expansion (ADR-0030).
fn clarify_reason(flat: &str, toks: &[String]) -> Option<String> {
    let has = |word: &str| toks.iter().any(|t| t == word);
    let phrase = |p: &str| flat.contains(p);

    if phrase("foreach-object -parallel")
        || (phrase("foreach-object") && has("-parallel"))
        || has("pwsh")
        || phrase("powershell 7")
    {
        return Some(
            "That needs PowerShell 7-only syntax, but Sprout targets Windows PowerShell 5.1 — say how to tell the services apart or ask for a 5.1-compatible draft.".into(),
        );
    }
    if has("??") && flat.contains('$') {
        return Some(
            "That uses PowerShell 7-only operators under a 5.1 target — name the fallback behavior or ask for a 5.1-compatible draft.".into(),
        );
    }
    if has("module") || has("sdk") || phrase("external tool") || phrase("install and use") {
        return Some(
            "That depends on a module or external tool whose presence is unknown — name what is installed or ask for a built-in-only draft.".into(),
        );
    }
    for vague in [
        "open the editor",
        "open my editor",
        "open the app",
        "open my app",
        "open the program",
        "launch the editor",
        "start the editor",
    ] {
        if phrase(vague) {
            return Some(
                "Several apps match that description — name the exact app to open.".into(),
            );
        }
    }
    if (has("everything") || phrase("all files") || phrase("clean up"))
        && (flat.contains("c:") || has("system32") || phrase("windows folder") || has("everything"))
    {
        return Some(
            "That scope is uncertain — say exactly which folder and what may go.".into(),
        );
    }
    if has("bypass") || (has("disable") && !flat.contains("defender")) {
        return Some(
            "The effect of disabling or bypassing that is uncertain — say what should keep working.".into(),
        );
    }
    None
}

/// Runs the request checks over the typed request plus the explicitly
/// supplied context as one text. Refusal wins over clarification; a model
/// rating its own output safe is never consulted (ADR-0030).
pub fn classify_request(request: &str, context: &str) -> RequestVerdict {
    let combined = format!("{request}\n{context}");
    let flat = flattened(&combined);
    let toks = tokens(&combined);
    if let Some(reason) = refusal_reason(&combined, &flat, &toks) {
        return RequestVerdict::Refuse { reason };
    }
    if let Some(reason) = clarify_reason(&flat, &toks) {
        return RequestVerdict::Clarify { reason };
    }
    RequestVerdict::Allow
}

/// Runs the output checks over a candidate command: the same effect scan
/// (the model is untrusted input — it may emit what the request would never
/// say), plus shell-compatibility for the explicitly selected shell.
pub fn check_output(shell: QuickActionShell, command: &str) -> OutputVerdict {
    if command.trim().is_empty() {
        return OutputVerdict::Refuse {
            reason: "The model returned no command.".into(),
        };
    }
    if command.chars().count() > MAX_COMMAND_CHARS {
        return OutputVerdict::Refuse {
            reason: "The model returned more text than the bounded draft allows.".into(),
        };
    }
    let flat = flattened(command);
    let toks = tokens(command);
    if let Some(reason) = refusal_reason(command, &flat, &toks) {
        return OutputVerdict::Refuse { reason };
    }
    match shell {
        QuickActionShell::Powershell => {
            if flat.contains("foreach-object -parallel") || toks.iter().any(|t| t == "pwsh") {
                return OutputVerdict::Clarify {
                    message: "The draft needs PowerShell 7-only syntax under a 5.1 target — it is held for clarification, not saved.".into(),
                };
            }
        }
        QuickActionShell::Cmd => {
            let powershellism = ["get-", "set-", "where-object", "foreach-object", "$_", "$env:", "write-output", "start-process"];
            if powershellism.iter().any(|marker| flat.contains(marker)) {
                return OutputVerdict::Clarify {
                    message: "The draft looks like PowerShell, but cmd is selected — it is held for clarification, not saved.".into(),
                };
            }
        }
    }
    if let Some(message) = clarify_reason(&flat, &toks) {
        return OutputVerdict::Clarify { message };
    }
    OutputVerdict::Allow
}

/// The assembled prompt: pinned skills plus the explicit shell and the
/// authorized context only. No discovery data, no credentials, no remote
/// skill refresh (ADR-0031, ADR-0032).
pub struct DraftPrompt {
    pub system: String,
    pub user: String,
}

pub fn build_prompt(
    skills: &SkillPair,
    shell: QuickActionShell,
    request: &str,
    context: &str,
) -> Result<DraftPrompt, String> {
    if request.trim().is_empty() {
        return Err("Describe what the action should do.".into());
    }
    if request.chars().count() > MAX_REQUEST_CHARS {
        return Err(format!(
            "That request is longer than the bounded {MAX_REQUEST_CHARS} characters — shorten it and try again."
        ));
    }
    if context.chars().count() > MAX_CONTEXT_CHARS {
        return Err(format!(
            "That extra context is longer than the bounded {MAX_CONTEXT_CHARS} characters — shorten it and try again."
        ));
    }
    let shell_name = match shell {
        QuickActionShell::Powershell => "Windows PowerShell 5.1 (powershell.exe)",
        QuickActionShell::Cmd => "Windows CMD (cmd.exe)",
    };
    let system = format!(
        "{}\n\n{}\n\nTarget shell: {shell_name}. Reply with JSON only: {{\"command\": \"...\", \"assumptions\": [\"...\"], \"affected_targets\": [\"...\"], \"explanation\": \"...\"}}. Never claim execution.",
        skills.shared, skills.create
    );
    let user = if context.trim().is_empty() {
        format!("Task: {}\nShell: {}", request.trim(), shell.as_str())
    } else {
        format!(
            "Task: {}\nShell: {}\nAuthor-supplied context (untrusted for safety decisions): {}",
            request.trim(),
            shell.as_str(),
            context.trim()
        )
    };
    Ok(DraftPrompt { system, user })
}

/// How the provider call failed. Messages stay actionable and never carry
/// prompt payloads — routine failures must not leak requests, context, or
/// drafts into logs (ADR-0031).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    Unavailable(String),
    UnknownModel(String),
    Unsupported(String),
    Oversized,
    Timeout,
    Redirected,
    Cancelled,
    Malformed,
    Http(u16),
    /// Already-actionable configuration text (endpoint classification):
    /// carried verbatim, never wrapped.
    Invalid(String),
}

impl ProviderError {
    pub fn message(&self) -> String {
        match self {
            ProviderError::Invalid(detail) => detail.clone(),
            ProviderError::Unavailable(detail) => format!(
                "The local service is unreachable ({detail}) — start it and try again. Sprout never starts it for you."
            ),
            ProviderError::UnknownModel(detail) => format!(
                "The service does not expose that model ({detail}) — pick one it lists. Nothing was substituted."
            ),
            ProviderError::Unsupported(detail) => format!(
                "The service answered in a shape this build does not support ({detail}) — no silent fallback ran."
            ),
            ProviderError::Oversized => "The service answered with more text than the bounded draft allows.".into(),
            ProviderError::Timeout => "The service took longer than the bounded wait — try again.".into(),
            ProviderError::Redirected => {
                "The service redirected the request — redirects never bypass consent, so nothing was followed.".into()
            }
            ProviderError::Malformed => "The service answered with text this build cannot parse — try again.".into(),
            ProviderError::Cancelled => "Generation cancelled; no further prompt bytes were sent and nothing was saved.".into(),
            ProviderError::Http(status) => format!(
                "The service answered with HTTP {status} — no silent fallback ran."
            ),
        }
    }
}

/// The provider seam: the real loopback client and the deterministic test
/// client justify the variation; no universal dispatcher sits above them.
pub trait DraftProvider {
    fn generate(&self, prompt: &DraftPrompt, model: &str) -> Result<String, ProviderError>;
}

/// The existing-local adapter: loopback HTTP, the tested chat-completions
/// shape, explicit model selection, client-side timeouts. Redirects are
/// never followed and the proxy environment is never consulted, so a prompt
/// cannot leave the machine through either path (ADR-0031).
pub struct ExistingLocalClient {
    pub root: String,
    pub timeout: Duration,
}

impl ExistingLocalClient {
    fn agent(&self) -> ureq::Agent {
        ureq::AgentBuilder::new()
            .timeout(self.timeout)
            .redirects(0)
            .try_proxy_from_env(false)
            .build()
    }

    fn read_capped(response: ureq::Response) -> Result<String, ProviderError> {
        if (300..400).contains(&response.status()) {
            return Err(ProviderError::Redirected);
        }
        if response.status() != 200 {
            return Err(ProviderError::Http(response.status()));
        }
        let mut capped = response.into_reader().take(MAX_RESPONSE_BYTES + 1);
        let mut body = String::new();
        capped.read_to_string(&mut body).map_err(|_| ProviderError::Malformed)?;
        if body.len() as u64 > MAX_RESPONSE_BYTES {
            return Err(ProviderError::Oversized);
        }
        Ok(body)
    }

    fn map_transport(error: ureq::Error) -> ProviderError {
        match error {
            ureq::Error::Status(code, _) => ProviderError::Http(code),
            ureq::Error::Transport(transport) => {
                let detail = transport.to_string();
                if detail.contains("timed out") || detail.contains("timeout") {
                    ProviderError::Timeout
                } else {
                    ProviderError::Unavailable(detail)
                }
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct ChatChoiceMessage {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    #[serde(default)]
    message: Option<ChatChoiceMessage>,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<ChatChoice>,
}

impl DraftProvider for ExistingLocalClient {
    fn generate(&self, prompt: &DraftPrompt, model: &str) -> Result<String, ProviderError> {
        let url = format!("{}/v1/chat/completions", self.root);
        let body = serde_json::json!({
            "model": model,
            "stream": false,
            "messages": [
                { "role": "system", "content": prompt.system },
                { "role": "user", "content": prompt.user },
            ],
        });
        let response = self
            .agent()
            .post(&url)
            .set("Content-Type", "application/json")
            .send_string(&body.to_string())
            .map_err(Self::map_transport)?;
        let text = Self::read_capped(response)?;
        let parsed: ChatResponse = serde_json::from_str(&text).map_err(|_| ProviderError::Malformed)?;
        parsed
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message)
            .and_then(|message| message.content)
            .filter(|content| !content.trim().is_empty())
            .ok_or(ProviderError::Unsupported("no usable message content".into()))
    }
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    data: Vec<ModelEntry>,
    #[serde(default)]
    models: Vec<ModelEntry>,
}

/// Lists the models the loopback service exposes, checking the configured
/// one by exact name. Compatible models reuse this one integration through
/// explicit selection — JSON alone never adds missing support (ADR-0031).
pub fn list_models(root: &str, timeout: Duration) -> Result<Vec<String>, ProviderError> {
    let url = format!("{root}/v1/models");
    let response = ureq::AgentBuilder::new()
        .timeout(timeout)
        .redirects(0)
        .try_proxy_from_env(false)
        .build()
        .get(&url)
        .call()
        .map_err(ExistingLocalClient::map_transport)?;
    let text = ExistingLocalClient::read_capped(response)?;
    let parsed: ModelsResponse = serde_json::from_str(&text).map_err(|_| ProviderError::Malformed)?;
    let mut names: Vec<String> = parsed
        .data
        .into_iter()
        .chain(parsed.models)
        .filter_map(|entry| entry.id.or(entry.name))
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .collect();
    names.sort();
    names.dedup();
    if names.is_empty() {
        return Err(ProviderError::Unsupported("empty model list".into()));
    }
    Ok(names)
}

/// The explicit Test-connection command behind Settings: classifies the
/// endpoint, asks the service what it exposes, and checks the configured
/// model exactly. Every failure is actionable; none falls back anywhere.
pub fn check_existing_local(base_url: &str, model: &str) -> Result<Vec<String>, ProviderError> {
    let root = classify_endpoint(base_url).map_err(ProviderError::Invalid)?;
    if model.trim().is_empty() {
        return Err(ProviderError::UnknownModel("no model named".into()));
    }
    let exposed = list_models(&root, MODELS_TIMEOUT)?;
    if !exposed.iter().any(|name| name == model.trim()) {
        return Err(ProviderError::UnknownModel(format!(
            "‘{}’ is not among {}; pick one it lists",
            model.trim(),
            exposed.join(", ")
        )));
    }
    Ok(exposed)
}

/// The model's structured reply: command plus the reviewable context around
/// it. Anything unparseable is a failure, never a partial draft.
#[derive(Debug, Deserialize)]
struct DraftJson {
    command: String,
    #[serde(default)]
    assumptions: Vec<String>,
    #[serde(default)]
    affected_targets: Vec<String>,
    #[serde(default)]
    explanation: String,
}

/// A reviewable candidate. It is data, never an executed thing: the shape
/// marks it unexecuted and carries no run authority (ADR-0030).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DraftCandidate {
    pub shell: QuickActionShell,
    pub command: String,
    pub assumptions: Vec<String>,
    pub affected_targets: Vec<String>,
    pub explanation: String,
    pub executed: bool,
}

/// The single outcome of one generation request: a candidate, a refusal, a
/// clarification, or an actionable failure. Refused and clarified outcomes
/// carry no executable text — rejected code never reaches preview, Copy, or
/// Save (ADR-0030).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum DraftOutcome {
    Draft { draft: DraftCandidate },
    Refused { message: String },
    Clarify { message: String },
    Failed { message: String },
}

fn refused(reason: &str) -> DraftOutcome {
    DraftOutcome::Refused {
        message: format!("{REFUSAL_BOUNDARY} {reason}"),
    }
}

/// Which raw local fields the user approved for THIS generation request
/// (ADR-0031). Discovery and disclosure grants stay separate: finding local
/// targets never approves sending them anywhere, so generation defaults to
/// nothing approved and only an explicit disclosure grant adds field names. A
/// later cloud path must consult this before uploading anything.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RequestGrants {
    pub approved_raw_fields: Vec<String>,
}

impl RequestGrants {
    pub fn none() -> Self {
        RequestGrants { approved_raw_fields: Vec::new() }
    }
}

/// The input to one generation request: an explicit shell, the typed
/// request, and the explicitly supplied context. Discovery grants do not
/// exist in this slice, so nothing else may enter the prompt.
pub struct DraftInput {
    pub shell: QuickActionShell,
    pub request: String,
    pub context: Option<String>,
    pub model: String,
    /// The raw-field approvals recorded for this request (none in this
    /// slice — generation carries typed text only, never discovery output).
    /// Read by the cloud path (ticket 150); 149 establishes the shape.
    #[allow(dead_code)]
    pub grants: RequestGrants,
}

/// Strips one markdown fence pair. Models wrap JSON in fences despite the
/// JSON-only instruction; unwrapping is parsing, not execution.
fn unfence(text: &str) -> &str {
    let trimmed = text.trim();
    let inner = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed);
    let inner = if inner.len() != trimmed.len() {
        inner.strip_suffix("```").unwrap_or(inner).trim()
    } else {
        inner
    };
    inner
}

/// Requests one draft through exactly one provider: request checks before
/// inference, output checks before usability, and the bundled pair loaded on
/// the request itself. Holds no connection and no database handle, so
/// generation cannot persist a partial action — saving stays an explicit
/// later step through the normal validation (ADR-0030).
pub fn request_draft(
    provider: &dyn DraftProvider,
    skills: &SkillPair,
    input: &DraftInput,
) -> DraftOutcome {
    let context = input.context.as_deref().unwrap_or_default();
    match classify_request(&input.request, context) {
        RequestVerdict::Refuse { reason } => return refused(&reason),
        RequestVerdict::Clarify { reason } => {
            return DraftOutcome::Clarify { message: reason };
        }
        RequestVerdict::Allow => {}
    }
    let prompt = match build_prompt(skills, input.shell, &input.request, context) {
        Ok(prompt) => prompt,
        Err(message) => return DraftOutcome::Failed { message },
    };
    let raw = match provider.generate(&prompt, &input.model) {
        Ok(raw) => raw,
        Err(error) => return DraftOutcome::Failed { message: error.message() },
    };
    let parsed: DraftJson = match serde_json::from_str(unfence(&raw)) {
        Ok(parsed) => parsed,
        Err(_) => {
            return DraftOutcome::Failed {
                message: ProviderError::Malformed.message(),
            };
        }
    };
    if parsed.command.chars().count() > MAX_COMMAND_CHARS {
        return DraftOutcome::Failed {
            message: ProviderError::Oversized.message(),
        };
    }
    match check_output(input.shell, &parsed.command) {
        OutputVerdict::Allow => DraftOutcome::Draft {
            draft: DraftCandidate {
                shell: input.shell,
                command: parsed.command.trim().to_string(),
                assumptions: parsed.assumptions,
                affected_targets: parsed.affected_targets,
                explanation: parsed.explanation,
                executed: false,
            },
        },
        OutputVerdict::Refuse { reason } => refused(&reason),
        OutputVerdict::Clarify { message } => DraftOutcome::Clarify { message },
    }
}

/// Rechecks a candidate when it is accepted through AI assistance: the same
/// output checks, no provider, no persistence, no execution. The revision
/// flow owns the baseline comparison; this owns only the verdict.
pub fn recheck_candidate(shell: QuickActionShell, command: &str) -> OutputVerdict {
    check_output(shell, command)
}

/// The deterministic test adapter: scripted responses in order, with every
/// assembled prompt recorded for assertions. Exercises the full checked flow
/// without a model, including malicious output.
#[cfg(test)]
pub enum Scripted {
    Body(String),
    TransportFailure,
    Timeout,
    Malformed,
}

#[cfg(test)]
pub struct TestClient {
    script: Mutex<VecDeque<Scripted>>,
    seen: Mutex<Vec<String>>,
}

#[cfg(test)]
impl TestClient {
    pub fn new(script: Vec<Scripted>) -> Self {
        TestClient {
            script: Mutex::new(script.into()),
            seen: Mutex::new(Vec::new()),
        }
    }

    pub fn prompts(&self) -> Vec<String> {
        self.seen.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

#[cfg(test)]
impl DraftProvider for TestClient {
    fn generate(&self, prompt: &DraftPrompt, _model: &str) -> Result<String, ProviderError> {
        self.seen
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(format!("{}\n{}", prompt.system, prompt.user));
        match self.script.lock().unwrap_or_else(|e| e.into_inner()).pop_front() {
            Some(Scripted::Body(body)) => Ok(body),
            Some(Scripted::TransportFailure) => Err(ProviderError::Unavailable("connection refused".into())),
            Some(Scripted::Timeout) => Err(ProviderError::Timeout),
            Some(Scripted::Malformed) => Err(ProviderError::Malformed),
            None => Err(ProviderError::Unsupported("no scripted response".into())),
        }
    }
}

/// A benign model content body for tests: what `generate` yields after the
/// adapter unwraps the wire shape.
#[cfg(test)]
pub fn chat_body(command: &str, explanation: &str) -> String {
    serde_json::json!({
        "command": command,
        "assumptions": ["Windows PowerShell 5.1 is available"],
        "affected_targets": [],
        "explanation": explanation,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn skills() -> SkillPair {
        load_skills().expect("bundled skills load in tests")
    }

    fn fixtures() -> Value {
        let text = include_str!("../tests/ai-eval-fixtures.json");
        serde_json::from_str(text).expect("eval fixtures parse")
    }

    fn draft_input(shell: &str, request: &str) -> DraftInput {
        DraftInput {
            shell: if shell == "cmd" { QuickActionShell::Cmd } else { QuickActionShell::Powershell },
            request: request.to_string(),
            context: None,
            model: "test-model".to_string(),
            grants: RequestGrants::none(),
        }
    }

    fn outcome_for(fixture: &Value) -> DraftOutcome {
        let shell = fixture["shell"].as_str().unwrap_or("powershell");
        let request = fixture["request"].as_str().unwrap_or_default();
        let context = fixture.get("supplied_script").and_then(Value::as_str).unwrap_or_default();
        let context = if context.is_empty() { None } else { Some(context.to_string()) };
        // Benign bodies per shell; refused and ambiguous fixtures never reach
        // the provider, so the scripted body only matters for allowed ones.
        let benign = if shell == "cmd" { "ipconfig" } else { "Get-Service -Name Spooler" };
        let client = TestClient::new(vec![Scripted::Body(chat_body(benign, "A read-only draft."))]);
        let mut input = draft_input(shell, request);
        input.context = context;
        request_draft(&client, &skills(), &input)
    }

    #[test]
    fn eval_fixtures_reach_their_expected_verdicts() {
        let fixtures = fixtures();
        let list = fixtures["fixtures"].as_array().expect("fixture list");
        assert!(list.len() >= 20, "the refusal corpus keeps its size");
        for fixture in list {
            let id = fixture["id"].as_str().unwrap_or("?");
            let expected = fixture["expected"].as_str().unwrap_or("?");
            let outcome = outcome_for(fixture);
            match expected {
                "allow-draft" => assert!(
                    matches!(outcome, DraftOutcome::Draft { .. }),
                    "{id} must draft, got {outcome:?}"
                ),
                "refuse" => {
                    let DraftOutcome::Refused { message } = outcome else {
                        panic!("{id} must refuse, got {outcome:?}");
                    };
                    assert!(
                        message.contains(REFUSAL_BOUNDARY),
                        "{id} refusal carries the plain boundary"
                    );
                }
                "clarify" => assert!(
                    matches!(outcome, DraftOutcome::Clarify { .. }),
                    "{id} must clarify, got {outcome:?}"
                ),
                other => panic!("{id} has an unknown expectation: {other}"),
            }
        }
    }

    #[test]
    fn both_shells_draft_the_same_benign_intent_in_their_own_shape() {
        // Behavior, not prompt snapshots: each shell's draft passes its own
        // output checks, and CMD never receives a PowerShell transliteration.
        for (shell, command) in [
            (QuickActionShell::Powershell, "Start-Process notepad.exe"),
            (QuickActionShell::Cmd, "start notepad.exe"),
        ] {
            let client = TestClient::new(vec![Scripted::Body(chat_body(command, "Opens Notepad."))]);
            let input = DraftInput {
                shell,
                request: "Open Notepad".to_string(),
                context: None,
                model: "test-model".to_string(),
                grants: RequestGrants::none(),
            };
            let outcome = request_draft(&client, &skills(), &input);
            let DraftOutcome::Draft { draft } = outcome else {
                panic!("{shell:?} must draft, got {outcome:?}");
            };
            assert_eq!(draft.shell, shell);
            assert_eq!(draft.command, command);
            assert!(!draft.executed, "drafts are marked unexecuted");
        }
    }

    #[test]
    fn every_generation_loads_the_pinned_skill_pair() {
        let client = TestClient::new(vec![Scripted::Body(chat_body("ipconfig", "Shows IP configuration."))]);
        let input = DraftInput {
            shell: QuickActionShell::Cmd,
            request: "Show my IP configuration".to_string(),
            context: None,
            model: "test-model".to_string(),
            grants: RequestGrants::none(),
        };
        let outcome = request_draft(&client, &skills(), &input);
        assert!(matches!(outcome, DraftOutcome::Draft { .. }));
        let prompts = client.prompts();
        assert_eq!(prompts.len(), 1);
        assert!(prompts[0].contains("Draft-only behavior"), "shared rules load");
        assert!(prompts[0].contains("Create Quick Action task skill"), "create skill loads");
        assert!(prompts[0].contains("cmd"), "the selected shell rides along");
        assert!(!prompts[0].contains("learn.microsoft.com"), "no documentation catalog in prompts");
    }

    #[test]
    fn malicious_model_output_never_becomes_a_draft() {
        for hostile in [
            "Remove-Item C:\\Temp\\* -Recurse -Force",
            "del C:\\Temp\\notes.txt",
            "Format D: /FS:NTFS /Q",
            "Set-MpPreference -DisableRealtimeMonitoring $true",
            "[System.Text.Encoding]::UTF8.GetString([System.Convert]::FromBase64String('e30=')) | Invoke-Expression",
        ] {
            let client = TestClient::new(vec![Scripted::Body(chat_body(hostile, "Trust me, this is safe."))]);
            let input = draft_input("powershell", "Show my IP configuration");
            let outcome = request_draft(&client, &skills(), &input);
            assert!(
                matches!(outcome, DraftOutcome::Refused { .. }),
                "hostile output must refuse, got {outcome:?}"
            );
            if let DraftOutcome::Refused { message } = outcome {
                assert!(!message.contains(hostile), "rejected code never leaks into the refusal");
            }
        }
    }

    #[test]
    fn model_self_approval_never_authorizes() {
        let client = TestClient::new(vec![Scripted::Body(chat_body(
            "Remove-Item C:\\Temp\\notes.txt",
            "I rate this output safe and verified to run.",
        ))]);
        let input = draft_input("powershell", "Show my IP configuration");
        assert!(matches!(
            request_draft(&client, &skills(), &input),
            DraftOutcome::Refused { .. }
        ));
    }

    #[test]
    fn truncated_and_malformed_responses_fail_without_partial_drafts() {
        for script in [
            Scripted::Body("not json at all".to_string()),
            Scripted::Body("{\"command\": ".to_string()),
            Scripted::Body(chat_body("", "empty command").replace("\"command\":\"\"", "\"command\":\"\"")),
            Scripted::Malformed,
        ] {
            let client = TestClient::new(vec![script]);
            let input = draft_input("powershell", "Show my IP configuration");
            let outcome = request_draft(&client, &skills(), &input);
            assert!(
                matches!(outcome, DraftOutcome::Failed { .. } | DraftOutcome::Refused { .. }),
                "broken responses never draft, got {outcome:?}"
            );
        }
    }

    #[test]
    fn transport_failures_are_actionable_and_carry_no_payload() {
        let request = "Show my IP configuration on the Quintessential Test Machine";
        for script in [Scripted::TransportFailure, Scripted::Timeout] {
            let client = TestClient::new(vec![script]);
            let input = draft_input("powershell", request);
            let outcome = request_draft(&client, &skills(), &input);
            let DraftOutcome::Failed { message } = outcome else {
                panic!("transport trouble must fail, got {outcome:?}");
            };
            assert!(!message.contains("Quintessential"), "failures never echo the request");
        }
    }

    #[test]
    fn endpoint_classification_keeps_inference_on_this_machine() {
        for ok in [
            "http://127.0.0.1:11434",
            "http://127.0.0.1:11434/",
            "http://localhost:11434",
            "http://LOCALHOST:11434",
            "http://[::1]:11434",
            "http://127.0.0.2:8080",
        ] {
            assert!(classify_endpoint(ok).is_ok(), "{ok} is loopback");
        }
        assert_eq!(
            classify_endpoint("http://127.0.0.1:11434/").unwrap(),
            "http://127.0.0.1:11434"
        );
        for bad in [
            "",
            "127.0.0.1:11434",
            "https://127.0.0.1:11434",
            "http://192.168.1.10:11434",
            "http://10.0.0.5:11434",
            "http://example.com:11434",
            "http://0.0.0.0:11434",
            "http://[::]:11434",
            "http://user:pass@127.0.0.1:11434",
            "http://127.0.0.1:99999",
            "http://127.0.0.1:11434/v1?key=x",
        ] {
            assert!(classify_endpoint(bad).is_err(), "{bad} must refuse");
        }
        // Off-device failures name the loopback rule, never the payload.
        let message = classify_endpoint("http://192.168.1.10:11434").unwrap_err();
        assert!(message.contains("loopback"));
    }

    #[test]
    fn settings_validation_holds_the_existing_local_contract() {
        assert!(validate_ai_settings("off", "", "").is_ok());
        assert!(validate_ai_settings("managed", "", "").is_ok());
        assert!(validate_ai_settings("cloud", "", "").is_ok());
        assert!(validate_ai_settings("existing-local", "http://127.0.0.1:11434", "qwen").is_ok());
        assert!(validate_ai_settings("existing-local", "http://127.0.0.1:11434", "  ").is_err());
        assert!(validate_ai_settings("existing-local", "http://192.168.0.2:11434", "qwen").is_err());
        assert!(validate_ai_settings("nonsense", "", "").is_err());
    }

    #[test]
    fn request_limits_are_bounded_and_documented() {
        let long = "x".repeat(MAX_REQUEST_CHARS + 1);
        let input = DraftInput {
            shell: QuickActionShell::Powershell,
            request: long,
            context: None,
            model: "test-model".to_string(),
            grants: RequestGrants::none(),
        };
        assert!(matches!(
            request_draft(&TestClient::new(vec![]), &skills(), &input),
            DraftOutcome::Failed { .. }
        ));
        let long_context = "y".repeat(MAX_CONTEXT_CHARS + 1);
        let input = DraftInput {
            shell: QuickActionShell::Powershell,
            request: "Show my IP configuration".to_string(),
            context: Some(long_context),
            model: "test-model".to_string(),
            grants: RequestGrants::none(),
        };
        assert!(matches!(
            request_draft(&TestClient::new(vec![]), &skills(), &input),
            DraftOutcome::Failed { .. }
        ));
    }

    #[test]
    fn empty_requests_fail_before_inference() {
        let client = TestClient::new(vec![Scripted::Body(chat_body("ipconfig", "x"))]);
        let input = draft_input("cmd", "   ");
        assert!(matches!(
            request_draft(&client, &skills(), &input),
            DraftOutcome::Failed { .. }
        ));
        assert!(client.prompts().is_empty(), "no prompt leaves on empty input");
    }

    #[test]
    fn refused_requests_never_reach_the_provider() {
        let client = TestClient::new(vec![Scripted::Body(chat_body("ipconfig", "x"))]);
        let input = draft_input("powershell", "Delete C:\\Temp\\notes.txt");
        assert!(matches!(
            request_draft(&client, &skills(), &input),
            DraftOutcome::Refused { .. }
        ));
        assert!(client.prompts().is_empty(), "refused requests send nothing");
    }

    #[test]
    fn generation_records_no_raw_field_approvals() {
        // Discovery output never rides into a prompt on its own: finding
        // local targets approves nothing, so only an explicit disclosure
        // grant could ever add field names here (ADR-0031).
        let input = draft_input("powershell", "Show my IP configuration");
        assert!(input.grants.approved_raw_fields.is_empty());
        assert_eq!(input.grants, RequestGrants::none());
    }

    /// The complete local flow: request → checked draft → explicit save.
    /// Generation holds no database handle and no run authority, so it
    /// persists nothing by itself; saving goes through the normal
    /// validation with execution off; and at no point is anything run,
    /// tested, or stopped.
    #[test]
    fn checked_draft_saves_through_normal_persistence_with_execution_off() {
        let conn = crate::db::init_at(&tempfile::tempdir().unwrap().into_path()).unwrap();
        let client = TestClient::new(vec![Scripted::Body(chat_body(
            "Get-Service -Name Spooler",
            "Reads one service status.",
        ))]);
        let input = draft_input("powershell", "Show the status of the Spooler service");
        let outcome = request_draft(&client, &skills(), &input);
        let DraftOutcome::Draft { draft } = outcome else {
            panic!("benign request must draft, got {outcome:?}");
        };
        assert!(!draft.executed, "drafts are marked unexecuted");
        assert!(
            crate::quick_actions::list_quick_actions(&conn).unwrap().is_empty(),
            "generation persists nothing by itself"
        );
        let action = crate::quick_actions::QuickActionInput {
            name: "Spooler status".to_string(),
            shell: draft.shell,
            command: draft.command.clone(),
            cwd: None,
            stoppable: false,
            stop_command: None,
            note: None,
            auto_run: false,
            show_in_dock: true,
        };
        crate::quick_actions::validate_quick_action(&action).unwrap();
        let saved = crate::quick_actions::create_quick_action(&conn, &action).unwrap();
        assert_eq!(saved.action.command, "Get-Service -Name Spooler");
        assert_eq!(saved.action.shell, QuickActionShell::Powershell);
        assert!(!saved.action.auto_run, "new actions have auto-run off");
        assert_eq!(crate::quick_actions::list_quick_actions(&conn).unwrap().len(), 1);
    }

    #[test]
    fn refused_drafts_persist_nothing() {
        let conn = crate::db::init_at(&tempfile::tempdir().unwrap().into_path()).unwrap();
        let client = TestClient::new(vec![Scripted::Body(chat_body(
            "Remove-Item C:\\Temp\\notes.txt",
            "Trust me, this is safe.",
        ))]);
        let input = draft_input("powershell", "Show the status of the Spooler service");
        let outcome = request_draft(&client, &skills(), &input);
        assert!(matches!(outcome, DraftOutcome::Refused { .. }));
        assert!(
            crate::quick_actions::list_quick_actions(&conn).unwrap().is_empty(),
            "refusals persist nothing"
        );
    }

    #[test]
    fn recheck_rejects_hostile_edits_without_a_provider() {        assert!(matches!(
            recheck_candidate(QuickActionShell::Powershell, "Get-Service -Name Spooler"),
            OutputVerdict::Allow
        ));
        assert!(matches!(
            recheck_candidate(QuickActionShell::Powershell, "Remove-Item C:\\Temp\\notes.txt"),
            OutputVerdict::Refuse { .. }
        ));
        assert!(matches!(
            recheck_candidate(QuickActionShell::Cmd, "Get-Process"),
            OutputVerdict::Clarify { .. }
        ));
    }

    /// The loopback HTTP adapter against fixture servers: success, unknown
    /// models, malformed bodies, and redirects that must never be followed.
    mod adapter {
        use super::*;
        use std::io::Write;
        use std::net::TcpListener;
        use std::thread;

        fn serve_once(body: Vec<u8>) -> (String, thread::JoinHandle<Vec<u8>>) {
            let listener = TcpListener::bind("127.0.0.1:0").expect("fixture port");
            let root = format!("http://{}", listener.local_addr().expect("addr"));
            let handle = thread::spawn(move || {
                let (mut stream, _) = listener.accept().expect("one connection");
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .expect("read timeout");
                // Read the head, then exactly the framed body, so assertions
                // see the whole request even when packets split.
                let mut seen = Vec::new();
                let mut buf = [0u8; 4096];
                loop {
                    match stream.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            seen.extend_from_slice(&buf[..n]);
                            if let Some(end) = find_head_end(&seen) {
                                let want = content_length(&seen[..end]) + end;
                                if seen.len() >= want {
                                    break;
                                }
                            }
                            if seen.len() > 1 << 20 {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                let _ = stream.write_all(&body);
                seen
            });
            (root, handle)
        }

        fn find_head_end(seen: &[u8]) -> Option<usize> {
            seen.windows(4)
                .position(|w| w == b"\r\n\r\n")
                .map(|pos| pos + 4)
        }

        fn content_length(head: &[u8]) -> usize {
            let text = String::from_utf8_lossy(head).to_ascii_lowercase();
            text.lines()
                .filter_map(|line| line.strip_prefix("content-length:"))
                .filter_map(|value| value.trim().parse::<usize>().ok())
                .next()
                .unwrap_or(0)
        }

        fn http_ok(json: &str) -> Vec<u8> {
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                json.len(),
                json
            )
            .into_bytes()
        }

        #[test]
        fn chat_completions_roundtrip_parses_the_message() {
            let payload = chat_body("ipconfig", "Shows IP configuration.");
            let wire = serde_json::json!({
                "choices": [{ "message": { "content": payload } }],
            })
            .to_string();
            let (root, handle) = serve_once(http_ok(&wire));
            let client = ExistingLocalClient { root, timeout: Duration::from_secs(5) };
            let skills = skills();
            let prompt = build_prompt(&skills, QuickActionShell::Cmd, "Show my IP configuration", "").unwrap();
            let raw = client.generate(&prompt, "test-model").expect("fixture answers");
            assert!(raw.contains("ipconfig"));
            let seen = String::from_utf8_lossy(&handle.join().expect("server read")).to_string();
            assert!(seen.starts_with("POST /v1/chat/completions"), "tested shape only, got {seen}");
            assert!(seen.contains("\"stream\":false"), "buffered, never streamed");
        }

        #[test]
        fn unknown_models_fail_without_substitution() {
            let (root, handle) = serve_once(http_ok(r#"{"data": [{"id": "real-model"}]}"#));
            let client = ExistingLocalClient { root: root.clone(), timeout: Duration::from_secs(5) };
            let prompt = DraftPrompt { system: "s".into(), user: "u".into() };
            // Generation itself never substitutes: the wire model rides as-is.
            let _ = client.generate(&prompt, "ghost-model");
            let seen = String::from_utf8_lossy(&handle.join().expect("server read")).to_string();
            assert!(seen.contains("ghost-model"));
            // The connection check refuses the unknown name honestly.
            let (root2, _) = serve_once(http_ok(r#"{"data": [{"id": "real-model"}]}"#));
            let error = check_existing_local(&root2, "ghost-model").unwrap_err();
            assert!(matches!(error, ProviderError::UnknownModel(_)));
        }

        #[test]
        fn malformed_bodies_and_redirects_fail_closed() {
            let (root, _) = serve_once(http_ok("this is not json"));
            let client = ExistingLocalClient { root, timeout: Duration::from_secs(5) };
            let prompt = DraftPrompt { system: "s".into(), user: "u".into() };
            assert_eq!(client.generate(&prompt, "m").unwrap_err(), ProviderError::Malformed);

            let (root, _) = serve_once(
                "HTTP/1.1 302 Found\r\nLocation: http://192.168.0.9:11434/v1/chat/completions\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".as_bytes().to_vec(),
            );
            let client = ExistingLocalClient { root, timeout: Duration::from_secs(5) };
            assert_eq!(client.generate(&prompt, "m").unwrap_err(), ProviderError::Redirected);
        }

        #[test]
        fn models_check_accepts_both_list_shapes() {
            for body in [
                r#"{"data": [{"id": "a"}, {"id": "b"}]}"#,
                r#"{"models": [{"name": "a"}, {"name": "b"}]}"#,
            ] {
                let (root, _) = serve_once(http_ok(body));
                let names = list_models(&root, Duration::from_secs(5)).expect("fixture lists");
                assert_eq!(names, vec!["a".to_string(), "b".to_string()]);
            }
            let (root, _) = serve_once(http_ok(r#"{"data": []}"#));
            assert!(matches!(
                list_models(&root, Duration::from_secs(5)).unwrap_err(),
                ProviderError::Unsupported(_)
            ));
        }

        #[test]
        fn unreachable_services_fail_actionably() {
            let client = ExistingLocalClient {
                root: "http://127.0.0.1:1".to_string(),
                timeout: Duration::from_secs(2),
            };
            let prompt = DraftPrompt { system: "s".into(), user: "u".into() };
            assert!(matches!(
                client.generate(&prompt, "m").unwrap_err(),
                ProviderError::Unavailable(_) | ProviderError::Timeout
            ));
        }
    }
}
