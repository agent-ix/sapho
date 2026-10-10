// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Production roster command with synthetic loopback inference and offline replay.
#![cfg(feature = "clm")]
use sapho_core::{
    Answer, BackendBinding, BackendId, BackendRegistry, Datum, DistributionPolicy, Inputs, ItemId,
    ModelBackend, ModelIdentity, ModelRequest, ModelResponse, PrimitiveRegistry, Probability,
    ProviderDescriptor, SourceId, Value,
};
use sapho_evidence::{
    Case, CaseOutcome, Dataset, LabelKind, LabelProvenance, Roster, RosterCall, RosterCase,
    RosterContributor, RosterMapping, RosterMode, RosterQuestionKind, Split, roster,
};
use sapho_graph::{Binding, Comparator, GraphSpec, NodeSpec, Operation};
use sapho_recording::Recording;
use sapho_runtime::{
    NodeStatus, ObservationConfig, ObservationMode, RunLimits, SystemObservationClock,
};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::TcpListener,
    path::Path,
    process::{Command, Output},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

fn command(args: &[&str], endpoint: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sapho"));
    command
        .args(args)
        .env_clear()
        .env("CLM_API_KEY", "synthetic-credential");
    if let Some(endpoint) = endpoint {
        command.env("CLM_BASE_URL", endpoint);
    }
    command.output().unwrap()
}
fn path(path: &Path) -> &str {
    path.to_str().unwrap()
}
fn graph() -> &'static str {
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
          question: {kind: boolean, instructions: "Does this hold?", yes: Holds, no: Absent}
  - id: alpha_ask
    operation: {kind: ask, backend: alpha}
    inputs:
      state: {kind: node, node: state, port: result}
      questions: {kind: node, node: questions, port: result}
  - id: beta_ask
    operation: {kind: ask, backend: beta}
    inputs:
      state: {kind: node, node: state, port: result}
      questions: {kind: node, node: questions, port: result}
  - id: alpha_probability
    operation: {kind: probability, question: q, labels: ["true"]}
    inputs: {answers: {kind: node, node: alpha_ask, port: answers}}
  - id: beta_probability
    operation: {kind: probability, question: q, labels: ["true"]}
    inputs: {answers: {kind: node, node: beta_ask, port: answers}}
  - id: flag
    operation: {kind: compare, comparator: greater_equal}
    inputs:
      a: {kind: node, node: beta_probability, port: result}
      b:
        kind: literal
        value: {id: threshold, value: {kind: probability, value: 0.5}}
        value_type: {kind: probability}
outputs:
  score: {kind: node, node: alpha_probability, port: result}
  flag: {kind: node, node: flag, port: result}
"#
}
fn data() -> Dataset {
    let mut cases = Vec::new();
    for (id, split, text, label, kind) in [
        ("d1", Split::Development, "first", true, LabelKind::Human),
        ("d2", Split::Development, "second", false, LabelKind::Agent),
        ("h1", Split::HeldOut, "first", true, LabelKind::Human),
        ("h2", Split::HeldOut, "second", false, LabelKind::Agent),
    ] {
        cases.push(Case {
            id: ItemId::new(id).unwrap(),
            split,
            inputs: Inputs::from([(
                "text".into(),
                Datum::new(id, Value::Text(text.into())).unwrap(),
            )]),
            labels: BTreeMap::from([("score".into(), label), ("flag".into(), label)]),
            label_provenance: LabelProvenance {
                kind,
                source: "synthetic-curator".into(),
                reference: "fixture".into(),
            },
        });
    }
    Dataset {
        id: SourceId::new("synthetic-roster").unwrap(),
        cases,
    }
}
fn serve(listener: TcpListener, expected: usize) {
    listener.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    for _ in 0..expected {
        let (mut stream, _) = loop {
            match listener.accept() {
                Ok(pair) => break pair,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(5))
                }
                Err(error) => panic!("loopback accept: {error}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut bytes = Vec::new();
        loop {
            let mut buffer = [0u8; 1024];
            let size = stream.read(&mut buffer).unwrap();
            assert!(size > 0);
            bytes.extend_from_slice(&buffer[..size]);
            if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..index]);
                let length: usize = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|value| value.trim().parse().ok())
                    })
                    .unwrap();
                if bytes.len() >= index + 4 + length {
                    break;
                }
            }
        }
        let request = String::from_utf8(bytes).unwrap();
        assert!(request.starts_with("POST /v1/systemone HTTP/1.1"));
        let wire: serde_json::Value =
            serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
        let requested = wire["model"].as_str().unwrap();
        let first = wire["state"]["text"] == "first";
        let actual = match (requested, first) {
            ("alpha-request", true) => "alpha-v1",
            ("alpha-request", false) => "alpha-v2",
            ("beta-request", _) => "beta-v1",
            _ => panic!("unexpected model"),
        };
        let probability = if first { 0.8 } else { 0.2 };
        let body = serde_json::to_vec(&serde_json::json!({"model": actual, "answers": {"q": {"type":"noul", "noul": probability}}, "usage": {"input_tokens": 10, "output_tokens": 2, "billing_units": 1}})).unwrap();
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .unwrap();
        stream.write_all(&body).unwrap();
    }
}
/// Trace: FR-063-AC-1, FR-063-AC-4, FR-063-AC-5, FR-064-AC-1,
/// FR-064-AC-2, FR-064-AC-3, FR-064-AC-5, FR-064-AC-6,
/// IT-009-SC-01, IT-009-SC-02, IT-009-SC-03, IT-009-SC-04, IT-009-SC-05
#[test]
fn production_roster_live_replay_and_literal_roundtrip() {
    let root = tempfile::tempdir().unwrap();
    let graph_path = root.path().join("graph.yaml");
    std::fs::write(&graph_path, graph()).unwrap();
    let graph_json_path = root.path().join("same-graph.json");
    std::fs::write(
        &graph_json_path,
        serde_json::to_vec(&GraphSpec::parse(graph()).unwrap()).unwrap(),
    )
    .unwrap();
    let yaml_artifact = sapho_cli::load_graph(&graph_path, None).unwrap();
    let json_artifact = sapho_cli::load_graph(&graph_json_path, None).unwrap();
    assert_ne!(yaml_artifact.source, json_artifact.source);
    assert_eq!(
        sapho_graph::graph_semantic_identity(&yaml_artifact.definition).unwrap(),
        sapho_graph::graph_semantic_identity(&json_artifact.definition).unwrap()
    );
    let dataset_path = root.path().join("dataset.json");
    std::fs::write(&dataset_path, serde_json::to_vec(&data()).unwrap()).unwrap();
    let bindings_path = root.path().join("bindings.yaml");
    std::fs::write(&bindings_path, "alpha: {provider: clm, model: alpha-request, distribution_policy: {kind: strict}}\nbeta: {provider: clm, model: beta-request, distribution_policy: {kind: strict}}\n").unwrap();
    let mapping_path = root.path().join("mapping.yaml");
    std::fs::write(&mapping_path, "score: {binding: alpha, question_kind: boolean}\nflag: {binding: beta, question_kind: boolean}\n").unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || serve(listener, 8));
    let roster_path = root.path().join("roster.json");
    let live = command(
        &[
            "roster",
            path(&graph_path),
            "--dataset",
            path(&dataset_path),
            "--bindings",
            path(&bindings_path),
            "--split",
            "development",
            "--mapping",
            path(&mapping_path),
            "--output",
            path(&roster_path),
        ],
        Some(&endpoint),
    );
    assert!(
        live.status.success(),
        "{}",
        String::from_utf8_lossy(&live.stderr)
    );
    let report: Roster = serde_json::from_slice(&std::fs::read(&roster_path).unwrap()).unwrap();
    assert_eq!(report.dataset.as_str(), "synthetic-roster");
    assert_eq!(report.split, Split::Development);
    assert!(
        report
            .graph_semantic_identity
            .starts_with("graph-v1:sha256:")
    );
    assert_eq!(report.entries.len(), 3);
    assert_eq!(
        report.entries.iter().map(|entry| entry.calls).sum::<u32>(),
        4
    );
    assert!(report.entries.iter().all(|entry| entry.latency.count > 0));
    assert!(
        report
            .entries
            .iter()
            .all(|entry| entry.descriptor.as_ref().unwrap().provider.as_deref() == Some("clm"))
    );
    assert!(!String::from_utf8_lossy(&live.stdout).contains(&endpoint));
    let mut recordings = Vec::new();
    for (index, text) in ["first", "second"].into_iter().enumerate() {
        let input = root.path().join(format!("input-{index}.json"));
        std::fs::write(
            &input,
            serde_json::to_vec(&serde_json::json!({"text": text})).unwrap(),
        )
        .unwrap();
        let recording = root.path().join(format!("recording-{index}.json"));
        let captured = command(
            &[
                "record",
                path(&graph_path),
                "--input",
                path(&input),
                "--bindings",
                path(&bindings_path),
                "--recording",
                path(&recording),
            ],
            Some(&endpoint),
        );
        assert!(
            captured.status.success(),
            "{}",
            String::from_utf8_lossy(&captured.stderr)
        );
        recordings.extend(
            Recording::from_json(&std::fs::read(recording).unwrap(), 1_048_576)
                .unwrap()
                .exchanges,
        );
    }
    server.join().unwrap();
    let replay_path = root.path().join("combined-recording.json");
    std::fs::write(
        &replay_path,
        serde_json::to_vec(&Recording {
            exchanges: recordings,
        })
        .unwrap(),
    )
    .unwrap();
    let held_path = root.path().join("held.json");
    let replay = command(
        &[
            "roster",
            path(&graph_path),
            "--dataset",
            path(&dataset_path),
            "--bindings",
            path(&bindings_path),
            "--replay",
            path(&replay_path),
            "--split",
            "held_out",
            "--mapping",
            path(&mapping_path),
            "--output",
            path(&held_path),
        ],
        Some("not-a-url"),
    );
    assert!(
        replay.status.success(),
        "{}",
        String::from_utf8_lossy(&replay.stderr)
    );
    let held: Roster = serde_json::from_slice(&std::fs::read(&held_path).unwrap()).unwrap();
    assert_eq!(held.split, Split::HeldOut);
    assert!(held.entries.iter().all(|entry| entry.latency.count == 0
        && entry.latency.absent_reason.as_deref() == Some("replay_only")));
    let ordinary = command(
        &[
            "measure",
            path(&graph_path),
            "--dataset",
            path(&dataset_path),
            "--bindings",
            path(&bindings_path),
            "--replay",
            path(&replay_path),
            "--split",
            "held_out",
        ],
        Some("not-a-url"),
    );
    assert!(
        ordinary.status.success(),
        "{}",
        String::from_utf8_lossy(&ordinary.stderr)
    );
    let ordinary: serde_json::Value = serde_json::from_slice(&ordinary.stdout).unwrap();
    assert_eq!(
        ordinary["graph"]["source"],
        serde_json::to_value(&yaml_artifact.source).unwrap()
    );
    assert_eq!(
        ordinary["measurement"]["per_kind"]["score"]["human"]["scored"],
        1
    );
    let selected = root.path().join("selection.json");
    std::fs::write(
        &selected,
        serde_json::to_vec(&serde_json::json!([
            {"field":"alpha","binding":"alpha","actual_model":"alpha-v1"},
            {"field":"beta","binding":"beta","actual_model":"beta-v1"}
        ]))
        .unwrap(),
    )
    .unwrap();
    let literal_path = root.path().join("literal.json");
    let projected = command(
        &[
            "roster-literal",
            "--roster",
            path(&roster_path),
            "--selection",
            path(&selected),
            "--output",
            path(&literal_path),
        ],
        None,
    );
    assert!(
        projected.status.success(),
        "{}",
        String::from_utf8_lossy(&projected.stderr)
    );
    let literal: Binding = serde_json::from_slice(&std::fs::read(&literal_path).unwrap()).unwrap();
    let injected = root.path().join("selection-injected.json");
    std::fs::write(
        &injected,
        r#"[{"field":"first","binding":"alpha","actual_model":"alpha-v1","brier":0.0}]"#,
    )
    .unwrap();
    let injected_output = root.path().join("injected.json");
    let refused = command(
        &[
            "roster-literal",
            "--roster",
            path(&roster_path),
            "--selection",
            path(&injected),
            "--output",
            path(&injected_output),
        ],
        None,
    );
    assert_eq!(refused.status.code(), Some(2));
    assert!(!injected_output.exists());
    let mut comparison = GraphSpec::parse("outputs: {result: {kind: literal, value: {id: x, value: {kind: boolean, value: true}}, value_type: {kind: boolean}}}").unwrap();
    comparison.nodes.push(NodeSpec {
        id: sapho_core::NodeId::new("profiles").unwrap(),
        operation: Operation::Record {},
        inputs: BTreeMap::from([("snapshot".into(), literal)]),
        guard: None,
    });
    comparison.nodes.push(NodeSpec {
        id: sapho_core::NodeId::new("same_calls").unwrap(),
        operation: Operation::Compare {
            comparator: Comparator::Less,
        },
        inputs: BTreeMap::from([
            (
                "a".into(),
                Binding::Node {
                    node: sapho_core::NodeId::new("profiles").unwrap(),
                    port: "result".into(),
                    path: vec!["snapshot".into(), "alpha".into(), "calls".into()],
                },
            ),
            (
                "b".into(),
                Binding::Node {
                    node: sapho_core::NodeId::new("profiles").unwrap(),
                    port: "result".into(),
                    path: vec!["snapshot".into(), "beta".into(), "calls".into()],
                },
            ),
        ]),
        guard: None,
    });
    comparison.outputs.insert(
        "result".into(),
        Binding::Node {
            node: sapho_core::NodeId::new("same_calls").unwrap(),
            port: "result".into(),
            path: vec![],
        },
    );
    let comparison_path = root.path().join("comparison.json");
    std::fs::write(&comparison_path, serde_json::to_vec(&comparison).unwrap()).unwrap();
    let validated = command(&["validate", path(&comparison_path)], None);
    assert!(
        validated.status.success(),
        "{}",
        String::from_utf8_lossy(&validated.stderr)
    );
    let empty_input = root.path().join("empty-input.json");
    std::fs::write(&empty_input, "{}").unwrap();
    let ran = command(
        &["run", path(&comparison_path), "--input", path(&empty_input)],
        None,
    );
    assert!(
        ran.status.success(),
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&ran.stdout).unwrap()["outputs"]["result"]["value"]
            ["value"],
        true
    );
    let collision = command(
        &[
            "roster",
            path(&graph_path),
            "--dataset",
            path(&dataset_path),
            "--bindings",
            path(&bindings_path),
            "--replay",
            path(&replay_path),
            "--split",
            "held_out",
            "--mapping",
            path(&mapping_path),
            "--output",
            path(&roster_path),
        ],
        Some("not-a-url"),
    );
    assert_eq!(collision.status.code(), Some(2));
    assert_eq!(
        report,
        serde_json::from_slice::<Roster>(&std::fs::read(&roster_path).unwrap()).unwrap()
    );
    let invalid = root.path().join("invalid-mapping.yaml");
    std::fs::write(
        &invalid,
        "score: {binding: alpha, question_kind: boolean, endpoint: secret}",
    )
    .unwrap();
    let rejected = command(
        &[
            "roster",
            path(&graph_path),
            "--dataset",
            path(&dataset_path),
            "--bindings",
            path(&bindings_path),
            "--replay",
            path(&replay_path),
            "--split",
            "held_out",
            "--mapping",
            path(&invalid),
            "--output",
            path(&root.path().join("never.json")),
        ],
        Some("not-a-url"),
    );
    assert_eq!(rejected.status.code(), Some(2));
    assert!(!root.path().join("never.json").exists());
    let wrong_kind = root.path().join("wrong-kind.yaml");
    std::fs::write(&wrong_kind, "score: {binding: alpha, question_kind: choice}\nflag: {binding: beta, question_kind: boolean}\n").unwrap();
    let rejected = command(
        &[
            "roster",
            path(&graph_path),
            "--dataset",
            path(&dataset_path),
            "--bindings",
            path(&bindings_path),
            "--replay",
            path(&replay_path),
            "--split",
            "held_out",
            "--mapping",
            path(&wrong_kind),
            "--output",
            path(&root.path().join("wrong-kind-roster.json")),
        ],
        Some("not-a-url"),
    );
    assert_eq!(rejected.status.code(), Some(2));
    assert!(!root.path().join("wrong-kind-roster.json").exists());
}

struct CustomBackend {
    calls: AtomicUsize,
}
#[async_trait::async_trait]
impl ModelBackend for CustomBackend {
    async fn infer(&self, request: &ModelRequest) -> sapho_core::Result<ModelResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(ModelResponse {
            model: format!("custom-{}", request.backend),
            raw: None,
            answers: request
                .questions
                .iter()
                .map(|q| {
                    (
                        q.id.clone(),
                        Answer::Boolean {
                            probability: Probability::new(0.75).unwrap(),
                        },
                    )
                })
                .collect(),
            usage: None,
        })
    }
}
/// Trace: FR-061-AC-4, FR-062-AC-6, FR-064-AC-4, IT-009-SC-05
#[tokio::test]
async fn custom_host_roster_needs_no_stock_provider_or_secret_field() {
    let spec = GraphSpec::parse(graph()).unwrap();
    let backend = Arc::new(CustomBackend {
        calls: AtomicUsize::new(0),
    });
    let mut registry = BackendRegistry::default();
    for name in ["alpha", "beta"] {
        registry
            .register(
                BackendId::new(name).unwrap(),
                BackendBinding {
                    backend: backend.clone(),
                    model: format!("requested-{name}"),
                    expected_model: None,
                    distribution_policy: DistributionPolicy::Strict {},
                },
            )
            .unwrap();
    }
    let runner = sapho_cli::Runner::new(&spec, &PrimitiveRegistry::default(), registry).unwrap();
    let mut dataset = data();
    dataset.cases.truncate(1);
    let case = &dataset.cases[0];
    let mut descriptors = BTreeMap::new();
    descriptors.insert(
        BackendId::new("alpha").unwrap(),
        ProviderDescriptor {
            provider: Some("custom-provider".into()),
            adapter: Some("rust-host".into()),
        },
    );
    let config = ObservationConfig {
        mode: ObservationMode::Live,
        descriptors,
        clock: Arc::new(SystemObservationClock),
    };
    let (report, calls) = runner
        .run_observed(&case.inputs, RunLimits::default(), None, &config)
        .await
        .unwrap();
    assert!(report.error.is_none());
    let outcome = CaseOutcome::Completed {
        outputs: report.outputs.unwrap(),
        models: report
            .trace
            .nodes
            .iter()
            .filter_map(|node| {
                node.model
                    .as_ref()?
                    .response
                    .as_ref()
                    .map(|response| ModelIdentity {
                        name: response.model.clone(),
                    })
            })
            .collect(),
    };
    let mut lineage = BTreeMap::new();
    lineage.insert(
        "score".into(),
        vec![RosterContributor {
            binding: BackendId::new("alpha").unwrap(),
            actual_model: Some("custom-alpha".into()),
            question_kind: Some(RosterQuestionKind::Boolean),
        }],
    );
    lineage.insert(
        "flag".into(),
        vec![RosterContributor {
            binding: BackendId::new("beta").unwrap(),
            actual_model: Some("custom-beta".into()),
            question_kind: Some(RosterQuestionKind::Boolean),
        }],
    );
    let mapped_calls = calls
        .into_iter()
        .map(|call| RosterCall {
            path: call.path,
            binding: call.binding,
            actual_model: call.actual_model,
            completed: call.status == NodeStatus::Completed,
            usage: call.usage,
            elapsed_micros: call.elapsed_micros,
            descriptor: call.descriptor,
            mode: RosterMode::Live,
        })
        .collect();
    let cases = BTreeMap::from([(
        case.id.clone(),
        RosterCase {
            outcome,
            calls: mapped_calls,
            lineage,
        },
    )]);
    let mappings = BTreeMap::from([
        (
            "score".into(),
            RosterMapping {
                binding: BackendId::new("alpha").unwrap(),
                question_kind: RosterQuestionKind::Boolean,
            },
        ),
        (
            "flag".into(),
            RosterMapping {
                binding: BackendId::new("beta").unwrap(),
                question_kind: RosterQuestionKind::Boolean,
            },
        ),
    ]);
    let roster = roster(
        &dataset,
        Split::Development,
        &sapho_graph::graph_semantic_identity(&spec).unwrap(),
        "0.1",
        &runner.inspection().signature.outputs,
        &mappings,
        &cases,
        10,
    )
    .unwrap();
    assert_eq!(roster.entries.len(), 2);
    assert_eq!(
        roster
            .entries
            .iter()
            .find(|entry| entry.binding.as_str() == "alpha")
            .unwrap()
            .descriptor
            .as_ref()
            .unwrap()
            .provider
            .as_deref(),
        Some("custom-provider")
    );
    assert!(
        roster
            .entries
            .iter()
            .find(|entry| entry.binding.as_str() == "beta")
            .unwrap()
            .descriptor
            .is_none()
    );
    let json = serde_json::to_string(&roster).unwrap();
    assert!(!json.contains("credential") && !json.contains("endpoint"));
    let invalid: serde_json::Value =
        serde_json::json!({"provider":"custom", "credential":"secret"});
    assert!(serde_json::from_value::<ProviderDescriptor>(invalid).is_err());
    let invalid_config = ObservationConfig {
        mode: ObservationMode::Live,
        descriptors: BTreeMap::from([(
            BackendId::new("alpha").unwrap(),
            ProviderDescriptor {
                provider: Some("https://secret".into()),
                adapter: None,
            },
        )]),
        clock: Arc::new(SystemObservationClock),
    };
    let before = backend.calls.load(Ordering::SeqCst);
    assert!(
        runner
            .run_observed(&case.inputs, RunLimits::default(), None, &invalid_config)
            .await
            .is_err()
    );
    assert_eq!(backend.calls.load(Ordering::SeqCst), before);
}
