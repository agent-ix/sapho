// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! IT-017 synthetic stock fit and exact replay; no live provider.
use sapho_core::{
    Answer, BackendId, CalibratedProbability, CalibrationMap, Datum, DistributionPolicy, Inputs,
    ItemId, ModelRequest, ModelResponse, NamedQuestion, NodeId, Probability, Question, SourceId,
    Value,
};
use sapho_evidence::{Case, Dataset, LabelKind, LabelProvenance, Split};
use sapho_graph::{Binding, GraphSpec, NodeSpec, Operation};
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
/// IT-017-SC-01, IT-017-SC-03, IT-017-SC-05, IT-017-SC-06
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
    assert!(String::from_utf8_lossy(&mismatch.stdout).contains("model_mismatch"));
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
}
