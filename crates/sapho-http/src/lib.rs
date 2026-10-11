// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Shared bounded native HTTP and complete-call admission for model adapters.
use async_trait::async_trait;
use reqwest::{Client, Url, header::HeaderMap};
use sapho_core::{ErrorCode, Result, SaphoError};
use std::{future::Future, time::Duration};
use tokio::{sync::Semaphore, time::Instant};

/// A bounded response without private headers or endpoint data.
pub struct HttpResponse {
    /// Numeric HTTP status.
    pub status: u16,
    /// Successful response body; non-success bodies are discarded.
    pub body: Vec<u8>,
}

/// Injectable native boundary; implementations enforce `max_bytes` while collecting.
#[async_trait]
pub trait Transport: Send + Sync {
    /// Send one POST attempt without retry or redirect.
    async fn post(&self, body: &[u8], max_bytes: usize) -> Result<HttpResponse>;
}

/// Native transport shared by System One and Decisions adapters.
pub struct NativeTransport {
    client: Client,
    endpoint: Url,
    error_message: &'static str,
}
/// Redacted native client construction failure.
#[derive(Debug)]
pub struct NativeConfigError;
impl NativeTransport {
    /// Construct a no-retry, no-redirect client without sending a request.
    pub fn new(
        endpoint: Url,
        headers: HeaderMap,
        timeout: Duration,
        error_message: &'static str,
    ) -> std::result::Result<Self, NativeConfigError> {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .timeout(timeout)
            .default_headers(headers)
            .build()
            .map_err(|_| NativeConfigError)?;
        Ok(Self {
            client,
            endpoint,
            error_message,
        })
    }
    fn failure(&self, code: ErrorCode) -> SaphoError {
        SaphoError::new(code, self.error_message)
    }
    fn transport_error(&self, error: reqwest::Error) -> SaphoError {
        self.failure(if error.is_timeout() {
            ErrorCode::DeadlineExceeded
        } else {
            ErrorCode::BackendFailed
        })
    }
}
#[async_trait]
impl Transport for NativeTransport {
    async fn post(&self, body: &[u8], max_bytes: usize) -> Result<HttpResponse> {
        let mut response = self
            .client
            .post(self.endpoint.clone())
            .body(body.to_vec())
            .send()
            .await
            .map_err(|error| self.transport_error(error))?;
        let status = response.status().as_u16();
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
            return Err(self.failure(ErrorCode::LimitExceeded));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| self.transport_error(error))?
        {
            if bytes
                .len()
                .checked_add(chunk.len())
                .is_none_or(|size| size > max_bytes)
            {
                return Err(self.failure(ErrorCode::LimitExceeded));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(HttpResponse {
            status,
            body: bytes,
        })
    }
}

/// Per-backend admission that holds a permit across transport and decoding.
pub struct CallGate {
    permits: Semaphore,
}
impl CallGate {
    /// Construct a gate after the caller validates its concurrency ceiling.
    pub fn new(in_flight: usize) -> Self {
        Self {
            permits: Semaphore::new(in_flight),
        }
    }
    /// Run one complete call inside its deadline, including the permit wait.
    pub async fn run<T, F, Fut>(
        &self,
        deadline: Instant,
        error_message: &'static str,
        work: F,
    ) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        tokio::time::timeout_at(deadline, async {
            let _permit = self
                .permits
                .acquire()
                .await
                .map_err(|_| SaphoError::new(ErrorCode::BackendFailed, error_message))?;
            if Instant::now() >= deadline {
                return Err(SaphoError::new(ErrorCode::DeadlineExceeded, error_message));
            }
            let value = work().await?;
            if Instant::now() >= deadline {
                return Err(SaphoError::new(ErrorCode::DeadlineExceeded, error_message));
            }
            Ok(value)
        })
        .await
        .map_err(|_| SaphoError::new(ErrorCode::DeadlineExceeded, error_message))?
    }
}
