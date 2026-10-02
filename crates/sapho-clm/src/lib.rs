// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Host-configured CLM System One adapter; wire and HTTP belong here, policy to core/runtime.
mod wire;
use async_trait::async_trait;
use ix_cli_kit::secrets::SecretValue;
use reqwest::{
    Client, Url,
    header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue},
};
use sapho_core::{ErrorCode, ModelBackend, ModelRequest, ModelResponse, Result, SaphoError};
use std::{net::IpAddr, sync::Arc, time::Duration};
use tokio::sync::Semaphore;

/// Reference CLM service model alias; an embedding host may choose another model.
pub use sapho_systemone::CLM_DEFAULT_MODEL as DEFAULT_MODEL;
/// Host-local reference service URL; no service is installed or started by Sapho.
pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8700";

/// Finite request limits, including time spent waiting for concurrency capacity.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Entire inference duration, including queue and body decode.
    pub timeout: Duration,
    /// Maximum serialized request bytes.
    pub request_bytes: usize,
    /// Maximum response bytes, independent of advertised size.
    pub response_bytes: usize,
    /// Maximum simultaneous HTTP calls for this backend.
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
        if self.timeout.is_zero()
            || std::time::Instant::now()
                .checked_add(self.timeout)
                .is_none()
            || self.request_bytes == 0
            || self.response_bytes == 0
            || self.in_flight == 0
            || self.in_flight > Semaphore::MAX_PERMITS
        {
            return Err(ConfigurationError::InvalidLimits);
        }
        Ok(())
    }
}
/// Redacted preparation failures; never retain supplied URLs, secrets or native error text.
#[derive(Debug, thiserror::Error)]
pub enum ConfigurationError {
    /// URL must be HTTP(S), with cleartext restricted to loopback, and no embedded secrets.
    #[error("Invalid CLM endpoint")]
    InvalidEndpoint,
    /// A finite positive limit is required.
    #[error("Invalid CLM limits")]
    InvalidLimits,
    /// Credential cannot form a safe authorization header.
    #[error("Invalid CLM credential")]
    InvalidCredential,
    /// Native HTTP client initialization was refused.
    #[error("Unable to prepare CLM transport")]
    Transport,
}
fn endpoint(base_url: &str) -> std::result::Result<Url, ConfigurationError> {
    let mut url = Url::parse(base_url).map_err(|_| ConfigurationError::InvalidEndpoint)?;
    let loopback = url.host_str().is_some_and(|host| {
        host.eq_ignore_ascii_case("localhost")
            || host
                .trim_matches(['[', ']'])
                .parse::<IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    });
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || (!loopback && url.scheme() == "http")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(ConfigurationError::InvalidEndpoint);
    }
    let path = format!("{}/v1/systemone", url.path().trim_end_matches('/'));
    url.set_path(&path);
    Ok(url)
}
/// Bounded HTTP result at the native transport seam; headers and secrets are not evidence.
pub struct HttpResponse {
    /// HTTP status used for typed error classification.
    pub status: u16,
    /// Complete response bytes within the caller's ceiling.
    pub body: Vec<u8>,
}
/// Native HTTP boundary for deterministic host tests and alternative bounded transports.
/// Implementations must enforce `max_bytes` while collecting response data and avoid
/// returning private input or credential text in errors. Backend also checks final length.
#[async_trait]
pub trait Transport: Send + Sync {
    /// Send one attempt; redirects and retries are prohibited.
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
        // Error bodies can echo credentials or inputs; never read or retain them.
        if status != 200 {
            return Ok(HttpResponse {
                status,
                body: Vec::new(),
            });
        }
        if response
            .content_length()
            .is_some_and(|length| length > u64::try_from(max_bytes).unwrap_or(u64::MAX))
        {
            return Err(failure(ErrorCode::LimitExceeded));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(transport_error)? {
            if bytes
                .len()
                .checked_add(chunk.len())
                .is_none_or(|size| size > max_bytes)
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
    SaphoError::new(code, "CLM request refused")
}
/// Asynchronous backend using one configured endpoint and finite per-backend concurrency.
/// Native credential acquisition must be completed by the synchronous host before construction.
pub struct ClmBackend {
    transport: Arc<dyn Transport>,
    limits: Limits,
    capacity: Semaphore,
}
impl ClmBackend {
    /// Construct the production HTTP adapter. No credential-store or model operations occur.
    pub fn new(
        base_url: &str,
        credential: Option<&SecretValue>,
        limits: Limits,
    ) -> std::result::Result<Self, ConfigurationError> {
        limits.validate()?;
        let endpoint = endpoint(base_url)?;
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if let Some(secret) = credential {
            if secret.expose_secret().trim().is_empty() {
                return Err(ConfigurationError::InvalidCredential);
            }
            let mut value = HeaderValue::from_str(&format!("Bearer {}", secret.expose_secret()))
                .map_err(|_| ConfigurationError::InvalidCredential)?;
            value.set_sensitive(true);
            headers.insert(AUTHORIZATION, value);
        }
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .default_headers(headers)
            .timeout(limits.timeout)
            .build()
            .map_err(|_| ConfigurationError::Transport)?;
        Self::with_transport(Arc::new(HttpTransport { client, endpoint }), limits)
    }
    /// Inject only the native HTTP seam; serialization/decoding/limits remain real.
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
impl ModelBackend for ClmBackend {
    async fn infer(&self, request: &ModelRequest) -> Result<ModelResponse> {
        let deadline = tokio::time::Instant::now()
            .checked_add(self.limits.timeout)
            .ok_or_else(|| failure(ErrorCode::DeadlineExceeded))?;
        let body = wire::encode(request, self.limits.request_bytes)?;
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
            let decoded = wire::decode(request, &response.body, self.limits.response_bytes)?;
            if tokio::time::Instant::now() >= deadline {
                return Err(failure(ErrorCode::DeadlineExceeded));
            }
            Ok(decoded)
        })
        .await
        .map_err(|_| failure(ErrorCode::DeadlineExceeded))?
    }
}
#[cfg(test)]
mod tests;
