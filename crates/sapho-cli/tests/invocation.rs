// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Real CLI workflows plus injected backend/native seams; no network or credentials.
use sapho_cli::*;
use sapho_core::*;
use sapho_evidence::{Case, Dataset, LabelKind, LabelProvenance, Split};
use sapho_graph::*;
use sapho_recording::*;
use sapho_runtime::RunLimits;
use std::{
    collections::BTreeMap,
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
fn offline() -> GraphSpec {
    GraphSpec::parse("inputs: {flag: {kind: boolean}}\nnodes: [{id: invert, operation: {kind: not}, inputs: {value: {kind: input, name: flag}}}]\noutputs: {result: {kind: node, node: invert, port: result}}").unwrap()
}
fn graph_file(root: &Path, name: &str, spec: &GraphSpec) -> std::path::PathBuf {
    let path = root.join(name);
    std::fs::write(&path, serde_json::to_vec(spec).unwrap()).unwrap();
    path
}
fn cli(args: &[&str], stdin: Option<&[u8]>) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_sapho"))
        .args(args)
        .env_clear()
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(input) = stdin {
        child.stdin.take().unwrap().write_all(input).unwrap();
    }
    child.wait_with_output().unwrap()
}
fn result(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "Invalid JSON: {e}; {}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}
fn path(path: &Path) -> &str {
    path.to_str().unwrap()
}
/// Trace: FR-046-AC-1, FR-033-AC-1, FR-033-AC-2, FR-033-AC-3, FR-034-AC-1, FR-034-AC-2, TC-033, TC-034
#[test]
fn cli_inspection_file_stdin_types_exits_and_refusals_are_real() {
    let root = tempfile::tempdir().unwrap();
    let graph = graph_file(root.path(), "graph.json", &offline());
    let inspected = cli(&["inspect", path(&graph)], None);
    assert!(inspected.status.success());
    let report: Inspection = serde_json::from_slice(&inspected.stdout).unwrap();
    assert_eq!(report.signature.outputs["result"], ValueType::Boolean);
    assert_eq!(
        report.groups[0].stages,
        vec![vec![NodeId::new("invert").unwrap()]]
    );
    let input = root.path().join("input.json");
    std::fs::write(&input, br#"{"flag":false}"#).unwrap();
    let run = cli(
        &[
            "run",
            path(&graph),
            "--input",
            path(&input),
            "--fail-on",
            "result",
        ],
        None,
    );
    assert_eq!(run.status.code(), Some(1));
    let run: RunReport = serde_json::from_slice(&run.stdout).unwrap();
    assert_eq!(run.outputs.unwrap()["result"].value, Value::Boolean(true));
    let stdin = cli(
        &["run", path(&graph), "--input", "-", "--fail-on", "result"],
        Some(br#"{"flag":true}"#),
    );
    assert_eq!(stdin.status.code(), Some(0));
    let typed = Inputs::from([(
        "flag".into(),
        Datum::new("original", Value::Boolean(true)).unwrap(),
    )]);
    let typed_run = cli(
        &["run", path(&graph), "--typed-input"],
        Some(&serde_json::to_vec(&typed).unwrap()),
    );
    assert_eq!(typed_run.status.code(), Some(0));
    let typed_run: RunReport = serde_json::from_slice(&typed_run.stdout).unwrap();
    assert_eq!(
        typed_run.trace.nodes[0].inputs["value"].id.as_str(),
        "original"
    );
    let bad = cli(&["run", path(&graph)], Some(br#"{"flag":0}"#));
    assert_eq!(bad.status.code(), Some(2));
    assert_eq!(result(&bad)["error"]["kind"], "engine");
    let limited = cli(
        &["run", path(&graph), "--max-data-bytes", "1"],
        Some(br#"{"flag":true}"#),
    );
    assert_eq!(limited.status.code(), Some(2));
    let limited: RunReport = serde_json::from_slice(&limited.stdout).unwrap();
    assert_eq!(limited.error.unwrap().code, ErrorCode::LimitExceeded);
    let bad_output = cli(
        &["run", path(&graph), "--fail-on", "absent"],
        Some(br#"{"flag":true}"#),
    );
    assert_eq!(bad_output.status.code(), Some(2));
    let unknown = root.path().join("graph.toml");
    std::fs::write(&unknown, "outputs = {}\n").unwrap();
    assert_eq!(
        cli(&["validate", path(&unknown)], None).status.code(),
        Some(2)
    );
    let yaml = root.path().join("graph.yaml");
    std::fs::write(&yaml,"inputs: {flag: {kind: boolean}}\nnodes: [{id: invert, operation: {kind: not}, inputs: {value: {kind: input, name: flag}}}]\noutputs: {result: {kind: node, node: invert, port: result}}").unwrap();
    let yaml_report: Inspection =
        serde_json::from_slice(&cli(&["inspect", path(&yaml)], None).stdout).unwrap();
    assert_eq!(yaml_report, report);
    let model = graph_file(root.path(), "model.json", &multilayer());
    assert!(cli(&["inspect", path(&model)], None).status.success());
    let mut native = offline();
    native.nodes[0].operation = Operation::Code {
        primitive: PrimitiveId::new("custom").unwrap(),
        params: BTreeMap::new(),
    };
    let native = graph_file(root.path(), "native.json", &native);
    let refused = cli(&["validate", path(&native)], None);
    assert_eq!(refused.status.code(), Some(2));
    assert_eq!(
        result(&refused)["error"]["detail"]["code"],
        "unknown_primitive"
    );
}
fn multilayer() -> GraphSpec {
    GraphSpec::parse(
        r#"
inputs: {text: {kind: text}}
nodes:
  - id: context
    operation: {kind: record}
    inputs: {text: {kind: input, name: text}}
  - id: questions
    operation:
      kind: questions
      questions:
        - id: q
          question: {kind: boolean, instructions: "Does the property hold?", yes: Holds, no: Absent}
  - id: ask
    operation: {kind: ask, backend: judge}
    inputs:
      state: {kind: node, node: context, port: result}
      questions: {kind: node, node: questions, port: result}
  - id: probability
    operation: {kind: probability, question: q, labels: ["true"]}
    inputs: {answers: {kind: node, node: ask, port: answers}}
  - id: context2
    operation: {kind: record}
    inputs:
      text: {kind: input, name: text}
      support: {kind: node, node: probability, port: result}
  - id: ask2
    operation: {kind: ask, backend: judge}
    inputs:
      state: {kind: node, node: context2, port: result}
      questions: {kind: node, node: questions, port: result}
  - id: result
    operation: {kind: probability, question: q, labels: ["true"]}
    inputs: {answers: {kind: node, node: ask2, port: answers}}
outputs: {result: {kind: node, node: result, port: result}}
"#,
    )
    .unwrap()
}
struct Scripted {
    calls: AtomicUsize,
    fail_at: Option<usize>,
}
#[async_trait::async_trait]
impl ModelBackend for Scripted {
    async fn infer(&self, request: &ModelRequest) -> Result<ModelResponse> {
        let index = self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail_at == Some(index) {
            return Err(SaphoError::new(
                ErrorCode::BackendFailed,
                "Scripted failure",
            ));
        }
        Ok(ModelResponse {
            model: request.model.clone(),
            digest: None,
            raw: Some(RawExchange {
                request: serde_json::to_string(request).unwrap(),
                response: format!("{{\"created_at\":\"call {index}\"}}"),
            }),
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
            usage: None,
        })
    }
}
fn backend(delegate: Arc<dyn ModelBackend>) -> BackendRegistry {
    let mut bindings = BackendRegistry::default();
    bindings
        .register(
            BackendId::new("judge").unwrap(),
            BackendBinding {
                backend: delegate,
                model: "synthetic".into(),
                expected_model: Some("synthetic".into()),
                distribution_policy: DistributionPolicy::Strict {},
            },
        )
        .unwrap();
    bindings
}
fn text_input(text: &str) -> Inputs {
    Inputs::from([(
        "text".into(),
        Datum::new("text", Value::Text(text.into())).unwrap(),
    )])
}
/// Trace: FR-034-AC-3, FR-035-AC-1, FR-035-AC-2, FR-035-AC-3, FR-035-AC-4, FR-037-AC-2, TC-035
#[tokio::test]
async fn scripted_multilayer_capture_replays_exactly_and_preserves_partial_failures() {
    let spec = multilayer();
    let live = Arc::new(Scripted {
        calls: AtomicUsize::new(0),
        fail_at: None,
    });
    let (bindings, recorders) = recording_bindings(
        &[BackendId::new("judge").unwrap()],
        &backend(live.clone()),
        1_000_000,
    )
    .unwrap();
    let original = Runner::new(&spec, &PrimitiveRegistry::default(), bindings)
        .unwrap()
        .run(&text_input("synthetic"), RunLimits::default(), None)
        .await;
    assert_eq!(original.exit, ExitStatus::Completed);
    assert_eq!(
        original.outputs.as_ref().unwrap()["result"].value,
        Value::Probability(Probability::new(0.8).unwrap())
    );
    assert_eq!(live.calls.load(Ordering::SeqCst), 2);
    let recording = recorders[0].snapshot().unwrap();
    assert_eq!(recording.exchanges.len(), 2);
    let offline = Runner::new(
        &spec,
        &PrimitiveRegistry::default(),
        replay_bindings(&recording, None, 1_000_000).unwrap(),
    )
    .unwrap();
    assert_eq!(
        offline
            .run(&text_input("synthetic"), RunLimits::default(), None)
            .await,
        original
    );
    let miss = offline
        .run(&text_input("changed"), RunLimits::default(), None)
        .await;
    assert_eq!(miss.error.unwrap().code, ErrorCode::ReplayMiss);
    assert_eq!(live.calls.load(Ordering::SeqCst), 2);
    let failed = Arc::new(Scripted {
        calls: AtomicUsize::new(0),
        fail_at: Some(1),
    });
    let (bindings, handles) = recording_bindings(
        &[BackendId::new("judge").unwrap()],
        &backend(failed),
        1_000_000,
    )
    .unwrap();
    let report = Runner::new(&spec, &PrimitiveRegistry::default(), bindings)
        .unwrap()
        .run(&text_input("synthetic"), RunLimits::default(), None)
        .await;
    assert_eq!(report.exit, ExitStatus::Refused);
    assert_eq!(
        report.error.as_ref().unwrap().code,
        ErrorCode::BackendFailed
    );
    assert_eq!(handles[0].snapshot().unwrap().exchanges.len(), 1);
    assert!(
        report
            .trace
            .nodes
            .iter()
            .any(|n| n.status == sapho_runtime::NodeStatus::Failed)
    );
    let root = tempfile::tempdir().unwrap();
    let graph = graph_file(root.path(), "graph.json", &spec);
    let saved = root.path().join("recording.json");
    recording.write_new(&saved, 1_000_000).unwrap();
    let typed = root.path().join("input.json");
    std::fs::write(
        &typed,
        serde_json::to_vec(&text_input("synthetic")).unwrap(),
    )
    .unwrap();
    let replay = cli(
        &[
            "replay",
            path(&graph),
            "--typed-input",
            "--input",
            path(&typed),
            "--recording",
            path(&saved),
        ],
        None,
    );
    assert!(replay.status.success());
    let printed: serde_json::Value = serde_json::from_slice(&replay.stdout).unwrap();
    let replay: RunReport = serde_json::from_slice(&replay.stdout).unwrap();
    assert_eq!(replay, original);
    // The replayed answer still carries the exchange recorded for it, byte for byte.
    let recorded = &recording.exchanges[0].response.raw;
    assert!(recorded.is_some());
    let replayed: Vec<_> = printed["trace"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|n| n["model"]["response"]["raw"].as_object())
        .collect();
    assert_eq!(replayed.len(), 2);
    assert_eq!(
        serde_json::to_value(recorded).unwrap(),
        serde_json::Value::Object(replayed[0].clone())
    );
}
/// Trace: FR-046-AC-3, FR-006-AC-5
#[test]
fn error_objects_hold_code_message_and_context_only() {
    let sentinel = "SENTINEL-BODY-2290";
    let error = SaphoError::new(ErrorCode::TooLarge, "Prompt does not fit")
        .with_context("context_tokens", "4096")
        .with_raw(RawExchange {
            request: sentinel.into(),
            response: sentinel.into(),
        })
        .with_usage(sapho_core::ExtractUsage {
            input_tokens: Some(1),
            ..Default::default()
        });
    for written in [
        serde_json::to_value(CliError::Engine(error.clone())).unwrap(),
        serde_json::to_value(RunReport {
            outputs: None,
            trace: Default::default(),
            error: Some(error.clone()),
            exit: ExitStatus::Refused,
        })
        .unwrap(),
    ] {
        assert!(!written.to_string().contains(sentinel));
    }
    let object = serde_json::to_value(CliError::Engine(error.clone())).unwrap();
    let members: std::collections::BTreeSet<_> = object["detail"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(members, ["code", "context", "message"].into());
    assert!(!CliError::Engine(error).to_string().contains(sentinel));
}
/// Trace: FR-035-AC-1, FR-035-AC-2, FR-034-AC-2
#[test]
fn conflicting_recorded_or_explicit_binding_metadata_is_refused_without_live_lookup() {
    let response = ModelResponse {
        model: "synthetic".into(),
        digest: None,
        raw: None,
        answers: BTreeMap::new(),
        usage: None,
    };
    let request = ModelRequest {
        backend: BackendId::new("judge").unwrap(),
        model: "synthetic".into(),
        expected_model: None,
        distribution_policy: DistributionPolicy::Strict {},
        state: Value::Record(BTreeMap::new()),
        questions: vec![],
    };
    let mut conflicting = request.clone();
    conflicting.distribution_policy = DistributionPolicy::approximate(0.02).unwrap();
    let recording = Recording {
        exchanges: vec![
            Exchange {
                request: request.clone(),
                response: response.clone(),
            },
            Exchange {
                request: conflicting,
                response: response.clone(),
            },
        ],
    };
    assert!(matches!(
        replay_bindings(&recording, None, 1_000_000),
        Err(CliError::Engine(SaphoError {
            code: ErrorCode::RecordingMismatch,
            ..
        }))
    ));
    let explicit = Bindings::from([(
        request.backend.clone(),
        BindingConfig {
            provider: Provider::Jev,
            model: "other".into(),
            expected_model: None,
            distribution_policy: DistributionPolicy::Strict {},
            ollama: None,
        },
    )]);
    let one = Recording {
        exchanges: vec![Exchange { request, response }],
    };
    assert!(matches!(
        replay_bindings(&one, Some(&explicit), 1_000_000),
        Err(CliError::Engine(SaphoError {
            code: ErrorCode::RecordingMismatch,
            ..
        }))
    ));
    assert!(matches!(
        Runner::new(
            &multilayer(),
            &PrimitiveRegistry::default(),
            replay_bindings(&Recording::default(), None, 1000).unwrap()
        ),
        Err(CliError::Engine(SaphoError {
            code: ErrorCode::UnknownBackend,
            ..
        }))
    ));
}
fn curator(reference: &str) -> LabelProvenance {
    LabelProvenance {
        kind: LabelKind::Human,
        source: "curator".into(),
        model_digest: None,
        reference: reference.into(),
    }
}
fn labelled() -> Dataset {
    Dataset {
        id: SourceId::new("original-labels").unwrap(),
        cases: vec![
            Case {
                id: ItemId::new("development").unwrap(),
                split: Split::Development,
                inputs: text_input("synthetic"),
                labels: BTreeMap::from([("result".into(), true)]),
                label_provenance: curator("curator original reference"),
            },
            Case {
                id: ItemId::new("heldout").unwrap(),
                split: Split::HeldOut,
                inputs: text_input("heldout: no matching recording"),
                labels: BTreeMap::from([("result".into(), false)]),
                label_provenance: curator("curator heldout reference"),
            },
        ],
    }
}
/// Trace: FR-036-AC-1, FR-036-AC-2, FR-036-AC-3, FR-036-AC-5, FR-037-AC-1, FR-037-AC-2, FR-037-AC-3, FR-038-AC-1, FR-038-AC-2, FR-038-AC-3
#[tokio::test]
async fn real_measure_tune_export_use_selected_labels_and_never_evaluate_heldout_in_tune() {
    let root = tempfile::tempdir().unwrap();
    let spec = multilayer();
    let graph = graph_file(root.path(), "graph.json", &spec);
    let recorder = Arc::new(
        RecordingBackend::new(
            Arc::new(Scripted {
                calls: AtomicUsize::new(0),
                fail_at: None,
            }),
            1_000_000,
        )
        .unwrap(),
    );
    Runner::new(
        &spec,
        &PrimitiveRegistry::default(),
        backend(recorder.clone()),
    )
    .unwrap()
    .run(&text_input("synthetic"), RunLimits::default(), None)
    .await;
    let saved = root.path().join("saved.json");
    recorder
        .snapshot()
        .unwrap()
        .write_new(&saved, 1_000_000)
        .unwrap();
    let dataset = root.path().join("dataset.json");
    std::fs::write(&dataset, serde_json::to_vec(&labelled()).unwrap()).unwrap();
    let measured = cli(
        &[
            "measure",
            path(&graph),
            "--dataset",
            path(&dataset),
            "--split",
            "development",
            "--replay",
            path(&saved),
        ],
        None,
    );
    assert!(measured.status.success());
    let report = result(&measured);
    assert_eq!(report["measurement"]["selected_cases"], 1);
    assert!(
        (report["measurement"]["outputs"]["result"]["metrics"]["brier"]
            .as_f64()
            .unwrap()
            - 0.04)
            .abs()
            < 1e-12
    );
    let heldout = cli(
        &[
            "measure",
            path(&graph),
            "--dataset",
            path(&dataset),
            "--split",
            "held_out",
            "--replay",
            path(&saved),
        ],
        None,
    );
    assert_eq!(heldout.status.code(), Some(2));
    assert_eq!(
        result(&heldout)["measurement"]["outputs"]["result"]["failed"],
        1
    );
    let mut changed = spec.clone();
    let Operation::Questions { questions } = &mut changed.nodes[1].operation else {
        panic!("questions expected")
    };
    let Question::Boolean { instructions, .. } = &mut questions[0].question else {
        panic!("question expected")
    };
    instructions.push_str(" changed");
    let changed = graph_file(root.path(), "changed.json", &changed);
    let tuned = cli(
        &[
            "tune",
            "--candidate",
            path(&changed),
            "--candidate",
            path(&graph),
            "--dataset",
            path(&dataset),
            "--output-name",
            "result",
            "--metric",
            "brier",
            "--replay",
            path(&saved),
        ],
        None,
    );
    assert!(tuned.status.success());
    let tuned = result(&tuned);
    assert_eq!(tuned["ranking"].as_array().unwrap().len(), 1);
    assert_eq!(
        tuned["candidates"][0]["measurement"]["outputs"]["result"]["failed"],
        1
    );
    assert_eq!(tuned["candidates"][1]["runs"].as_array().unwrap().len(), 1);
    let no_winner = cli(
        &[
            "tune",
            "--candidate",
            path(&changed),
            "--dataset",
            path(&dataset),
            "--output-name",
            "result",
            "--metric",
            "brier",
            "--replay",
            path(&saved),
        ],
        None,
    );
    assert_eq!(no_winner.status.code(), Some(2));
    assert_eq!(result(&no_winner)["error"]["kind"], "no_rankable_candidate");
    let exported = root.path().join("training.jsonl");
    let export = cli(
        &[
            "export-training",
            "--dataset",
            path(&dataset),
            "--output",
            path(&exported),
        ],
        None,
    );
    assert!(export.status.success());
    let text = std::fs::read_to_string(&exported).unwrap();
    assert_eq!(text.lines().count(), 1);
    assert!(!text.contains("heldout"));
    let preserved = std::fs::read(&exported).unwrap();
    assert_eq!(
        cli(
            &[
                "export-training",
                "--dataset",
                path(&dataset),
                "--output",
                path(&exported)
            ],
            None
        )
        .status
        .code(),
        Some(2)
    );
    assert_eq!(std::fs::read(exported).unwrap(), preserved);
    let mut own = labelled();
    own.cases[0].label_provenance = LabelProvenance {
        kind: LabelKind::Model,
        source: "synthetic".into(),
        model_digest: None,
        reference: "own earlier answer".into(),
    };
    std::fs::write(&dataset, serde_json::to_vec(&own).unwrap()).unwrap();
    let own = cli(
        &[
            "measure",
            path(&graph),
            "--dataset",
            path(&dataset),
            "--split",
            "development",
            "--replay",
            path(&saved),
        ],
        None,
    );
    assert!(
        own.status.success(),
        "a self-sourced case alone does not exit 2"
    );
    let own = result(&own);
    assert_eq!(
        own["measurement"]["self_source"],
        serde_json::json!(["development"])
    );
    assert!(
        own["measurement"]["outputs"]
            .as_object()
            .unwrap()
            .is_empty()
    );
}
/// Trace: NFR-005-M-3, FR-041-AC-3, FR-034-AC-1, FR-035-AC-3
#[test]
fn selector_pipeline_and_offline_recording_use_real_commands_without_overwriting() {
    let root = tempfile::tempdir().unwrap();
    let schema = root.path().join("schema.json");
    std::fs::write(&schema, r#"{"kind":"boolean"}"#).unwrap();
    let selected = cli(
        &[
            "select",
            "json",
            "--input",
            "-",
            "--pointer",
            "/flag",
            "--schema",
            path(&schema),
            "--port",
            "flag",
        ],
        Some(br#"{"flag":false}"#),
    );
    assert!(selected.status.success());
    let graph = graph_file(root.path(), "graph.json", &offline());
    let recorded = root.path().join("recorded.json");
    let trace = root.path().join("trace.json");
    let run = cli(
        &[
            "record",
            path(&graph),
            "--typed-input",
            "--recording",
            path(&recorded),
            "--trace",
            path(&trace),
        ],
        Some(&selected.stdout),
    );
    assert!(run.status.success());
    assert_eq!(Recording::read(&recorded, 1000).unwrap().exchanges.len(), 0);
    assert_eq!(
        cli(
            &[
                "replay",
                path(&graph),
                "--typed-input",
                "--recording",
                path(&recorded)
            ],
            Some(&selected.stdout)
        )
        .stdout,
        run.stdout
    );
    let before = std::fs::read(&recorded).unwrap();
    assert_eq!(
        cli(
            &[
                "record",
                path(&graph),
                "--typed-input",
                "--recording",
                path(&recorded)
            ],
            Some(&selected.stdout)
        )
        .status
        .code(),
        Some(2)
    );
    assert_eq!(std::fs::read(recorded).unwrap(), before);
    let list_schema = root.path().join("list.json");
    std::fs::write(&list_schema, r#"{"kind":"list","item":{"kind":"text"}}"#).unwrap();
    let empty = cli(
        &[
            "select",
            "json",
            "--input",
            "-",
            "--schema",
            path(&list_schema),
        ],
        Some(b"[]"),
    );
    assert!(empty.status.success());
    let selected: Inputs = serde_json::from_slice(&empty.stdout).unwrap();
    assert_eq!(selected["items"].value, Value::List(vec![]));
}
/// Trace: FR-034-AC-2
#[cfg(not(feature = "jev"))]
#[test]
fn live_provider_is_explicitly_refused_when_jev_feature_is_absent() {
    let root = tempfile::tempdir().unwrap();
    let graph = graph_file(root.path(), "graph.json", &multilayer());
    let metadata = root.path().join("bindings.yaml");
    std::fs::write(
        &metadata,
        "judge: {provider: jev, model: synthetic, distribution_policy: {kind: strict}}",
    )
    .unwrap();
    let refusal = cli(
        &["run", path(&graph), "--bindings", path(&metadata)],
        Some(br#"{"text":"synthetic"}"#),
    );
    assert_eq!(refusal.status.code(), Some(2));
    assert_eq!(result(&refusal)["error"]["kind"], "feature");
}

/// Trace: NFR-005-M-2, FR-034-AC-3, FR-033-AC-2
#[tokio::test]
async fn custom_host_inspection_captures_contract_without_executing_native_work() {
    struct Invert(Arc<AtomicUsize>);
    impl Primitive for Invert {
        fn signature(&self) -> Signature {
            Signature {
                inputs: BTreeMap::from([("flag".into(), ValueType::Boolean)]),
                outputs: BTreeMap::from([("result".into(), ValueType::Boolean)]),
            }
        }
        fn execute(
            &self,
            context: &PrimitiveContext,
            inputs: &Inputs,
            _: &BTreeMap<String, Value>,
        ) -> sapho_core::Result<Inputs> {
            context.check_cancelled()?;
            self.0.fetch_add(1, Ordering::Relaxed);
            let Some(Datum {
                value: Value::Boolean(flag),
                ..
            }) = inputs.get("flag")
            else {
                return Err(SaphoError::new(ErrorCode::TypeMismatch, "Boolean required"));
            };
            Ok(Inputs::from([(
                "result".into(),
                Datum::new("custom", Value::Boolean(!flag))?,
            )]))
        }
    }
    let count = Arc::new(AtomicUsize::new(0));
    let mut primitives = PrimitiveRegistry::default();
    primitives
        .register(
            PrimitiveId::new("invert").unwrap(),
            Arc::new(Invert(count.clone())),
        )
        .unwrap();
    let spec = GraphSpec::parse("inputs: {flag: {kind: boolean}}\nnodes: [{id: custom, operation: {kind: code, primitive: invert}, inputs: {flag: {kind: input, name: flag}}}]\noutputs: {result: {kind: node, node: custom, port: result}}").unwrap();
    let inspected = inspect(&spec, &primitives).unwrap();
    assert_eq!(
        inspected.primitives,
        vec![PrimitiveId::new("invert").unwrap()]
    );
    let runner = Runner::new(&spec, &primitives, BackendRegistry::default()).unwrap();
    assert_eq!(count.load(Ordering::Relaxed), 0);
    let inputs = plain_inputs(&inspected.signature, &serde_json::json!({"flag": false})).unwrap();
    let report = runner
        .run(&inputs, RunLimits::default(), Some("result"))
        .await;
    assert_eq!(report.exit, ExitStatus::Finding);
    assert_eq!(
        report.outputs.unwrap()["result"].value,
        Value::Boolean(true)
    );
    assert_eq!(count.load(Ordering::Relaxed), 1);
}
/// Trace: FR-042-AC-1, FR-042-AC-2, FR-042-AC-3, FR-037-AC-1, FR-037-AC-2, FR-037-AC-3, FR-039-AC-1, FR-041-AC-3, TC-042
#[test]
fn packaged_workflows_validate_tune_combine_collect_record_and_replay_offline() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let plugin = repo.join("plugins/sapho");
    let manifest: serde_json::Value =
        decode_json(&std::fs::read(plugin.join("plugin.json")).unwrap(), 10000).unwrap();
    assert_eq!(manifest["name"], "sapho");
    assert_eq!(manifest["license"], "AGPL-3.0-or-later");
    assert!(manifest.get("hooks").is_none());
    let mut names = std::fs::read_dir(plugin.join("skills"))
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            let skill = entry.path().join("SKILL.md").canonicalize().unwrap();
            assert!(skill.starts_with(&plugin));
            let text = std::fs::read_to_string(skill).unwrap();
            let frontmatter = text
                .strip_prefix("---\n")
                .unwrap()
                .split_once("\n---\n")
                .unwrap()
                .0;
            let data: serde_json::Value = parse_config(frontmatter, GraphFormat::Yaml).unwrap();
            let name = data["name"].as_str().unwrap();
            assert_eq!(entry.file_name().to_str().unwrap(), name);
            assert!(!data["description"].as_str().unwrap().is_empty());
            for reference in text
                .split("https://github.com/agent-ix/sapho/blob/main/")
                .skip(1)
            {
                let relative = reference.split(')').next().unwrap();
                let target = repo.join(relative).canonicalize().unwrap();
                assert!(target.starts_with(&repo));
                assert!(target.is_file());
            }
            name.to_owned()
        })
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(names, vec!["create", "record", "tune"]);
    assert_eq!(std::fs::read_dir(&plugin).unwrap().count(), 2); // manifest and skills, no hooks
    let root = tempfile::tempdir().unwrap();
    let review = repo.join("examples/graphs/review.yaml");
    let conservative = repo.join("examples/graphs/review-conservative.yaml");
    let dataset = repo.join("examples/data/review-dataset.json");
    assert!(cli(&["validate", path(&review)], None).status.success());
    let candidate_report = root.path().join("tuning.json");
    let tuned = cli(
        &[
            "tune",
            "--candidate",
            path(&review),
            "--candidate",
            path(&conservative),
            "--dataset",
            path(&dataset),
            "--output-name",
            "needs_review",
            "--metric",
            "agreement",
            "--output",
            path(&candidate_report),
        ],
        None,
    );
    assert!(
        tuned.status.success(),
        "{}",
        String::from_utf8_lossy(&tuned.stdout)
    );
    let tuning = result(&tuned);
    assert_eq!(tuning["ranking"][0]["input_index"], 0);
    assert_eq!(tuning["ranking"][0]["score"], 1.0);
    assert_eq!(tuning["ranking"][1]["score"], 0.5);
    assert_eq!(tuning["candidates"][0]["measurement"]["selected_cases"], 2);
    assert_eq!(tuning["candidates"][0]["runs"].as_array().unwrap().len(), 2);
    assert_eq!(
        decode_json::<serde_json::Value>(&std::fs::read(&candidate_report).unwrap(), 100000)
            .unwrap(),
        tuning
    );
    let tied = cli(
        &[
            "tune",
            "--candidate",
            path(&review),
            "--candidate",
            path(&review),
            "--dataset",
            path(&dataset),
            "--output-name",
            "needs_review",
            "--metric",
            "agreement",
        ],
        None,
    );
    assert!(tied.status.success());
    assert_eq!(result(&tied)["ranking"][0]["input_index"], 0);
    assert_eq!(result(&tied)["ranking"][1]["input_index"], 1);
    let missing = root.path().join("missing.yaml");
    let rejected = cli(
        &[
            "tune",
            "--candidate",
            path(&missing),
            "--candidate",
            path(&review),
            "--dataset",
            path(&dataset),
            "--output-name",
            "needs_review",
            "--metric",
            "agreement",
        ],
        None,
    );
    assert!(rejected.status.success());
    assert_eq!(result(&rejected)["ranking"][0]["input_index"], 1);
    assert!(result(&rejected)["candidates"][0]["error"].is_object());
    let combined = cli(
        &[
            "run",
            path(&repo.join("examples/graphs/combine.yaml")),
            "--input",
            path(&repo.join("examples/data/combine-input.json")),
        ],
        None,
    );
    assert!(
        combined.status.success(),
        "{}",
        String::from_utf8_lossy(&combined.stdout)
    );
    assert!(
        (result(&combined)["outputs"]["strength"]["value"]["value"]
            .as_f64()
            .unwrap()
            - 0.65)
            .abs()
            < 1e-12
    );
    assert_eq!(
        result(&combined)["outputs"]["needs_review"]["value"]["value"],
        true
    );
    let files = cli(
        &[
            "select",
            "files",
            "--root",
            path(root.path()),
            "--include",
            "*.rs",
        ],
        None,
    );
    assert!(files.status.success());
    let input: Inputs = decode_json(&files.stdout, 10000).unwrap();
    assert_eq!(input["items"].value, Value::List(vec![]));
    let graph = repo.join("examples/graphs/files.yaml");
    let recorded = root.path().join("saved.json");
    let trace = root.path().join("trace.json");
    let captured = cli(
        &[
            "record",
            path(&graph),
            "--typed-input",
            "--recording",
            path(&recorded),
            "--trace",
            path(&trace),
        ],
        Some(&files.stdout),
    );
    assert!(captured.status.success());
    assert_eq!(
        result(&captured)["outputs"]["context"]["value"]["value"]["files"]["value"],
        serde_json::json!([])
    );
    let replayed = cli(
        &[
            "replay",
            path(&graph),
            "--typed-input",
            "--recording",
            path(&recorded),
        ],
        Some(&files.stdout),
    );
    assert!(replayed.status.success());
    assert_eq!(replayed.stdout, captured.stdout);
    std::fs::write(root.path().join("one.rs"), "original example\n").unwrap();
    let files = cli(
        &[
            "select",
            "files",
            "--root",
            path(root.path()),
            "--include",
            "*.rs",
        ],
        None,
    );
    let selected: Inputs = decode_json(&files.stdout, 10000).unwrap();
    let Value::List(items) = &selected["items"].value else {
        panic!("list expected")
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].sources[0].end, Some(17));
    assert!(
        cli(&["run", path(&graph), "--typed-input"], Some(&files.stdout))
            .status
            .success()
    );
}
/// Trace: NFR-005-M-1, FR-041-AC-1, FR-041-AC-3
#[cfg(unix)]
#[test]
fn acquisition_refuses_fifos_expired_reads_and_cumulative_json_overflow() {
    use std::time::Duration;
    let root = tempfile::tempdir().unwrap();
    let regular = root.path().join("input.json");
    std::fs::write(&regular, b"{\"a\": true}").unwrap();
    assert_eq!(
        read_bytes_with_timeout(&regular, 11, Duration::from_secs(1)).unwrap(),
        b"{\"a\": true}"
    );
    assert!(matches!(
        read_bytes_with_timeout(&regular, 20, Duration::from_nanos(1)),
        Err(CliError::Engine(SaphoError {
            code: ErrorCode::DeadlineExceeded,
            ..
        }))
    ));
    let fifo = root.path().join("fifo");
    assert!(
        Command::new("/usr/bin/mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    assert!(matches!(
        read_bytes(&fifo, 100),
        Err(CliError::InputFileType(_))
    ));
    let schema = root.path().join("schema.json");
    std::fs::write(&schema, r#"{"kind":"boolean"}"#).unwrap();
    let mut blocked = Command::new(env!("CARGO_BIN_EXE_sapho"))
        .args([
            "select",
            "json",
            "--input",
            "-",
            "--schema",
            path(&schema),
            "--timeout-secs",
            "1",
        ])
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let open_writer = blocked.stdin.take().unwrap();
    let deadline = blocked.wait_with_output().unwrap();
    drop(open_writer);
    assert_eq!(deadline.status.code(), Some(2));
    assert_eq!(
        result(&deadline)["error"]["detail"]["code"],
        "deadline_exceeded"
    );
    let refused = cli(
        &[
            "select",
            "json",
            "--input",
            path(&regular),
            "--pointer",
            "/a",
            "--schema",
            path(&schema),
            "--max-total-bytes",
            "5",
        ],
        None,
    );
    assert_eq!(refused.status.code(), Some(2));
    assert_eq!(
        result(&refused)["error"]["detail"]["code"],
        "limit_exceeded"
    );
}

/// Trace: FR-040-AC-1, FR-040-AC-2, FR-034-AC-1, TC-040
#[cfg(unix)]
#[test]
fn real_git_selectors_feed_the_same_typed_graph_boundary() {
    let root = tempfile::tempdir().unwrap();
    let git = |args: &[&str]| {
        let result = Command::new("git")
            .current_dir(root.path())
            .args(args)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        String::from_utf8(result.stdout).unwrap().trim().to_owned()
    };
    git(&["init", "--quiet"]);
    git(&["config", "user.name", "Synthetic workflow"]);
    git(&["config", "user.email", "example@invalid.test"]);
    let file = root.path().join("-literal.txt");
    std::fs::write(&file, "base\n").unwrap();
    git(&["add", "--", "-literal.txt"]);
    git(&["commit", "--quiet", "-m", "Original test baseline"]);
    let base = git(&["rev-parse", "HEAD"]);
    std::fs::write(&file, "stage\n").unwrap();
    git(&["add", "--", "-literal.txt"]);
    std::fs::write(&file, "work\n").unwrap();
    let graph = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/graphs/files.yaml");
    let check = |mode: &str, references: &[&str], expected: &str| {
        let mut args = vec!["select", "git", "--root", path(root.path()), "--mode", mode];
        args.extend_from_slice(references);
        let selected = cli(&args, None);
        assert!(
            selected.status.success(),
            "{}",
            String::from_utf8_lossy(&selected.stdout)
        );
        let inputs: Inputs = decode_json(&selected.stdout, 10000).unwrap();
        let Value::List(items) = &inputs["items"].value else {
            panic!("list expected")
        };
        assert_eq!(items.len(), 1);
        let Value::Record(fields) = &items[0].value else {
            panic!("record expected")
        };
        assert_eq!(fields["path"], Value::Text("-literal.txt".into()));
        let Value::Text(patch) = &fields["text"] else {
            panic!("patch expected")
        };
        assert!(patch.contains(expected));
        assert_eq!(
            items[0].sources[0].end,
            Some(u64::try_from(patch.len()).unwrap())
        );
        let executed = cli(
            &["run", path(&graph), "--typed-input"],
            Some(&selected.stdout),
        );
        assert!(executed.status.success());
        let report: RunReport = decode_json(&executed.stdout, 100000).unwrap();
        assert_eq!(
            report.outputs.unwrap()["context"].value,
            Value::Record(BTreeMap::from([(
                "files".into(),
                inputs["items"].value.clone()
            )]))
        );
    };
    check("working_tree", &[], "+work\n");
    check("staged", &[], "+stage\n");
    git(&["commit", "--quiet", "-m", "Original staged change"]);
    let head = git(&["rev-parse", "HEAD"]);
    check("revisions", &["--base", &base, "--head", &head], "+stage\n");
    let invalid = cli(
        &[
            "select",
            "git",
            "--root",
            path(root.path()),
            "--mode",
            "revisions",
            "--base",
            "missing-ref",
            "--head",
            &head,
        ],
        None,
    );
    assert_eq!(invalid.status.code(), Some(2));
    assert_eq!(result(&invalid)["error"]["detail"]["kind"], "revision");
}
