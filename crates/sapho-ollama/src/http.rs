// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Bounded, serialized HTTP to one Ollama server (FR-051).
use reqwest::{
    Client, Url,
    header::{HeaderMap, HeaderName, HeaderValue},
    redirect::Policy,
};
use sapho_core::{ErrorCode, ExtractError, ExtractUsage};
use std::{
    future::Future,
    time::{Duration, Instant},
};
use tokio::sync::Semaphore;

/// Host-local default endpoint; Sapho never starts a server.
pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:11434";

/// At most one Ollama request in flight in this process, across every server handle.
static IN_FLIGHT: Semaphore = Semaphore::const_new(1);

/// Time and size bounds for each call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Whole call, including waiting for the process-wide permit.
    pub timeout: Duration,
    /// Largest serialized request body.
    pub request_bytes: usize,
    /// Largest response body read.
    pub response_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(600),
            request_bytes: 4 * 1024 * 1024,
            response_bytes: 16 * 1024 * 1024,
        }
    }
}

/// One Ollama endpoint with its bounds; clones share the connection pool.
#[derive(Debug, Clone)]
pub struct Server {
    client: Client,
    base: Url,
    limits: Limits,
    headers: HeaderMap,
}
/// A complete response within the ceiling; the body is decoded only where it is needed.
pub(crate) struct Reply {
    pub status: u16,
    body: Vec<u8>,
    elapsed_ms: Option<u64>,
}

pub(crate) fn failure(
    code: ErrorCode,
    reason: &'static str,
    message: &'static str,
) -> ExtractError {
    ExtractError::new(code, message).with_reason(reason)
}

impl Server {
    /// Check the URL and limits and prepare the client; nothing is sent.
    ///
    /// The URL must be `http` or `https` without userinfo, query or fragment, and every
    /// limit must be nonzero. Redirects are never followed, and system proxies are not used.
    pub fn new(base_url: &str, limits: Limits) -> Result<Self, ExtractError> {
        let refused = |reason| failure(ErrorCode::Config, reason, "Invalid Ollama endpoint");
        let mut base = Url::parse(base_url).map_err(|_| refused("invalid_base_url"))?;
        if !matches!(base.scheme(), "http" | "https")
            || base.host_str().is_none()
            || !base.username().is_empty()
            || base.password().is_some()
            || base.query().is_some()
            || base.fragment().is_some()
        {
            return Err(refused("invalid_base_url"));
        }
        if limits.timeout.is_zero() || limits.request_bytes == 0 || limits.response_bytes == 0 {
            return Err(refused("zero_limit"));
        }
        let path = format!("{}/", base.path().trim_end_matches('/'));
        base.set_path(&path);
        let client = Client::builder()
            .redirect(Policy::none())
            .no_proxy()
            .build()
            .map_err(|_| refused("client_unavailable"))?;
        Ok(Self {
            client,
            base,
            limits,
            headers: HeaderMap::new(),
        })
    }
    /// Send `name: value` with every request, for a server behind an authenticating gateway.
    ///
    /// The value is marked sensitive, and headers never enter a raw exchange.
    pub fn with_header(mut self, name: &str, value: &str) -> Result<Self, ExtractError> {
        let refused = || {
            failure(
                ErrorCode::Config,
                "invalid_header",
                "Invalid request header",
            )
        };
        let name = HeaderName::from_bytes(name.as_bytes()).map_err(|_| refused())?;
        let mut value = HeaderValue::from_str(value).map_err(|_| refused())?;
        value.set_sensitive(true);
        self.headers.insert(name, value);
        Ok(self)
    }
    /// The ceiling on a serialized request, checked before anything is sent.
    pub(crate) fn check_request(&self, body: &str) -> Result<(), ExtractError> {
        if body.len() > self.limits.request_bytes {
            return Err(failure(
                ErrorCode::LimitExceeded,
                "request_too_large",
                "Request exceeds the byte ceiling",
            ));
        }
        Ok(())
    }
    /// Run `work` holding the process-wide permit, within the call timeout.
    ///
    /// Dropping the returned future releases the permit.
    pub(crate) async fn exclusive<T>(
        &self,
        work: impl Future<Output = Result<T, ExtractError>>,
    ) -> Result<T, ExtractError> {
        let held = async {
            let _permit = IN_FLIGHT.acquire().await.map_err(|_| {
                failure(
                    ErrorCode::BackendFailed,
                    "permit_closed",
                    "Request permit unavailable",
                )
            })?;
            work.await
        };
        tokio::time::timeout(self.limits.timeout, held)
            .await
            .map_err(|_| {
                failure(
                    ErrorCode::DeadlineExceeded,
                    "timeout",
                    "Ollama call timed out",
                )
            })?
    }
    /// One attempt: POST `body` to `path` and read at most the response ceiling.
    pub(crate) async fn post(&self, path: &str, body: &str) -> Result<Reply, ExtractError> {
        let unreachable = || {
            failure(
                ErrorCode::BackendFailed,
                "connection_failed",
                "Ollama request failed",
            )
        };
        let started = Instant::now();
        let url = self.base.join(path).map_err(|_| unreachable())?;
        let mut response = self
            .client
            .post(url)
            .headers(self.headers.clone())
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body.to_owned())
            .send()
            .await
            .map_err(|_| unreachable())?;
        let status = response.status().as_u16();
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| unreachable())? {
            if bytes.len().saturating_add(chunk.len()) > self.limits.response_bytes {
                return Err(failure(
                    ErrorCode::LimitExceeded,
                    "response_too_large",
                    "Response exceeds the byte ceiling",
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(Reply {
            status,
            body: bytes,
            elapsed_ms: u64::try_from(started.elapsed().as_millis()).ok(),
        })
    }
}
impl Reply {
    pub fn success(&self) -> bool {
        (200..300).contains(&self.status)
    }
    /// Only the elapsed time is known for a reply that carries no readable exchange.
    fn elapsed_only(&self) -> ExtractUsage {
        ExtractUsage {
            elapsed_ms: self.elapsed_ms,
            ..ExtractUsage::default()
        }
    }
    /// The body as text, when it is valid UTF-8.
    pub fn text(&self) -> Option<&str> {
        std::str::from_utf8(&self.body).ok()
    }
    /// A non-success status is a failure whatever its body says: 404 is
    /// `model_not_found`, anything else `http_status`. No exchange is retained.
    pub fn status_error(&self) -> ExtractError {
        if self.status == 404 {
            failure(
                ErrorCode::BackendFailed,
                "model_not_found",
                "The server does not know the model",
            )
        } else {
            failure(
                ErrorCode::BackendFailed,
                "http_status",
                "Ollama answered with an error status",
            )
        }
        .with_usage(self.elapsed_only())
    }
    /// The body of a success reply. A body that is not text cannot be held in a raw
    /// exchange, so only the elapsed time is reported.
    pub fn success_text(&self) -> Result<&str, ExtractError> {
        self.text().ok_or_else(|| {
            failure(
                ErrorCode::InvalidAnswer,
                "malformed_response",
                "The response body is not valid UTF-8",
            )
            .with_usage(self.elapsed_only())
        })
    }
    /// A refusal the server's body names, as a failure with no retained exchange.
    pub fn refusal(&self, error: ExtractError) -> ExtractError {
        error.with_usage(self.elapsed_only())
    }
}
