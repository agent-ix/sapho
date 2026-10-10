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
use sapho_graph::{Binding, Comparator, GraphBody, GraphSpec, NodeSpec, Operation, compile};
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

/// Trace: FR-084-AC-5, IT-017-SC-07
#[tokio::test]
async fn pure_calibration_can_compute_but_stock_identity_check_refuses_no_ask_source() {
    let spec = GraphSpec {
        inputs: BTreeMap::from([("raw".into(), ValueType::Probability)]),
        nodes: vec![node(
            "calibrate",
            Operation::Calibrate,
            BTreeMap::from([
                (
                    "value".into(),
                    Binding::Input {
                        name: "raw".into(),
                        path: Vec::new(),
                    },
                ),
                (
                    "map".into(),
                    literal(
                        Value::CalibrationMap(Box::new(map())),
                        ValueType::CalibrationMap,
                    ),
                ),
            ]),
            None,
        )],
        outputs: BTreeMap::from([("result".into(), from("calibrate", "result"))]),
        subgraphs: BTreeMap::new(),
    };
    let inputs = Inputs::from([(
        "raw".into(),
        Datum::new("raw", Value::Probability(Probability::new(0.8).unwrap())).unwrap(),
    )]);
    let pure = Engine::new(
        compile(&spec, &PrimitiveRegistry::default()).unwrap(),
        BackendRegistry::default(),
    )
    .unwrap();
    assert!(matches!(
        pure.run(&inputs, RunLimits::default())
            .await
            .unwrap()
            .outputs["result"]
            .value,
        Value::CalibratedProbability(_)
    ));
    let checked = pure.with_calibration_identity_check();
    let failure = checked
        .run(&inputs, RunLimits::default())
        .await
        .unwrap_err();
    assert_eq!(failure.error.code, ErrorCode::ModelMismatch);
    assert_eq!(failure.error.context["observed_count"], "0");
}

/// Trace: FR-084-AC-5, IT-017-SC-07
#[tokio::test]
async fn two_ask_raw_lineage_refuses_before_dependent_ask() {
    let mut spec = graph(map());
    spec.nodes
        .iter_mut()
        .find(|node| node.id.as_str() == "raw")
        .unwrap()
        .guard = Some(literal(Value::Boolean(true), ValueType::Boolean));
    let mut second = spec
        .nodes
        .iter()
        .find(|node| node.id.as_str() == "fast")
        .unwrap()
        .clone();
    second.id = NodeId::new("second").unwrap();
    second.operation = Operation::Ask {
        backend: BackendId::new("fast_b").unwrap(),
    };
    let mut other_raw = spec
        .nodes
        .iter()
        .find(|node| node.id.as_str() == "raw")
        .unwrap()
        .clone();
    other_raw.id = NodeId::new("other_raw").unwrap();
    other_raw.guard = None;
    other_raw
        .inputs
        .insert("answers".into(), from("second", "answers"));
    spec.nodes.push(second);
    spec.nodes.push(other_raw);
    spec.nodes.push(node(
        "coalesce",
        Operation::Coalesce,
        BTreeMap::from([
            ("value".into(), from("raw", "result")),
            ("default".into(), from("other_raw", "result")),
        ]),
        None,
    ));
    spec.nodes
        .iter_mut()
        .find(|node| node.id.as_str() == "calibrate")
        .unwrap()
        .inputs
        .insert("value".into(), from("coalesce", "result"));
    let mut registry = BackendRegistry::default();
    let mut backends = Vec::new();
    for (binding, model) in [
        ("fast_a", "model-a"),
        ("fast_b", "model-b"),
        ("slow_b", "slow-model"),
    ] {
        let backend = Arc::new(Scripted {
            name: model,
            calls: AtomicUsize::new(0),
        });
        registry
            .register(
                BackendId::new(binding).unwrap(),
                BackendBinding {
                    backend: backend.clone(),
                    model: "requested".into(),
                    expected_model: None,
                    distribution_policy: DistributionPolicy::Strict {},
                },
            )
            .unwrap();
        backends.push(backend);
    }
    let checked = Engine::new(
        compile(&spec, &PrimitiveRegistry::default()).unwrap(),
        registry,
    )
    .unwrap()
    .with_calibration_identity_check();
    let failure = checked
        .run(&Inputs::new(), RunLimits::default())
        .await
        .unwrap_err();
    assert_eq!(failure.error.code, ErrorCode::ModelMismatch);
    assert_eq!(failure.error.context["observed_count"], "2");
    assert_eq!(backends[0].calls.load(Ordering::SeqCst), 1);
    assert_eq!(backends[1].calls.load(Ordering::SeqCst), 1);
    assert_eq!(backends[2].calls.load(Ordering::SeqCst), 0);
}

/// Trace: FR-084-AC-6, IT-017-SC-08
#[tokio::test]
async fn mapped_item_uses_only_its_own_ask_response() {
    let question = NamedQuestion {
        id: "q".into(),
        question: Question::Boolean {
            instructions: "Decide".into(),
            yes: "Yes".into(),
            no: "No".into(),
        },
    };
    let mut nodes = vec![node(
        "questions",
        Operation::Questions {
            questions: vec![question],
        },
        BTreeMap::new(),
        None,
    )];
    let mut registry = BackendRegistry::default();
    let mut backends = Vec::new();
    for (suffix, model) in [("a", "model-a"), ("b", "model-b")] {
        let backend = Arc::new(Scripted {
            name: model,
            calls: AtomicUsize::new(0),
        });
        registry
            .register(
                BackendId::new(format!("fast_{suffix}")).unwrap(),
                BackendBinding {
                    backend: backend.clone(),
                    model: "requested".into(),
                    expected_model: None,
                    distribution_policy: DistributionPolicy::Strict {},
                },
            )
            .unwrap();
        backends.push(backend);
        nodes.push(node(
            &format!("ask_{suffix}"),
            Operation::Ask {
                backend: BackendId::new(format!("fast_{suffix}")).unwrap(),
            },
            BTreeMap::from([
                (
                    "state".into(),
                    literal(
                        Value::Record(BTreeMap::new()),
                        ValueType::Record {
                            fields: BTreeMap::new(),
                        },
                    ),
                ),
                ("questions".into(), from("questions", "result")),
            ]),
            None,
        ));
        nodes.push(node(
            &format!("raw_{suffix}"),
            Operation::Probability {
                question: "q".into(),
                labels: vec!["true".into()],
            },
            BTreeMap::from([("answers".into(), from(&format!("ask_{suffix}"), "answers"))]),
            None,
        ));
    }
    nodes.push(node(
        "list",
        Operation::List {
            item_type: ValueType::Probability,
            order: vec!["first".into(), "second".into()],
        },
        BTreeMap::from([
            ("first".into(), from("raw_a", "result")),
            ("second".into(), from("raw_b", "result")),
        ]),
        None,
    ));
    nodes.push(node(
        "map",
        Operation::Map {
            graph: "each".into(),
        },
        BTreeMap::from([
            ("items".into(), from("list", "result")),
            ("questions".into(), from("questions", "result")),
        ]),
        None,
    ));
    let downstream = Arc::new(Scripted {
        name: "downstream-model",
        calls: AtomicUsize::new(0),
    });
    registry
        .register(
            BackendId::new("slow_b").unwrap(),
            BackendBinding {
                backend: downstream.clone(),
                model: "requested".into(),
                expected_model: None,
                distribution_policy: DistributionPolicy::Strict {},
            },
        )
        .unwrap();
    let child = GraphBody {
        inputs: BTreeMap::from([
            ("item".into(), ValueType::Probability),
            ("questions".into(), ValueType::Questions),
        ]),
        nodes: vec![
            node(
                "calibrate",
                Operation::Calibrate,
                BTreeMap::from([
                    (
                        "value".into(),
                        Binding::Input {
                            name: "item".into(),
                            path: Vec::new(),
                        },
                    ),
                    (
                        "map".into(),
                        literal(
                            Value::CalibrationMap(Box::new(map())),
                            ValueType::CalibrationMap,
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
                    (
                        "state".into(),
                        literal(
                            Value::Record(BTreeMap::new()),
                            ValueType::Record {
                                fields: BTreeMap::new(),
                            },
                        ),
                    ),
                    (
                        "questions".into(),
                        Binding::Input {
                            name: "questions".into(),
                            path: Vec::new(),
                        },
                    ),
                ]),
                Some(from("guard", "result")),
            ),
        ],
        outputs: BTreeMap::from([("result".into(), from("slow", "model"))]),
    };
    let spec = GraphSpec {
        inputs: BTreeMap::new(),
        nodes,
        outputs: BTreeMap::from([("mapped".into(), from("map", "result"))]),
        subgraphs: BTreeMap::from([("each".into(), child)]),
    };
    let mut filtered = spec.clone();
    filtered.nodes.push(node(
        "filter",
        Operation::Filter,
        BTreeMap::from([
            ("items".into(), from("list", "result")),
            (
                "mask".into(),
                literal(
                    Value::List(vec![
                        Datum::new("[\"root\",\"list\",\"first\"]", Value::Boolean(true)).unwrap(),
                        Datum::new("[\"root\",\"list\",\"second\"]", Value::Boolean(true)).unwrap(),
                    ]),
                    ValueType::list(ValueType::Boolean),
                ),
            ),
        ]),
        None,
    ));
    filtered
        .nodes
        .iter_mut()
        .find(|node| node.id.as_str() == "map")
        .unwrap()
        .inputs
        .insert("items".into(), from("filter", "result"));
    let mut collected = spec.clone();
    collected.nodes.push(node(
        "outer",
        Operation::List {
            item_type: ValueType::list(ValueType::Probability),
            order: vec!["group".into()],
        },
        BTreeMap::from([("group".into(), from("list", "result"))]),
        None,
    ));
    collected.nodes.push(node(
        "collect",
        Operation::Collect,
        BTreeMap::from([("items".into(), from("outer", "result"))]),
        None,
    ));
    collected
        .nodes
        .iter_mut()
        .find(|node| node.id.as_str() == "map")
        .unwrap()
        .inputs
        .insert("items".into(), from("collect", "result"));
    for (run, variant) in [spec, filtered, collected].into_iter().enumerate() {
        let engine = Engine::new(
            compile(&variant, &PrimitiveRegistry::default()).unwrap(),
            registry.clone(),
        )
        .unwrap()
        .with_calibration_identity_check();
        let failure = engine
            .run(&Inputs::new(), RunLimits::default())
            .await
            .unwrap_err();
        assert_eq!(failure.error.code, ErrorCode::ModelMismatch);
        assert!(failure.trace.nodes.iter().any(|node| {
            node.path.len() == 4
                && node.path[1] == "map"
                && node.path[3] == "calibrate"
                && node.status == NodeStatus::Completed
        }));
        assert!(failure.trace.nodes.iter().any(|node| {
            node.path.len() == 4
                && node.path[1] == "map"
                && node.path[3] == "calibrate"
                && node.status == NodeStatus::Failed
        }));
        assert_eq!(backends[0].calls.load(Ordering::SeqCst), run + 1);
        assert_eq!(backends[1].calls.load(Ordering::SeqCst), run + 1);
        assert_eq!(downstream.calls.load(Ordering::SeqCst), run + 1);
    }
}
