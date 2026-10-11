// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Typed, bounded adapter for the official OpenAI Decisions endpoint.
//!
//! A host prepares credentials before constructing this backend. The transport seam
//! permits deterministic tests without account access; the codec and limits stay real.
mod wire;
use async_trait::async_trait;
use ix_cli_kit::secrets::SecretValue;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use sapho_core::{
    ErrorCode, ModelBackend, ModelRequest, ModelResponse, RawExchange, Result, SaphoError,
};
use sapho_http::{CallGate, NativeTransport};
pub use sapho_http::{HttpResponse, Transport};
use std::{sync::Arc, time::Duration};

/// The only production URL used by this adapter.
pub const ENDPOINT: &str = "https://api.openai.com/v1/decisions";
/// The sole model supported by the current public beta contract.
pub const MODEL: &str = "gpt-6-luna";

/// Per-adapter bounds, including waiting for concurrency capacity.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Complete call deadline.
    pub timeout: Duration,
    /// Maximum serialized request bytes.
    pub request_bytes: usize,
    /// Maximum successful response bytes.
    pub response_bytes: usize,
    /// Maximum simultaneous HTTP calls.
    pub in_flight: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            request_bytes: 1_048_576,
            response_bytes: 8 * 1_048_576,
            in_flight: 4,
        }
    }
}
impl Limits {
    fn validate(self) -> std::result::Result<(), ConfigurationError> {
        let max = Self::default();
        if self.timeout.is_zero()
            || self.timeout > max.timeout
            || self.request_bytes == 0
            || self.request_bytes > max.request_bytes
            || self.response_bytes == 0
            || self.response_bytes > max.response_bytes
            || self.in_flight == 0
            || self.in_flight > max.in_flight
        {
            return Err(ConfigurationError::InvalidLimits);
        }
        Ok(())
    }
}
/// Redacted adapter preparation errors.
#[derive(Debug, thiserror::Error)]
pub enum ConfigurationError {
    /// A bound is zero or exceeds its maximum.
    #[error("Invalid Decisions limits")]
    InvalidLimits,
    /// Bearer credential cannot form a safe header.
    #[error("Invalid Decisions credential")]
    InvalidCredential,
    /// Native HTTP client initialization failed.
    #[error("Unable to prepare Decisions transport")]
    Transport,
}
fn failure(code: ErrorCode) -> SaphoError {
    SaphoError::new(code, "Decisions request refused")
}
/// One prepared Decisions backend with a private bearer credential.
pub struct DecisionsBackend {
    transport: Arc<dyn Transport>,
    limits: Limits,
    capacity: CallGate,
}
impl DecisionsBackend {
    /// Prepare the fixed production endpoint without making a model request.
    pub fn new(
        secret: &SecretValue,
        limits: Limits,
    ) -> std::result::Result<Self, ConfigurationError> {
        let endpoint = reqwest::Url::parse(ENDPOINT).map_err(|_| ConfigurationError::Transport)?;
        Self::prepare_http(secret, limits, endpoint)
    }
    fn prepare_http(
        secret: &SecretValue,
        limits: Limits,
        endpoint: reqwest::Url,
    ) -> std::result::Result<Self, ConfigurationError> {
        limits.validate()?;
        if secret.expose_secret().is_empty()
            || !secret
                .expose_secret()
                .bytes()
                .all(|byte| byte.is_ascii_graphic())
        {
            return Err(ConfigurationError::InvalidCredential);
        }
        let mut authorization =
            HeaderValue::from_str(&format!("Bearer {}", secret.expose_secret()))
                .map_err(|_| ConfigurationError::InvalidCredential)?;
        authorization.set_sensitive(true);
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, authorization);
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let transport = NativeTransport::new(
            endpoint,
            headers,
            limits.timeout,
            "Decisions request refused",
        )
        .map_err(|_| ConfigurationError::Transport)?;
        Self::with_transport(Arc::new(transport), limits)
    }
    /// Inject only transport; codec, admission and limits remain active.
    pub fn with_transport(
        transport: Arc<dyn Transport>,
        limits: Limits,
    ) -> std::result::Result<Self, ConfigurationError> {
        limits.validate()?;
        Ok(Self {
            transport,
            limits,
            capacity: CallGate::new(limits.in_flight),
        })
    }
}
#[async_trait]
impl ModelBackend for DecisionsBackend {
    async fn infer(&self, request: &ModelRequest) -> Result<ModelResponse> {
        let deadline = tokio::time::Instant::now()
            .checked_add(self.limits.timeout)
            .ok_or_else(|| failure(ErrorCode::DeadlineExceeded))?;
        let body = wire::encode(request, self.limits.request_bytes)?;
        self.capacity
            .run(deadline, "Decisions request refused", || async {
                let response = self
                    .transport
                    .post(&body, self.limits.response_bytes)
                    .await?;
                if response.status != 200 {
                    let code = match response.status {
                        401 | 403 => ErrorCode::Unauthorized,
                        429 => ErrorCode::RateLimited,
                        400 | 422 => ErrorCode::ServiceValidation,
                        _ => ErrorCode::BackendFailed,
                    };
                    return Err(
                        failure(code).with_context("http_status", response.status.to_string())
                    );
                }
                if response.body.len() > self.limits.response_bytes {
                    return Err(failure(ErrorCode::LimitExceeded));
                }
                let response_text = std::str::from_utf8(&response.body)
                    .map_err(|_| failure(ErrorCode::InvalidAnswer))?;
                let raw = RawExchange {
                    request: String::from_utf8(body)
                        .map_err(|_| failure(ErrorCode::InvalidValue))?,
                    response: response_text.to_owned(),
                };
                let decoded = wire::decode(request, &response.body, self.limits.response_bytes)
                    .map_err(|error| error.with_raw(raw.clone()))?;
                if tokio::time::Instant::now() >= deadline {
                    return Err(failure(ErrorCode::DeadlineExceeded).with_raw(raw));
                }
                Ok(ModelResponse {
                    raw: Some(raw),
                    ..decoded
                })
            })
            .await
    }
}
#[cfg(test)]
mod tests;
