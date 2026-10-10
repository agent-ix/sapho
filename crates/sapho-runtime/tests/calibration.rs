// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! IT-017-SC-08: stock identity check before dependent guarded work.
use async_trait::async_trait;
use sapho_core::{
    Answer, BackendBinding, BackendId, BackendRegistry, CalibratedProbability, CalibrationKnot,
    CalibrationMap, Datum, DistributionPolicy, ErrorCode, Inputs, ModelBackend, ModelRequest,
    ModelResponse, NamedQuestion, NodeId, PrimitiveRegistry, Probability, Question, Result,
    SourceId, SourceRef, Value, ValueType,
};
use sapho_graph::{Binding, Comparator, GraphSpec, NodeSpec, Operation, compile};
use sapho_runtime::{Engine, NodeStatus, RunLimits};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

struct Scripted {
    name: &'static str,
    calls: AtomicUsize,
}
#[async_trait]
impl ModelBackend for Scripted {
    async fn infer(&self, request: &ModelRequest) -> Result<ModelResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(ModelResponse {
            model: self.name.into(),
            raw: None,
            usage: None,
            answers: request
                .questions
                .iter()
                .map(|q| {
                    (
                        q.id.clone(),
                        Answer::Boolean {
                            probability: Probability::new(0.8).unwrap(),
                        },
                    )
                })
                .collect(),
        })
    }
}
fn literal(value: Value, value_type: ValueType) -> Binding {
    Binding::Literal {
        value: Datum::new("literal", value).unwrap(),
        value_type,
    }
}
fn sourced_literal(value: Value, value_type: ValueType, source: &str) -> Binding {
    let mut datum = Datum::new("literal", value).unwrap();
    datum.sources.push(SourceRef {
        source: SourceId::new(source).unwrap(),
        start: None,
        end: None,
    });
    Binding::Literal {
        value: datum,
        value_type,
    }
}
fn from(node: &str, port: &str) -> Binding {
    Binding::Node {
        node: NodeId::new(node).unwrap(),
        port: port.into(),
        path: Vec::new(),
    }
}
fn node(
    id: &str,
    operation: Operation,
    inputs: BTreeMap<String, Binding>,
    guard: Option<Binding>,
) -> NodeSpec {
    NodeSpec {
        id: NodeId::new(id).unwrap(),
        operation,
        inputs,
        guard,
    }
}
fn graph(map: CalibrationMap) -> GraphSpec {
    let questions = vec![NamedQuestion {
        id: "q".into(),
        question: Question::Boolean {
            instructions: "Decide".into(),
            yes: "Yes".into(),
            no: "No".into(),
        },
    }];
    let state = sourced_literal(
        Value::Record(BTreeMap::from([(
            "text".into(),
            Value::Text("example".into()),
        )])),
        ValueType::Record {
            fields: BTreeMap::from([("text".into(), ValueType::Text)]),
        },
        "state-origin",
    );
    let nodes = vec![
        node(
            "questions",
            Operation::Questions { questions },
            BTreeMap::new(),
            None,
        ),
        node(
            "fast",
            Operation::Ask {
                backend: BackendId::new("fast_a").unwrap(),
            },
            BTreeMap::from([
                ("state".into(), state.clone()),
                ("questions".into(), from("questions", "result")),
            ]),
            None,
        ),
        node(
            "raw",
            Operation::Probability {
                question: "q".into(),
                labels: vec!["true".into()],
            },
            BTreeMap::from([("answers".into(), from("fast", "answers"))]),
            None,
        ),
        node(
            "calibrate",
            Operation::Calibrate,
            BTreeMap::from([
                ("value".into(), from("raw", "result")),
                (
                    "map".into(),
                    sourced_literal(
                        Value::CalibrationMap(Box::new(map)),
                        ValueType::CalibrationMap,
                        "map-origin",
                    ),
                ),
            ]),
            None,
        ),
        node(
            "convert",
            Operation::CalibratedAsProbability,
            BTreeMap::from([("value".into(), from("calibrate", "result"))]),
            None,
        ),
        node(
            "guard",
            Operation::Compare {
                comparator: Comparator::Greater,
            },
            BTreeMap::from([
                ("a".into(), from("convert", "result")),
                (
                    "b".into(),
                    literal(
                        Value::Probability(Probability::new(0.4).unwrap()),
                        ValueType::Probability,
                    ),
                ),
            ]),
            None,
        ),
        node(
            "slow",
            Operation::Ask {
                backend: BackendId::new("slow_b").unwrap(),
            },
            BTreeMap::from([
                ("state".into(), state),
                ("questions".into(), from("questions", "result")),
            ]),
            Some(from("guard", "result")),
        ),
    ];
    GraphSpec {
        inputs: BTreeMap::new(),
        nodes,
        outputs: BTreeMap::from([
            ("decision".into(), from("guard", "result")),
            ("slow_model".into(), from("slow", "model")),
        ]),
        subgraphs: BTreeMap::new(),
    }
}
fn map() -> CalibrationMap {
    let d = "a".repeat(64);
    CalibrationMap::new(
        SourceId::new("curated").unwrap(),
        format!("dataset-development-v1:sha256:{d}"),
        format!("calibration-observations-v1:sha256:{d}"),
        format!("graph-v1:sha256:{d}"),
        "raw_support".into(),
        BackendId::new("fast_a").unwrap(),
        "model-a".into(),
        20,
        vec![
            CalibrationKnot {
                raw: Probability::new(0.05).unwrap(),
                calibrated: CalibratedProbability::new(0.2).unwrap(),
            },
            CalibrationKnot {
                raw: Probability::new(0.95).unwrap(),
                calibrated: CalibratedProbability::new(0.8).unwrap(),
            },
        ],
    )
    .unwrap()
}
fn engine(name: &'static str) -> (Engine, Arc<Scripted>, Arc<Scripted>) {
    let fast = Arc::new(Scripted {
        name,
        calls: AtomicUsize::new(0),
    });
    let slow = Arc::new(Scripted {
        name: "slow-model",
        calls: AtomicUsize::new(0),
    });
    let mut registry = BackendRegistry::default();
    for (id, backend) in [("fast_a", fast.clone()), ("slow_b", slow.clone())] {
        registry
            .register(
                BackendId::new(id).unwrap(),
                BackendBinding {
                    backend,
                    model: "requested".into(),
                    expected_model: None,
                    distribution_policy: DistributionPolicy::Strict {},
                },
            )
            .unwrap();
    }
    (
        Engine::new(
            compile(&graph(map()), &PrimitiveRegistry::default()).unwrap(),
            registry,
        )
        .unwrap()
        .with_calibration_identity_check(),
        fast,
        slow,
    )
}
/// Trace: FR-082-AC-2, FR-082-AC-4, FR-084-AC-5, FR-084-AC-6, IT-017-SC-08
#[tokio::test]
async fn stale_model_refuses_before_guarded_ask_even_without_public_calibrated_output() {
    let (good, fast, slow) = engine("model-a");
    let result = good
        .run(&Inputs::new(), RunLimits::default())
        .await
        .unwrap();
    assert_eq!(result.outputs["decision"].value, Value::Boolean(true));
    assert_eq!(fast.calls.load(Ordering::SeqCst), 1);
    assert_eq!(slow.calls.load(Ordering::SeqCst), 1);
    let calibrated = result
        .trace
        .nodes
        .iter()
        .find(|node| node.path == ["root", "calibrate"])
        .unwrap();
    let sources = &calibrated.outputs["result"].sources;
    assert!(
        sources
            .iter()
            .any(|source| source.source.as_str() == "state-origin")
    );
    assert!(
        sources
            .iter()
            .any(|source| source.source.as_str() == "map-origin")
    );
    let (bad, fast, slow) = engine("model-b");
    let failure = bad
        .run(&Inputs::new(), RunLimits::default())
        .await
        .unwrap_err();
    assert_eq!(failure.error.code, ErrorCode::ModelMismatch);
    assert_eq!(fast.calls.load(Ordering::SeqCst), 1);
    assert_eq!(slow.calls.load(Ordering::SeqCst), 0);
    assert!(
        failure
            .trace
            .nodes
            .iter()
            .any(|n| matches!(n.operation, Operation::Ask { .. })
                && n.status == NodeStatus::Completed
                && n.model.as_ref().and_then(|m| m.response.as_ref()).is_some())
    );
}
