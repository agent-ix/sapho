// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Bounded local structured-question backend (FR-048). Confidence is model
//! self-report, never calibrated probability evidence. No domain logic lives here.
use async_trait::async_trait;
use futures::StreamExt;
mod capacity;
mod prompt;
pub use prompt::PromptFormat;
use reqwest::{Client, Url};
use sapho_core::{
    Answer, ErrorCode, ModelBackend, ModelRequest, ModelResponse, Result, SaphoError,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::Semaphore;
mod bounds;
pub use bounds::{RequestUpperBound, TextFieldSize};

/// Explicit finite generation and capture bounds.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Includes semaphore, HTTP and decode.
    pub timeout: Duration,
    /// Complete serialized request cap.
    pub request_bytes: usize,
    /// Raw HTTP response cap.
    pub response_bytes: usize,
    /// Total per-backend capture capacity.
    pub capture_bytes: usize,
    /// Service context tokens.
    pub context_tokens: usize,
    /// Output reserve.
    pub output_tokens: usize,
    /// Explicit model thinking selection, absent means service default.
    pub think: Option<bool>,
    /// Explicit lossless prompt encoding; changing it creates a new recipe.
    pub prompt_format: PromptFormat,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(480),
            request_bytes: 1_048_576,
            response_bytes: 1_048_576,
            capture_bytes: 64 * 1_048_576,
            context_tokens: 32768,
            output_tokens: 4096,
            think: None,
            prompt_format: PromptFormat::ModelRequestJson,
        }
    }
}
impl Limits {
    fn validate(&self) -> Result<()> {
        if self.timeout.is_zero()
            || self.request_bytes == 0
            || self.response_bytes == 0
            || self.capture_bytes < self.response_bytes
            || self.context_tokens <= self.output_tokens
            || self.output_tokens == 0
        {
            return Err(error(ErrorCode::Config, "Invalid Ollama bounds"));
        }
        Ok(())
    }
}
/// Captured exchange, independent of typed answer validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    /// Typed request identity, linking the call to its native trace node.
    pub typed_request_sha256: String,
    /// Complete prompt decomposition and conservative limits for this one call.
    pub prompt: PromptBudget,
    /// Service-reported digest/placement rechecked under capacity before generation.
    /// Alias inspection and served weights are not an atomic identity proof.
    #[serde(default)]
    pub model_observation: Option<ModelInfo>,
    /// Elapsed invocation time, including local capacity wait.
    pub elapsed_ms: u64,
    /// Optional reported service counts and durations, independent of answer validity.
    pub telemetry: Option<Telemetry>,
    /// True only when send was attempted.
    pub made_call: bool,
    /// Exact native request.
    pub request: Vec<u8>,
    /// HTTP status when received.
    pub status: Option<u16>,
    /// Exact bounded raw response, possibly incomplete.
    pub response: Vec<u8>,
    /// Confidence interpretation.
    pub confidence_evidence: String,
}
/// Numeric prompt facts. Components describe serialized bytes, never token counts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptBudget {
    /// Actual prompt encoding. Historical receipts used complete request JSON.
    #[serde(default)]
    pub format: PromptFormat,
    /// Instruction-prefix bytes.
    pub instruction_bytes: usize,
    /// Serialized typed state bytes.
    pub state_bytes: usize,
    /// Serialized ordered questions bytes.
    pub question_bytes: usize,
    /// Remaining typed request envelope bytes.
    pub envelope_bytes: usize,
    /// Entire prompt bytes for this one provider invocation.
    pub prompt_bytes: usize,
    /// Explicit model context.
    pub context_tokens: usize,
    /// Explicit generation reserve.
    pub output_tokens: usize,
    /// Conservative template reserve.
    pub template_reserve: usize,
}
impl PromptBudget {
    /// Measure complete rendered bytes without a provider, service probe, capacity lease or dispatch.
    pub fn measure(request: &ModelRequest, limits: &Limits) -> Result<Self> {
        limits.validate()?;
        request.validate()?;
        sapho_core::bounded_json(request, limits.request_bytes)?;
        let rendered = prompt::render(request, limits.prompt_format)?;
        Ok(Self::rendered(&rendered, limits))
    }
    fn rendered(rendered: &prompt::Rendered, limits: &Limits) -> Self {
        Self {
            format: limits.prompt_format,
            instruction_bytes: rendered.instruction_bytes,
            state_bytes: rendered.state_bytes,
            question_bytes: rendered.question_bytes,
            envelope_bytes: rendered.envelope_bytes,
            prompt_bytes: rendered.text.len(),
            context_tokens: limits.context_tokens,
            output_tokens: limits.output_tokens,
            template_reserve: 1024,
        }
    }
    /// Conservative available prompt bytes, with checked token/reserve subtraction.
    pub fn allowed_prompt_bytes(&self) -> Option<usize> {
        self.context_tokens
            .checked_sub(self.output_tokens)?
            .checked_sub(self.template_reserve)
    }
    /// Whether this one complete rendered prompt fits the conservative bound.
    pub fn allowed(&self) -> bool {
        self.allowed_prompt_bytes()
            .is_some_and(|n| self.prompt_bytes <= n)
    }
}
/// Source-free single-request admission facts, computed without provider I/O.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestBudget {
    /// Complete chosen prompt encoding and context/output/reserve facts.
    pub prompt: PromptBudget,
    /// Canonical complete core request identity, for immutable plan/receipt linkage.
    pub typed_request_sha256: String,
    /// Canonical complete core request bytes.
    pub typed_request_bytes: usize,
    /// Exact HTTP JSON body bytes, including answer schema and options.
    pub wire_request_bytes: usize,
    /// Explicit maximum for each complete typed/wire request.
    pub request_bytes_limit: usize,
}
impl RequestBudget {
    /// Use the dispatch encoder without service inspection, capacity acquisition or HTTP.
    pub fn measure(request: &ModelRequest, limits: &Limits) -> Result<Self> {
        limits.validate()?;
        request.validate()?;
        let typed = sapho_core::bounded_json(request, limits.request_bytes)?;
        let typed_request_bytes = typed.len();
        let rendered = prompt::render(request, limits.prompt_format)?;
        let wire = wire_request(request, &rendered.text, limits);
        // Counting does not allocate a wire body. The validated typed input is
        // already bounded; the envelope only repeats bounded fields/schema data.
        let wire_request_bytes = sapho_core::measured_json_bytes(&wire, usize::MAX)?;
        Ok(Self {
            prompt: PromptBudget::rendered(&rendered, limits),
            typed_request_sha256: format!("{:x}", Sha256::digest(&typed)),
            typed_request_bytes,
            wire_request_bytes,
            request_bytes_limit: limits.request_bytes,
        })
    }
    /// Both byte caps and the conservative context bound must hold.
    pub fn allowed(&self) -> bool {
        self.typed_request_bytes <= self.request_bytes_limit
            && self.wire_request_bytes <= self.request_bytes_limit
            && self.prompt.allowed()
    }
}
#[derive(Serialize)]
struct Options {
    num_ctx: usize,
    num_predict: usize,
    temperature: u8,
}
#[derive(Serialize)]
struct WireRequest<'a> {
    // Alphabetical field order preserves the existing serde_json object bytes.
    format: serde_json::Value,
    model: &'a str,
    options: Options,
    prompt: &'a str,
    stream: bool,
    think: Option<bool>,
}
fn wire_request<'a>(
    request: &'a ModelRequest,
    prompt: &'a str,
    limits: &Limits,
) -> WireRequest<'a> {
    WireRequest {
        format: answer_schema(request),
        model: &request.model,
        options: Options {
            num_ctx: limits.context_tokens,
            num_predict: limits.output_tokens,
            temperature: 0,
        },
        prompt,
        stream: false,
        think: limits.think,
    }
}
/// Service-reported telemetry. Missing fields remain unavailable, never zero.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Telemetry {
    /// Service-reported actual model.
    pub model: String,
    /// Prompt evaluation tokens.
    pub input_tokens: Option<u64>,
    /// Generated tokens.
    pub output_tokens: Option<u64>,
    /// Service total duration in nanoseconds.
    pub total_duration_ns: Option<u64>,
    /// Model loading duration in nanoseconds.
    pub load_duration_ns: Option<u64>,
    /// Prompt evaluation duration in nanoseconds.
    pub prompt_duration_ns: Option<u64>,
    /// Generation duration in nanoseconds.
    pub generation_duration_ns: Option<u64>,
}
#[derive(Default)]
struct Capture {
    used: usize,
    receipts: Vec<Receipt>,
}
// Cancellation drops this guard after dispatch; partial wire evidence survives.
struct Captured<'a> {
    receipt: Receipt,
    capture: &'a Mutex<Capture>,
    saved: bool,
    started: Instant,
}
impl std::ops::Deref for Captured<'_> {
    type Target = Receipt;
    fn deref(&self) -> &Receipt {
        &self.receipt
    }
}
impl std::ops::DerefMut for Captured<'_> {
    fn deref_mut(&mut self) -> &mut Receipt {
        &mut self.receipt
    }
}
impl Captured<'_> {
    fn save(&mut self) -> Result<()> {
        if !self.saved {
            self.receipt.elapsed_ms =
                u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
            self.receipt.telemetry = serde_json::from_slice::<Wire>(&self.receipt.response)
                .ok()
                .map(|wire| Telemetry {
                    model: wire.model,
                    input_tokens: wire.prompt_eval_count,
                    output_tokens: wire.eval_count,
                    total_duration_ns: wire.total_duration,
                    load_duration_ns: wire.load_duration,
                    prompt_duration_ns: wire.prompt_eval_duration,
                    generation_duration_ns: wire.eval_duration,
                });
            let mut c = self
                .capture
                .lock()
                .map_err(|_| error(ErrorCode::BackendFailed, "Capture lock poisoned"))?;
            c.used = c
                .used
                .saturating_add(self.receipt.request.len())
                .saturating_add(self.receipt.response.len());
            c.receipts.push(self.receipt.clone());
            self.saved = true;
        }
        Ok(())
    }
}
impl Drop for Captured<'_> {
    fn drop(&mut self) {
        let _ = self.save();
    }
}
/// Source-free live progress; dispatch attempt does not imply HTTP200 or correctness.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Progress {
    /// Native HTTP send attempts, including failures and indeterminate outcomes.
    pub dispatch_attempts: usize,
    /// Finished or cancelled bounded wire receipts.
    pub receipts: usize,
    /// A serialized inference invocation holds the gate.
    pub in_flight: bool,
}
/// Source-free installed-model and GPU-placement diagnosis, never inference.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelInfo {
    /// Requested installed model.
    pub model: String,
    /// Installed digest reported by service; not atomic served-weight proof.
    pub digest: String,
    /// Advertised generation capability.
    pub completion: bool,
    /// Currently loaded context, absent when unloaded.
    pub loaded_context: Option<u64>,
    /// Loaded model bytes, zero when unloaded.
    pub loaded_bytes: u64,
    /// Bytes reported placed in GPU memory, zero when unloaded.
    pub gpu_bytes: u64,
}
/// Caller-selected reported model identity, rechecked under shared capacity.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelPin {
    /// Exact service alias/selector.
    pub model: String,
    /// Expected installed digest reported by the service.
    pub digest: String,
}
/// Host-managed local backend; never installs or starts a service.
pub struct OllamaBackend {
    client: Client,
    url: Url,
    limits: Limits,
    gate: Semaphore,
    capacity: Option<std::fs::File>,
    capture: Mutex<Capture>,
    started: AtomicUsize,
    model_pin: Mutex<Option<ModelPin>>,
    pin_started: AtomicBool,
}
fn error(code: ErrorCode, message: &str) -> SaphoError {
    SaphoError::new(code, message)
}
impl OllamaBackend {
    /// Check loopback endpoint and positive finite bounds without inference.
    pub fn new(base: &str, limits: Limits) -> Result<Self> {
        let mut url =
            Url::parse(base).map_err(|_| error(ErrorCode::Config, "Invalid local endpoint"))?;
        let local = url.host_str().is_some_and(|h| {
            h == "localhost" || h.parse::<std::net::IpAddr>().is_ok_and(|a| a.is_loopback())
        });
        if !local
            || !["http", "https"].contains(&url.scheme())
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
        {
            return Err(error(
                ErrorCode::Config,
                "Explicit loopback base URL required",
            ));
        }
        limits.validate()?;
        url.set_path("/api/generate");
        let client = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| error(ErrorCode::BackendFailed, "Cannot prepare local HTTP"))?;
        Ok(Self {
            client,
            url,
            limits,
            gate: Semaphore::new(1),
            capacity: None,
            capture: Mutex::new(Capture::default()),
            started: AtomicUsize::new(0),
            model_pin: Mutex::new(None),
            pin_started: AtomicBool::new(false),
        })
    }
    /// Prepare a shared process capacity lease synchronously, before async execution.
    /// Every cooperating worker for a local service must use the same stable file.
    pub fn new_shared(base: &str, limits: Limits, path: &std::path::Path) -> Result<Self> {
        let mut backend = Self::new(base, limits)?;
        backend.capacity = Some(capacity::prepare(path)?);
        Ok(backend)
    }
    /// Bind one expected reported digest before any generation. Rebinding a
    /// different pin is refused. The first invocation seals the optional pin;
    /// binding must precede invocation. This does not assert atomic served-weight identity.
    pub fn pin_model(&self, pin: ModelPin) -> Result<()> {
        if pin.model.is_empty()
            || pin.model.len() > 4096
            || pin.digest.is_empty()
            || pin.digest.len() > 256
        {
            return Err(error(ErrorCode::Config, "Invalid local model pin"));
        }
        let mut stored = self
            .model_pin
            .lock()
            .map_err(|_| error(ErrorCode::BackendFailed, "Local model pin unavailable"))?;
        if let Some(prior) = stored.as_ref() {
            if prior.model != pin.model || prior.digest != pin.digest {
                return Err(error(ErrorCode::ModelMismatch, "Local model pin differs"));
            }
        } else {
            if self.pin_started.load(Ordering::Acquire) {
                return Err(error(
                    ErrorCode::Config,
                    "Cannot add a model pin after an invocation starts",
                ));
            }
            *stored = Some(pin);
        }
        Ok(())
    }
    /// Inspect installed model digest/capabilities and current GPU placement without loading it.
    pub async fn inspect(&self, model: &str) -> Result<ModelInfo> {
        if model.is_empty() || model.len() > 4096 {
            return Err(error(ErrorCode::InvalidValue, "Explicit model required"));
        }
        tokio::time::timeout(self.limits.timeout, async {
            let tags = self.inspect_endpoint("/api/tags", None).await?;
            let installed = tags
                .get("models")
                .and_then(serde_json::Value::as_array)
                .and_then(|models| {
                    models
                        .iter()
                        .find(|m| m.get("name").and_then(serde_json::Value::as_str) == Some(model))
                })
                .ok_or_else(|| {
                    error(
                        ErrorCode::Config,
                        "Selected model is not installed under this exact alias",
                    )
                })?;
            let digest = installed
                .get("digest")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| error(ErrorCode::InvalidAnswer, "Installed model digest absent"))?
                .to_owned();
            let show = self
                .inspect_endpoint("/api/show", Some(serde_json::json!({"model":model})))
                .await?;
            let completion = show
                .get("capabilities")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|a| a.iter().any(|v| v.as_str() == Some("completion")));
            let ps = self.inspect_endpoint("/api/ps", None).await?;
            let loaded = ps
                .get("models")
                .and_then(serde_json::Value::as_array)
                .and_then(|models| {
                    models
                        .iter()
                        .find(|m| m.get("name").and_then(serde_json::Value::as_str) == Some(model))
                });
            let number = |key: &str| {
                loaded
                    .and_then(|m| m.get(key))
                    .and_then(serde_json::Value::as_u64)
            };
            Ok(ModelInfo {
                model: model.to_owned(),
                digest,
                completion,
                loaded_context: number("context_length"),
                loaded_bytes: number("size").unwrap_or(0),
                gpu_bytes: number("size_vram").unwrap_or(0),
            })
        })
        .await
        .map_err(|_| {
            error(
                ErrorCode::DeadlineExceeded,
                "Local model inspection deadline exceeded",
            )
        })?
    }
    async fn inspect_endpoint(
        &self,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let mut url = self.url.clone();
        url.set_path(path);
        let request =
            if let Some(body) = body {
                self.client
                    .post(url)
                    .header("Content-Type", "application/json")
                    .body(serde_json::to_vec(&body).map_err(|_| {
                        error(ErrorCode::Config, "Cannot encode inspection request")
                    })?)
            } else {
                self.client.get(url)
            };
        let response = request.send().await.map_err(|_| {
            error(
                ErrorCode::BackendFailed,
                "Local model inspection unavailable",
            )
        })?;
        if !response.status().is_success() {
            return Err(error(
                ErrorCode::BackendFailed,
                "Local model inspection refused",
            ));
        }
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk
                .map_err(|_| error(ErrorCode::BackendFailed, "Local inspection body failed"))?;
            if bytes.len().saturating_add(chunk.len()) > self.limits.response_bytes {
                return Err(error(
                    ErrorCode::LimitExceeded,
                    "Local inspection response exceeded cap",
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes).map_err(|_| {
            error(
                ErrorCode::InvalidAnswer,
                "Invalid model inspection response",
            )
        })
    }
    /// Source-free counters for a live monitoring host; no raw capture cloning.
    pub fn progress(&self) -> Result<Progress> {
        let capture = self
            .capture
            .lock()
            .map_err(|_| error(ErrorCode::BackendFailed, "Capture lock poisoned"))?;
        Ok(Progress {
            dispatch_attempts: self.started.load(Ordering::Relaxed),
            receipts: capture.receipts.len(),
            in_flight: self.gate.available_permits() == 0,
        })
    }
    /// Snapshot bounded raw exchanges outside async execution.
    pub fn receipts(&self) -> Result<Vec<Receipt>> {
        self.capture
            .lock()
            .map(|c| c.receipts.clone())
            .map_err(|_| error(ErrorCode::BackendFailed, "Capture lock poisoned"))
    }
    async fn call(&self, request: &ModelRequest) -> Result<ModelResponse> {
        let started = Instant::now();
        // Seal the optional pin under the same lock used by pin_model before any
        // await, so a concurrent first invocation cannot race first-time binding.
        let pin = {
            let stored = self
                .model_pin
                .lock()
                .map_err(|_| error(ErrorCode::BackendFailed, "Local model pin unavailable"))?;
            self.pin_started.store(true, Ordering::Release);
            stored.clone()
        };
        let _slot = self
            .gate
            .acquire()
            .await
            .map_err(|_| error(ErrorCode::BackendFailed, "Local provider closed"))?;
        let _capacity = match &self.capacity {
            Some(file) => Some(capacity::acquire(file).await?),
            None => None,
        };
        let encoded = sapho_core::bounded_json(request, self.limits.request_bytes)?;
        let rendered = prompt::render(request, self.limits.prompt_format)?;
        let budget = PromptBudget::rendered(&rendered, &self.limits);
        let prompt = rendered.text;
        let mut receipt = Captured {
            receipt: Receipt {
                typed_request_sha256: format!("{:x}", Sha256::digest(&encoded)),
                prompt: budget,
                elapsed_ms: 0,
                telemetry: None,
                model_observation: None,
                made_call: false,
                request: Vec::new(),
                status: None,
                response: Vec::new(),
                confidence_evidence: "model_self_report_uncalibrated".into(),
            },
            capture: &self.capture,
            saved: false,
            started,
        };
        // One UTF8 byte per token is deliberately conservative, with template reserve.
        if !receipt.prompt.allowed() {
            return Err(error(
                ErrorCode::LimitExceeded,
                "Complete input exceeds conservative context bound; no truncation",
            )
            .with_context("prompt_bytes", prompt.len().to_string())
            .with_context("output_tokens", self.limits.output_tokens.to_string())
            .with_context(
                "template_reserve",
                receipt.prompt.template_reserve.to_string(),
            )
            .with_context("context_tokens", self.limits.context_tokens.to_string())
            .with_context("token_upper_bound", "one_utf8_byte_per_token"));
        }
        if let Some(pin) = pin {
            if request.model != pin.model {
                return Err(error(
                    ErrorCode::ModelMismatch,
                    "Request model differs from pinned alias",
                ));
            }
            let observed = self.inspect(&pin.model).await?;
            let matches = observed.completion && observed.digest == pin.digest;
            receipt.model_observation = Some(observed);
            if !matches {
                return Err(error(
                    ErrorCode::ModelMismatch,
                    "Installed local model digest/capability drift; no generation dispatch",
                ));
            }
        }
        let body = sapho_core::bounded_json(
            &wire_request(request, &prompt, &self.limits),
            self.limits.request_bytes,
        )?;
        let used = self
            .capture
            .lock()
            .map_err(|_| error(ErrorCode::BackendFailed, "Capture lock poisoned"))?
            .used;
        if used
            .checked_add(body.len())
            .and_then(|n| n.checked_add(self.limits.response_bytes))
            .is_none_or(|n| n > self.limits.capture_bytes)
        {
            return Err(error(
                ErrorCode::LimitExceeded,
                "Raw capture capacity exhausted before dispatch",
            ));
        }
        receipt.request = body.clone();
        receipt.made_call = true;
        self.started.fetch_add(1, Ordering::Relaxed);
        let response = match self
            .client
            .post(self.url.clone())
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(_) => {
                receipt.save()?;
                return Err(error(
                    ErrorCode::BackendFailed,
                    "Local HTTP send failed; call outcome unknown",
                ));
            }
        };
        receipt.status = Some(response.status().as_u16());
        let success = response.status().is_success();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(_) => {
                    receipt.save()?;
                    return Err(error(ErrorCode::BackendFailed, "Local HTTP body failed"));
                }
            };
            if receipt.response.len().saturating_add(chunk.len()) > self.limits.response_bytes {
                let remaining = self
                    .limits
                    .response_bytes
                    .saturating_sub(receipt.response.len());
                receipt.response.extend(chunk.iter().take(remaining));
                receipt.save()?;
                return Err(error(
                    ErrorCode::LimitExceeded,
                    "Local HTTP response cap exceeded",
                ));
            }
            receipt.response.extend_from_slice(&chunk);
        }
        let bytes = receipt.response.clone();
        receipt.save()?;
        if !success {
            return Err(error(
                ErrorCode::BackendFailed,
                "Local service returned non-success status",
            ));
        }
        decode(&bytes)
    }
}
fn answer_schema(request: &ModelRequest) -> serde_json::Value {
    let mut properties = serde_json::Map::new();
    for named in &request.questions {
        let fields = match &named.question {
            sapho_core::Question::Boolean { .. } => {
                serde_json::json!({"kind":{"const":"boolean"},"probability":{"type":"number","minimum":0,"maximum":1}})
            }
            sapho_core::Question::Choice { options, .. } => {
                serde_json::json!({"kind":{"const":"choice"},"selected":{"type":"string","enum":options.iter().map(|o|&o.label).collect::<Vec<_>>()},"confidence":{"type":"number","minimum":0,"maximum":1},"probabilities":{"type":"null"}})
            }
            sapho_core::Question::Score { levels, .. } => {
                serde_json::json!({"kind":{"const":"score"},"expected":{"type":"number","minimum":0,"maximum":levels.len().saturating_sub(1)},"confidence":{"type":"number","minimum":0,"maximum":1},"probabilities":{"type":"null"}})
            }
        };
        let required = fields
            .as_object()
            .map(|m| m.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        properties.insert(named.id.clone(),serde_json::json!({"type":"object","properties":fields,"required":required,"additionalProperties":false}));
    }
    serde_json::json!({"type":"object","properties":{"answers":{"type":"object","required":properties.keys().cloned().collect::<Vec<_>>(),"properties":properties,"additionalProperties":false}},"required":["answers"],"additionalProperties":false})
}
#[derive(Deserialize)]
struct Wire {
    model: String,
    response: String,
    done: bool,
    done_reason: Option<String>,
    prompt_eval_count: Option<u64>,
    eval_count: Option<u64>,
    total_duration: Option<u64>,
    load_duration: Option<u64>,
    prompt_eval_duration: Option<u64>,
    eval_duration: Option<u64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Answers {
    answers: BTreeMap<String, Answer>,
}
fn decode(bytes: &[u8]) -> Result<ModelResponse> {
    // Wire may include provider timing/metadata; typed answers reject extra fields.
    let wire: Wire = serde_json::from_slice(bytes)
        .map_err(|_| error(ErrorCode::InvalidAnswer, "Invalid local wire response"))?;
    if !wire.done || wire.done_reason.as_deref() != Some("stop") {
        return Err(error(
            ErrorCode::InvalidAnswer,
            "Local generation incomplete or truncated",
        ));
    }
    let answers: Answers = serde_json::from_str(&wire.response)
        .map_err(|_| error(ErrorCode::InvalidAnswer, "Invalid typed local answers"))?;
    // This provider's declared schema requests no invented distributions. Enforce
    // that contract even if a server returns JSON that violates its own schema;
    // the complete raw response has already been captured by call().
    if answers.answers.values().any(|answer| {
        matches!(
            answer,
            Answer::Choice {
                probabilities: Some(_),
                ..
            } | Answer::Score {
                probabilities: Some(_),
                ..
            }
        )
    }) {
        return Err(error(
            ErrorCode::InvalidAnswer,
            "Local answer violates requested null-distribution schema",
        ));
    }
    Ok(ModelResponse {
        model: wire.model,
        answers: answers.answers,
        usage: wire
            .prompt_eval_count
            .zip(wire.eval_count)
            .map(|(input_tokens, output_tokens)| sapho_core::Usage {
                billing_units: None,
                input_tokens,
                output_tokens,
            }),
    })
}
#[async_trait]
impl ModelBackend for OllamaBackend {
    async fn infer(&self, request: &ModelRequest) -> Result<ModelResponse> {
        request.validate()?;
        tokio::time::timeout(self.limits.timeout, self.call(request))
            .await
            .map_err(|_| {
                error(
                    ErrorCode::DeadlineExceeded,
                    "Local inference deadline exceeded; call may be indeterminate",
                )
            })?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> ModelRequest {
        ModelRequest {
            distribution_policy: sapho_core::DistributionPolicy::Strict {},
            backend: sapho_core::BackendId::new("judge").unwrap(),
            model: "local-test".into(),
            expected_model: Some("local-test".into()),
            state: sapho_core::Value::Record(BTreeMap::new()),
            questions: vec![sapho_core::NamedQuestion {
                id: "x".into(),
                question: sapho_core::Question::Choice {
                    instructions: "select".into(),
                    options: vec![
                        sapho_core::ChoiceOption {
                            label: "yes".into(),
                            description: "yes".into(),
                        },
                        sapho_core::ChoiceOption {
                            label: "no".into(),
                            description: "no".into(),
                        },
                    ],
                },
            }],
        }
    }
    /// Trace: FR-048-AC-1
    #[test]
    fn self_report_confidence_is_not_fabricated_distribution() {
        let bytes=serde_json::to_vec(&serde_json::json!({"model":"local-test","done":true,"done_reason":"stop","response":"{\"answers\":{\"x\":{\"kind\":\"choice\",\"selected\":\"yes\",\"confidence\":0.7,\"probabilities\":null}}}"})).unwrap();
        let response = decode(&bytes).unwrap();
        sapho_core::validate_response(&request(), &response).unwrap();
        assert!(matches!(
            response.answers.get("x"),
            Some(Answer::Choice {
                probabilities: None,
                ..
            })
        ));
        let missing=serde_json::to_vec(&serde_json::json!({"model":"local-test","done":true,"done_reason":"stop","response":"{\"answers\":{\"x\":{\"kind\":\"choice\",\"selected\":\"yes\",\"probabilities\":null}}}"})).unwrap();
        assert!(decode(&missing).is_err());
    }
    /// Trace: FR-048-AC-10
    #[test]
    fn returned_distributions_violate_the_declared_local_schema() {
        for answer in [
            serde_json::json!({"kind":"choice","selected":"yes","confidence":0.7,"probabilities":{"yes":0.7,"no":0.3}}),
            serde_json::json!({"kind":"score","expected":1,"confidence":0.7,"probabilities":{"0":0.3,"1":0.7}}),
        ] {
            let body = serde_json::to_vec(&serde_json::json!({"model":"synthetic","done":true,"done_reason":"stop","response":serde_json::to_string(&serde_json::json!({"answers":{"x":answer}})).unwrap()})).unwrap();
            assert_eq!(decode(&body).unwrap_err().code, ErrorCode::InvalidAnswer);
        }
    }
    /// Trace: FR-048-AC-2
    #[tokio::test]
    async fn preflight_never_dispatches_or_captures_a_call() {
        assert!(OllamaBackend::new("http://example.com", Limits::default()).is_err());
        let backend = OllamaBackend::new(
            "http://127.0.0.1:1",
            Limits {
                context_tokens: 1025,
                output_tokens: 1,
                ..Limits::default()
            },
        )
        .unwrap();
        let failure = backend.infer(&request()).await.unwrap_err();
        assert_eq!(failure.code, ErrorCode::LimitExceeded);
        // First-time binding is refused even after a no-dispatch invocation.
        assert_eq!(
            backend
                .pin_model(ModelPin {
                    model: "local-test".into(),
                    digest: "late".into()
                })
                .unwrap_err()
                .code,
            ErrorCode::Config
        );
        assert_eq!(
            failure.context.get("output_tokens").map(String::as_str),
            Some("1")
        );
        assert_eq!(
            failure.context.get("template_reserve").map(String::as_str),
            Some("1024")
        );
        assert_eq!(
            failure.context.get("context_tokens").map(String::as_str),
            Some("1025")
        );
        assert_eq!(
            failure.context.get("token_upper_bound").map(String::as_str),
            Some("one_utf8_byte_per_token")
        );
        assert!(
            failure
                .context
                .get("prompt_bytes")
                .unwrap()
                .parse::<usize>()
                .unwrap()
                > 0
        );
        assert_eq!(failure.context.len(), 5);
        let receipts = backend.receipts().unwrap();
        assert_eq!(receipts.len(), 1);
        assert!(!receipts[0].made_call);
        assert!(receipts[0].request.is_empty());
        let budget = &receipts[0].prompt;
        assert_eq!(
            budget.instruction_bytes
                + budget.state_bytes
                + budget.question_bytes
                + budget.envelope_bytes,
            budget.prompt_bytes
        );
        assert_eq!(receipts[0].typed_request_sha256.len(), 64);
    }
    async fn server(body: &str, delay: Duration) -> (String, tokio::task::JoinHandle<()>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let body = body.to_owned();
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0; 65536];
            let n = socket.read(&mut buf).await.unwrap();
            assert!(String::from_utf8_lossy(&buf[..n]).starts_with("POST /api/generate"));
            tokio::time::sleep(delay).await;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = socket.write_all(response.as_bytes()).await;
        });
        (url, task)
    }
    /// Trace: FR-048-AC-3
    #[tokio::test]
    async fn invalid_http200_preserves_made_call_and_raw_bytes() {
        let (url, task) = server("invalid-json", Duration::ZERO).await;
        let backend = OllamaBackend::new(&url, Limits::default()).unwrap();
        assert_eq!(
            backend.infer(&request()).await.unwrap_err().code,
            ErrorCode::InvalidAnswer
        );
        task.await.unwrap();
        let receipts = backend.receipts().unwrap();
        assert_eq!(receipts.len(), 1);
        assert!(receipts[0].made_call);
        assert_eq!(receipts[0].status, Some(200));
        assert_eq!(receipts[0].response, b"invalid-json");
    }
    /// Trace: FR-048-AC-7, FR-048-AC-9
    #[tokio::test]
    async fn invalid_answers_retain_service_usage_and_prompt_budget() {
        let body = r#"{"model":"local-test","response":"invalid answers","done":true,"done_reason":"stop","prompt_eval_count":19,"eval_count":3,"total_duration":12000000,"load_duration":1000000,"prompt_eval_duration":4000000,"eval_duration":7000000}"#;
        let (url, task) = server(body, Duration::ZERO).await;
        let backend = OllamaBackend::new(&url, Limits::default()).unwrap();
        assert_eq!(
            backend.infer(&request()).await.unwrap_err().code,
            ErrorCode::InvalidAnswer
        );
        task.await.unwrap();
        let receipts = backend.receipts().unwrap();
        let telemetry = receipts[0].telemetry.as_ref().unwrap();
        assert_eq!(telemetry.input_tokens, Some(19));
        assert_eq!(telemetry.output_tokens, Some(3));
        assert_eq!(telemetry.total_duration_ns, Some(12000000));
        assert!(receipts[0].made_call);
        let budget = &receipts[0].prompt;
        assert_eq!(
            budget.prompt_bytes,
            budget.instruction_bytes
                + budget.state_bytes
                + budget.question_bytes
                + budget.envelope_bytes
        );
        assert_eq!(receipts[0].response, body.as_bytes());
        let admission = RequestBudget::measure(&request(), &Limits::default()).unwrap();
        assert!(admission.allowed());
        assert_eq!(admission.wire_request_bytes, receipts[0].request.len());
        assert_eq!(
            admission.typed_request_sha256,
            receipts[0].typed_request_sha256
        );
        assert_eq!(admission.prompt.format, PromptFormat::ModelRequestJson);
        assert_eq!(admission.prompt.prompt_bytes, budget.prompt_bytes);
    }
    /// Trace: FR-048-AC-10
    #[tokio::test]
    async fn schema_violating_distribution_retains_wire_and_usage_without_retry() {
        let body = r#"{"model":"local-test","response":"{\"answers\":{\"x\":{\"kind\":\"choice\",\"selected\":\"yes\",\"confidence\":0.7,\"probabilities\":{\"yes\":0.7,\"no\":0.3}}}}","done":true,"done_reason":"stop","prompt_eval_count":19,"eval_count":3}"#;
        let (url, task) = server(body, Duration::ZERO).await;
        let backend = OllamaBackend::new(&url, Limits::default()).unwrap();
        assert_eq!(
            backend.infer(&request()).await.unwrap_err().code,
            ErrorCode::InvalidAnswer
        );
        task.await.unwrap();
        let receipts = backend.receipts().unwrap();
        assert_eq!(receipts.len(), 1);
        assert!(receipts[0].made_call);
        assert_eq!(receipts[0].status, Some(200));
        assert_eq!(receipts[0].response, body.as_bytes());
        assert_eq!(
            receipts[0].telemetry.as_ref().unwrap().input_tokens,
            Some(19)
        );
        assert_eq!(
            receipts[0].telemetry.as_ref().unwrap().output_tokens,
            Some(3)
        );
    }
    /// Trace: FR-048-AC-2, FR-048-AC-7, FR-048-AC-8, FR-048-AC-9
    #[tokio::test]
    async fn explicit_framing_dispatches_exact_measured_prompt_and_one_byte_over_refuses() {
        let invalid = Limits {
            output_tokens: 0,
            ..Limits::default()
        };
        assert_eq!(
            RequestBudget::measure(&request(), &invalid)
                .unwrap_err()
                .code,
            ErrorCode::Config
        );
        assert_eq!(
            PromptBudget::measure(&request(), &invalid)
                .unwrap_err()
                .code,
            ErrorCode::Config
        );
        let mut input = request();
        let owner = "Complete café owner\nFIELD {fake}\nQuotes \" and slash \\";
        input.state = sapho_core::Value::Record(BTreeMap::from([
            ("owner".into(), sapho_core::Value::Text(owner.into())),
            (
                "interpretation".into(),
                sapho_core::Value::Text(
                    serde_json::to_string(
                        &serde_json::json!({"owner":owner,"evidence":[0.7,null]}),
                    )
                    .unwrap(),
                ),
            ),
        ]));
        let format = PromptFormat::FramedStateV1;
        let mut limits = Limits {
            prompt_format: format,
            ..Limits::default()
        };
        let measured = PromptBudget::measure(&input, &limits).unwrap();
        limits.context_tokens = measured.prompt_bytes + limits.output_tokens + 1024;
        let body = r#"{"model":"local-test","done":true,"done_reason":"stop","response":"{\"answers\":{\"x\":{\"kind\":\"choice\",\"selected\":\"yes\",\"confidence\":0.7,\"probabilities\":null}}}","prompt_eval_count":17,"eval_count":3}"#;
        let (url, task) = server(body, Duration::ZERO).await;
        let backend = OllamaBackend::new(&url, limits).unwrap();
        let response = backend.infer(&input).await.unwrap();
        sapho_core::validate_response(&input, &response).unwrap();
        task.await.unwrap();
        let captured = backend.receipts().unwrap();
        assert_eq!(captured.len(), 1);
        assert!(captured[0].made_call);
        assert_eq!(captured[0].prompt.format, format);
        assert_eq!(captured[0].prompt.prompt_bytes, measured.prompt_bytes);
        let wire: serde_json::Value = serde_json::from_slice(&captured[0].request).unwrap();
        let prompt = wire["prompt"].as_str().unwrap();
        assert_eq!(prompt, prompt::render(&input, format).unwrap().text);
        assert!(prompt.contains(owner));
        assert_eq!(
            captured[0].telemetry.as_ref().unwrap().input_tokens,
            Some(17)
        );
        assert_eq!(
            captured[0].telemetry.as_ref().unwrap().output_tokens,
            Some(3)
        );
        assert_eq!(
            captured[0].typed_request_sha256,
            format!("{:x}", Sha256::digest(serde_json::to_vec(&input).unwrap()))
        );

        let admitted = RequestBudget::measure(&input, &limits).unwrap();
        assert!(admitted.allowed());
        assert_eq!(
            admitted.typed_request_sha256,
            captured[0].typed_request_sha256
        );
        assert_eq!(admitted.wire_request_bytes, captured[0].request.len());
        assert_eq!(
            admitted.typed_request_bytes,
            serde_json::to_vec(&input).unwrap().len()
        );
        assert_eq!(captured[0].request, serde_json::to_vec(&serde_json::json!({
            "format":answer_schema(&input), "model":input.model,
            "options":{"num_ctx":limits.context_tokens,"num_predict":limits.output_tokens,"temperature":0},
            "prompt":prompt,"stream":false,"think":limits.think,
        })).unwrap());
        let byte_limited = Limits {
            request_bytes: admitted.wire_request_bytes - 1,
            ..limits
        };
        assert!(admitted.typed_request_bytes <= byte_limited.request_bytes);
        let wire_refusal = RequestBudget::measure(&input, &byte_limited).unwrap();
        assert!(!wire_refusal.allowed());
        assert!(wire_refusal.prompt.allowed());
        let refused = OllamaBackend::new("http://127.0.0.1:1", byte_limited).unwrap();
        assert_eq!(
            refused.infer(&input).await.unwrap_err().code,
            ErrorCode::LimitExceeded
        );
        assert_eq!(refused.progress().unwrap().dispatch_attempts, 0);
        assert!(!refused.receipts().unwrap()[0].made_call);

        limits.context_tokens -= 1;
        let refused = OllamaBackend::new("http://127.0.0.1:1", limits).unwrap();
        assert!(!PromptBudget::measure(&input, &limits).unwrap().allowed());
        assert_eq!(
            refused.infer(&input).await.unwrap_err().code,
            ErrorCode::LimitExceeded
        );
        assert_eq!(refused.progress().unwrap().dispatch_attempts, 0);
        let capture = refused.receipts().unwrap();
        assert_eq!(capture.len(), 1);
        assert!(!capture[0].made_call);
        assert_eq!(capture[0].prompt.prompt_bytes, measured.prompt_bytes);
        assert!(capture[0].request.is_empty());
        assert!(capture[0].response.is_empty());
    }

    /// Trace: FR-048-AC-5, FR-048-AC-4
    #[tokio::test]
    async fn competing_backends_wait_before_http_and_timeout_without_dispatch() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let dir = tempfile::tempdir().unwrap();
        let capacity = dir.path().join("service.lock");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (observed_tx, observed_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0; 65536];
            let n = socket.read(&mut buf).await.unwrap();
            assert!(String::from_utf8_lossy(&buf[..n]).starts_with("POST /api/generate"));
            observed_tx.send(()).unwrap();
            release_rx.await.unwrap();
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 12\r\nConnection: close\r\n\r\ninvalid-json").await.unwrap();
        });
        let owner = std::sync::Arc::new(
            OllamaBackend::new_shared(&url, Limits::default(), &capacity).unwrap(),
        );
        let active = owner.clone();
        let first = tokio::spawn(async move { active.infer(&request()).await });
        tokio::time::timeout(Duration::from_secs(2), observed_rx)
            .await
            .unwrap()
            .unwrap();
        let waiter = OllamaBackend::new_shared(
            &url,
            Limits {
                timeout: Duration::from_millis(60),
                ..Default::default()
            },
            &capacity,
        )
        .unwrap();
        assert_eq!(
            waiter.infer(&request()).await.unwrap_err().code,
            ErrorCode::DeadlineExceeded
        );
        assert_eq!(waiter.progress().unwrap().dispatch_attempts, 0);
        assert!(waiter.receipts().unwrap().is_empty());
        assert_eq!(owner.progress().unwrap().dispatch_attempts, 1);
        release_tx.send(()).unwrap();
        assert_eq!(
            first.await.unwrap().unwrap_err().code,
            ErrorCode::InvalidAnswer
        );
        server.await.unwrap();
        // The completed owner released capacity, even though its answer was invalid.
        let file = capacity::prepare(&capacity).unwrap();
        let lease = tokio::time::timeout(Duration::from_secs(1), capacity::acquire(&file))
            .await
            .unwrap()
            .unwrap();
        drop(lease);
    }
    /// Trace: FR-048-AC-4
    #[tokio::test]
    async fn timeout_retains_dispatched_attempt_without_retry() {
        let (url, task) = server("{}", Duration::from_secs(1)).await;
        let backend = OllamaBackend::new(
            &url,
            Limits {
                timeout: Duration::from_millis(50),
                ..Limits::default()
            },
        )
        .unwrap();
        assert_eq!(
            backend.infer(&request()).await.unwrap_err().code,
            ErrorCode::DeadlineExceeded
        );
        let receipts = backend.receipts().unwrap();
        assert_eq!(receipts.len(), 1);
        assert!(receipts[0].made_call);
        task.abort();
    }
}

#[cfg(test)]
mod inspection_tests {
    use super::*;
    /// Trace: FR-048-AC-12
    #[tokio::test]
    async fn pinned_digest_drift_refuses_generation_and_retains_observation() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            for path in ["/api/tags", "/api/show", "/api/ps"] {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut bytes = [0; 4096];
                let n = stream.read(&mut bytes).await.unwrap();
                assert!(String::from_utf8_lossy(&bytes[..n]).contains(path));
                let body = match path {
                    "/api/tags" => {
                        serde_json::json!({"models":[{"name":"local-test","digest":"changed"}]})
                    }
                    "/api/show" => serde_json::json!({"capabilities":["completion"]}),
                    _ => serde_json::json!({"models":[]}),
                }
                .to_string();
                stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
            }
        });
        let tmp = tempfile::tempdir().unwrap();
        let backend =
            OllamaBackend::new_shared(&url, Limits::default(), &tmp.path().join("capacity.lock"))
                .unwrap();
        backend
            .pin_model(ModelPin {
                model: "local-test".into(),
                digest: "original".into(),
            })
            .unwrap();
        assert_eq!(
            backend
                .pin_model(ModelPin {
                    model: "local-test".into(),
                    digest: "other".into()
                })
                .unwrap_err()
                .code,
            ErrorCode::ModelMismatch
        );
        let request = ModelRequest {
            backend: sapho_core::BackendId::new("judge").unwrap(),
            model: "local-test".into(),
            expected_model: Some("local-test".into()),
            distribution_policy: sapho_core::DistributionPolicy::Strict {},
            state: sapho_core::Value::Record(BTreeMap::from([(
                "owner".into(),
                sapho_core::Value::Text("synthetic".into()),
            )])),
            questions: vec![sapho_core::NamedQuestion {
                id: "x".into(),
                question: sapho_core::Question::Boolean {
                    instructions: "Synthetic check".into(),
                    yes: "yes".into(),
                    no: "no".into(),
                },
            }],
        };
        assert_eq!(
            backend.infer(&request).await.unwrap_err().code,
            ErrorCode::ModelMismatch
        );
        task.await.unwrap();
        assert_eq!(backend.progress().unwrap().dispatch_attempts, 0);
        let receipts = backend.receipts().unwrap();
        assert_eq!(receipts.len(), 1);
        assert!(!receipts[0].made_call);
        assert_eq!(
            receipts[0].model_observation.as_ref().unwrap().digest,
            "changed"
        );
        assert!(receipts[0].request.is_empty() && receipts[0].response.is_empty());
        // The model pin cannot release or bypass the owner's capacity discipline.
        let file = capacity::prepare(&tmp.path().join("capacity.lock")).unwrap();
        let lease = capacity::acquire(&file).await.unwrap();
        drop(lease);
    }
    /// Trace: FR-048-AC-2
    #[tokio::test]
    async fn model_inspection_reports_digest_and_gpu_without_inference() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            for path in ["/api/tags", "/api/show", "/api/ps"] {
                let (mut s, _) = listener.accept().await.unwrap();
                let mut b = [0; 4096];
                let n = s.read(&mut b).await.unwrap();
                assert!(String::from_utf8_lossy(&b[..n]).contains(path));
                let body=match path{"/api/tags"=>serde_json::json!({"models":[{"name":"local-test","digest":"abc"}]}),"/api/show"=>serde_json::json!({"capabilities":["completion"]}),_=>serde_json::json!({"models":[{"name":"local-test","size":100,"size_vram":100,"context_length":32768}]})}.to_string();
                let reply = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                s.write_all(reply.as_bytes()).await.unwrap();
            }
        });
        let backend = OllamaBackend::new(&url, Limits::default()).unwrap();
        let info = backend.inspect("local-test").await.unwrap();
        task.await.unwrap();
        assert!(info.completion);
        assert_eq!(info.digest, "abc");
        assert_eq!(info.loaded_context, Some(32768));
        assert_eq!(info.gpu_bytes, 100);
        assert_eq!(backend.progress().unwrap().dispatch_attempts, 0);
        assert!(backend.receipts().unwrap().is_empty());
    }
}
