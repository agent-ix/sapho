// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Bounded provider-independent typed observations, separate from wire receipts.
use crate::{Error, ErrorCode, Result, runner::Capture};
use sapho_core::{ModelBackend, ModelRequest, ModelResponse, SaphoError, Usage};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

/// Observed delegate outcome; returning a response does not imply answer validity.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Delegate is still being awaited.
    InFlight,
    /// Delegate returned a typed response, before engine validation.
    Returned,
    /// Delegate returned a structured failure.
    Failed,
    /// Awaiting future was dropped; transport outcome is unknown.
    Cancelled,
    /// Delegate returned but response retention exceeded the explicit bound.
    RetentionFailed,
}
/// Source-free invocation observation. HTTP dispatch and raw wire are not inferred.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    /// Wire version.
    pub schema: u32,
    /// Serial receipt index within one backend session.
    pub ordinal: usize,
    /// Exact typed request serialized for retained input.
    pub typed_request_sha256: String,
    /// Exact typed response, absent on failure/cancellation.
    pub typed_response_sha256: Option<String>,
    /// Caller-selected model identity.
    pub model: String,
    /// Actual returned model identity when available within the 256-byte metadata bound.
    pub returned_model: Option<String>,
    /// Monotonic elapsed delegate time.
    pub elapsed_ms: u64,
    /// Observed lifecycle, separate from engine and transport outcomes.
    pub outcome: Outcome,
    /// Structured failure code; private error prose is excluded.
    pub error_code: Option<sapho_core::ErrorCode>,
    /// Provider-reported usage only; missing is unknown.
    pub usage: Option<Usage>,
}
struct Entry {
    observation: Observation,
    request: Vec<u8>,
    response: Option<Vec<u8>>,
}
#[derive(Default)]
struct State {
    entries: Vec<Entry>,
    bytes: usize,
}
/// Decorator shared by all provider bindings. It performs no storage or native I/O.
pub struct Meter {
    delegate: Arc<dyn ModelBackend>,
    state: Mutex<State>,
    max_bytes: usize,
    max_calls: usize,
}
fn failure(code: sapho_core::ErrorCode) -> SaphoError {
    SaphoError::new(code, "Backend observation refused")
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
impl Meter {
    /// Bound both retained typed bodies and observation count before calls.
    pub fn new(
        delegate: Arc<dyn ModelBackend>,
        max_bytes: usize,
        max_calls: usize,
    ) -> Result<Self> {
        if max_bytes == 0 || max_calls == 0 || max_calls > 100_000 {
            return Err(Error::new(
                ErrorCode::Invalid,
                "positive bounded observation limits required",
            ));
        }
        Ok(Self {
            delegate,
            state: Mutex::new(State::default()),
            max_bytes,
            max_calls,
        })
    }
    /// Clone source-free metadata outside async execution; no transport claims.
    pub fn observations(&self) -> Result<Vec<Observation>> {
        let state = self
            .state
            .lock()
            .map_err(|_| Error::new(ErrorCode::Storage, "observation lock poisoned"))?;
        Ok(state
            .entries
            .iter()
            .map(|e| e.observation.clone())
            .collect())
    }
    /// Capture complete bounded typed bodies and receipts, for private host storage.
    pub fn capture(&self) -> Result<Vec<Capture>> {
        let state = self
            .state
            .lock()
            .map_err(|_| Error::new(ErrorCode::Storage, "observation lock poisoned"))?;
        let mut captures = Vec::new();
        for entry in &state.entries {
            captures.push(Capture {
                kind: "typed_model_request".into(),
                bytes: entry.request.clone(),
            });
            if let Some(response) = &entry.response {
                captures.push(Capture {
                    kind: "typed_model_response".into(),
                    bytes: response.clone(),
                });
            }
            captures.push(Capture {
                kind: "backend_observation".into(),
                bytes: sapho_core::bounded_json(&entry.observation, 4096).map_err(|_| {
                    Error::new(ErrorCode::Refused, "observation metadata too large")
                })?,
            });
        }
        Ok(captures)
    }
}
struct Pending<'a> {
    meter: &'a Meter,
    ordinal: usize,
    started: Instant,
}
impl Pending<'_> {
    fn finish(&self, response: &sapho_core::Result<ModelResponse>) -> sapho_core::Result<()> {
        let encoded = response
            .as_ref()
            .ok()
            .map(|r| sapho_core::bounded_json(r, self.meter.max_bytes))
            .transpose();
        let mut state = self
            .meter
            .state
            .lock()
            .map_err(|_| failure(sapho_core::ErrorCode::BackendFailed))?;
        if let Ok(response) = response
            && let Some(entry) = state.entries.get_mut(self.ordinal)
        {
            entry.observation.usage = response.usage.clone();
            entry.observation.returned_model =
                (response.model.len() <= 256).then(|| response.model.clone());
        }
        let retained = match encoded {
            Ok(Some(bytes))
                if state
                    .bytes
                    .checked_add(bytes.len())
                    .is_some_and(|n| n <= self.meter.max_bytes) =>
            {
                state.bytes += bytes.len();
                Some(bytes)
            }
            Ok(None) => None,
            _ => {
                if let Some(entry) = state.entries.get_mut(self.ordinal) {
                    entry.observation.outcome = Outcome::RetentionFailed;
                    entry.observation.error_code = Some(sapho_core::ErrorCode::LimitExceeded);
                }
                return Err(failure(sapho_core::ErrorCode::LimitExceeded));
            }
        };
        let entry = state
            .entries
            .get_mut(self.ordinal)
            .ok_or_else(|| failure(sapho_core::ErrorCode::InvalidValue))?;
        match response {
            Ok(response) => {
                entry.observation.outcome = Outcome::Returned;
                entry.observation.usage = response.usage.clone();
                entry.observation.returned_model =
                    (response.model.len() <= 256).then(|| response.model.clone());
            }
            Err(error) => {
                entry.observation.outcome = Outcome::Failed;
                entry.observation.error_code = Some(error.code);
            }
        }
        entry.observation.typed_response_sha256 = retained.as_ref().map(|bytes| hash(bytes));
        entry.response = retained;
        Ok(())
    }
}
impl Drop for Pending<'_> {
    fn drop(&mut self) {
        if let Ok(mut state) = self.meter.state.lock()
            && let Some(entry) = state.entries.get_mut(self.ordinal)
        {
            entry.observation.elapsed_ms =
                u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
            if entry.observation.outcome == Outcome::InFlight {
                entry.observation.outcome = Outcome::Cancelled;
            }
        }
    }
}
#[async_trait::async_trait]
impl ModelBackend for Meter {
    async fn infer(&self, request: &ModelRequest) -> sapho_core::Result<ModelResponse> {
        request.validate()?;
        if request.model.len() > 256 {
            return Err(failure(sapho_core::ErrorCode::LimitExceeded));
        }
        let bytes = sapho_core::bounded_json(request, self.max_bytes)?;
        let ordinal = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| failure(sapho_core::ErrorCode::BackendFailed))?;
            let total = state
                .bytes
                .checked_add(bytes.len())
                .and_then(|n| n.checked_add(4096))
                .filter(|n| *n <= self.max_bytes)
                .ok_or_else(|| failure(sapho_core::ErrorCode::LimitExceeded))?;
            if state.entries.len() >= self.max_calls {
                return Err(failure(sapho_core::ErrorCode::LimitExceeded));
            }
            let ordinal = state.entries.len();
            state.bytes = total;
            state.entries.push(Entry {
                observation: Observation {
                    schema: 1,
                    ordinal,
                    typed_request_sha256: hash(&bytes),
                    typed_response_sha256: None,
                    model: request.model.clone(),
                    returned_model: None,
                    elapsed_ms: 0,
                    outcome: Outcome::InFlight,
                    error_code: None,
                    usage: None,
                },
                request: bytes,
                response: None,
            });
            ordinal
        };
        let pending = Pending {
            meter: self,
            ordinal,
            started: Instant::now(),
        };
        let response = self.delegate.infer(request).await;
        pending.finish(&response)?;
        drop(pending);
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sapho_core::{BackendId, DistributionPolicy, Value};
    use std::{
        collections::BTreeMap,
        future::Future,
        sync::atomic::{AtomicUsize, Ordering},
        task::{Context, Poll, Waker},
    };
    struct Delegate {
        calls: Arc<AtomicUsize>,
        mode: Outcome,
    }
    #[async_trait::async_trait]
    impl ModelBackend for Delegate {
        async fn infer(&self, request: &ModelRequest) -> sapho_core::Result<ModelResponse> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            match self.mode {
                Outcome::Failed => Err(SaphoError::new(
                    sapho_core::ErrorCode::Unauthorized,
                    "PRIVATE ERROR",
                )),
                Outcome::Cancelled => std::future::pending().await,
                _ => Ok(ModelResponse {
                    model: request.model.clone(),
                    answers: BTreeMap::new(),
                    usage: Some(Usage {
                        input_tokens: 17,
                        output_tokens: 3,
                        billing_units: None,
                    }),
                }),
            }
        }
    }
    fn request() -> ModelRequest {
        ModelRequest {
            backend: BackendId::new("judge").unwrap(),
            distribution_policy: DistributionPolicy::Strict {},
            model: "synthetic".into(),
            expected_model: None,
            state: Value::Record(BTreeMap::from([(
                "owner".into(),
                Value::Text("Original café owner".into()),
            )])),
            questions: vec![],
        }
    }
    /// Trace: FR-050-AC-5
    #[test]
    fn delegate_returns_and_failures_keep_typed_bodies_usage_and_safe_observations() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        for mode in [Outcome::Returned, Outcome::Failed] {
            let calls = Arc::new(AtomicUsize::new(0));
            let meter = Meter::new(
                Arc::new(Delegate {
                    calls: calls.clone(),
                    mode,
                }),
                65536,
                1,
            )
            .unwrap();
            let response = runtime.block_on(meter.infer(&request()));
            let observations = meter.observations().unwrap();
            assert_eq!(observations.len(), 1);
            assert_eq!(observations[0].outcome, mode);
            let captures = meter.capture().unwrap();
            let req = captures
                .iter()
                .find(|c| c.kind == "typed_model_request")
                .unwrap();
            assert_eq!(hash(&req.bytes), observations[0].typed_request_sha256);
            assert_eq!(
                serde_json::from_slice::<ModelRequest>(&req.bytes).unwrap(),
                request()
            );
            let serialized = serde_json::to_string(&observations).unwrap();
            assert!(!serialized.contains("PRIVATE ERROR"));
            assert!(!serialized.contains("Original café"));
            if mode == Outcome::Returned {
                let body = captures
                    .iter()
                    .find(|c| c.kind == "typed_model_response")
                    .unwrap();
                assert_eq!(
                    serde_json::from_slice::<ModelResponse>(&body.bytes).unwrap(),
                    response.unwrap()
                );
                assert_eq!(observations[0].usage.as_ref().unwrap().input_tokens, 17);
            } else {
                assert_eq!(
                    response.unwrap_err().code,
                    sapho_core::ErrorCode::Unauthorized
                );
                assert!(observations[0].usage.is_none());
            }
            assert_eq!(
                runtime.block_on(meter.infer(&request())).unwrap_err().code,
                sapho_core::ErrorCode::LimitExceeded
            );
            assert_eq!(calls.load(Ordering::SeqCst), 1);
        }
    }
    /// Trace: FR-050-AC-5
    #[test]
    fn response_retention_failure_keeps_usage_and_request_without_retry_or_success() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let input = request();
        // Exactly enough for immutable request plus reserved metadata; the
        // delegate may return, but no room remains for its response body.
        let cap = sapho_core::bounded_json(&input, 65536).unwrap().len() + 4096;
        let meter = Meter::new(
            Arc::new(Delegate {
                calls: calls.clone(),
                mode: Outcome::Returned,
            }),
            cap,
            1,
        )
        .unwrap();
        let error = runtime.block_on(meter.infer(&input)).unwrap_err();
        assert_eq!(error.code, sapho_core::ErrorCode::LimitExceeded);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let observed = meter.observations().unwrap();
        assert_eq!(observed.len(), 1);
        assert_eq!(observed[0].outcome, Outcome::RetentionFailed);
        assert_eq!(
            observed[0].error_code,
            Some(sapho_core::ErrorCode::LimitExceeded)
        );
        assert_eq!(observed[0].returned_model.as_deref(), Some("synthetic"));
        assert_eq!(observed[0].usage.as_ref().unwrap().input_tokens, 17);
        assert_eq!(observed[0].usage.as_ref().unwrap().output_tokens, 3);
        assert!(observed[0].typed_response_sha256.is_none());
        let captured = meter.capture().unwrap();
        assert_eq!(captured.len(), 2);
        assert_eq!(
            serde_json::from_slice::<ModelRequest>(
                &captured
                    .iter()
                    .find(|c| c.kind == "typed_model_request")
                    .unwrap()
                    .bytes
            )
            .unwrap(),
            input
        );
        let summary = captured
            .iter()
            .find(|c| c.kind == "backend_observation")
            .unwrap();
        assert!(!String::from_utf8_lossy(&summary.bytes).contains("Original café owner"));
        assert_eq!(
            runtime.block_on(meter.infer(&input)).unwrap_err().code,
            sapho_core::ErrorCode::LimitExceeded
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let refused = Meter::new(
            Arc::new(Delegate {
                calls: calls.clone(),
                mode: Outcome::Returned,
            }),
            cap - 1,
            1,
        )
        .unwrap();
        assert_eq!(
            runtime.block_on(refused.infer(&input)).unwrap_err().code,
            sapho_core::ErrorCode::LimitExceeded
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(refused.observations().unwrap().is_empty());
        assert!(refused.capture().unwrap().is_empty());
    }

    /// Trace: FR-050-AC-5
    #[test]
    fn dropped_future_is_cancelled_without_a_dispatch_claim_or_retry() {
        let calls = Arc::new(AtomicUsize::new(0));
        let meter = Meter::new(
            Arc::new(Delegate {
                calls: calls.clone(),
                mode: Outcome::Cancelled,
            }),
            65536,
            1,
        )
        .unwrap();
        let request = request();
        let mut future = Box::pin(meter.infer(&request));
        assert!(matches!(
            future
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop())),
            Poll::Pending
        ));
        drop(future);
        let observations = meter.observations().unwrap();
        assert_eq!(observations[0].outcome, Outcome::Cancelled);
        assert!(observations[0].typed_response_sha256.is_none());
        assert!(observations[0].usage.is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(meter.capture().unwrap().len(), 2);
    }
}
