// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Public opt-in model-call sidecar acceptance fixtures.
use async_trait::async_trait;
use sapho_core::{
    Answer, BackendBinding, BackendId, BackendRegistry, Datum, DistributionPolicy, ErrorCode,
    Inputs, ModelBackend, ModelRequest, ModelResponse, NodeId, PrimitiveRegistry, Probability,
    ProviderDescriptor, Result, SaphoError, Usage, Value, ValueType,
};
use sapho_graph::{Binding, GraphSpec, compile};
use sapho_runtime::{Engine, ObservationClock, ObservationConfig, ObservationMode, RunLimits};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

struct Ticks {
    base: Instant,
    next: AtomicUsize,
}
impl ObservationClock for Ticks {
    fn now(&self) -> Instant {
        self.base + Duration::from_micros((self.next.fetch_add(1, Ordering::SeqCst) * 10) as u64)
    }
}
struct Scripted {
    fail: bool,
    calls: AtomicUsize,
}
#[async_trait]
impl ModelBackend for Scripted {
    async fn infer(&self, request: &ModelRequest) -> Result<ModelResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            return Err(SaphoError::new(ErrorCode::BackendFailed, "scripted"));
        }
        Ok(ModelResponse {
            model: "actual-a".into(),
            raw: None,
            answers: request
                .questions
                .iter()
                .map(|question| {
                    (
                        question.id.clone(),
                        Answer::Boolean {
                            probability: Probability::new(0.8).unwrap(),
                        },
                    )
                })
                .collect(),
            usage: Some(Usage {
                billing_units: None,
                input_tokens: 11,
                output_tokens: 3,
            }),
        })
    }
}
fn graph() -> GraphSpec {
    GraphSpec::parse(
        r#"
inputs: {text: {kind: text}}
nodes:
  - id: state
    operation: {kind: record}
    inputs: {text: {kind: input, name: text}}
  - id: questions
    operation:
      kind: questions
      questions:
        - id: q
          question: {kind: boolean, instructions: "Does it hold?", yes: Holds, no: Absent}
  - id: ask
    operation: {kind: ask, backend: judge}
    inputs:
      state: {kind: node, node: state, port: result}
      questions: {kind: node, node: questions, port: result}
outputs: {answer: {kind: node, node: ask, port: answers}}
"#,
    )
    .unwrap()
}
fn engine(spec: &GraphSpec, backend: Arc<Scripted>) -> Engine {
    let mut registry = BackendRegistry::default();
    registry
        .register(
            BackendId::new("judge").unwrap(),
            BackendBinding {
                backend,
                model: "requested".into(),
                expected_model: None,
                distribution_policy: DistributionPolicy::Strict {},
            },
        )
        .unwrap();
    Engine::new(
        compile(spec, &PrimitiveRegistry::default()).unwrap(),
        registry,
    )
    .unwrap()
}
fn input() -> Inputs {
    Inputs::from([(
        "text".into(),
        Datum::new("case", Value::Text("yes".into())).unwrap(),
    )])
}
fn config(mode: ObservationMode) -> ObservationConfig {
    ObservationConfig {
        mode,
        descriptors: BTreeMap::from([(
            BackendId::new("judge").unwrap(),
            ProviderDescriptor {
                provider: Some("custom".into()),
                adapter: Some("fixture".into()),
            },
        )]),
        clock: Arc::new(Ticks {
            base: Instant::now(),
            next: AtomicUsize::new(0),
        }),
    }
}
/// Trace: FR-061-AC-1, FR-061-AC-2, FR-061-AC-3, FR-061-AC-4, FR-061-AC-5, IT-009-SC-01, IT-009-SC-03
#[tokio::test]
async fn observed_calls_keep_timing_and_descriptors_outside_trace() {
    let spec = graph();
    let backend = Arc::new(Scripted {
        fail: false,
        calls: AtomicUsize::new(0),
    });
    let runner = engine(&spec, backend.clone());
    let (result, calls) = runner
        .run_observed(
            &input(),
            RunLimits::default(),
            &config(ObservationMode::Live),
        )
        .await
        .unwrap();
    let result = result.unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].path, vec!["root", "ask"]);
    assert_eq!(calls[0].binding.as_str(), "judge");
    assert_eq!(calls[0].actual_model.as_deref(), Some("actual-a"));
    assert_eq!(calls[0].usage.as_ref().unwrap().input_tokens, 11);
    assert_eq!(calls[0].elapsed_micros, Some(10));
    assert_eq!(
        calls[0].descriptor.as_ref().unwrap().provider.as_deref(),
        Some("custom")
    );
    let ordinary = runner.run(&input(), RunLimits::default()).await.unwrap();
    assert_eq!(result.trace, ordinary.trace);
    assert!(
        !serde_json::to_string(&result.trace)
            .unwrap()
            .contains("fixture")
    );
    let (_, replay) = runner
        .run_observed(
            &input(),
            RunLimits::default(),
            &config(ObservationMode::Replay),
        )
        .await
        .unwrap();
    assert_eq!(replay[0].elapsed_micros, None);
    let mut skipped = graph();
    skipped
        .nodes
        .iter_mut()
        .find(|node| node.id.as_str() == "ask")
        .unwrap()
        .guard = Some(Binding::Literal {
        value: Datum::new("guard", Value::Boolean(false)).unwrap(),
        value_type: ValueType::Boolean,
    });
    let before = backend.calls.load(Ordering::SeqCst);
    let (_, none) = engine(&skipped, backend.clone())
        .run_observed(
            &input(),
            RunLimits::default(),
            &config(ObservationMode::Live),
        )
        .await
        .unwrap();
    assert!(none.is_empty());
    assert_eq!(backend.calls.load(Ordering::SeqCst), before);
    let failed = engine(
        &spec,
        Arc::new(Scripted {
            fail: true,
            calls: AtomicUsize::new(0),
        }),
    );
    let (_, calls) = failed
        .run_observed(
            &input(),
            RunLimits::default(),
            &config(ObservationMode::Live),
        )
        .await
        .unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].actual_model, None);
    assert_eq!(calls[0].usage, None);
    assert_eq!(calls[0].elapsed_micros, Some(10));
    let invalid: serde_json::Value =
        serde_json::json!({"provider":"safe","endpoint":"https://secret"});
    assert!(serde_json::from_value::<ProviderDescriptor>(invalid).is_err());
    let mut malformed = config(ObservationMode::Live);
    malformed
        .descriptors
        .get_mut(&BackendId::new("judge").unwrap())
        .unwrap()
        .adapter = Some("https://secret".into());
    let before = backend.calls.load(Ordering::SeqCst);
    assert!(
        runner
            .run_observed(&input(), RunLimits::default(), &malformed)
            .await
            .is_err()
    );
    assert_eq!(backend.calls.load(Ordering::SeqCst), before);
}

struct ParallelFailure {
    slow: bool,
    starts: Arc<AtomicUsize>,
}
#[async_trait]
impl ModelBackend for ParallelFailure {
    async fn infer(&self, _request: &ModelRequest) -> Result<ModelResponse> {
        self.starts.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(if self.slow { 200 } else { 20 })).await;
        Err(SaphoError::new(
            ErrorCode::BackendFailed,
            "scripted parallel failure",
        ))
    }
}

/// Trace: FR-061-AC-2, FR-062-AC-1, IT-009-SC-01
#[tokio::test]
async fn parallel_failure_observes_started_sibling_with_live_duration() {
    let mut spec = graph();
    let mut sibling = spec
        .nodes
        .iter()
        .find(|node| node.id.as_str() == "ask")
        .unwrap()
        .clone();
    sibling.id = NodeId::new("other").unwrap();
    sibling.operation = sapho_graph::Operation::Ask {
        backend: BackendId::new("slow").unwrap(),
    };
    spec.nodes.push(sibling);
    spec.outputs.insert(
        "other".into(),
        Binding::Node {
            node: NodeId::new("other").unwrap(),
            port: "answers".into(),
            path: vec![],
        },
    );
    let starts = Arc::new(AtomicUsize::new(0));
    let mut registry = BackendRegistry::default();
    for (name, slow) in [("judge", false), ("slow", true)] {
        registry
            .register(
                BackendId::new(name).unwrap(),
                BackendBinding {
                    backend: Arc::new(ParallelFailure {
                        slow,
                        starts: starts.clone(),
                    }),
                    model: "requested".into(),
                    expected_model: None,
                    distribution_policy: DistributionPolicy::Strict {},
                },
            )
            .unwrap();
    }
    let engine = Engine::new(
        compile(&spec, &PrimitiveRegistry::default()).unwrap(),
        registry,
    )
    .unwrap();
    let (result, calls) = engine
        .run_observed(
            &input(),
            RunLimits::default(),
            &config(ObservationMode::Live),
        )
        .await
        .unwrap();
    assert_eq!(result.unwrap_err().error.code, ErrorCode::BackendFailed);
    assert_eq!(starts.load(Ordering::SeqCst), 2);
    assert_eq!(calls.len(), 2);
    assert!(calls.iter().all(|call| call.elapsed_micros.is_some()));
    assert!(calls.iter().all(|call| call.actual_model.is_none()));
}
