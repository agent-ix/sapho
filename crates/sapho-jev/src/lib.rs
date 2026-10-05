// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Hosted System One adapter over the authoritative SDK (FR-025/026).
//!
//! # Supply an SDK client
//!
//! ```no_run
//! use std::sync::Arc;
//! use typesafe_sdk_client::Client;
//! use typesafe_sdk_config::Builder;
//! use typesafe_sdk_env::Process;
//! use typesafe_sdk_http::Reqwest;
//! let config = Builder::new().build(&Process)?;
//! let client = Client::with_transport(config, Arc::new(Reqwest::new()?));
//! let _backend = sapho_jev::JevBackend::new(client);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Resolve credentials and endpoint configuration in the synchronous host.
//! Register this backend with a core BackendBinding that chooses the model and
//! distribution policy. Inference disables SDK retries and returns raw typed
//! responses for core validation; provider error bodies are omitted from diagnostics.
use async_trait::async_trait;
use sapho_core::{
    Answer, ErrorCode, ModelBackend, ModelRequest, ModelResponse, Probability, Result, SaphoError,
    Usage,
};
use std::collections::BTreeMap;
use typesafe_sdk_client::{Client, RequestOptions};
use typesafe_sdk_retry::RetryPolicy;

/// ModelBackend implementation using a host-configured TypeSafe SDK client.
/// Auth, endpoint and transport belong to the supplied client, never graph config.
pub struct JevBackend {
    client: Client,
}
impl JevBackend {
    /// Accept an SDK client; each inference explicitly disables SDK retries.
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}
pub use sapho_systemone::build_request;
#[async_trait]
impl ModelBackend for JevBackend {
    async fn infer(&self, request: &ModelRequest) -> Result<ModelResponse> {
        let sdk = build_request(request)?;
        let retry = RetryPolicy {
            max_retries: 0,
            ..RetryPolicy::default()
        };
        let raw = self
            .client
            .system_one_with(sdk, &RequestOptions::new().retry(retry))
            .await
            .map_err(classify)?
            .data;
        let mut answers = BTreeMap::new();
        for (id, answer) in raw.answers {
            let answer = match answer {
                typesafe_sdk_answers::Answer::Noul(a) => Answer::Boolean {
                    probability: probability(a.noul)?,
                },
                typesafe_sdk_answers::Answer::Choice(a) => Answer::Choice {
                    selected: a.choice,
                    confidence: probability(a.confidence)?,
                    probabilities: Some(
                        a.probabilities
                            .into_iter()
                            .map(|(k, p)| Ok((k, probability(p)?)))
                            .collect::<Result<_>>()?,
                    ),
                },
                typesafe_sdk_answers::Answer::Score(a) => Answer::Score {
                    expected: a.score,
                    confidence: probability(a.confidence)?,
                    probabilities: Some(
                        a.probabilities
                            .into_iter()
                            .map(|(k, p)| Ok((k, probability(p)?)))
                            .collect::<Result<_>>()?,
                    ),
                },
            };
            answers.insert(id, answer);
        }
        Ok(ModelResponse {
            model: raw.model,
            digest: None,
            answers,
            usage: Some(Usage {
                billing_units: None,
                input_tokens: raw.usage.input_tokens,
                output_tokens: raw.usage.output_tokens,
            }),
        })
    }
}
fn probability(value: f64) -> Result<Probability> {
    Probability::new(value).map_err(|_| {
        SaphoError::new(
            ErrorCode::InvalidAnswer,
            "Provider returned non-unit probability",
        )
    })
}
fn classify(error: typesafe_sdk_error::Error) -> SaphoError {
    let code = match &error {
        typesafe_sdk_error::Error::Api(a) => match a.status {
            401 | 403 => ErrorCode::Unauthorized,
            400 | 422 => ErrorCode::ServiceValidation,
            429 => ErrorCode::RateLimited,
            _ => ErrorCode::BackendFailed,
        },
        typesafe_sdk_error::Error::Connection { .. } => ErrorCode::BackendFailed,
        typesafe_sdk_error::Error::Timeout { .. } | typesafe_sdk_error::Error::UserAbort { .. } => {
            ErrorCode::DeadlineExceeded
        }
        typesafe_sdk_error::Error::Invalid(_) => ErrorCode::ServiceValidation,
    };
    // Keep provider body and transport diagnostics out of retained errors: they can echo auth.
    let mut e = SaphoError::new(code, "System One request failed");
    if let Some(status) = error.status() {
        e = e.with_context("http_status", status.to_string());
    }
    e
}

#[cfg(test)]
mod tests;
