// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Exact-one guarded router and existing escalation through public contracts.
use crate::common::*;
use async_trait::async_trait;
use sapho::{core::*, graph::*, recording::*, runtime::NodeStatus};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

struct FixedAnswer {
    model: String,
    probability: f64,
    calls: AtomicUsize,
}
#[async_trait]
impl ModelBackend for FixedAnswer {
    async fn infer(&self, _: &ModelRequest) -> Result<ModelResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(ModelResponse {
            model: self.model.clone(),
            raw: None,
            usage: None,
            answers: BTreeMap::from([(
                "accepted".into(),
                Answer::Boolean {
                    probability: Probability::new(self.probability)?,
                },
            )]),
        })
    }
}
fn fixed(model: &str, probability: f64) -> Arc<FixedAnswer> {
    Arc::new(FixedAnswer {
        model: model.into(),
        probability,
        calls: AtomicUsize::new(0),
    })
}
fn register(
    registry: &mut BackendRegistry,
    name: &str,
    model: &str,
    backend: Arc<dyn ModelBackend>,
) {
    registry
        .register(
            BackendId::new(name).unwrap(),
            BackendBinding {
                backend,
                model: model.into(),
                expected_model: Some(model.into()),
                distribution_policy: DistributionPolicy::Strict {},
            },
        )
        .unwrap();
}
fn example(name: &str) -> GraphSpec {
    GraphSpec::parse(&std::fs::read_to_string(format!("examples/graphs/{name}.yaml")).unwrap())
        .unwrap()
}
fn routed_input(risk: f64, low: f64, high: f64) -> Inputs {
    Inputs::from([
        (
            "risk".into(),
            datum("risk", Value::Probability(probability(risk))),
        ),
        (
            "low".into(),
            datum("low", Value::Probability(probability(low))),
        ),
        (
            "high".into(),
            datum("high", Value::Probability(probability(high))),
        ),
    ])
}
fn router_backends() -> (
    BackendRegistry,
    Vec<Arc<FixedAnswer>>,
    Vec<Arc<RecordingBackend>>,
) {
    let mut registry = BackendRegistry::default();
    let mut fakes = Vec::new();
    let mut recorders = Vec::new();
    for (name, model, p) in [
        ("tier_a", "model-a", 0.7),
        ("tier_b", "model-b", 0.5),
        ("tier_c", "model-c", 0.9),
    ] {
        let fake = fixed(model, p);
        let recorder = Arc::new(RecordingBackend::new(fake.clone(), 1_000_000).unwrap());
        register(&mut registry, name, model, recorder.clone());
        fakes.push(fake);
        recorders.push(recorder);
    }
    (registry, fakes, recorders)
}
fn combined_recording(recorders: &[Arc<RecordingBackend>]) -> Recording {
    let mut recording = Recording::default();
    for recorder in recorders {
        recording
            .exchanges
            .extend(recorder.snapshot().unwrap().exchanges);
    }
    Recording::from_json(&recording.to_json(1_000_000).unwrap(), 1_000_000).unwrap()
}
fn replay_registry(recording: &Recording, entries: &[(&str, &str)]) -> BackendRegistry {
    let replay = Arc::new(ReplayBackend::new(recording, 1_000_000).unwrap());
    let mut registry = BackendRegistry::default();
    for (name, model) in entries {
        register(&mut registry, name, model, replay.clone());
    }
    registry
}

/// Trace: FR-077-AC-1, FR-078-AC-2, IT-015-SC-02
#[tokio::test]
async fn router_calls_one_of_three_backends_and_replays_matched_answer_and_model() {
    let graph = example("router");
    let (registry, fakes, recorders) = router_backends();
    let live = engine(&graph, &PrimitiveRegistry::default(), registry);
    let cases = [
        (0.1, "low_ask", "model-a", 0.7),
        (0.5, "middle_ask", "model-b", 0.5),
        (0.9, "high_ask", "model-c", 0.9),
    ];
    let mut outcomes = Vec::new();
    for (risk, selected, model, p) in cases {
        let result = live
            .run(&routed_input(risk, 0.3, 0.7), limits())
            .await
            .unwrap();
        assert_eq!(result.outputs["model"].value, Value::Text(model.into()));
        assert_eq!(
            result.outputs["confidence"].value,
            Value::Probability(probability(p))
        );
        assert!(matches!(result.outputs["answers"].value, Value::Answers(_)));
        let asks = result
            .trace
            .nodes
            .iter()
            .filter(|node| node.path.last().is_some_and(|part| part.ends_with("_ask")))
            .collect::<Vec<_>>();
        assert_eq!(asks.len(), 3);
        assert_eq!(
            asks.iter()
                .filter(|node| node.status == NodeStatus::Skipped)
                .count(),
            2
        );
        assert_eq!(
            asks.iter()
                .find(|node| node.status == NodeStatus::Completed)
                .unwrap()
                .path
                .last()
                .unwrap(),
            selected
        );
        for skipped in asks
            .iter()
            .filter(|node| node.status == NodeStatus::Skipped)
        {
            assert_eq!(skipped.outputs["answers"].value, Value::Optional(None));
            assert_eq!(skipped.outputs["model"].value, Value::Optional(None));
        }
        outcomes.push(result.outputs);
    }
    assert_eq!(
        fakes
            .iter()
            .map(|fake| fake.calls.load(Ordering::SeqCst))
            .collect::<Vec<_>>(),
        [1, 1, 1]
    );
    let recording = combined_recording(&recorders);
    assert_eq!(recording.exchanges.len(), 3);
    let replay = engine(
        &graph,
        &PrimitiveRegistry::default(),
        replay_registry(
            &recording,
            &[
                ("tier_a", "model-a"),
                ("tier_b", "model-b"),
                ("tier_c", "model-c"),
            ],
        ),
    );
    for (index, risk) in [0.1, 0.5, 0.9].into_iter().enumerate() {
        let result = replay
            .run(&routed_input(risk, 0.3, 0.7), limits())
            .await
            .unwrap();
        assert_eq!(result.outputs, outcomes[index]);
    }
}

/// Trace: FR-077-AC-2, FR-077-AC-3, IT-015-SC-03
#[tokio::test]
async fn all_skipped_and_overlapping_guards_refuse_with_count_names_and_node_path() {
    let mut no_route = example("router");
    for ask in no_route
        .nodes
        .iter_mut()
        .filter(|node| node.id.as_str().ends_with("_ask"))
    {
        ask.guard = Some(lit(Value::Boolean(false), ValueType::Boolean));
    }
    let (registry, fakes, _) = router_backends();
    let failure = engine(&no_route, &PrimitiveRegistry::default(), registry)
        .run(&routed_input(0.5, 0.3, 0.7), limits())
        .await
        .unwrap_err();
    assert_eq!(failure.error.code, ErrorCode::InvalidValue);
    assert_eq!(failure.error.context["present_count"], "0");
    assert_eq!(failure.error.context["present_operands"], "");
    assert_eq!(
        failure.error.context["node_path"],
        r#"["root","chosen_answers"]"#
    );
    assert_eq!(
        failure
            .trace
            .nodes
            .iter()
            .filter(|node| node.status == NodeStatus::Skipped)
            .count(),
        3
    );
    assert_eq!(
        failure
            .trace
            .nodes
            .iter()
            .find(|node| node
                .path
                .last()
                .is_some_and(|part| part == "chosen_answers"))
            .unwrap()
            .status,
        NodeStatus::Failed
    );
    assert!(
        fakes
            .iter()
            .all(|fake| fake.calls.load(Ordering::SeqCst) == 0)
    );

    let (registry, fakes, _) = router_backends();
    let failure = engine(&example("router"), &PrimitiveRegistry::default(), registry)
        .run(&routed_input(0.5, 0.8, 0.2), limits())
        .await
        .unwrap_err();
    assert_eq!(failure.error.code, ErrorCode::InvalidValue);
    assert_eq!(failure.error.context["present_count"], "2");
    assert_eq!(failure.error.context["present_operands"], "high,low");
    assert_eq!(
        failure.error.context["node_path"],
        r#"["root","chosen_answers"]"#
    );
    assert_eq!(
        fakes
            .iter()
            .map(|fake| fake.calls.load(Ordering::SeqCst))
            .sum::<usize>(),
        2
    );
}

/// Trace: FR-077-AC-4, IT-015-SC-01
#[test]
fn merge_bounds_and_exact_optional_types_compile_without_backend_name_assumptions() {
    for n in 2..=32 {
        let operands = (0..n)
            .map(|index| {
                (
                    format!("p{index:02}"),
                    lit(Value::Optional(None), ValueType::optional(ValueType::Text)),
                )
            })
            .collect();
        let spec = graph(
            vec![NodeSpec {
                id: NodeId::new("merge").unwrap(),
                operation: Operation::MergePresent {},
                inputs: operands,
                guard: None,
            }],
            output("merge", "result"),
        );
        assert_eq!(
            compile(&spec, &PrimitiveRegistry::default())
                .unwrap()
                .signature()
                .outputs["result"],
            ValueType::Text
        );
        let json = serde_json::to_string(&spec).unwrap();
        assert!(GraphSpec::parse(&json).is_ok());
    }
    for n in [1, 33] {
        let operands = (0..n)
            .map(|index| {
                (
                    format!("p{index:02}"),
                    lit(Value::Optional(None), ValueType::optional(ValueType::Text)),
                )
            })
            .collect();
        let spec = graph(
            vec![NodeSpec {
                id: NodeId::new("merge").unwrap(),
                operation: Operation::MergePresent {},
                inputs: operands,
                guard: None,
            }],
            output("merge", "result"),
        );
        assert!(compile(&spec, &PrimitiveRegistry::default()).is_err());
    }
    let mut mixed = graph(
        vec![node(
            "merge",
            Operation::MergePresent {},
            [
                (
                    "a",
                    lit(Value::Optional(None), ValueType::optional(ValueType::Text)),
                ),
                (
                    "b",
                    lit(
                        Value::Optional(None),
                        ValueType::optional(ValueType::Number),
                    ),
                ),
            ],
        )],
        output("merge", "result"),
    );
    assert_eq!(
        compile(&mixed, &PrimitiveRegistry::default())
            .err()
            .unwrap()
            .code,
        ErrorCode::TypeMismatch
    );
    mixed.nodes[0]
        .inputs
        .get_mut("b")
        .unwrap()
        .clone_from(&lit(Value::Text("plain".into()), ValueType::Text));
    assert_eq!(
        compile(&mixed, &PrimitiveRegistry::default())
            .err()
            .unwrap()
            .code,
        ErrorCode::TypeMismatch
    );
    assert!(serde_json::from_str::<Operation>(r#"{"kind":"merge_present","unknown":1}"#).is_err());
    let yaml = std::fs::read_to_string("examples/graphs/router.yaml").unwrap();
    let duplicated = yaml.replace(
        "low: {kind: node, node: low_ask, port: answers}",
        "low: {kind: node, node: low_ask, port: answers}\n      low: {kind: node, node: low_ask, port: answers}",
    );
    assert!(GraphSpec::parse(&duplicated).is_err());
    let mut plain_ask = ask_graph(None);
    plain_ask.nodes.push(node(
        "merge",
        Operation::MergePresent {},
        [
            ("a", output("ask", "answers")),
            ("b", output("ask", "answers")),
        ],
    ));
    plain_ask
        .outputs
        .insert("result".into(), output("merge", "result"));
    assert_eq!(
        compile(&plain_ask, &PrimitiveRegistry::default())
            .err()
            .unwrap()
            .code,
        ErrorCode::TypeMismatch
    );
    for input in plain_ask.nodes.last_mut().unwrap().inputs.values_mut() {
        *input = output("ask", "model");
    }
    assert_eq!(
        compile(&plain_ask, &PrimitiveRegistry::default())
            .err()
            .unwrap()
            .code,
        ErrorCode::TypeMismatch
    );
}

/// Trace: FR-077-AC-4, IT-015-SC-01
#[tokio::test]
async fn router_accepts_host_chosen_backend_ids() {
    let mut graph = example("router");
    for node in &mut graph.nodes {
        if let Operation::Ask { backend } = &mut node.operation {
            *backend = BackendId::new(match backend.as_str() {
                "tier_a" => "local_alpha",
                "tier_b" => "local_beta",
                "tier_c" => "local_gamma",
                _ => panic!("unexpected backend"),
            })
            .unwrap();
        }
    }
    let mut registry = BackendRegistry::default();
    for (name, model, p) in [
        ("local_alpha", "model-a", 0.7),
        ("local_beta", "model-b", 0.5),
        ("local_gamma", "model-c", 0.9),
    ] {
        register(&mut registry, name, model, fixed(model, p));
    }
    let output = engine(&graph, &PrimitiveRegistry::default(), registry)
        .run(&routed_input(0.5, 0.3, 0.7), limits())
        .await
        .unwrap();
    assert_eq!(output.outputs["model"].value, Value::Text("model-b".into()));
}

/// Trace: FR-077-AC-5, IT-015-SC-04
#[tokio::test]
async fn false_zero_empty_text_and_empty_list_are_present_and_sources_are_inherited() {
    for (value, ty) in [
        (Value::Boolean(false), ValueType::Boolean),
        (Value::Number(0.0), ValueType::Number),
        (Value::Text(String::new()), ValueType::Text),
        (Value::List(vec![]), ValueType::list(ValueType::Text)),
    ] {
        let mut selected = datum("selected", Value::Optional(Some(Box::new(value.clone()))));
        selected.sources.push(SourceRef {
            source: SourceId::new("selected-source").unwrap(),
            start: None,
            end: None,
        });
        let mut absent = datum("absent", Value::Optional(None));
        absent.sources.push(SourceRef {
            source: SourceId::new("absent-source").unwrap(),
            start: None,
            end: None,
        });
        let spec = graph(
            vec![node(
                "merge",
                Operation::MergePresent {},
                [
                    (
                        "selected",
                        Binding::Literal {
                            value: selected,
                            value_type: ValueType::optional(ty.clone()),
                        },
                    ),
                    (
                        "absent",
                        Binding::Literal {
                            value: absent,
                            value_type: ValueType::optional(ty),
                        },
                    ),
                ],
            )],
            output("merge", "result"),
        );
        let result = engine(
            &spec,
            &PrimitiveRegistry::default(),
            BackendRegistry::default(),
        )
        .run(&Inputs::new(), limits())
        .await
        .unwrap();
        assert_eq!(result.outputs["result"].value, value);
        assert_eq!(result.outputs["result"].sources.len(), 2);
    }
}

/// Trace: FR-077-AC-6, FR-078-AC-3, IT-015-SC-05
#[tokio::test]
async fn escalation_keeps_fast_then_optional_expert_and_replays_both_cases() {
    let graph = example("escalation");
    for (fast_probability, expected_expert) in [(0.9, false), (0.5, true)] {
        let fast = fixed("fast-model", fast_probability);
        let expert = fixed("expert-model", 0.8);
        let fast_record = Arc::new(RecordingBackend::new(fast.clone(), 1_000_000).unwrap());
        let expert_record = Arc::new(RecordingBackend::new(expert.clone(), 1_000_000).unwrap());
        let mut registry = BackendRegistry::default();
        register(&mut registry, "fast", "fast-model", fast_record.clone());
        register(
            &mut registry,
            "expert",
            "expert-model",
            expert_record.clone(),
        );
        let input = Inputs::from([(
            "statement".into(),
            datum("statement", Value::Text("synthetic".into())),
        )]);
        let result = engine(&graph, &PrimitiveRegistry::default(), registry)
            .run(&input, limits())
            .await
            .unwrap();
        assert_eq!(fast.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            expert.calls.load(Ordering::SeqCst),
            usize::from(expected_expert)
        );
        assert_eq!(
            result.outputs["model"].value,
            Value::Text(
                if expected_expert {
                    "expert-model"
                } else {
                    "fast-model"
                }
                .into()
            )
        );
        assert_eq!(
            result.outputs["confidence"].value,
            Value::Probability(probability(if expected_expert {
                0.8
            } else {
                fast_probability
            }))
        );
        assert_eq!(
            result
                .trace
                .nodes
                .iter()
                .find(|node| node.path.last().is_some_and(|part| part == "expert"))
                .unwrap()
                .status,
            if expected_expert {
                NodeStatus::Completed
            } else {
                NodeStatus::Skipped
            }
        );
        let recording = combined_recording(&[fast_record, expert_record]);
        assert_eq!(
            recording.exchanges.len(),
            if expected_expert { 2 } else { 1 }
        );
        let replay = engine(
            &graph,
            &PrimitiveRegistry::default(),
            replay_registry(
                &recording,
                &[("fast", "fast-model"), ("expert", "expert-model")],
            ),
        );
        assert_eq!(
            replay.run(&input, limits()).await.unwrap().outputs,
            result.outputs
        );
    }
}

/// Trace: FR-078-AC-1, FR-078-AC-4, IT-015-SC-06
#[test]
fn ci_validates_both_documented_offline_graphs() {
    let makefile = std::fs::read_to_string("Makefile").unwrap();
    let guide = std::fs::read_to_string("docs/user-guide.md").unwrap();
    for name in ["router", "escalation"] {
        let file = format!("examples/graphs/{name}.yaml");
        assert!(std::path::Path::new(&file).is_file());
        assert!(guide.contains(&format!("../{file}")));
        assert!(makefile.contains(&format!("validate {file}")));
        assert!(compile(&example(name), &PrimitiveRegistry::default()).is_ok());
    }
    assert!(makefile.contains("ci: fmt-check lint test deny audit-unsafe docs docs-examples spec"));
    assert!(guide.contains("2–32"));
    assert!(guide.contains("InvalidValue"));
    assert!(guide.contains("outside graph literals"));
}
