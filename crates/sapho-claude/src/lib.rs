// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Bounded Claude Messages backend for typed Sapho questions.
mod wire;
use async_trait::async_trait;
use ix_cli_kit::secrets::SecretValue;
use reqwest::{
    Client, Url,
    header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue},
};
use sapho_core::{
    ErrorCode, ModelBackend, ModelRequest, ModelResponse, RawExchange, Result, SaphoError,
};
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;

/// Stock Messages URL.
pub const ENDPOINT: &str = "https://api.anthropic.com/v1/messages";
/// Stable Messages version.
pub const API_VERSION: &str = "2023-06-01";
/// Per-adapter resource ceilings.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Total deadline, including permit wait and decode.
    pub timeout: Duration,
    /// Maximum serialized request bytes.
    pub request_bytes: usize,
    /// Maximum successful response bytes.
    pub response_bytes: usize,
    /// Maximum simultaneous calls.
    pub in_flight: usize,
    /// Provider output-token ceiling.
    pub max_tokens: u32,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            request_bytes: 1_048_576,
            response_bytes: 8 * 1_048_576,
            in_flight: 4,
            max_tokens: 1024,
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
            || self.max_tokens == 0
            || self.max_tokens > 4096
        {
            return Err(ConfigurationError::InvalidLimits);
        }
        Ok(())
    }
}
/// Redacted preparation refusal.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigurationError {
    /// Invalid bound.
    #[error("Invalid Claude limits")]
    InvalidLimits,
    /// Invalid bearer value.
    #[error("Invalid Claude credential")]
    InvalidCredential,
    /// Invalid workspace header.
    #[error("Invalid Claude workspace ID")]
    InvalidWorkspace,
    /// Invalid endpoint URL.
    #[error("Invalid Claude endpoint")]
    InvalidEndpoint,
    /// Native HTTP setup failed.
    #[error("Unable to prepare Claude transport")]
    Transport,
}
/// Bounded HTTP result; failure bodies are discarded.
pub struct HttpResponse {
    /// HTTP status.
    pub status: u16,
    /// Success body.
    pub body: Vec<u8>,
}
/// Injectable HTTP seam; implementations enforce `max_bytes` while collecting.
#[async_trait]
pub trait Transport: Send + Sync {
    /// Send one POST, without retry or redirect.
    async fn post(&self, body: &[u8], max_bytes: usize) -> Result<HttpResponse>;
}
struct HttpTransport {
    client: Client,
    endpoint: Url,
}
#[async_trait]
impl Transport for HttpTransport {
    async fn post(&self, body: &[u8], max_bytes: usize) -> Result<HttpResponse> {
        let mut response = self
            .client
            .post(self.endpoint.clone())
            .body(body.to_vec())
            .send()
            .await
            .map_err(transport_error)?;
        let status = response.status().as_u16();
        if status != 200 {
            return Ok(HttpResponse {
                status,
                body: Vec::new(),
            });
        }
        if response
            .content_length()
            .is_some_and(|n| n > max_bytes as u64)
        {
            return Err(failure(ErrorCode::LimitExceeded));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(transport_error)? {
            if bytes
                .len()
                .checked_add(chunk.len())
                .is_none_or(|n| n > max_bytes)
            {
                return Err(failure(ErrorCode::LimitExceeded));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(HttpResponse {
            status,
            body: bytes,
        })
    }
}
fn transport_error(error: reqwest::Error) -> SaphoError {
    failure(if error.is_timeout() {
        ErrorCode::DeadlineExceeded
    } else {
        ErrorCode::BackendFailed
    })
}
fn failure(code: ErrorCode) -> SaphoError {
    SaphoError::new(code, "Claude request refused")
}
/// Validate a private workspace header value.
pub fn validate_workspace_id(id: &str) -> std::result::Result<(), ConfigurationError> {
    let suffix = id
        .strip_prefix("wrkspc_")
        .ok_or(ConfigurationError::InvalidWorkspace)?;
    if suffix.is_empty()
        || id.len() > 128
        || !suffix.as_bytes()[0].is_ascii_alphanumeric()
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(ConfigurationError::InvalidWorkspace);
    }
    Ok(())
}
fn checked_endpoint(text: &str) -> std::result::Result<Url, ConfigurationError> {
    let url = Url::parse(text).map_err(|_| ConfigurationError::InvalidEndpoint)?;
    let host = url.host_str().ok_or(ConfigurationError::InvalidEndpoint)?;
    let loopback = host == "localhost" || host == "127.0.0.1" || host == "[::1]";
    if !(url.scheme() == "https" || (url.scheme() == "http" && loopback))
        || url.path() != "/v1/messages"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(ConfigurationError::InvalidEndpoint);
    }
    Ok(url)
}
/// Prepared Claude backend with credential held only in native transport headers.
pub struct ClaudeBackend {
    transport: Arc<dyn Transport>,
    limits: Limits,
    capacity: Semaphore,
}
impl ClaudeBackend {
    /// Prepare stock production transport without contacting the service.
    pub fn new(
        secret: &SecretValue,
        workspace_id: Option<&str>,
        limits: Limits,
    ) -> std::result::Result<Self, ConfigurationError> {
        Self::at_endpoint(ENDPOINT, secret, workspace_id, limits)
    }
    /// Prepare host-selected HTTPS or loopback HTTP transport.
    pub fn at_endpoint(
        endpoint: &str,
        secret: &SecretValue,
        workspace_id: Option<&str>,
        limits: Limits,
    ) -> std::result::Result<Self, ConfigurationError> {
        limits.validate()?;
        let endpoint = checked_endpoint(endpoint)?;
        let key = secret.expose_secret();
        if key.is_empty() || key.len() > 8192 || !key.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(ConfigurationError::InvalidCredential);
        }
        let mut bearer = HeaderValue::from_str(&format!("Bearer {key}"))
            .map_err(|_| ConfigurationError::InvalidCredential)?;
        bearer.set_sensitive(true);
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, bearer);
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert("anthropic-version", HeaderValue::from_static(API_VERSION));
        if let Some(id) = workspace_id {
            validate_workspace_id(id)?;
            headers.insert(
                "anthropic-workspace-id",
                HeaderValue::from_str(id).map_err(|_| ConfigurationError::InvalidWorkspace)?,
            );
        }
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .timeout(limits.timeout)
            .default_headers(headers)
            .build()
            .map_err(|_| ConfigurationError::Transport)?;
        Self::with_transport(Arc::new(HttpTransport { client, endpoint }), limits)
    }
    /// Inject transport while retaining codec, admission and limits.
    pub fn with_transport(
        transport: Arc<dyn Transport>,
        limits: Limits,
    ) -> std::result::Result<Self, ConfigurationError> {
        limits.validate()?;
        Ok(Self {
            transport,
            limits,
            capacity: Semaphore::new(limits.in_flight),
        })
    }
}
#[async_trait]
impl ModelBackend for ClaudeBackend {
    async fn infer(&self, request: &ModelRequest) -> Result<ModelResponse> {
        let deadline = tokio::time::Instant::now()
            .checked_add(self.limits.timeout)
            .ok_or_else(|| failure(ErrorCode::DeadlineExceeded))?;
        let body = wire::encode(request, self.limits.max_tokens, self.limits.request_bytes)?;
        tokio::time::timeout_at(deadline, async {
            let _permit = self
                .capacity
                .acquire()
                .await
                .map_err(|_| failure(ErrorCode::BackendFailed))?;
            if tokio::time::Instant::now() >= deadline {
                return Err(failure(ErrorCode::DeadlineExceeded));
            }
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
                return Err(failure(code).with_context("http_status", response.status.to_string()));
            }
            if response.body.len() > self.limits.response_bytes {
                return Err(failure(ErrorCode::LimitExceeded));
            }
            let response_text = std::str::from_utf8(&response.body)
                .map_err(|_| failure(ErrorCode::InvalidAnswer))?;
            let raw = RawExchange {
                request: String::from_utf8(body).map_err(|_| failure(ErrorCode::InvalidValue))?,
                response: response_text.to_owned(),
            };
            let decoded = wire::decode(request, &response.body, self.limits.response_bytes)
                .map_err(|error| error.with_raw(raw.clone()))?;
            sapho_core::validate_response(request, &decoded).map_err(|error| {
                SaphoError::new(error.code, "Claude answer refused").with_raw(raw.clone())
            })?;
            if tokio::time::Instant::now() >= deadline {
                return Err(failure(ErrorCode::DeadlineExceeded).with_raw(raw));
            }
            Ok(ModelResponse {
                raw: Some(raw),
                ..decoded
            })
        })
        .await
        .map_err(|_| failure(ErrorCode::DeadlineExceeded))?
    }
}
#[cfg(test)]
mod tests;
