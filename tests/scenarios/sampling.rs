// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Indexed repeated-Ask capture, replay and typed runtime evidence.
use crate::common::*;
use async_trait::async_trait;
use sapho::{core::*, graph::*, recording::*, runtime::RunLimits};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

struct Indexed {
    probabilities: Vec<f64>,
    models: Vec<&'static str>,
    requests: Mutex<Vec<ModelRequest>>,
}
#[async_trait]
impl ModelBackend for Indexed {
    async fn infer(&self, request: &ModelRequest) -> Result<ModelResponse> {
        self.requests.lock().unwrap().push(request.clone());
        let index = usize::try_from(request.sample_index.unwrap_or(0)).unwrap();
        let probability = *self
            .probabilities
            .get(index)
            .ok_or_else(|| SaphoError::new(ErrorCode::BackendFailed, "Sample absent"))?;
        Ok(ModelResponse {
            model: self.models[index].into(),
            raw: None,
            usage: None,
            answers: BTreeMap::from([(
                "q".into(),
                Answer::Boolean {
                    probability: Probability::new(probability)?,
                },
            )]),
        })
    }
}
fn varying_model_binding(backend: Arc<dyn ModelBackend>) -> BackendRegistry {
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
    registry
}
fn sampled_graph(count: u64) -> GraphSpec {
    let mut spec = ask_graph(None);
    spec.nodes
        .iter_mut()
        .find(|node| node.id.as_str() == "ask")
        .unwrap()
        .operation = Operation::Ask {
        backend: BackendId::new("judge").unwrap(),
        samples: count,
    };
    if count > 1 {
        spec.outputs
            .insert("models".into(), output("ask", "models"));
        spec.outputs
            .insert("disagreement".into(), output("ask", "disagreement"));
    }
    spec
}

struct Scripted {
    seen: Mutex<Vec<ModelRequest>>,
    active: AtomicUsize,
    maximum: AtomicUsize,
    reverse_failures: bool,
    stall_second: bool,
}
#[async_trait]
impl ModelBackend for Scripted {
    async fn infer(&self, request: &ModelRequest) -> Result<ModelResponse> {
        let current = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.maximum.fetch_max(current, Ordering::SeqCst);
        self.seen.lock().unwrap().push(request.clone());
        let index = request.sample_index.unwrap_or(0);
        let delay = if self.stall_second && index == 1 {
            200
        } else if self.reverse_failures && index == 0 {
            30
        } else {
            2
        };
        tokio::time::sleep(Duration::from_millis(delay)).await;
        self.active.fetch_sub(1, Ordering::SeqCst);
        if self.reverse_failures && index < 2 {
            return Err(SaphoError::new(
                if index == 0 {
                    ErrorCode::BackendFailed
                } else {
                    ErrorCode::InvalidValue
                },
                "Scripted failure",
            ));
        }
        if self.stall_second && index == 0 {
            return Err(SaphoError::new(ErrorCode::BackendFailed, "First failed"));
        }
        Ok(ModelResponse {
            model: "model-1".into(),
            raw: None,
            usage: None,
            answers: BTreeMap::from([(
                "q".into(),
                Answer::Boolean {
                    probability: Probability::new(0.8)?,
                },
            )]),
        })
    }
}
fn scripted(reverse_failures: bool, stall_second: bool) -> Arc<Scripted> {
    Arc::new(Scripted {
        seen: Mutex::new(Vec::new()),
        active: AtomicUsize::new(0),
        maximum: AtomicUsize::new(0),
        reverse_failures,
        stall_second,
    })
}

/// Trace: FR-074-AC-1, FR-074-AC-2, FR-075-AC-1, FR-075-AC-3, FR-075-AC-4, FR-076-AC-1, IT-014-SC-01, IT-014-SC-02, IT-014-SC-04, IT-014-SC-07
#[tokio::test]
async fn three_identical_content_requests_keep_distinct_indexed_recordings_and_lists() {
    let live = Arc::new(Indexed {
        probabilities: vec![0.8, 0.2, 0.9],
        models: vec!["model-a", "model-b", "model-c"],
        requests: Mutex::new(Vec::new()),
    });
    let recorder = Arc::new(RecordingBackend::new(live.clone(), 1_000_000).unwrap());
    let mut spec = sampled_graph(3);
    let source = SourceRef {
        source: SourceId::new("synthetic").unwrap(),
        start: Some(0),
        end: Some(3),
    };
    let ask = spec
        .nodes
        .iter_mut()
        .find(|node| node.id.as_str() == "ask")
        .unwrap();
    let Binding::Literal { value, .. } = ask.inputs.get_mut("state").unwrap() else {
        panic!("literal state")
    };
    value.sources.push(source.clone());
    let result = engine(
        &spec,
        &PrimitiveRegistry::default(),
        varying_model_binding(recorder.clone()),
    )
    .run(&Inputs::new(), limits())
    .await
    .unwrap();
    let Value::List(answers) = &result.outputs["result"].value else {
        panic!("answer list")
    };
    let Value::List(models) = &result.outputs["models"].value else {
        panic!("model list")
    };
    assert_eq!(
        answers.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(),
        ["0", "1", "2"]
    );
    assert_eq!(
        models.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(),
        ["0", "1", "2"]
    );
    let answer_by_id = answers
        .iter()
        .map(|datum| (datum.id.as_str(), &datum.value))
        .collect::<BTreeMap<_, _>>();
    let paired = models
        .iter()
        .map(|datum| {
            (
                datum.id.as_str(),
                &datum.value,
                answer_by_id[datum.id.as_str()],
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        paired
            .iter()
            .map(|(id, model, _)| (*id, *model))
            .collect::<Vec<_>>(),
        [
            ("0", &Value::Text("model-a".into())),
            ("1", &Value::Text("model-b".into())),
            ("2", &Value::Text("model-c".into())),
        ]
    );
    for (index, (_, _, answer)) in paired.iter().enumerate() {
        let Value::Answers(by_question) = answer else {
            panic!("sample answer")
        };
        let Answer::Boolean { probability } = by_question.values["q"] else {
            panic!("boolean sample")
        };
        assert_eq!(probability.get(), [0.8, 0.2, 0.9][index]);
    }
    assert_eq!(
        result.outputs["disagreement"].value,
        Value::Degree(Degree::new(1.0 / 3.0).unwrap())
    );
    assert!(
        answers
            .iter()
            .all(|datum| datum.sources == [source.clone()])
    );
    assert_eq!(
        answers
            .iter()
            .map(|d| d.sources.clone())
            .collect::<Vec<_>>(),
        models.iter().map(|d| d.sources.clone()).collect::<Vec<_>>()
    );
    let typed = serde_json::to_vec(&result.outputs).unwrap();
    assert_eq!(
        serde_json::from_slice::<Inputs>(&typed).unwrap(),
        result.outputs
    );
    {
        let requests = live.requests.lock().unwrap();
        assert_eq!(
            requests
                .iter()
                .map(|request| request.sample_index)
                .collect::<Vec<_>>(),
            [Some(0), Some(1), Some(2)]
        );
        assert!(
            requests.windows(2).all(
                |pair| pair[0].state == pair[1].state && pair[0].questions == pair[1].questions
            )
        );
    }
    let recording = recorder.snapshot().unwrap();
    assert_eq!(recording.exchanges.len(), 3);
    assert_eq!(
        recording
            .exchanges
            .iter()
            .map(|e| e.request.sample_index)
            .collect::<Vec<_>>(),
        [Some(0), Some(1), Some(2)]
    );
    let loaded = Recording::from_json(&recording.to_json(1_000_000).unwrap(), 1_000_000).unwrap();
    let replay = Arc::new(ReplayBackend::new(&loaded, 1_000_000).unwrap());
    let replayed = engine(
        &spec,
        &PrimitiveRegistry::default(),
        varying_model_binding(replay),
    )
    .run(&Inputs::new(), limits())
    .await
    .unwrap();
    assert_eq!(replayed.outputs, result.outputs);
    let mut conflict = loaded.clone();
    let mut changed = conflict.exchanges[1].clone();
    changed.response.answers.insert(
        "q".into(),
        Answer::Boolean {
            probability: Probability::new(0.6).unwrap(),
        },
    );
    conflict.exchanges.push(changed);
    assert_eq!(
        ReplayBackend::new(&conflict, 1_000_000).err().unwrap().code,
        ErrorCode::RecordingMismatch
    );
}

/// Trace: FR-074-AC-3, FR-075-AC-2, FR-076-AC-5, IT-014-SC-01, IT-014-SC-06
#[tokio::test]
async fn one_sample_keeps_legacy_ports_request_and_trace_json() {
    let omitted = ask_graph(None);
    let explicit = sampled_graph(1);
    assert_eq!(
        serde_json::to_vec(&omitted).unwrap(),
        serde_json::to_vec(&explicit).unwrap()
    );
    let live = Arc::new(Indexed {
        probabilities: vec![0.8],
        models: vec!["model-1"],
        requests: Mutex::new(Vec::new()),
    });
    let recorder = Arc::new(RecordingBackend::new(live, 1_000_000).unwrap());
    let result = engine(
        &explicit,
        &PrimitiveRegistry::default(),
        bindings(recorder.clone()),
    )
    .run(&Inputs::new(), limits())
    .await
    .unwrap();
    assert!(matches!(result.outputs["result"].value, Value::Answers(_)));
    let trace = serde_json::to_string(&result.trace).unwrap();
    // Frozen bytes captured from origin/main e77b81e's one-call Ask, before sampling.
    assert_eq!(
        trace,
        include_str!("../fixtures/sampling-legacy-trace.json").trim_end()
    );
    let recording = recorder.snapshot().unwrap();
    assert_eq!(
        serde_json::to_string(&recording.exchanges[0].request).unwrap(),
        include_str!("../fixtures/sampling-legacy-request.json").trim_end()
    );
    assert_eq!(recording.exchanges[0].request.sample_index, None);
    assert!(
        !String::from_utf8(recording.to_json(1_000_000).unwrap())
            .unwrap()
            .contains("sample_index")
    );
    let replay = Arc::new(ReplayBackend::new(&recording, 1_000_000).unwrap());
    assert_eq!(
        engine(&explicit, &PrimitiveRegistry::default(), bindings(replay))
            .run(&Inputs::new(), limits())
            .await
            .unwrap()
            .outputs,
        result.outputs
    );
}

/// Trace: FR-074-AC-2, FR-076-AC-3, FR-076-AC-4, IT-014-SC-03, IT-014-SC-05
#[tokio::test]
async fn request_budget_refuses_before_dispatch_and_later_replay_index_misses() {
    let live = Arc::new(Indexed {
        probabilities: vec![0.8, 0.2, 0.9],
        models: vec!["model-1"; 3],
        requests: Mutex::new(Vec::new()),
    });
    let recorder = Arc::new(RecordingBackend::new(live.clone(), 1_000_000).unwrap());
    let spec = sampled_graph(3);
    let mut short = limits();
    short.model_requests = 2;
    let fail = engine(
        &spec,
        &PrimitiveRegistry::default(),
        bindings(recorder.clone()),
    )
    .run(&Inputs::new(), short)
    .await
    .unwrap_err();
    assert_eq!(fail.error.code, ErrorCode::LimitExceeded);
    assert!(live.requests.lock().unwrap().is_empty());
    let _ = engine(
        &spec,
        &PrimitiveRegistry::default(),
        bindings(recorder.clone()),
    )
    .run(
        &Inputs::new(),
        RunLimits {
            model_requests: 3,
            ..limits()
        },
    )
    .await
    .unwrap();
    let replay = Arc::new(ReplayBackend::new(&recorder.snapshot().unwrap(), 1_000_000).unwrap());
    let mut wider = limits();
    wider.concurrency = 2;
    let fail = engine(
        &sampled_graph(4),
        &PrimitiveRegistry::default(),
        bindings(replay),
    )
    .run(&Inputs::new(), wider)
    .await
    .unwrap_err();
    assert_eq!(fail.error.code, ErrorCode::ReplayMiss);
    let samples = fail
        .trace
        .nodes
        .iter()
        .find(|node| node.path.last().is_some_and(|name| name == "ask"))
        .unwrap()
        .samples
        .as_ref()
        .unwrap();
    assert_eq!(
        samples.iter().map(|item| item.index).collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    assert_eq!(
        samples[3].error.as_ref().unwrap().code,
        ErrorCode::ReplayMiss
    );
    let one_at_a_time =
        Arc::new(ReplayBackend::new(&recorder.snapshot().unwrap(), 1_000_000).unwrap());
    let mut narrow = limits();
    narrow.concurrency = 1;
    let narrow_fail = engine(
        &sampled_graph(4),
        &PrimitiveRegistry::default(),
        bindings(one_at_a_time),
    )
    .run(&Inputs::new(), narrow)
    .await
    .unwrap_err();
    assert_eq!(narrow_fail.error.code, ErrorCode::ReplayMiss);
    assert_eq!(
        narrow_fail
            .trace
            .nodes
            .iter()
            .find_map(|node| node.samples.as_ref())
            .unwrap()
            .iter()
            .map(|item| item.index)
            .collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
}

/// Trace: FR-075-AC-1, FR-075-AC-2, IT-014-SC-01
#[test]
fn repeated_ask_types_and_count_schema_are_checked_in_yaml_and_json() {
    let repeated = sampled_graph(3);
    let json = serde_json::to_string(&repeated).unwrap();
    let yaml = "inputs: {}\nnodes:\n  - id: q\n    operation:\n      kind: questions\n      questions:\n        - id: q\n          question: {kind: boolean, instructions: True?, yes: Yes, no: No}\n  - id: ask\n    operation: {kind: ask, backend: judge, samples: 3}\n    inputs:\n      state: {kind: literal, value: {id: literal, value: {kind: record, value: {}}, sources: []}, value_type: {kind: record, fields: {}}}\n      questions: {kind: node, node: q, port: result}\noutputs: {result: {kind: node, node: ask, port: answers}}\n";
    let from_json: GraphSpec = serde_json::from_str(&json).unwrap();
    let from_yaml = GraphSpec::parse(yaml).unwrap();
    for spec in [&from_json, &from_yaml] {
        let compiled = compile(spec, &PrimitiveRegistry::default()).unwrap();
        assert_eq!(
            compiled.signature().outputs["result"],
            ValueType::list(ValueType::Answers)
        );
    }
    let mut singular = repeated.clone();
    singular
        .outputs
        .insert("result".into(), output("ask", "model"));
    assert!(compile(&singular, &PrimitiveRegistry::default()).is_err());
    let mut zero = repeated.clone();
    zero.nodes
        .iter_mut()
        .find(|node| node.id.as_str() == "ask")
        .unwrap()
        .operation = Operation::Ask {
        backend: BackendId::new("judge").unwrap(),
        samples: 0,
    };
    assert!(compile(&zero, &PrimitiveRegistry::default()).is_err());
    let mut huge = repeated.clone();
    huge.nodes
        .iter_mut()
        .find(|node| node.id.as_str() == "ask")
        .unwrap()
        .operation = Operation::Ask {
        backend: BackendId::new("judge").unwrap(),
        samples: u64::from(u32::MAX) + 2,
    };
    assert!(compile(&huge, &PrimitiveRegistry::default()).is_err());
    for bad in ["1.5", "18446744073709551616", "\"3\""] {
        let malformed = json.replace("\"samples\":3", &format!("\"samples\":{bad}"));
        assert!(
            serde_json::from_str::<GraphSpec>(&malformed).is_err(),
            "{bad}"
        );
    }
    let unknown = json.replace("\"samples\":3", "\"samples\":3,\"seed\":9");
    assert!(serde_json::from_str::<GraphSpec>(&unknown).is_err());
}

/// Trace: FR-076-AC-6, IT-014-SC-05
#[tokio::test]
async fn reverse_completion_failures_keep_lowest_index_error_and_stop_later_waves() {
    let backend = scripted(true, false);
    let mut limit = limits();
    limit.concurrency = 2;
    let fail = engine(
        &sampled_graph(4),
        &PrimitiveRegistry::default(),
        bindings(backend.clone()),
    )
    .run(&Inputs::new(), limit)
    .await
    .unwrap_err();
    assert_eq!(fail.error.code, ErrorCode::BackendFailed);
    let samples = fail
        .trace
        .nodes
        .iter()
        .find_map(|node| node.samples.as_ref())
        .unwrap();
    assert_eq!(samples.iter().map(|s| s.index).collect::<Vec<_>>(), [0, 1]);
    assert_eq!(
        samples[0].error.as_ref().unwrap().code,
        ErrorCode::BackendFailed
    );
    assert_eq!(
        samples[1].error.as_ref().unwrap().code,
        ErrorCode::InvalidValue
    );
    assert_eq!(backend.seen.lock().unwrap().len(), 2);
}

/// Trace: FR-076-AC-6, IT-014-SC-05
#[tokio::test]
async fn failed_sample_waits_for_timed_out_peer_and_retains_both() {
    let backend = scripted(false, true);
    let mut limit = limits();
    limit.concurrency = 2;
    limit.duration = Duration::from_millis(40);
    let fail = engine(
        &sampled_graph(4),
        &PrimitiveRegistry::default(),
        bindings(backend.clone()),
    )
    .run(&Inputs::new(), limit)
    .await
    .unwrap_err();
    assert_eq!(fail.error.code, ErrorCode::BackendFailed);
    let samples = fail
        .trace
        .nodes
        .iter()
        .find_map(|node| node.samples.as_ref())
        .unwrap();
    assert_eq!(samples.iter().map(|s| s.index).collect::<Vec<_>>(), [0, 1]);
    assert_eq!(
        samples[1].error.as_ref().unwrap().code,
        ErrorCode::DeadlineExceeded
    );
    assert_eq!(backend.seen.lock().unwrap().len(), 2);
}

fn sibling_graph() -> GraphSpec {
    let mut graph = sampled_graph(3);
    let mut second = graph
        .nodes
        .iter()
        .find(|node| node.id.as_str() == "ask")
        .unwrap()
        .clone();
    second.id = NodeId::new("ask2").unwrap();
    second.operation = Operation::Ask {
        backend: BackendId::new("judge2").unwrap(),
        samples: 3,
    };
    graph.nodes.push(second);
    graph
        .outputs
        .insert("second".into(), output("ask2", "answers"));
    graph
}
fn sibling_bindings(backend: Arc<dyn ModelBackend>) -> BackendRegistry {
    let mut registry = bindings(backend.clone());
    registry
        .register(
            BackendId::new("judge2").unwrap(),
            BackendBinding {
                backend,
                model: "model-1".into(),
                expected_model: Some("model-1".into()),
                distribution_policy: DistributionPolicy::Strict {},
            },
        )
        .unwrap();
    registry
}

struct FailFirstOnce {
    failed: AtomicBool,
    seen: Mutex<Vec<ModelRequest>>,
}

#[async_trait]
impl ModelBackend for FailFirstOnce {
    async fn infer(&self, request: &ModelRequest) -> Result<ModelResponse> {
        self.seen.lock().unwrap().push(request.clone());
        if request.backend.as_str() == "judge"
            && request.sample_index == Some(0)
            && !self.failed.swap(true, Ordering::SeqCst)
        {
            return Err(SaphoError::new(
                ErrorCode::BackendFailed,
                "First Ask failed",
            ));
        }
        Ok(ModelResponse {
            model: "model-1".into(),
            raw: None,
            usage: None,
            answers: BTreeMap::from([(
                "q".into(),
                Answer::Boolean {
                    probability: Probability::new(0.8)?,
                },
            )]),
        })
    }
}

/// Trace: FR-076-AC-7, IT-014-SC-08
#[tokio::test]
async fn first_reserved_ask_failure_releases_undispatched_sibling_reservation() {
    let backend = Arc::new(FailFirstOnce {
        failed: AtomicBool::new(false),
        seen: Mutex::new(Vec::new()),
    });
    let mut limit = limits();
    limit.concurrency = 2;
    limit.model_requests = 6;
    let fail = engine(
        &sibling_graph(),
        &PrimitiveRegistry::default(),
        sibling_bindings(backend.clone()),
    )
    .run(&Inputs::new(), limit.clone())
    .await
    .unwrap_err();
    assert_eq!(fail.error.code, ErrorCode::BackendFailed);
    assert_eq!(
        fail.trace
            .nodes
            .iter()
            .find(|node| node.path.last().is_some_and(|id| id == "ask2"))
            .unwrap()
            .samples
            .as_ref()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(backend.seen.lock().unwrap().len(), 2);
    assert!(
        backend
            .seen
            .lock()
            .unwrap()
            .iter()
            .all(|request| request.backend.as_str() == "judge")
    );

    let result = engine(
        &sibling_graph(),
        &PrimitiveRegistry::default(),
        sibling_bindings(backend.clone()),
    )
    .run(&Inputs::new(), limit)
    .await
    .unwrap();
    assert!(matches!(result.outputs["second"].value, Value::List(_)));
    let seen = backend.seen.lock().unwrap();
    assert_eq!(seen.len(), 8);
    assert_eq!(
        seen[2..]
            .iter()
            .filter(|request| request.backend.as_str() == "judge2")
            .count(),
        3
    );
}

/// Trace: FR-076-AC-3, FR-076-AC-7, IT-014-SC-08
#[tokio::test]
async fn sibling_asks_share_one_admission_ledger_and_model_permits() {
    let backend = scripted(false, false);
    let mut limit = limits();
    limit.concurrency = 2;
    limit.model_requests = 6;
    let result = engine(
        &sibling_graph(),
        &PrimitiveRegistry::default(),
        sibling_bindings(backend.clone()),
    )
    .run(&Inputs::new(), limit.clone())
    .await
    .unwrap();
    assert_eq!(backend.seen.lock().unwrap().len(), 6);
    assert!(backend.maximum.load(Ordering::SeqCst) <= 2);
    assert!(matches!(result.outputs["result"].value, Value::List(_)));
    assert!(matches!(result.outputs["second"].value, Value::List(_)));
    let mut identities = backend
        .seen
        .lock()
        .unwrap()
        .iter()
        .map(|r| (r.backend.as_str().to_owned(), r.sample_index.unwrap()))
        .collect::<Vec<_>>();
    identities.sort();
    identities.dedup();
    assert_eq!(identities.len(), 6);

    let backend = scripted(false, false);
    limit.model_requests = 4;
    let fail = engine(
        &sibling_graph(),
        &PrimitiveRegistry::default(),
        sibling_bindings(backend.clone()),
    )
    .run(&Inputs::new(), limit)
    .await
    .unwrap_err();
    assert_eq!(fail.error.code, ErrorCode::LimitExceeded);
    assert!(
        backend
            .seen
            .lock()
            .unwrap()
            .iter()
            .all(|r| r.backend.as_str() != "judge2")
    );
    assert!(backend.seen.lock().unwrap().len() <= 3);
}

/// Trace: FR-076-AC-3, FR-076-AC-7, IT-014-SC-05, IT-014-SC-08
#[tokio::test]
async fn mapped_repeated_asks_charge_the_same_run_ledger_as_outer_ask() {
    let mut graph = sampled_graph(3);
    let child = GraphBody {
        inputs: BTreeMap::from([("item".into(), record_type([]))]),
        nodes: vec![
            node(
                "q",
                Operation::Questions {
                    questions: vec![question()],
                },
                [],
            ),
            node(
                "ask",
                Operation::Ask {
                    backend: BackendId::new("judge2").unwrap(),
                    samples: 3,
                },
                [
                    ("state", input("item")),
                    ("questions", output("q", "result")),
                ],
            ),
        ],
        outputs: BTreeMap::from([("result".into(), output("ask", "answers"))]),
    };
    graph.subgraphs.insert("child".into(), child);
    graph.nodes.push(node(
        "map",
        Operation::Map {
            graph: "child".into(),
        },
        [(
            "items",
            lit(
                list([("a", record([])), ("b", record([]))]),
                ValueType::list(record_type([])),
            ),
        )],
    ));
    graph
        .outputs
        .insert("mapped".into(), output("map", "result"));
    let backend = scripted(false, false);
    let mut limit = limits();
    limit.concurrency = 2;
    limit.model_requests = 9;
    let result = engine(
        &graph,
        &PrimitiveRegistry::default(),
        sibling_bindings(backend.clone()),
    )
    .run(&Inputs::new(), limit.clone())
    .await
    .unwrap();
    assert!(matches!(result.outputs["mapped"].value, Value::List(_)));
    assert_eq!(backend.seen.lock().unwrap().len(), 9);
    assert!(backend.maximum.load(Ordering::SeqCst) <= 2);
    let backend = scripted(false, false);
    limit.model_requests = 8;
    let fail = engine(
        &graph,
        &PrimitiveRegistry::default(),
        sibling_bindings(backend.clone()),
    )
    .run(&Inputs::new(), limit)
    .await
    .unwrap_err();
    assert_eq!(fail.error.code, ErrorCode::LimitExceeded);
    assert!(backend.seen.lock().unwrap().len() <= 8);
}
