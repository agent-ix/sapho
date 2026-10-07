// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Record/replay integration through the same public executor.
use crate::common::*;
use sapho::{core::*, graph::*, recording::*};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
fn threshold_graph(cutoff: f64) -> GraphSpec {
    let mut g = ask_graph(None);
    g.nodes.extend([
        node(
            "probability",
            Operation::Probability {
                question: "q".into(),
                labels: vec!["true".into()],
            },
            [("answers", output("ask", "answers"))],
        ),
        node(
            "degree",
            Operation::Degree,
            [("value", output("probability", "result"))],
        ),
        node(
            "cutoff",
            Operation::Compare {
                comparator: Comparator::GreaterEqual,
            },
            [
                ("a", output("degree", "result")),
                ("b", lit(degree(cutoff), ValueType::Degree)),
            ],
        ),
    ]);
    g.outputs
        .insert("result".into(), output("cutoff", "result"));
    g
}
/// Trace: FR-027-AC-1, FR-028-AC-1, FR-029-AC-1, FR-029-AC-2, FR-029-AC-3, FR-017-AC-1
#[tokio::test]
async fn replay_recomputes_logic_and_never_delegates_live() {
    let live = Arc::new(Scripted {
        calls: AtomicUsize::new(0),
        answer: 0.8,
    });
    let recorder = Arc::new(RecordingBackend::new(live.clone(), 1_000_000).unwrap());
    let mut spec = threshold_graph(0.5);
    let source = SourceRef {
        source: SourceId::new("synthetic-evidence").unwrap(),
        start: Some(4),
        end: Some(8),
    };
    let Binding::Literal { value, .. } = spec.nodes[1].inputs.get_mut("state").unwrap() else {
        panic!("literal state expected");
    };
    value.sources.push(source.clone());
    let original = engine(
        &spec,
        &PrimitiveRegistry::default(),
        bindings(recorder.clone()),
    )
    .run(&Inputs::new(), limits())
    .await
    .unwrap();
    assert_eq!(original.outputs["result"].value, Value::Boolean(true));
    assert!(original.outputs["result"].sources.contains(&source));
    let cutoff = original
        .trace
        .nodes
        .iter()
        .find(|n| n.path.last().is_some_and(|s| s == "cutoff"))
        .unwrap();
    assert_eq!(cutoff.inputs["b"].value, degree(0.5));
    assert!(cutoff.inputs["a"].sources.contains(&source));
    let ask = original
        .trace
        .nodes
        .iter()
        .find_map(|n| n.model.as_ref())
        .unwrap();
    assert_eq!(
        ask.response.as_ref().unwrap().answers["q"],
        Answer::Boolean {
            probability: probability(0.8)
        }
    );
    assert_eq!(live.calls.load(Ordering::SeqCst), 1);
    let saved = recorder.snapshot().unwrap();
    assert_eq!(saved.exchanges.len(), 1);
    let bytes = saved.to_json(1_000_000).unwrap();
    let loaded = Recording::from_json(&bytes, 1_000_000).unwrap();
    let replay = Arc::new(ReplayBackend::new(&loaded, 1_000_000).unwrap());
    let repeated = engine(
        &spec,
        &PrimitiveRegistry::default(),
        bindings(replay.clone()),
    )
    .run(&Inputs::new(), limits())
    .await
    .unwrap();
    assert_eq!(repeated.outputs, original.outputs);
    assert_eq!(repeated.trace, original.trace);
    let adjusted = engine(
        &threshold_graph(0.9),
        &PrimitiveRegistry::default(),
        bindings(replay.clone()),
    )
    .run(&Inputs::new(), limits())
    .await
    .unwrap();
    assert_eq!(adjusted.outputs["result"].value, Value::Boolean(false));
    assert_eq!(live.calls.load(Ordering::SeqCst), 1);
    let mut changed = spec;
    let Operation::Questions { questions } = &mut changed.nodes[0].operation else {
        panic!("question node expected");
    };
    let Question::Boolean { instructions, .. } = &mut questions[0].question else {
        panic!("Boolean question expected");
    };
    instructions.push_str(" Changed question.");
    assert_eq!(
        engine(&changed, &PrimitiveRegistry::default(), bindings(replay))
            .run(&Inputs::new(), limits())
            .await
            .unwrap_err()
            .error
            .code,
        ErrorCode::ReplayMiss
    );
    assert_eq!(live.calls.load(Ordering::SeqCst), 1);
}
/// Trace: FR-027-AC-3, FR-028-AC-3
#[tokio::test]
async fn saved_exchanges_are_bounded_and_exclusive_file_writes_do_not_overwrite() {
    let live = Arc::new(Scripted {
        calls: AtomicUsize::new(0),
        answer: 0.8,
    });
    let recorder = Arc::new(RecordingBackend::new(live, 100_000).unwrap());
    engine(
        &ask_graph(None),
        &PrimitiveRegistry::default(),
        bindings(recorder.clone()),
    )
    .run(&Inputs::new(), limits())
    .await
    .unwrap();
    let saved = recorder.snapshot().unwrap();
    let bytes = saved.to_json(100_000).unwrap();
    assert_eq!(
        saved.to_json(bytes.len() - 1).unwrap_err().code,
        ErrorCode::LimitExceeded
    );
    assert_eq!(
        Recording::from_json(&bytes, bytes.len() - 1)
            .unwrap_err()
            .code,
        ErrorCode::LimitExceeded
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("recording.json");
    saved.write_new(&path, 100_000).unwrap();
    assert_eq!(Recording::read(&path, 100_000).unwrap(), saved);
    assert_eq!(
        saved.write_new(&path, 100_000).unwrap_err().code,
        ErrorCode::RecordingIo
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let mut conflict = saved.clone();
    let mut other = saved.exchanges[0].clone();
    other.response.answers.insert(
        "q".into(),
        Answer::Boolean {
            probability: probability(0.2),
        },
    );
    conflict.exchanges.push(other);
    assert_eq!(
        ReplayBackend::new(&conflict, 100_000).err().unwrap().code,
        ErrorCode::RecordingMismatch
    );
    let mut bad = saved;
    bad.exchanges[0].response.answers.clear();
    assert_eq!(
        ReplayBackend::new(&bad, 100_000).err().unwrap().code,
        ErrorCode::RecordingMismatch
    );
}
/// Trace: FR-028-AC-1, FR-028-AC-3, FR-056-AC-5
#[tokio::test]
async fn streamed_loading_gives_the_replay_of_typed_loading_and_refuses_the_same_recordings() {
    let live = Arc::new(Scripted {
        calls: AtomicUsize::new(0),
        answer: 0.8,
    });
    let recorder = Arc::new(RecordingBackend::new(live, 100_000).unwrap());
    engine(
        &ask_graph(None),
        &PrimitiveRegistry::default(),
        bindings(recorder.clone()),
    )
    .run(&Inputs::new(), limits())
    .await
    .unwrap();
    let saved = recorder.snapshot().unwrap();
    let bytes = saved.to_json(100_000).unwrap();
    let request = saved.exchanges[0].request.clone();
    let mut seen = Vec::new();
    let streamed = ReplayBackend::from_json(&bytes, 100_000, |exchange| {
        seen.push(exchange.request.clone());
        Ok(())
    })
    .unwrap();
    let typed =
        ReplayBackend::new(&Recording::from_json(&bytes, 100_000).unwrap(), 100_000).unwrap();
    assert_eq!(seen, vec![request.clone()]);
    assert_eq!(
        streamed.infer(&request).await.unwrap(),
        typed.infer(&request).await.unwrap()
    );
    assert_eq!(
        streamed.infer(&request).await.unwrap(),
        saved.exchanges[0].response
    );
    let mut other = request.clone();
    other.model.push_str("-changed");
    assert_eq!(
        streamed.infer(&other).await.unwrap_err().code,
        ErrorCode::ReplayMiss
    );
    // An empty recording loads, and a callback refusal ends the load with its own code.
    ReplayBackend::from_json(br#"{"exchanges":[]}"#, 100, |_| Ok(())).unwrap();
    // A check validates without indexing: a conflict is the index's refusal, not the check's.
    ReplayBackend::check_json(&bytes, 100_000).unwrap();
    assert_eq!(
        ReplayBackend::check_json(br#"{"exchanges":[{"x":1}]}"#, 100)
            .unwrap_err()
            .code,
        ErrorCode::RecordingMismatch
    );
    let refused = ReplayBackend::from_json(&bytes, 100_000, |_| {
        Err(SaphoError::new(ErrorCode::UnknownBackend, "stop"))
    });
    assert_eq!(refused.err().unwrap().code, ErrorCode::UnknownBackend);

    // Every refusal of the typed path is a refusal of the streamed path with the same code.
    let mut conflict = saved.clone();
    let mut changed = saved.exchanges[0].clone();
    changed.response.answers.insert(
        "q".into(),
        Answer::Boolean {
            probability: probability(0.2),
        },
    );
    conflict.exchanges.push(changed);
    let mut invalid = saved.clone();
    invalid.exchanges[0].response.answers.clear();
    let mut identical = saved.clone();
    identical.exchanges.push(saved.exchanges[0].clone());
    ReplayBackend::from_json(&identical.to_json(100_000).unwrap(), 100_000, |_| Ok(())).unwrap();
    let text = |value: &serde_json::Value| serde_json::to_vec(value).unwrap();
    let document = serde_json::to_value(&saved).unwrap();
    let mut unknown = document.clone();
    unknown["surprise"] = serde_json::json!(1);
    let mut member = document.clone();
    member["exchanges"][0]["surprise"] = serde_json::json!(1);
    let cases: Vec<(Vec<u8>, ErrorCode)> = vec![
        (
            br#"{"exchanges":[],"surprise":[]}"#.to_vec(),
            ErrorCode::RecordingMismatch,
        ),
        (
            br#"{"surprise":{"a":[1,2]},"exchanges":[]}"#.to_vec(),
            ErrorCode::RecordingMismatch,
        ),
        (
            serde_json::to_vec(&conflict).unwrap(),
            ErrorCode::RecordingMismatch,
        ),
        (
            serde_json::to_vec(&invalid).unwrap(),
            ErrorCode::RecordingMismatch,
        ),
        (text(&unknown), ErrorCode::RecordingMismatch),
        (text(&member), ErrorCode::RecordingMismatch),
        (b"{}".to_vec(), ErrorCode::RecordingMismatch),
        (b"[]".to_vec(), ErrorCode::RecordingMismatch),
        (b"not json".to_vec(), ErrorCode::RecordingMismatch),
        (
            [bytes.clone(), b" 1".to_vec()].concat(),
            ErrorCode::RecordingMismatch,
        ),
        (
            br#"{"exchanges":[],"exchanges":[]}"#.to_vec(),
            ErrorCode::RecordingMismatch,
        ),
        (
            br#"{"exchanges":{"a":1}}"#.to_vec(),
            ErrorCode::RecordingMismatch,
        ),
    ];
    for (document, code) in cases {
        let typed =
            Recording::from_json(&document, 100_000).and_then(|r| ReplayBackend::new(&r, 100_000));
        let streamed = ReplayBackend::from_json(&document, 100_000, |_| Ok(()));
        assert_eq!(
            typed.err().unwrap().code,
            code,
            "{}",
            String::from_utf8_lossy(&document)
        );
        assert_eq!(
            streamed.err().unwrap().code,
            code,
            "{}",
            String::from_utf8_lossy(&document)
        );
    }
    ReplayBackend::from_json(&bytes, bytes.len(), |_| Ok(())).unwrap();
    assert_eq!(
        ReplayBackend::from_json(&bytes, bytes.len() - 1, |_| Ok(()))
            .err()
            .unwrap()
            .code,
        ErrorCode::LimitExceeded
    );
}
/// Trace: FR-028-AC-2
#[tokio::test]
async fn exact_matching_includes_option_order_model_state_and_backend() {
    let request = ModelRequest {
        distribution_policy: sapho::core::DistributionPolicy::Strict {},
        backend: BackendId::new("judge").unwrap(),
        model: "model-1".into(),
        expected_model: None,
        state: record([("text", Value::Text("original".into()))]),
        questions: vec![NamedQuestion {
            id: "q".into(),
            question: Question::Choice {
                instructions: "Pick the described class".into(),
                options: vec![
                    ChoiceOption {
                        label: "left".into(),
                        description: "left".into(),
                    },
                    ChoiceOption {
                        label: "right".into(),
                        description: "right".into(),
                    },
                ],
            },
        }],
    };
    let response = ModelResponse {
        model: "model-1".into(),
        digest: None,
        raw: None,
        answers: BTreeMap::from([(
            "q".into(),
            Answer::Choice {
                selected: "left".into(),
                confidence: probability(0.8),
                probabilities: Some(BTreeMap::from([
                    ("left".into(), probability(0.8)),
                    ("right".into(), probability(0.2)),
                ])),
            },
        )]),
        usage: None,
    };
    let recording = Recording {
        exchanges: vec![Exchange {
            request: request.clone(),
            response: response.clone(),
        }],
    };
    let replay = ReplayBackend::new(&recording, 100_000).unwrap();
    assert_eq!(replay.infer(&request).await.unwrap(), response);
    let mut variants = Vec::new();
    let mut r = request.clone();
    if let Question::Choice { options, .. } = &mut r.questions[0].question {
        options.reverse();
    }
    variants.push(r);
    let mut r = request.clone();
    r.model = "other".into();
    variants.push(r);
    let mut r = request.clone();
    r.backend = BackendId::new("other").unwrap();
    variants.push(r);
    let mut r = request;
    r.state = record([("text", Value::Text("changed".into()))]);
    variants.push(r);
    for r in variants {
        assert_eq!(
            replay.infer(&r).await.unwrap_err().code,
            ErrorCode::ReplayMiss
        );
    }
}
struct Failing;
#[async_trait::async_trait]
impl ModelBackend for Failing {
    async fn infer(&self, _: &ModelRequest) -> Result<ModelResponse> {
        Err(SaphoError::new(ErrorCode::BackendFailed, "failed backend"))
    }
}
/// Trace: FR-027-AC-2
#[tokio::test]
async fn failed_calls_never_become_successful_saved_exchanges() {
    let recorder = Arc::new(RecordingBackend::new(Arc::new(Failing), 100_000).unwrap());
    let error = engine(
        &ask_graph(None),
        &PrimitiveRegistry::default(),
        bindings(recorder.clone()),
    )
    .run(&Inputs::new(), limits())
    .await
    .unwrap_err();
    assert_eq!(error.error.code, ErrorCode::BackendFailed);
    assert!(recorder.snapshot().unwrap().exchanges.is_empty());
    let too_small = Arc::new(
        RecordingBackend::new(
            Arc::new(Scripted {
                calls: AtomicUsize::new(0),
                answer: 0.8,
            }),
            1,
        )
        .unwrap(),
    );
    assert_eq!(
        engine(
            &ask_graph(None),
            &PrimitiveRegistry::default(),
            bindings(too_small.clone())
        )
        .run(&Inputs::new(), limits())
        .await
        .unwrap_err()
        .error
        .code,
        ErrorCode::LimitExceeded
    );
    assert!(too_small.snapshot().unwrap().exchanges.is_empty());
}

/// Trace: FR-005-AC-4, FR-020-AC-4, FR-027-AC-1, FR-028-AC-4, FR-010-AC-3
#[tokio::test]
async fn approximate_graph_preserves_raw_trace_and_recording_and_replay_policy_identity() {
    struct Rounded {
        total: f64,
    }
    #[async_trait::async_trait]
    impl ModelBackend for Rounded {
        async fn infer(&self, request: &ModelRequest) -> Result<ModelResponse> {
            Ok(ModelResponse {
                model: request.model.clone(),
                digest: None,
                raw: None,
                usage: None,
                answers: BTreeMap::from([(
                    "q".into(),
                    Answer::Choice {
                        selected: "z".into(),
                        confidence: probability(0.43),
                        probabilities: Some(BTreeMap::from([
                            ("z".into(), probability(0.6)),
                            ("a".into(), probability(self.total - 0.6)),
                        ])),
                    },
                )]),
            })
        }
    }
    fn configured(backend: Arc<dyn ModelBackend>, policy: DistributionPolicy) -> BackendRegistry {
        let mut registry = BackendRegistry::default();
        registry
            .register(
                BackendId::new("judge").unwrap(),
                BackendBinding {
                    backend,
                    model: "model-1".into(),
                    expected_model: None,
                    distribution_policy: policy,
                },
            )
            .unwrap();
        registry
    }
    let mut spec = ask_graph(None);
    spec.nodes[0].operation = Operation::Questions {
        questions: vec![NamedQuestion {
            id: "q".into(),
            question: Question::Choice {
                instructions: "Choose the synthetic class".into(),
                options: vec![
                    ChoiceOption {
                        label: "z".into(),
                        description: "first class".into(),
                    },
                    ChoiceOption {
                        label: "a".into(),
                        description: "second class".into(),
                    },
                ],
            },
        }],
    };
    spec.nodes.push(node(
        "mass",
        Operation::Probability {
            question: "q".into(),
            labels: vec!["z".into(), "a".into()],
        },
        [("answers", output("ask", "answers"))],
    ));
    spec.outputs.insert("mass".into(), output("mass", "result"));
    for total in [0.99, 1.01] {
        let policy = DistributionPolicy::approximate(0.01).unwrap();
        let live = Arc::new(Rounded { total });
        let recorder = Arc::new(RecordingBackend::new(live.clone(), 1_000_000).unwrap());
        let result = engine(
            &spec,
            &PrimitiveRegistry::default(),
            configured(recorder.clone(), policy),
        )
        .run(&Inputs::new(), limits())
        .await
        .unwrap();
        assert!(
            (match result.outputs["mass"].value {
                Value::Probability(p) => p.get(),
                _ => panic!("probability"),
            } - 1.0)
                .abs()
                < 1e-12
        );
        let Value::Answers(answers) = &result.outputs["result"].value else {
            panic!("answers")
        };
        assert_eq!(answers.distribution_policy, policy);
        assert_eq!(
            answers.distribution_state("q").unwrap(),
            DistributionState::Approximate
        );
        assert!(
            (answers
                .distribution_adjustment("q")
                .unwrap()
                .unwrap()
                .raw_mass
                - total)
                .abs()
                < 1e-12
        );
        let evidence = result
            .trace
            .nodes
            .iter()
            .find_map(|n| n.model.as_ref())
            .unwrap();
        assert_eq!(evidence.request.distribution_policy, policy);
        assert_eq!(evidence.response.as_ref().unwrap().answers, answers.values);
        let saved = recorder.snapshot().unwrap();
        assert_eq!(saved.exchanges.len(), 1);
        assert_eq!(
            &saved.exchanges[0].response,
            evidence.response.as_ref().unwrap()
        );
        let loaded = Recording::from_json(&saved.to_json(1_000_000).unwrap(), 1_000_000).unwrap();
        assert_eq!(loaded, saved);
        let replay = Arc::new(ReplayBackend::new(&loaded, 1_000_000).unwrap());
        let replayed = engine(
            &spec,
            &PrimitiveRegistry::default(),
            configured(replay.clone(), policy),
        )
        .run(&Inputs::new(), limits())
        .await
        .unwrap();
        assert_eq!(replayed.outputs, result.outputs);
        assert_eq!(
            serde_json::to_value(&replayed.trace).unwrap(),
            serde_json::to_value(&result.trace).unwrap()
        );
        let miss = engine(
            &spec,
            &PrimitiveRegistry::default(),
            configured(replay, DistributionPolicy::Strict {}),
        )
        .run(&Inputs::new(), limits())
        .await
        .unwrap_err();
        assert_eq!(miss.error.code, ErrorCode::ReplayMiss);
        let refused = engine(
            &spec,
            &PrimitiveRegistry::default(),
            configured(live, DistributionPolicy::Strict {}),
        )
        .run(&Inputs::new(), limits())
        .await
        .unwrap_err();
        assert_eq!(refused.error.code, ErrorCode::InvalidAnswer);
        assert_eq!(refused.error.context["question"], "q");
        let raw = refused
            .trace
            .nodes
            .iter()
            .find_map(|n| n.model.as_ref())
            .unwrap()
            .response
            .as_ref()
            .unwrap();
        assert_eq!(raw.answers, answers.values);
        let bad_recorder =
            Arc::new(RecordingBackend::new(Arc::new(Rounded { total: 0.98 }), 1_000_000).unwrap());
        let bad = engine(
            &spec,
            &PrimitiveRegistry::default(),
            configured(bad_recorder.clone(), policy),
        )
        .run(&Inputs::new(), limits())
        .await
        .unwrap_err();
        assert_eq!(bad.error.code, ErrorCode::InvalidAnswer);
        assert!(bad_recorder.snapshot().unwrap().exchanges.is_empty());
    }
}
