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
/// Trace: FR-028-AC-2
#[tokio::test]
async fn exact_matching_includes_option_order_model_state_and_backend() {
    let request = ModelRequest {
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
