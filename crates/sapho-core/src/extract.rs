// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! One-call structured extraction port (FR-048).
//!
//! An [`Extractor`] performs a single stateless exchange; [`extract`] is the only way
//! to obtain an [`ExtractResponse`], so every implementation gets the same request
//! checks and schema validation. [`ScriptedExtractor`] is a test double for hosts.
use crate::{ErrorCode, SaphoError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::VecDeque, sync::Mutex};

/// One item's extraction: fixed task text, one input and the schema the answer must match.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractRequest {
    /// Requested model identity.
    pub model: String,
    /// Task text, identical for every item of a task.
    pub instructions: String,
    /// This item's text and declared data, never another item or an earlier exchange.
    pub input: String,
    /// JSON Schema (draft 2020-12) object the answer must match.
    pub schema: Value,
}

/// The exact bytes of one exchange, for storage by content hash.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawExchange {
    /// Request bytes as sent.
    pub request: Vec<u8>,
    /// Response bytes as received.
    pub response: Vec<u8>,
}

/// Provider-reported usage; a figure the provider did not report stays absent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractUsage {
    /// Prompt tokens.
    pub input_tokens: Option<u64>,
    /// Generated tokens.
    pub output_tokens: Option<u64>,
    /// Elapsed milliseconds for the whole exchange.
    pub elapsed_ms: Option<u64>,
    /// Milliseconds the provider spent loading the model.
    pub load_ms: Option<u64>,
    /// Milliseconds the provider spent on the prompt.
    pub prompt_ms: Option<u64>,
    /// Milliseconds the provider spent generating.
    pub generation_ms: Option<u64>,
}

/// The model that answered: its reported name and, when known, the digest of its weights.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelIdentity {
    /// Model name as the provider reported it.
    pub name: String,
    /// Weights digest, `sha256:<hex>` for providers that report one.
    pub digest: Option<String>,
}

/// What an [`Extractor`] hands back before the core has checked the answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Completion {
    /// The answer as JSON text bytes; the core parses and schema-checks them.
    pub answer: Vec<u8>,
    /// Reported usage.
    pub usage: ExtractUsage,
    /// Exact request and response bytes.
    pub raw: RawExchange,
    /// The model that answered.
    pub model: ModelIdentity,
}

/// A schema-checked answer. Only [`extract`] constructs it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct ExtractResponse {
    /// The parsed answer, valid against the request schema.
    pub value: Value,
    /// Reported usage.
    pub usage: ExtractUsage,
    /// Exact request and response bytes.
    pub raw: RawExchange,
    /// The model that answered.
    pub model: ModelIdentity,
}

/// The numbers behind a [`ErrorCode::TooLarge`] refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TooLarge {
    /// The provider's prompt token count, when it gave one.
    pub reported_input_tokens: Option<u64>,
    /// The context size the request was held to.
    pub context_tokens: u64,
    /// Tokens reserved for the answer.
    pub reserved_output_tokens: u64,
}

/// A refused, failed or unusable extraction.
#[derive(Debug, Clone, PartialEq, Serialize, thiserror::Error)]
#[error("{code:?}: {message}")]
pub struct ExtractError {
    /// Machine-readable category.
    pub code: ErrorCode,
    /// Entry from the adapter's closed reason list, when it has one for this refusal.
    pub reason: Option<&'static str>,
    /// Explanation free of request and response content.
    pub message: Box<str>,
    /// JSON Pointer of the first schema violation.
    pub pointer: Option<String>,
    /// Detail of a `TooLarge` refusal.
    pub too_large: Option<TooLarge>,
    /// The exchange, when one took place.
    pub raw: Option<RawExchange>,
    /// Usage, when the provider reported it.
    pub usage: Option<ExtractUsage>,
}
impl ExtractError {
    /// A refusal with no reason, pointer, detail or exchange.
    pub fn new(code: ErrorCode, message: impl Into<Box<str>>) -> Self {
        Self {
            code,
            reason: None,
            message: message.into(),
            pointer: None,
            too_large: None,
            raw: None,
            usage: None,
        }
    }
    /// Name the adapter's reason for the refusal.
    pub fn with_reason(mut self, reason: &'static str) -> Self {
        self.reason = Some(reason);
        self
    }
    /// Retain the exchange that produced the refusal.
    pub fn with_raw(mut self, raw: RawExchange) -> Self {
        self.raw = Some(raw);
        self
    }
    /// Retain the usage the provider reported.
    pub fn with_usage(mut self, usage: ExtractUsage) -> Self {
        self.usage = Some(usage);
        self
    }
    /// Attach the numbers of a `TooLarge` refusal.
    pub fn with_too_large(mut self, detail: TooLarge) -> Self {
        self.too_large = Some(detail);
        self
    }
}
impl From<ExtractError> for SaphoError {
    /// Keep the reason and `TooLarge` numbers as named context fields.
    fn from(error: ExtractError) -> Self {
        let mut out = SaphoError::new(error.code, error.message);
        if let Some(reason) = error.reason {
            out = out.with_context("reason", reason);
        }
        if let Some(pointer) = error.pointer {
            out = out.with_context("pointer", pointer);
        }
        if let Some(detail) = error.too_large {
            if let Some(reported) = detail.reported_input_tokens {
                out = out.with_context("reported_input_tokens", reported.to_string());
            }
            out = out
                .with_context("context_tokens", detail.context_tokens.to_string())
                .with_context(
                    "reserved_output_tokens",
                    detail.reserved_output_tokens.to_string(),
                );
        }
        out
    }
}

/// One stateless exchange with a structured-output provider.
///
/// Implementations carry no conversation or session between calls and never check the
/// answer themselves; callers go through [`extract`].
#[async_trait]
pub trait Extractor: Send + Sync {
    /// Perform the single exchange for an already checked request.
    async fn exchange(&self, request: &ExtractRequest) -> Result<Completion, ExtractError>;
}

fn config(reason: &'static str, message: &'static str) -> ExtractError {
    ExtractError::new(ErrorCode::Config, message).with_reason(reason)
}

/// Check the request, invoke the single exchange and check the answer.
///
/// Requests the core refuses never reach the implementation: an empty field, a schema
/// that is not an object or does not compile, and a `$ref` that leaves the schema
/// document are `Config`. The schema validator never reads a file or URL.
pub async fn extract(
    extractor: &dyn Extractor,
    request: &ExtractRequest,
) -> Result<ExtractResponse, ExtractError> {
    if request.model.is_empty() {
        return Err(config("empty_model", "Empty model"));
    }
    if request.instructions.is_empty() {
        return Err(config("empty_instructions", "Empty instructions"));
    }
    if request.input.is_empty() {
        return Err(config("empty_input", "Empty input"));
    }
    if !request.schema.is_object() {
        return Err(config("schema_not_object", "Schema is not an object"));
    }
    if has_foreign_ref(&request.schema) {
        return Err(config(
            "schema_external_ref",
            "Schema reference leaves the schema document",
        ));
    }
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .build(&request.schema)
        .map_err(|_| config("schema_invalid", "Schema does not compile"))?;
    let exchange = extractor.exchange(request).await?;
    let invalid = |reason: &'static str, message: &'static str, pointer: Option<String>| {
        let mut error = ExtractError::new(ErrorCode::InvalidAnswer, message)
            .with_reason(reason)
            .with_raw(exchange.raw.clone())
            .with_usage(exchange.usage.clone());
        error.pointer = pointer;
        error
    };
    let value: Value = serde_json::from_slice(&exchange.answer)
        .map_err(|_| invalid("not_json", "Answer is not JSON", None))?;
    if let Some(violation) = validator.iter_errors(&value).next() {
        return Err(invalid(
            "schema_violation",
            "Answer violates the schema",
            Some(violation.instance_path().to_string()),
        ));
    }
    Ok(ExtractResponse {
        value,
        usage: exchange.usage,
        raw: exchange.raw,
        model: exchange.model,
    })
}

/// True when any `$ref` or `$dynamicRef` is not a same-document fragment.
fn has_foreign_ref(schema: &Value) -> bool {
    match schema {
        Value::Object(members) => members.iter().any(|(key, value)| {
            if matches!(key.as_str(), "$ref" | "$dynamicRef") {
                value.as_str().is_none_or(|target| !target.starts_with('#'))
            } else {
                has_foreign_ref(value)
            }
        }),
        Value::Array(items) => items.iter().any(has_foreign_ref),
        _ => false,
    }
}

/// An [`Extractor`] that replays configured outcomes in order and records its requests.
#[derive(Debug, Default)]
pub struct ScriptedExtractor {
    script: Mutex<VecDeque<Result<Completion, ExtractError>>>,
    received: Mutex<Vec<ExtractRequest>>,
}
impl ScriptedExtractor {
    /// Replay `script` front to back, one outcome per call.
    pub fn new(script: impl IntoIterator<Item = Result<Completion, ExtractError>>) -> Self {
        Self {
            script: Mutex::new(script.into_iter().collect()),
            received: Mutex::default(),
        }
    }
    /// Every request received so far, in order.
    pub fn requests(&self) -> Vec<ExtractRequest> {
        self.received
            .lock()
            .map(|received| received.clone())
            .unwrap_or_default()
    }
}
#[async_trait]
impl Extractor for ScriptedExtractor {
    async fn exchange(&self, request: &ExtractRequest) -> Result<Completion, ExtractError> {
        if let Ok(mut received) = self.received.lock() {
            received.push(request.clone());
        }
        self.script
            .lock()
            .ok()
            .and_then(|mut script| script.pop_front())
            .unwrap_or_else(|| {
                Err(ExtractError::new(
                    ErrorCode::BackendFailed,
                    "Scripted extractor has no outcome left",
                ))
            })
    }
}
