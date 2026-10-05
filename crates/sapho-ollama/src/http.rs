// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Bounded, serialized HTTP to one Ollama server (FR-051).
use reqwest::{Client, Url, redirect::Policy};
use sapho_core::{ErrorCode, ExtractError, RawExchange};
use std::{future::Future, time::Duration};
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
}
/// A complete response: status and body bytes within the ceiling.
pub(crate) struct Reply {
    pub status: u16,
    pub body: Vec<u8>,
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
        })
    }
    /// The ceiling on a serialized request, checked before anything is sent.
    pub(crate) fn check_request(&self, body: &[u8]) -> Result<(), ExtractError> {
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
    pub(crate) async fn post(&self, path: &str, body: &[u8]) -> Result<Reply, ExtractError> {
        let unreachable = || {
            failure(
                ErrorCode::BackendFailed,
                "connection_failed",
                "Ollama request failed",
            )
        };
        let url = self.base.join(path).map_err(|_| unreachable())?;
        let mut response = self
            .client
            .post(url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body.to_vec())
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
        })
    }
}
impl Reply {
    /// The exchange as it travelled, for retention inside an error or response.
    pub fn raw(&self, request: &[u8]) -> RawExchange {
        RawExchange {
            request: request.to_vec(),
            response: self.body.clone(),
        }
    }
}
