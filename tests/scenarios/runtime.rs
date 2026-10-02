// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Functional acceptance through the embedding facade and real executor.
use crate::common::*;
use sapho::{core::*, graph::*, runtime::*};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

/// Trace: FR-018-AC-1, FR-018-AC-3
#[tokio::test]
async fn boolean_truth_tables_are_crisp() {
    for a in [false, true] {
        for b in [false, true] {
            let args = || {
                vec![
                    ("a", Value::Boolean(a), ValueType::Boolean),
                    ("b", Value::Boolean(b), ValueType::Boolean),
                ]
            };
            assert_eq!(
                operation(Operation::And, args()).await,
                Value::Boolean(a && b)
            );
            assert_eq!(
                operation(Operation::Or, args()).await,
                Value::Boolean(a || b)
            );
        }
        assert_eq!(
            operation(
                Operation::Not,
                vec![("value", Value::Boolean(a), ValueType::Boolean)]
            )
            .await,
            Value::Boolean(!a)
        );
    }
}
/// Trace: FR-019-AC-1, FR-019-AC-3
#[tokio::test]
async fn threshold_comparisons_define_the_equality_boundary() {
    for a in [0.49, 0.5, 0.51] {
        for comparator in [
            Comparator::Equal,
            Comparator::Less,
            Comparator::LessEqual,
            Comparator::Greater,
            Comparator::GreaterEqual,
        ] {
            let expected = match comparator {
                Comparator::Equal => a == 0.5,
                Comparator::Less => a < 0.5,
                Comparator::LessEqual => a <= 0.5,
                Comparator::Greater => a > 0.5,
                Comparator::GreaterEqual => a >= 0.5,
            };
            assert_eq!(
                operation(
                    Operation::Compare { comparator },
                    vec![
                        ("a", Value::Number(a), ValueType::Number),
                        ("b", Value::Number(0.5), ValueType::Number)
                    ]
                )
                .await,
                Value::Boolean(expected)
            );
        }
    }
}
/// Trace: FR-021-AC-1, FR-021-AC-2, FR-021-AC-3, FR-023-AC-1, FR-023-AC-3
#[tokio::test]
async fn explicit_degrees_and_complement_keep_their_semantic_type() {
    assert_eq!(
        operation(
            Operation::Degree,
            vec![(
                "value",
                Value::Probability(probability(0.7)),
                ValueType::Probability
            )]
        )
        .await,
        degree(0.7)
    );
    for v in [0.0, 0.25, 1.0] {
        assert_eq!(
            operation(
                Operation::Complement,
                vec![("value", degree(v), ValueType::Degree)]
            )
            .await,
            degree(1.0 - v)
        );
    }
    for v in [-0.01, 1.01] {
        let spec = graph(
            vec![node(
                "degree",
                Operation::Degree,
                [("value", lit(Value::Number(v), ValueType::Number))],
            )],
            output("degree", "result"),
        );
        let err = engine(
            &spec,
            &PrimitiveRegistry::default(),
            BackendRegistry::default(),
        )
        .run(&Inputs::new(), limits())
        .await
        .unwrap_err();
        assert_eq!(err.error.code, ErrorCode::InvalidValue);
        assert_eq!(err.trace.nodes[0].status, NodeStatus::Failed);
    }
}
/// Trace: FR-022-AC-1, FR-022-AC-2, FR-022-AC-3
#[tokio::test]
async fn degree_reductions_use_explicit_empty_values_and_aligned_weights() {
    let values = || list([("a", degree(0.2)), ("b", degree(0.8))]);
    for (reducer, want) in [(Reducer::Min, 0.2), (Reducer::Max, 0.8)] {
        assert_eq!(
            operation(
                Operation::Reduce {
                    reducer,
                    empty: Degree::new(0.4).unwrap()
                },
                vec![("values", values(), ValueType::list(ValueType::Degree))]
            )
            .await,
            degree(want)
        );
    }
    let args = vec![
        ("values", values(), ValueType::list(ValueType::Degree)),
        (
            "weights",
            list([
                ("b", Value::Number(f64::MAX)),
                ("a", Value::Number(f64::MAX)),
            ]),
            ValueType::list(ValueType::Number),
        ),
    ];
    assert_eq!(
        operation(
            Operation::Reduce {
                reducer: Reducer::WeightedMean,
                empty: Degree::new(0.4).unwrap()
            },
            args
        )
        .await,
        degree(0.5)
    );
    assert_eq!(
        operation(
            Operation::Reduce {
                reducer: Reducer::Min,
                empty: Degree::new(0.4).unwrap()
            },
            vec![(
                "values",
                Value::List(vec![]),
                ValueType::list(ValueType::Degree)
            )]
        )
        .await,
        degree(0.4)
    );
    for weights in [
        list([("a", Value::Number(0.0)), ("b", Value::Number(0.0))]),
        list([("a", Value::Number(-1.0)), ("b", Value::Number(1.0))]),
        list([("a", Value::Number(1.0)), ("other", Value::Number(1.0))]),
    ] {
        let spec = graph(
            vec![node(
                "r",
                Operation::Reduce {
                    reducer: Reducer::WeightedMean,
                    empty: Degree::new(0.0).unwrap(),
                },
                [
                    ("values", lit(values(), ValueType::list(ValueType::Degree))),
                    ("weights", lit(weights, ValueType::list(ValueType::Number))),
                ],
            )],
            output("r", "result"),
        );
        assert!(
            engine(
                &spec,
                &PrimitiveRegistry::default(),
                BackendRegistry::default()
            )
            .run(&Inputs::new(), limits())
            .await
            .is_err()
        );
    }
}
/// Trace: FR-024-AC-1, FR-024-AC-2
#[tokio::test]
async fn coalescing_changes_only_explicit_absence() {
    for (value, ty, default) in [
        (
            Value::Boolean(false),
            ValueType::Boolean,
            Value::Boolean(true),
        ),
        (Value::Number(0.0), ValueType::Number, Value::Number(1.0)),
        (
            Value::List(vec![]),
            ValueType::list(ValueType::Boolean),
            list([("default", Value::Boolean(true))]),
        ),
    ] {
        assert_eq!(
            operation(
                Operation::Coalesce,
                vec![
                    (
                        "value",
                        Value::Optional(Some(Box::new(value.clone()))),
                        ValueType::optional(ty.clone())
                    ),
                    ("default", default.clone(), ty.clone())
                ]
            )
            .await,
            value
        );
        assert_eq!(
            operation(
                Operation::Coalesce,
                vec![
                    (
                        "value",
                        Value::Optional(None),
                        ValueType::optional(ty.clone())
                    ),
                    ("default", default.clone(), ty)
                ]
            )
            .await,
            default
        );
    }
}
/// Trace: FR-009-AC-2, FR-009-AC-3, FR-017-AC-2
#[tokio::test]
async fn guarded_model_nodes_skip_work_and_wrap_present_answers() {
    let backend = Arc::new(Scripted {
        calls: AtomicUsize::new(0),
        answer: 0.8,
    });
    let skipped = engine(
        &ask_graph(Some(false)),
        &PrimitiveRegistry::default(),
        bindings(backend.clone()),
    )
    .run(&Inputs::new(), limits())
    .await
    .unwrap();
    assert_eq!(backend.calls.load(Ordering::SeqCst), 0);
    assert_eq!(skipped.outputs["result"].value, Value::Optional(None));
    assert_eq!(skipped.trace.nodes[1].status, NodeStatus::Skipped);
    assert!(skipped.trace.nodes[1].model.is_none());
    assert_eq!(
        skipped.trace.nodes[1].guard.as_ref().unwrap().value,
        Value::Boolean(false)
    );
    assert_eq!(
        skipped.trace.nodes[1].dependencies,
        vec![vec!["root".to_owned(), "q".to_owned()]]
    );
    let ran = engine(
        &ask_graph(Some(true)),
        &PrimitiveRegistry::default(),
        bindings(backend.clone()),
    )
    .run(&Inputs::new(), limits())
    .await
    .unwrap();
    assert_eq!(backend.calls.load(Ordering::SeqCst), 1);
    assert!(matches!(
        ran.outputs["result"].value,
        Value::Optional(Some(_))
    ));
}
/// Trace: FR-003-AC-3, FR-010-AC-2, FR-010-AC-3
#[tokio::test]
async fn native_failures_and_wrong_outputs_never_reach_dependents() {
    for result in [
        Ok(Inputs::from([(
            "result".into(),
            datum("wrong", Value::Text("bad".into())),
        )])),
        Ok(Inputs::new()),
        Err(SaphoError::new(ErrorCode::CodeFailed, "native refusal")),
    ] {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = calls.clone();
        let signature = Signature {
            inputs: BTreeMap::new(),
            outputs: BTreeMap::from([("result".into(), ValueType::Boolean)]),
        };
        let mut registry = PrimitiveRegistry::default();
        registry
            .register(
                PrimitiveId::new("native").unwrap(),
                Arc::new(Native {
                    signature,
                    f: Box::new(move |_, _| {
                        c.fetch_add(1, Ordering::SeqCst);
                        result.clone()
                    }),
                }),
            )
            .unwrap();
        let spec = graph(
            vec![
                node(
                    "native",
                    Operation::Code {
                        primitive: PrimitiveId::new("native").unwrap(),
                        params: BTreeMap::new(),
                    },
                    [],
                ),
                node(
                    "dependent",
                    Operation::Not,
                    [("value", output("native", "result"))],
                ),
            ],
            output("dependent", "result"),
        );
        let e = engine(&spec, &registry, BackendRegistry::default());
        let failed = e.run(&Inputs::new(), limits()).await.unwrap_err();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(failed.trace.nodes.len(), 1);
        assert_eq!(failed.trace.nodes[0].status, NodeStatus::Failed);
        let invalid = Inputs::from([("extra".into(), datum("x", Value::Boolean(false)))]);
        assert_eq!(
            e.run(&invalid, limits()).await.unwrap_err().error.code,
            ErrorCode::MissingInput
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
fn string_list(ids: &[&str]) -> Value {
    Value::List(
        ids.iter()
            .map(|id| datum(id, Value::Text("same".into())))
            .collect(),
    )
}
/// Trace: FR-014-AC-1, FR-014-AC-2, FR-014-AC-3, FR-011-AC-1
#[tokio::test]
async fn pairs_preserve_all_occurrences_and_check_expansion_first() {
    let left = string_list(&["a/b", "a"]);
    let right = string_list(&["c", "b/c", "other"]);
    let spec = graph(
        vec![node(
            "pair",
            Operation::Pairs,
            [
                ("left", lit(left, ValueType::list(ValueType::Text))),
                ("right", lit(right, ValueType::list(ValueType::Text))),
            ],
        )],
        output("pair", "result"),
    );
    let e = engine(
        &spec,
        &PrimitiveRegistry::default(),
        BackendRegistry::default(),
    );
    let mut cap = limits();
    cap.collection_items = 6;
    let out = e.run(&Inputs::new(), cap.clone()).await.unwrap();
    let Value::List(pairs) = &out.outputs["result"].value else {
        panic!("expected pairs")
    };
    assert_eq!(pairs.len(), 6);
    assert_eq!(
        pairs.iter().map(|d| &d.id).collect::<BTreeSet<_>>().len(),
        6
    );
    assert_ne!(pairs[0].id, pairs[4].id);
    cap.collection_items = 5;
    assert_eq!(
        e.run(&Inputs::new(), cap).await.unwrap_err().error.code,
        ErrorCode::LimitExceeded
    );
}
/// Trace: FR-015-AC-1, FR-015-AC-2, FR-015-AC-3
#[tokio::test]
async fn joins_are_ordered_many_to_many_and_bounded() {
    let ty = record_type([("key", ValueType::Text)]);
    let left = list([
        ("l1", record([("key", Value::Text("x".into()))])),
        ("l2", record([("key", Value::Text("x".into()))])),
    ]);
    let right = list([
        ("r1", record([("key", Value::Text("x".into()))])),
        ("r2", record([("key", Value::Text("x".into()))])),
        ("r3", record([("key", Value::Text("y".into()))])),
    ]);
    let op = Operation::Join {
        left_key: "key".into(),
        right_key: "key".into(),
    };
    let spec = graph(
        vec![node(
            "join",
            op,
            [
                ("left", lit(left, ValueType::list(ty.clone()))),
                ("right", lit(right, ValueType::list(ty))),
            ],
        )],
        output("join", "result"),
    );
    let e = engine(
        &spec,
        &PrimitiveRegistry::default(),
        BackendRegistry::default(),
    );
    let out = e.run(&Inputs::new(), limits()).await.unwrap();
    let Value::List(items) = &out.outputs["result"].value else {
        panic!("expected list")
    };
    assert_eq!(items.len(), 4);
    assert_eq!(items[0].id.as_str(), "[\"l1\",\"r1\"]");
    assert_eq!(items[3].id.as_str(), "[\"l2\",\"r2\"]");
    let mut cap = limits();
    cap.collection_items = 3;
    assert_eq!(
        e.run(&Inputs::new(), cap).await.unwrap_err().error.code,
        ErrorCode::LimitExceeded
    );
    let mut bad = spec;
    if let Operation::Join { left_key, .. } = &mut bad.nodes[0].operation {
        *left_key = "missing".into();
    }
    assert_eq!(
        compile(&bad, &PrimitiveRegistry::default())
            .err()
            .unwrap()
            .code,
        ErrorCode::UnknownReference
    );
}
/// Trace: FR-013-AC-1, FR-013-AC-2, FR-013-AC-3
#[tokio::test]
async fn masks_match_item_identity_instead_of_position() {
    let args = |mask| {
        vec![
            (
                "items",
                string_list(&["a", "b", "c"]),
                ValueType::list(ValueType::Text),
            ),
            ("mask", mask, ValueType::list(ValueType::Boolean)),
        ]
    };
    let selected = operation(
        Operation::Filter,
        args(list([
            ("c", Value::Boolean(true)),
            ("a", Value::Boolean(true)),
            ("b", Value::Boolean(false)),
        ])),
    )
    .await;
    let Value::List(items) = selected else {
        panic!("list")
    };
    assert_eq!(
        items.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(),
        vec!["a", "c"]
    );
    for mask in [
        list([("a", Value::Boolean(true))]),
        list([
            ("a", Value::Boolean(true)),
            ("b", Value::Boolean(false)),
            ("wrong", Value::Boolean(false)),
        ]),
    ] {
        let spec = graph(
            vec![node(
                "filter",
                Operation::Filter,
                args(mask).into_iter().map(|(k, v, t)| (k, lit(v, t))),
            )],
            output("filter", "result"),
        );
        assert!(
            engine(
                &spec,
                &PrimitiveRegistry::default(),
                BackendRegistry::default()
            )
            .run(&Inputs::new(), limits())
            .await
            .is_err()
        );
    }
}
/// Trace: FR-016-AC-1, FR-016-AC-2, FR-016-AC-3
#[tokio::test]
async fn collection_flattening_preserves_order_and_refuses_identity_collisions() {
    let input = list([
        ("outer1", string_list(&["a", "b"])),
        ("empty", Value::List(vec![])),
        ("outer2", string_list(&["c"])),
    ]);
    let out = operation(
        Operation::Collect,
        vec![(
            "items",
            input,
            ValueType::list(ValueType::list(ValueType::Text)),
        )],
    )
    .await;
    let Value::List(items) = out else {
        panic!("list")
    };
    assert_eq!(
        items.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(),
        vec!["a", "b", "c"]
    );
    let input = list([
        ("outer1", string_list(&["a"])),
        ("outer2", string_list(&["a"])),
    ]);
    let spec = graph(
        vec![node(
            "c",
            Operation::Collect,
            [(
                "items",
                lit(input, ValueType::list(ValueType::list(ValueType::Text))),
            )],
        )],
        output("c", "result"),
    );
    assert_eq!(
        engine(
            &spec,
            &PrimitiveRegistry::default(),
            BackendRegistry::default()
        )
        .run(&Inputs::new(), limits())
        .await
        .unwrap_err()
        .error
        .code,
        ErrorCode::DuplicateId
    );
    assert_eq!(
        operation(
            Operation::Collect,
            vec![(
                "items",
                Value::List(vec![]),
                ValueType::list(ValueType::list(ValueType::Text))
            )]
        )
        .await,
        Value::List(vec![])
    );
}
/// Trace: NFR-002-M-1, FR-012-AC-1, FR-012-AC-2, FR-012-AC-3, FR-011-AC-2, FR-002-AC-2
#[tokio::test]
async fn mapped_subgraphs_keep_ids_context_and_shared_limits() {
    let counter = Arc::new(AtomicUsize::new(0));
    let c = counter.clone();
    let mut registry = PrimitiveRegistry::default();
    registry
        .register(
            PrimitiveId::new("consumer.add").unwrap(),
            Arc::new(Native {
                signature: Signature {
                    inputs: BTreeMap::from([
                        ("item".into(), ValueType::Number),
                        ("offset".into(), ValueType::Number),
                    ]),
                    outputs: BTreeMap::from([("result".into(), ValueType::Number)]),
                },
                f: Box::new(move |i, _| {
                    c.fetch_add(1, Ordering::SeqCst);
                    let Value::Number(a) = i["item"].value else {
                        panic!("item")
                    };
                    let Value::Number(b) = i["offset"].value else {
                        panic!("offset")
                    };
                    Ok(Inputs::from([(
                        "result".into(),
                        datum("derived", Value::Number(a + b)),
                    )]))
                }),
            }),
        )
        .unwrap();
    let sub = GraphBody {
        inputs: BTreeMap::from([
            ("item".into(), ValueType::Number),
            ("offset".into(), ValueType::Number),
        ]),
        nodes: vec![node(
            "add",
            Operation::Code {
                primitive: PrimitiveId::new("consumer.add").unwrap(),
                params: BTreeMap::new(),
            },
            [("item", input("item")), ("offset", input("offset"))],
        )],
        outputs: BTreeMap::from([("result".into(), output("add", "result"))]),
    };
    let source = SourceRef {
        source: SourceId::new("synthetic-sentence").unwrap(),
        start: Some(3),
        end: Some(5),
    };
    let mut items = vec![
        datum("a", Value::Number(1.0)),
        datum("b", Value::Number(2.0)),
        datum("c", Value::Number(3.0)),
    ];
    items[1].sources.push(source.clone());
    let mut spec = graph(
        vec![node(
            "map",
            Operation::Map {
                graph: "add".into(),
            },
            [
                (
                    "items",
                    lit(Value::List(items), ValueType::list(ValueType::Number)),
                ),
                ("offset", lit(Value::Number(10.0), ValueType::Number)),
            ],
        )],
        output("map", "result"),
    );
    spec.subgraphs.insert("add".into(), sub);
    let e = engine(&spec, &registry, BackendRegistry::default());
    let out = e.run(&Inputs::new(), limits()).await.unwrap();
    let Value::List(items) = &out.outputs["result"].value else {
        panic!("list")
    };
    assert_eq!(
        items.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(),
        vec!["a", "b", "c"]
    );
    assert_eq!(
        items.iter().map(|d| &d.value).collect::<Vec<_>>(),
        vec![
            &Value::Number(11.0),
            &Value::Number(12.0),
            &Value::Number(13.0)
        ]
    );
    assert!(items[1].sources.contains(&source));
    assert_eq!(counter.load(Ordering::SeqCst), 3);
    let mut cap = limits();
    cap.node_instances = 3;
    assert_eq!(
        e.run(&Inputs::new(), cap).await.unwrap_err().error.code,
        ErrorCode::LimitExceeded
    );
    let mut empty = spec;
    if let Binding::Literal { value, .. } = empty.nodes[0].inputs.get_mut("items").unwrap() {
        value.value = Value::List(vec![]);
    }
    let count = counter.load(Ordering::SeqCst);
    let out = engine(&empty, &registry, BackendRegistry::default())
        .run(&Inputs::new(), limits())
        .await
        .unwrap();
    assert_eq!(out.outputs["result"].value, Value::List(vec![]));
    assert_eq!(counter.load(Ordering::SeqCst), count);
}
struct DropSignal(Arc<AtomicBool>);
impl Drop for DropSignal {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
struct Pending {
    entered: Arc<tokio::sync::Notify>,
    dropped: Arc<AtomicBool>,
}
#[async_trait::async_trait]
impl ModelBackend for Pending {
    async fn infer(&self, _: &ModelRequest) -> Result<ModelResponse> {
        let _guard = DropSignal(self.dropped.clone());
        self.entered.notify_one();
        std::future::pending().await
    }
}
/// Trace: FR-011-AC-3, FR-010-AC-3
#[tokio::test(start_paused = true)]
async fn deadlines_cancel_pending_backend_futures_with_partial_evidence() {
    let entered = Arc::new(tokio::sync::Notify::new());
    let dropped = Arc::new(AtomicBool::new(false));
    let e = Arc::new(engine(
        &ask_graph(None),
        &PrimitiveRegistry::default(),
        bindings(Arc::new(Pending {
            entered: entered.clone(),
            dropped: dropped.clone(),
        })),
    ));
    let mut cap = limits();
    cap.duration = Duration::from_secs(5);
    let task = tokio::spawn(async move { e.run(&Inputs::new(), cap).await });
    entered.notified().await;
    tokio::time::advance(Duration::from_secs(6)).await;
    let err = task.await.unwrap().unwrap_err();
    assert_eq!(err.error.code, ErrorCode::DeadlineExceeded);
    assert_eq!(err.trace.nodes[0].status, NodeStatus::Completed);
    assert_eq!(err.trace.nodes[1].status, NodeStatus::Failed);
    assert!(dropped.load(Ordering::SeqCst));
}
struct Invalid;
#[async_trait::async_trait]
impl ModelBackend for Invalid {
    async fn infer(&self, r: &ModelRequest) -> Result<ModelResponse> {
        Ok(ModelResponse {
            model: r.model.clone(),
            answers: BTreeMap::new(),
            usage: None,
        })
    }
}
/// Trace: FR-017-AC-1, FR-017-AC-3
#[tokio::test]
async fn invalid_raw_answers_remain_in_the_failed_trace() {
    let err = engine(
        &ask_graph(None),
        &PrimitiveRegistry::default(),
        bindings(Arc::new(Invalid)),
    )
    .run(&Inputs::new(), limits())
    .await
    .unwrap_err();
    assert_eq!(err.error.code, ErrorCode::MissingAnswer);
    let evidence = err.trace.nodes[1].model.as_ref().unwrap();
    assert_eq!(evidence.request.questions.len(), 1);
    assert!(evidence.response.as_ref().unwrap().answers.is_empty());
}
struct Concurrent {
    barrier: Arc<tokio::sync::Barrier>,
    active: AtomicUsize,
    maximum: AtomicUsize,
}
#[async_trait::async_trait]
impl ModelBackend for Concurrent {
    async fn infer(&self, r: &ModelRequest) -> Result<ModelResponse> {
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.maximum.fetch_max(active, Ordering::SeqCst);
        self.barrier.wait().await;
        self.active.fetch_sub(1, Ordering::SeqCst);
        Ok(ModelResponse {
            model: r.model.clone(),
            answers: BTreeMap::from([(
                "q".into(),
                Answer::Boolean {
                    probability: probability(0.8),
                },
            )]),
            usage: None,
        })
    }
}
/// Trace: FR-010-AC-1
#[tokio::test]
async fn independent_model_work_is_bounded_and_trace_order_is_stable() {
    let mut spec = ask_graph(None);
    let mut other = spec.nodes[1].clone();
    other.id = NodeId::new("other").unwrap();
    spec.nodes.push(other);
    spec.outputs
        .insert("other".into(), output("other", "answers"));
    let barrier = Arc::new(tokio::sync::Barrier::new(3));
    let backend = Arc::new(Concurrent {
        barrier: barrier.clone(),
        active: AtomicUsize::new(0),
        maximum: AtomicUsize::new(0),
    });
    let e = Arc::new(engine(
        &spec,
        &PrimitiveRegistry::default(),
        bindings(backend.clone()),
    ));
    let mut cap = limits();
    cap.concurrency = 2;
    let task = tokio::spawn(async move { e.run(&Inputs::new(), cap).await });
    barrier.wait().await;
    let out = task.await.unwrap().unwrap();
    assert_eq!(backend.maximum.load(Ordering::SeqCst), 2);
    assert_eq!(
        out.trace
            .nodes
            .iter()
            .map(|n| n.path.last().unwrap().as_str())
            .collect::<Vec<_>>(),
        vec!["q", "ask", "other"]
    );
    assert_eq!(out.outputs.len(), 2);
}
