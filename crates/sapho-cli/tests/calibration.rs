// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! IT-017 synthetic stock fit and exact replay; no live provider.
use sapho_core::{
    Answer, BackendId, CalibratedProbability, CalibrationMap, Datum, DistributionPolicy, Inputs,
    ItemId, ModelRequest, ModelResponse, NamedQuestion, NodeId, Probability, Question, SourceId,
    Value, ValueType,
};
use sapho_evidence::{Case, Dataset, LabelKind, LabelProvenance, Split, calibration_cases_digest};
use sapho_graph::{Binding, Comparator, GraphBody, GraphSpec, NodeSpec, Operation};
use sapho_recording::{Exchange, Recording};
use std::{collections::BTreeMap, path::Path, process::Command};
fn path(p: &Path) -> &str {
    p.to_str().unwrap()
}
fn raw_graph() -> &'static str {
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
  - id: ask
    operation: {kind: ask, backend: fast_a}
    inputs:
      state: {kind: node, node: state, port: result}
      questions: {kind: node, node: questions, port: result}
  - id: probability
    operation: {kind: probability, question: q, labels: ["true"]}
    inputs: {answers: {kind: node, node: ask, port: answers}}
outputs: {raw_support: {kind: node, node: probability, port: result}}
"#
}
fn fixtures() -> (Dataset, Recording) {
    let mut cases = Vec::new();
    let mut exchanges = Vec::new();
    let question = NamedQuestion {
        id: "q".into(),
        question: Question::Boolean {
            instructions: "Does this hold?".into(),
            yes: "Holds".into(),
            no: "Absent".into(),
        },
    };
    for i in 0..22 {
        let raw = if i < 10 || i == 20 { 0.05 } else { 0.95 };
        let label = if i < 10 { i < 2 } else { i < 18 };
        let text = format!("case-text-{i}");
        let input = Datum::new(format!("input-{i}"), Value::Text(text.clone())).unwrap();
        cases.push(Case {
            id: ItemId::new(format!("case-{i:02}")).unwrap(),
            split: if i < 20 {
                Split::Development
            } else {
                Split::HeldOut
            },
            inputs: Inputs::from([("text".into(), input)]),
            labels: BTreeMap::from([("raw_support".into(), label)]),
            label_provenance: LabelProvenance {
                kind: LabelKind::Human,
                source: "annotator".into(),
                reference: format!("row-{i}"),
            },
        });
        exchanges.push(Exchange {
            request: ModelRequest {
                backend: BackendId::new("fast_a").unwrap(),
                model: "requested".into(),
                expected_model: None,
                distribution_policy: DistributionPolicy::Strict {},
                state: Value::Record(BTreeMap::from([("text".into(), Value::Text(text))])),
                questions: vec![question.clone()],
            },
            response: ModelResponse {
                model: "model-a".into(),
                raw: None,
                answers: BTreeMap::from([(
                    "q".into(),
                    Answer::Boolean {
                        probability: Probability::new(raw).unwrap(),
                    },
                )]),
                usage: None,
            },
        });
    }
    (
        Dataset {
            id: SourceId::new("curated").unwrap(),
            cases,
        },
        Recording { exchanges },
    )
}
fn command(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_sapho"))
        .args(args)
        .env_clear()
        .output()
        .unwrap()
}
/// Trace: FR-081-AC-4, FR-082-AC-3, FR-083-AC-5,
/// FR-084-AC-1, FR-084-AC-2, FR-084-AC-3, FR-084-AC-4,
/// IT-017-SC-01, IT-017-SC-03, IT-017-SC-05, IT-017-SC-06, IT-017-SC-07, IT-017-SC-08
#[test]
fn development_fit_replays_and_writes_identical_exclusive_literals() {
    let root = tempfile::tempdir().unwrap();
    let graph = root.path().join("raw.yaml");
    let dataset = root.path().join("dataset.json");
    let recording = root.path().join("recording.json");
    std::fs::write(&graph, raw_graph()).unwrap();
    let (data, exchanges) = fixtures();
    std::fs::write(&dataset, serde_json::to_vec(&data).unwrap()).unwrap();
    std::fs::write(&recording, exchanges.to_json(8 * 1_048_576).unwrap()).unwrap();
    let first = root.path().join("map-a.json");
    let second = root.path().join("map-b.json");
    for output in [&first, &second] {
        let run = command(&[
            "fit-calibration",
            path(&graph),
            "--dataset",
            path(&dataset),
            "--replay",
            path(&recording),
            "--output-name",
            "raw_support",
            "--binding",
            "fast_a",
            "--split",
            "development",
            "--output",
            path(output),
        ]);
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
    }
    let bytes = std::fs::read(&first).unwrap();
    assert_eq!(bytes, std::fs::read(&second).unwrap());
    let literal: Binding = serde_json::from_slice(&bytes).unwrap();
    let Binding::Literal { value, value_type } = literal else {
        panic!("literal")
    };
    assert_eq!(value_type, sapho_core::ValueType::CalibrationMap);
    let Value::CalibrationMap(map) = value.value else {
        panic!("map")
    };
    assert_eq!(map.case_count, 20);
    assert_eq!(map.dataset_id.as_str(), "curated");
    assert_eq!(map.split, "development");
    assert_eq!(map.method, "isotonic-pava-linear-v1");
    assert_eq!(map.raw_output, "raw_support");
    assert_eq!(map.fit_binding.as_str(), "fast_a");
    assert_eq!(
        map.fit_graph_semantic_identity,
        sapho_graph::graph_semantic_identity(&GraphSpec::parse(raw_graph()).unwrap()).unwrap()
    );
    assert!(
        map.fit_cases_digest
            .starts_with("dataset-development-v1:sha256:")
    );
    assert!(
        map.fit_observation_digest
            .starts_with("calibration-observations-v1:sha256:")
    );
    assert!(map.map_id.starts_with("calibration-v1:sha256:"));
    assert_eq!(map.fit_actual_model, "model-a");
    assert_eq!(
        map.knots
            .iter()
            .map(|k| (k.raw.get(), k.calibrated.get()))
            .collect::<Vec<_>>(),
        vec![(0.05, 0.2), (0.95, 0.8)]
    );
    map.validate().unwrap();
    let literal_text = String::from_utf8(bytes.clone()).unwrap();
    for private in [
        "case-text",
        "annotator",
        "requested",
        "endpoint",
        "credential",
    ] {
        assert!(!literal_text.contains(private));
    }
    let roundtrip: Dataset = serde_json::from_slice(&serde_json::to_vec(&data).unwrap()).unwrap();
    roundtrip.validate(1024).unwrap();
    assert_eq!(
        serde_json::to_vec(&data).unwrap(),
        serde_json::to_vec(&roundtrip).unwrap()
    );
    assert_eq!(
        map.fit_cases_digest,
        calibration_cases_digest(&data, "raw_support", 1024).unwrap()
    );
    let formatted = root.path().join("dataset-formatted.json");
    std::fs::write(&formatted, serde_json::to_string_pretty(&data).unwrap()).unwrap();
    let formatted_data: Dataset =
        serde_json::from_slice(&std::fs::read(&formatted).unwrap()).unwrap();
    assert_eq!(
        map.fit_cases_digest,
        calibration_cases_digest(&formatted_data, "raw_support", 1024).unwrap()
    );
    let mut changed_label = data.clone();
    *changed_label.cases[0]
        .labels
        .get_mut("raw_support")
        .unwrap() = false;
    assert_ne!(
        map.fit_cases_digest,
        calibration_cases_digest(&changed_label, "raw_support", 1024).unwrap()
    );
    let mut changed_input = data.clone();
    changed_input.cases[0].inputs.insert(
        "text".into(),
        Datum::new("input-0", Value::Text("changed-selected-input".into())).unwrap(),
    );
    assert_ne!(
        map.fit_cases_digest,
        calibration_cases_digest(&changed_input, "raw_support", 1024).unwrap()
    );
    let mut changed_provenance = data.clone();
    changed_provenance.cases[0].label_provenance.reference = "changed-row".into();
    assert_ne!(
        map.fit_cases_digest,
        calibration_cases_digest(&changed_provenance, "raw_support", 1024).unwrap()
    );
    let mut changed_held_out = data.clone();
    changed_held_out.cases[20]
        .labels
        .insert("raw_support".into(), true);
    changed_held_out.cases[20].label_provenance.reference = "changed-held-out".into();
    changed_held_out.cases[20].inputs.insert(
        "text".into(),
        Datum::new("input-20", Value::Text("changed-held-out-input".into())).unwrap(),
    );
    assert_eq!(
        map.fit_cases_digest,
        calibration_cases_digest(&changed_held_out, "raw_support", 1024).unwrap()
    );
    let mut calibrated = GraphSpec::parse(raw_graph()).unwrap();
    calibrated.nodes.push(NodeSpec {
        id: NodeId::new("calibrate").unwrap(),
        operation: Operation::Calibrate,
        inputs: BTreeMap::from([
            (
                "value".into(),
                Binding::Node {
                    node: NodeId::new("probability").unwrap(),
                    port: "result".into(),
                    path: Vec::new(),
                },
            ),
            (
                "map".into(),
                Binding::Literal {
                    value: Datum::new("calibration_map", Value::CalibrationMap(map.clone()))
                        .unwrap(),
                    value_type: sapho_core::ValueType::CalibrationMap,
                },
            ),
        ]),
        guard: None,
    });
    calibrated.outputs.insert(
        "calibrated_support".into(),
        Binding::Node {
            node: NodeId::new("calibrate").unwrap(),
            port: "result".into(),
            path: Vec::new(),
        },
    );
    let calibrated_path = root.path().join("calibrated.json");
    std::fs::write(&calibrated_path, serde_json::to_vec(&calibrated).unwrap()).unwrap();
    let no_source = GraphSpec {
        inputs: BTreeMap::from([("raw".into(), ValueType::Probability)]),
        nodes: vec![NodeSpec {
            id: NodeId::new("calibrate").unwrap(),
            operation: Operation::Calibrate,
            inputs: BTreeMap::from([
                (
                    "value".into(),
                    Binding::Input {
                        name: "raw".into(),
                        path: Vec::new(),
                    },
                ),
                (
                    "map".into(),
                    Binding::Literal {
                        value: Datum::new("calibration_map", Value::CalibrationMap(map.clone()))
                            .unwrap(),
                        value_type: ValueType::CalibrationMap,
                    },
                ),
            ]),
            guard: None,
        }],
        outputs: BTreeMap::from([(
            "calibrated_support".into(),
            Binding::Node {
                node: NodeId::new("calibrate").unwrap(),
                port: "result".into(),
                path: Vec::new(),
            },
        )]),
        subgraphs: BTreeMap::new(),
    };
    let no_source_path = root.path().join("no-source.json");
    std::fs::write(&no_source_path, serde_json::to_vec(&no_source).unwrap()).unwrap();
    let no_source_input = root.path().join("no-source-input.json");
    let raw_input = Inputs::from([(
        "raw".into(),
        Datum::new("raw", Value::Probability(Probability::new(0.8).unwrap())).unwrap(),
    )]);
    std::fs::write(&no_source_input, serde_json::to_vec(&raw_input).unwrap()).unwrap();
    let no_source_run = command(&[
        "run",
        path(&no_source_path),
        "--typed-input",
        "--input",
        path(&no_source_input),
    ]);
    assert!(!no_source_run.status.success());
    let no_source_failure: serde_json::Value =
        serde_json::from_slice(&no_source_run.stdout).unwrap();
    assert_eq!(no_source_failure["error"]["code"], "model_mismatch");
    assert_eq!(no_source_failure["error"]["context"]["observed_count"], "0");
    let mut two_source = calibrated.clone();
    let first_probability = two_source
        .nodes
        .iter_mut()
        .find(|node| node.id.as_str() == "probability")
        .unwrap();
    first_probability.guard = Some(Binding::Literal {
        value: Datum::new("guard", Value::Boolean(true)).unwrap(),
        value_type: ValueType::Boolean,
    });
    let mut second_ask = two_source
        .nodes
        .iter()
        .find(|node| node.id.as_str() == "ask")
        .unwrap()
        .clone();
    second_ask.id = NodeId::new("second_ask").unwrap();
    second_ask.operation = Operation::Ask {
        backend: BackendId::new("fast_b").unwrap(),
    };
    let mut second_probability = two_source
        .nodes
        .iter()
        .find(|node| node.id.as_str() == "probability")
        .unwrap()
        .clone();
    second_probability.id = NodeId::new("second_probability").unwrap();
    second_probability.guard = None;
    second_probability.inputs.insert(
        "answers".into(),
        Binding::Node {
            node: second_ask.id.clone(),
            port: "answers".into(),
            path: Vec::new(),
        },
    );
    two_source.nodes.extend([second_ask, second_probability]);
    two_source.nodes.push(NodeSpec {
        id: NodeId::new("choose_raw").unwrap(),
        operation: Operation::Coalesce,
        inputs: BTreeMap::from([
            (
                "value".into(),
                Binding::Node {
                    node: NodeId::new("probability").unwrap(),
                    port: "result".into(),
                    path: Vec::new(),
                },
            ),
            (
                "default".into(),
                Binding::Node {
                    node: NodeId::new("second_probability").unwrap(),
                    port: "result".into(),
                    path: Vec::new(),
                },
            ),
        ]),
        guard: None,
    });
    two_source
        .nodes
        .iter_mut()
        .find(|node| node.id.as_str() == "calibrate")
        .unwrap()
        .inputs
        .insert(
            "value".into(),
            Binding::Node {
                node: NodeId::new("choose_raw").unwrap(),
                port: "result".into(),
                path: Vec::new(),
            },
        );
    let two_source_path = root.path().join("two-source.json");
    std::fs::write(&two_source_path, serde_json::to_vec(&two_source).unwrap()).unwrap();
    let mut two_exchanges = exchanges.clone();
    let mut second_exchange = two_exchanges.exchanges[0].clone();
    second_exchange.request.backend = BackendId::new("fast_b").unwrap();
    second_exchange.response.model = "model-b".into();
    two_exchanges.exchanges.push(second_exchange);
    let two_recording = root.path().join("two-source-recording.json");
    std::fs::write(
        &two_recording,
        two_exchanges.to_json(8 * 1_048_576).unwrap(),
    )
    .unwrap();
    let two_source_input = root.path().join("two-source-input.json");
    std::fs::write(&two_source_input, b"{\"text\":\"case-text-0\"}").unwrap();
    let two_source_replay = command(&[
        "replay",
        path(&two_source_path),
        "--recording",
        path(&two_recording),
        "--input",
        path(&two_source_input),
    ]);
    assert!(!two_source_replay.status.success());
    let two_source_failure: serde_json::Value =
        serde_json::from_slice(&two_source_replay.stdout).unwrap();
    assert_eq!(two_source_failure["error"]["code"], "model_mismatch");
    assert_eq!(
        two_source_failure["error"]["context"]["observed_count"],
        "2"
    );
    let mut guarded = calibrated.clone();
    let mut unrelated_ask = guarded
        .nodes
        .iter()
        .find(|node| node.id.as_str() == "ask")
        .unwrap()
        .clone();
    unrelated_ask.id = NodeId::new("unrelated_ask").unwrap();
    unrelated_ask.operation = Operation::Ask {
        backend: BackendId::new("fast_b").unwrap(),
    };
    let mut unrelated_probability = guarded
        .nodes
        .iter()
        .find(|node| node.id.as_str() == "probability")
        .unwrap()
        .clone();
    unrelated_probability.id = NodeId::new("unrelated_probability").unwrap();
    unrelated_probability.inputs.insert(
        "answers".into(),
        Binding::Node {
            node: unrelated_ask.id.clone(),
            port: "answers".into(),
            path: Vec::new(),
        },
    );
    let guard = NodeSpec {
        id: NodeId::new("unrelated_guard").unwrap(),
        operation: Operation::Compare {
            comparator: Comparator::Greater,
        },
        inputs: BTreeMap::from([
            (
                "a".into(),
                Binding::Node {
                    node: unrelated_probability.id.clone(),
                    port: "result".into(),
                    path: Vec::new(),
                },
            ),
            (
                "b".into(),
                Binding::Literal {
                    value: Datum::new(
                        "threshold",
                        Value::Probability(Probability::new(0.5).unwrap()),
                    )
                    .unwrap(),
                    value_type: ValueType::Probability,
                },
            ),
        ]),
        guard: None,
    };
    guarded
        .nodes
        .extend([unrelated_ask, unrelated_probability, guard]);
    guarded
        .nodes
        .iter_mut()
        .find(|node| node.id.as_str() == "calibrate")
        .unwrap()
        .guard = Some(Binding::Node {
        node: NodeId::new("unrelated_guard").unwrap(),
        port: "result".into(),
        path: Vec::new(),
    });
    guarded.outputs.remove("calibrated_support");
    let guarded_path = root.path().join("unrelated-guard.json");
    std::fs::write(&guarded_path, serde_json::to_vec(&guarded).unwrap()).unwrap();
    let mut guarded_recording = exchanges.clone();
    let mut unrelated_exchange = guarded_recording.exchanges[0].clone();
    unrelated_exchange.request.backend = BackendId::new("fast_b").unwrap();
    unrelated_exchange.response.model = "model-b".into();
    unrelated_exchange.response.answers.insert(
        "q".into(),
        Answer::Boolean {
            probability: Probability::new(0.95).unwrap(),
        },
    );
    guarded_recording.exchanges.push(unrelated_exchange);
    let guarded_recording_path = root.path().join("unrelated-guard-recording.json");
    std::fs::write(
        &guarded_recording_path,
        guarded_recording.to_json(8 * 1_048_576).unwrap(),
    )
    .unwrap();
    let guarded_input_path = root.path().join("unrelated-guard-input.json");
    std::fs::write(&guarded_input_path, b"{\"text\":\"case-text-0\"}").unwrap();
    let guarded_run = command(&[
        "replay",
        path(&guarded_path),
        "--recording",
        path(&guarded_recording_path),
        "--input",
        path(&guarded_input_path),
    ]);
    assert!(
        guarded_run.status.success(),
        "{}",
        String::from_utf8_lossy(&guarded_run.stdout)
    );
    let guarded_report: serde_json::Value = serde_json::from_slice(&guarded_run.stdout).unwrap();
    assert_eq!(
        guarded_report["calibration_identity"][0]["status"],
        "reported_name_match"
    );
    let mut mapped = calibrated.clone();
    let mut mapped_calibrate = mapped
        .nodes
        .iter()
        .find(|node| node.id.as_str() == "calibrate")
        .unwrap()
        .clone();
    mapped_calibrate.inputs.insert(
        "value".into(),
        Binding::Input {
            name: "raw".into(),
            path: Vec::new(),
        },
    );
    mapped.nodes.retain(|node| node.id.as_str() != "calibrate");
    mapped.outputs.remove("calibrated_support");
    mapped.subgraphs.insert(
        "each".into(),
        GraphBody {
            inputs: BTreeMap::from([
                ("item".into(), ValueType::Boolean),
                ("raw".into(), ValueType::Probability),
            ]),
            nodes: vec![mapped_calibrate],
            outputs: BTreeMap::from([(
                "result".into(),
                Binding::Node {
                    node: NodeId::new("calibrate").unwrap(),
                    port: "result".into(),
                    path: Vec::new(),
                },
            )]),
        },
    );
    mapped.nodes.push(NodeSpec {
        id: NodeId::new("map").unwrap(),
        operation: Operation::Map {
            graph: "each".into(),
        },
        inputs: BTreeMap::from([
            (
                "items".into(),
                Binding::Literal {
                    value: Datum::new(
                        "items",
                        Value::List(vec![
                            Datum::new("first", Value::Boolean(true)).unwrap(),
                            Datum::new("second", Value::Boolean(false)).unwrap(),
                        ]),
                    )
                    .unwrap(),
                    value_type: ValueType::list(ValueType::Boolean),
                },
            ),
            (
                "raw".into(),
                Binding::Node {
                    node: NodeId::new("probability").unwrap(),
                    port: "result".into(),
                    path: Vec::new(),
                },
            ),
        ]),
        guard: None,
    });
    mapped.outputs.insert(
        "mapped".into(),
        Binding::Node {
            node: NodeId::new("map").unwrap(),
            port: "result".into(),
            path: Vec::new(),
        },
    );
    let mapped_path = root.path().join("mapped-calibration.json");
    std::fs::write(&mapped_path, serde_json::to_vec(&mapped).unwrap()).unwrap();
    let mapped_run = command(&[
        "replay",
        path(&mapped_path),
        "--recording",
        path(&recording),
        "--input",
        path(&guarded_input_path),
    ]);
    assert!(
        mapped_run.status.success(),
        "{}",
        String::from_utf8_lossy(&mapped_run.stdout)
    );
    let mapped_report: serde_json::Value = serde_json::from_slice(&mapped_run.stdout).unwrap();
    assert_eq!(
        mapped_report["calibration_identity"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(
        mapped_report["calibration_identity"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["status"] == "reported_name_match")
    );
    for action in ["validate", "inspect"] {
        let checked = command(&[action, path(&calibrated_path)]);
        assert!(
            checked.status.success(),
            "{}",
            String::from_utf8_lossy(&checked.stderr)
        );
        assert!(String::from_utf8_lossy(&checked.stdout).contains("calibrated_probability"));
    }
    assert_ne!(
        sapho_graph::graph_semantic_identity(&GraphSpec::parse(raw_graph()).unwrap()).unwrap(),
        sapho_graph::graph_semantic_identity(&calibrated).unwrap(),
    );
    let mut dual_data = data.clone();
    for case in &mut dual_data.cases {
        case.labels
            .insert("calibrated_support".into(), case.labels["raw_support"]);
    }
    let dual_path = root.path().join("dual-dataset.json");
    std::fs::write(&dual_path, serde_json::to_vec(&dual_data).unwrap()).unwrap();
    let measured = command(&[
        "measure",
        path(&calibrated_path),
        "--dataset",
        path(&dual_path),
        "--replay",
        path(&recording),
        "--split",
        "development",
    ]);
    assert!(
        measured.status.success(),
        "{}",
        String::from_utf8_lossy(&measured.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&measured.stdout).unwrap();
    assert_eq!(
        report["measurement"]["outputs"]["raw_support"]["scored"],
        20
    );
    assert_eq!(
        report["measurement"]["outputs"]["calibrated_support"]["scored"],
        20
    );
    assert!(
        (report["measurement"]["outputs"]["raw_support"]["metrics"]["ece"]
            .as_f64()
            .unwrap()
            - 0.15)
            .abs()
            < 1e-9
    );
    assert!(
        report["measurement"]["outputs"]["calibrated_support"]["metrics"]["ece"]
            .as_f64()
            .unwrap()
            .abs()
            < 1e-9
    );
    assert_eq!(
        report["runs"][0]["report"]["calibration_identity"][0]["status"],
        "reported_name_match"
    );
    let mut other_knots = map.knots.clone();
    other_knots[0].calibrated = CalibratedProbability::new(0.3).unwrap();
    let other_map = CalibrationMap::new(
        map.dataset_id.clone(),
        map.fit_cases_digest.clone(),
        map.fit_observation_digest.clone(),
        map.fit_graph_semantic_identity.clone(),
        map.raw_output.clone(),
        map.fit_binding.clone(),
        map.fit_actual_model.clone(),
        map.case_count,
        other_knots,
    )
    .unwrap();
    let mut changed_graph = calibrated.clone();
    let node = changed_graph
        .nodes
        .iter_mut()
        .find(|node| node.id.as_str() == "calibrate")
        .unwrap();
    node.inputs.insert(
        "map".into(),
        Binding::Literal {
            value: Datum::new(
                "calibration_map",
                Value::CalibrationMap(Box::new(other_map)),
            )
            .unwrap(),
            value_type: sapho_core::ValueType::CalibrationMap,
        },
    );
    assert_ne!(
        sapho_graph::graph_semantic_identity(&calibrated).unwrap(),
        sapho_graph::graph_semantic_identity(&changed_graph).unwrap()
    );
    let changed_graph_path = root.path().join("changed-calibration.json");
    std::fs::write(
        &changed_graph_path,
        serde_json::to_vec(&changed_graph).unwrap(),
    )
    .unwrap();
    let changed_measure = command(&[
        "measure",
        path(&changed_graph_path),
        "--dataset",
        path(&dual_path),
        "--replay",
        path(&recording),
        "--split",
        "development",
    ]);
    assert!(
        changed_measure.status.success(),
        "{}",
        String::from_utf8_lossy(&changed_measure.stderr)
    );
    let changed_report: serde_json::Value =
        serde_json::from_slice(&changed_measure.stdout).unwrap();
    let asks = |value: &serde_json::Value| {
        value["runs"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|case| case["report"]["trace"]["nodes"].as_array().unwrap().iter())
            .filter(|node| node["operation"]["kind"] == "ask")
            .map(|node| node["model"].clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(asks(&report), asks(&changed_report));
    assert_ne!(report["graph"]["source"], changed_report["graph"]["source"]);
    assert_ne!(
        report["measurement"]["outputs"]["calibrated_support"]["metrics"]["brier"],
        changed_report["measurement"]["outputs"]["calibrated_support"]["metrics"]["brier"]
    );
    let held_measure = command(&[
        "measure",
        path(&calibrated_path),
        "--dataset",
        path(&dual_path),
        "--replay",
        path(&recording),
        "--split",
        "held_out",
    ]);
    assert!(
        held_measure.status.success(),
        "{}",
        String::from_utf8_lossy(&held_measure.stderr)
    );
    let held_report: serde_json::Value = serde_json::from_slice(&held_measure.stdout).unwrap();
    assert_eq!(held_report["measurement"]["selected_cases"], 2);
    assert_eq!(
        held_report["measurement"]["outputs"]["raw_support"]["scored"],
        2
    );
    assert_eq!(
        held_report["measurement"]["outputs"]["calibrated_support"]["scored"],
        2
    );
    let mut changed_recording = exchanges.clone();
    for exchange in &mut changed_recording.exchanges {
        exchange.response.model = "model-b".into();
    }
    let changed_path = root.path().join("changed-recording.json");
    std::fs::write(
        &changed_path,
        changed_recording.to_json(8 * 1_048_576).unwrap(),
    )
    .unwrap();
    let refit_path = root.path().join("model-b-map.json");
    let refit = command(&[
        "fit-calibration",
        path(&graph),
        "--dataset",
        path(&dataset),
        "--replay",
        path(&changed_path),
        "--output-name",
        "raw_support",
        "--binding",
        "fast_a",
        "--split",
        "development",
        "--output",
        path(&refit_path),
    ]);
    assert!(
        refit.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&refit.stdout),
        String::from_utf8_lossy(&refit.stderr)
    );
    let refitted: Binding = serde_json::from_slice(&std::fs::read(&refit_path).unwrap()).unwrap();
    let Binding::Literal { value, .. } = refitted else {
        panic!("refitted literal")
    };
    let Value::CalibrationMap(refitted_map) = value.value else {
        panic!("refitted map")
    };
    assert_eq!(refitted_map.fit_actual_model, "model-b");
    assert_ne!(refitted_map.map_id, map.map_id);
    let mapped_mismatch = command(&[
        "replay",
        path(&mapped_path),
        "--recording",
        path(&changed_path),
        "--input",
        path(&guarded_input_path),
    ]);
    assert!(!mapped_mismatch.status.success());
    let mapped_failure: serde_json::Value =
        serde_json::from_slice(&mapped_mismatch.stdout).unwrap();
    assert_eq!(mapped_failure["error"]["code"], "model_mismatch");
    assert!(
        mapped_failure["trace"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |node| node["path"] == serde_json::json!(["root", "map", "first", "calibrate"])
                    && node["status"] == "failed"
            )
    );
    assert!(
        mapped_failure["trace"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["operation"]["kind"] == "ask"
                && node["model"]["response"]["model"] == "model-b")
    );
    let mismatch = command(&[
        "measure",
        path(&calibrated_path),
        "--dataset",
        path(&dual_path),
        "--replay",
        path(&changed_path),
        "--split",
        "development",
    ]);
    assert!(!mismatch.status.success());
    assert!(String::from_utf8_lossy(&mismatch.stdout).contains("model_mismatch"));
    let input_path = root.path().join("case-input.json");
    std::fs::write(&input_path, b"{\"text\":\"case-text-0\"}").unwrap();
    let replay_mismatch = command(&[
        "replay",
        path(&calibrated_path),
        "--recording",
        path(&changed_path),
        "--input",
        path(&input_path),
    ]);
    assert!(!replay_mismatch.status.success());
    let replay_report: serde_json::Value = serde_json::from_slice(&replay_mismatch.stdout).unwrap();
    assert_eq!(replay_report["error"]["code"], "model_mismatch");
    assert!(
        replay_report["trace"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["operation"]["kind"] == "ask"
                && node["model"]["response"]["model"] == "model-b")
    );
    #[cfg(feature = "clm")]
    {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(std::time::Instant::now() < deadline, "no provider request");
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(error) => panic!("accept failed: {error}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                .unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut buffer = [0; 1024];
                let n = stream.read(&mut buffer).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buffer[..n]);
                if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..index]);
                    let length: usize = headers
                        .lines()
                        .find_map(|line| {
                            line.to_lowercase()
                                .strip_prefix("content-length:")
                                .map(|value| value.trim().parse().unwrap())
                        })
                        .unwrap();
                    if bytes.len() >= index + 4 + length {
                        break;
                    }
                }
            }
            let body = br#"{"model":"model-b","answers":{"q":{"type":"noul","noul":0.05}},"usage":{"input_tokens":1,"output_tokens":0,"billing_units":1}}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(body).unwrap();
        });
        let bindings = root.path().join("live-bindings.yaml");
        std::fs::write(
            &bindings,
            "fast_a:\n  provider: clm\n  model: requested\n  distribution_policy: {kind: strict}\n",
        )
        .unwrap();
        let live_mismatch = Command::new(env!("CARGO_BIN_EXE_sapho"))
            .env_clear()
            .env("CLM_BASE_URL", endpoint)
            .env("CLM_API_KEY", "synthetic-credential")
            .args([
                "run",
                path(&calibrated_path),
                "--input",
                path(&input_path),
                "--bindings",
                path(&bindings),
            ])
            .output()
            .unwrap();
        server.join().unwrap();
        assert!(!live_mismatch.status.success());
        let live_report: serde_json::Value = serde_json::from_slice(&live_mismatch.stdout).unwrap();
        assert_eq!(live_report["error"]["code"], "model_mismatch");
        assert_eq!(live_report["error"]["context"]["map_id"], map.map_id);
        assert_eq!(live_report["error"]["context"]["expected_model"], "model-a");
        assert_eq!(live_report["error"]["context"]["observed_model"], "model-b");
        assert_eq!(
            live_report["error"]["context"]["expected_binding"],
            "fast_a"
        );
        assert_eq!(
            live_report["error"]["context"]["observed_binding"],
            "fast_a"
        );
    }
    let mut changed_prediction = exchanges.clone();
    changed_prediction.exchanges[0].response.answers.insert(
        "q".into(),
        Answer::Boolean {
            probability: Probability::new(0.15).unwrap(),
        },
    );
    let prediction_path = root.path().join("changed-prediction.json");
    std::fs::write(
        &prediction_path,
        changed_prediction.to_json(8 * 1_048_576).unwrap(),
    )
    .unwrap();
    let mismatch = command(&[
        "measure",
        path(&calibrated_path),
        "--dataset",
        path(&dual_path),
        "--replay",
        path(&prediction_path),
        "--split",
        "development",
    ]);
    assert!(!mismatch.status.success());
    let mismatch_report: serde_json::Value = serde_json::from_slice(&mismatch.stdout).unwrap();
    assert_eq!(mismatch_report["error"]["code"], "model_mismatch");
    assert!(mismatch_report.get("measurement").is_none());
    assert!(
        mismatch_report["runs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|case| {
                case["report"]["trace"]["nodes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|node| {
                        node["operation"]["kind"] == "ask"
                            && node["model"]["response"]["model"] == "model-a"
                    })
            })
    );
    let held = root.path().join("held.json");
    let run = command(&[
        "fit-calibration",
        path(&graph),
        "--dataset",
        path(&dataset),
        "--replay",
        path(&recording),
        "--output-name",
        "raw_support",
        "--binding",
        "fast_a",
        "--split",
        "held_out",
        "--output",
        path(&held),
    ]);
    assert!(!run.status.success());
    assert!(!held.exists());
    let existing = command(&[
        "fit-calibration",
        path(&graph),
        "--dataset",
        path(&dataset),
        "--replay",
        path(&recording),
        "--output-name",
        "raw_support",
        "--binding",
        "fast_a",
        "--split",
        "development",
        "--output",
        path(&first),
    ]);
    assert!(!existing.status.success());
    assert_eq!(bytes, std::fs::read(&first).unwrap());

    let development_only = root.path().join("development-only-recording.json");
    let mut no_held_out = exchanges.clone();
    no_held_out.exchanges.truncate(20);
    std::fs::write(
        &development_only,
        no_held_out.to_json(8 * 1_048_576).unwrap(),
    )
    .unwrap();
    let no_held_out_map = root.path().join("no-held-out-map.json");
    let development_fit = command(&[
        "fit-calibration",
        path(&graph),
        "--dataset",
        path(&dataset),
        "--replay",
        path(&development_only),
        "--output-name",
        "raw_support",
        "--binding",
        "fast_a",
        "--split",
        "development",
        "--output",
        path(&no_held_out_map),
    ]);
    assert!(development_fit.status.success());
    assert_eq!(bytes, std::fs::read(&no_held_out_map).unwrap());

    let mut answers_graph = GraphSpec::parse(raw_graph()).unwrap();
    answers_graph.outputs.insert(
        "answers".into(),
        Binding::Node {
            node: NodeId::new("ask").unwrap(),
            port: "answers".into(),
            path: Vec::new(),
        },
    );
    let answers_path = root.path().join("answers-output.json");
    std::fs::write(&answers_path, serde_json::to_vec(&answers_graph).unwrap()).unwrap();
    let non_probability_output = root.path().join("non-probability-map.json");
    let non_probability = command(&[
        "fit-calibration",
        path(&answers_path),
        "--dataset",
        path(&dataset),
        "--replay",
        path(&recording),
        "--output-name",
        "answers",
        "--binding",
        "fast_a",
        "--split",
        "development",
        "--output",
        path(&non_probability_output),
    ]);
    assert!(!non_probability.status.success());
    assert!(String::from_utf8_lossy(&non_probability.stdout).contains("type_mismatch"));
    assert!(!non_probability_output.exists());

    let mut no_development = data.clone();
    for case in &mut no_development.cases {
        case.split = Split::HeldOut;
    }
    let no_development_path = root.path().join("no-development.json");
    std::fs::write(
        &no_development_path,
        serde_json::to_vec(&no_development).unwrap(),
    )
    .unwrap();
    let absent_development_output = root.path().join("no-development-map.json");
    let absent_development = command(&[
        "fit-calibration",
        path(&graph),
        "--dataset",
        path(&no_development_path),
        "--replay",
        path(&recording),
        "--output-name",
        "raw_support",
        "--binding",
        "fast_a",
        "--split",
        "development",
        "--output",
        path(&absent_development_output),
    ]);
    assert!(!absent_development.status.success());
    assert!(
        String::from_utf8_lossy(&absent_development.stdout).contains("empty_split"),
        "{}",
        String::from_utf8_lossy(&absent_development.stdout)
    );
    assert!(!absent_development_output.exists());

    let mut failed_recording = exchanges.clone();
    failed_recording.exchanges.remove(0);
    let failed_recording_path = root.path().join("failed-fit-recording.json");
    std::fs::write(
        &failed_recording_path,
        failed_recording.to_json(8 * 1_048_576).unwrap(),
    )
    .unwrap();
    let failed_fit_output = root.path().join("failed-fit-map.json");
    let failed_fit = command(&[
        "fit-calibration",
        path(&graph),
        "--dataset",
        path(&dataset),
        "--replay",
        path(&failed_recording_path),
        "--output-name",
        "raw_support",
        "--binding",
        "fast_a",
        "--split",
        "development",
        "--output",
        path(&failed_fit_output),
    ]);
    assert!(!failed_fit.status.success());
    assert!(String::from_utf8_lossy(&failed_fit.stdout).contains("model_mismatch"));
    assert!(!failed_fit_output.exists());

    let wrong_binding_output = root.path().join("wrong-binding-map.json");
    let wrong_binding = command(&[
        "fit-calibration",
        path(&graph),
        "--dataset",
        path(&dataset),
        "--replay",
        path(&recording),
        "--output-name",
        "raw_support",
        "--binding",
        "fast_b",
        "--split",
        "development",
        "--output",
        path(&wrong_binding_output),
    ]);
    assert!(!wrong_binding.status.success());
    assert!(String::from_utf8_lossy(&wrong_binding.stdout).contains("model_mismatch"));
    assert!(!wrong_binding_output.exists());
}
