// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Public command-level literal sweep and unchanged tune/replay contracts.
use async_trait::async_trait;
use sapho_cli::{Runner, sweep};
use sapho_core::{
    BackendBinding, BackendId, BackendRegistry, Datum, DistributionPolicy, ModelBackend,
    ModelRequest, ModelResponse, PrimitiveRegistry, Probability, Value, ValueType,
};
use sapho_graph::{Binding, GraphFormat, GraphSpec, NodeSpec, Operation};
use sapho_recording::RecordingBackend;
use sapho_runtime::RunLimits;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::Arc,
};

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sapho"))
        .args(args)
        .env_clear()
        .output()
        .unwrap()
}
fn json(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&output.stdout)))
}
fn path(path: &Path) -> &str {
    path.to_str().unwrap()
}
fn source_graph() -> GraphSpec {
    let mut graph =
        GraphSpec::parse(include_str!("../../../examples/graphs/code-review.yaml")).unwrap();
    let compare = graph
        .nodes
        .iter_mut()
        .find(|node| node.id.as_str() == "risky_contract")
        .unwrap();
    let Binding::Literal { value, .. } = compare.inputs.get_mut("b").unwrap() else {
        panic!("cutoff")
    };
    value.id = sapho_core::ItemId::new("cutoff").unwrap();
    let mut weights = Datum::new(
        "weights",
        Value::List(vec![
            Datum::new("contract", Value::Number(0.25)).unwrap(),
            Datum::new("testing", Value::Number(0.75)).unwrap(),
        ]),
    )
    .unwrap();
    weights.sources.push(sapho_core::SourceRef {
        source: sapho_core::SourceId::new("synthetic").unwrap(),
        start: Some(0),
        end: Some(2),
    });
    graph.nodes.push(NodeSpec {
        id: sapho_core::NodeId::new("metadata").unwrap(),
        operation: Operation::Record {},
        inputs: BTreeMap::from([
            (
                "decision".into(),
                Binding::Node {
                    node: sapho_core::NodeId::new("needs_review").unwrap(),
                    port: "result".into(),
                    path: vec![],
                },
            ),
            (
                "weights".into(),
                Binding::Literal {
                    value: weights,
                    value_type: ValueType::list(ValueType::Number),
                },
            ),
        ]),
        guard: None,
    });
    graph.outputs.insert(
        "metadata".into(),
        Binding::Node {
            node: sapho_core::NodeId::new("metadata").unwrap(),
            port: "result".into(),
            path: vec![],
        },
    );
    graph
}
fn write(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap();
}
fn literal_cutoff(graph: &GraphSpec) -> f64 {
    let node = graph
        .nodes
        .iter()
        .find(|node| node.id.as_str() == "risky_contract")
        .unwrap();
    let Binding::Literal { value, .. } = &node.inputs["b"] else {
        panic!("literal")
    };
    let Value::Probability(p) = value.value else {
        panic!("probability")
    };
    p.get()
}

/// Trace: FR-079-AC-1, FR-079-AC-2, FR-079-AC-5, FR-080-AC-1, FR-080-AC-2, IT-016-SC-01, IT-016-SC-02, IT-016-SC-04
#[test]
fn stock_sweep_validates_full_grid_and_never_overwrites_sources_or_destination() {
    let root = tempfile::tempdir().unwrap();
    let graph = source_graph();
    let base = root.path().join("base.json");
    let base_bytes = serde_json::to_vec(&graph).unwrap();
    write(&base, &base_bytes);
    let grid = root.path().join("grid.json");
    let grid_bytes = br#"{"axes":[{"id":"cutoff","values":[0.6,0.7,0.8]},{"id":"weights","values":[[0.25,0.75],[0.5,0.5]]}]}"#;
    write(&grid, grid_bytes);
    let first = root.path().join("first");
    let second = root.path().join("second");
    for dest in [&first, &second] {
        let result = cli(&[
            "sweep",
            path(&base),
            "--grid",
            path(&grid),
            "--output-dir",
            path(dest),
        ]);
        assert_eq!(
            result.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&result.stdout)
        );
        assert_eq!(json(&result)["candidates"], 6);
    }
    assert_eq!(std::fs::read(&base).unwrap(), base_bytes);
    assert_eq!(std::fs::read(&grid).unwrap(), grid_bytes);
    for index in 0..6 {
        let name = format!("candidate-{index:04}.json");
        let bytes = std::fs::read(first.join(&name)).unwrap();
        assert_eq!(bytes, std::fs::read(second.join(&name)).unwrap());
        let candidate =
            GraphSpec::parse_with_format(std::str::from_utf8(&bytes).unwrap(), GraphFormat::Json)
                .unwrap();
        assert_eq!(literal_cutoff(&candidate), [0.6, 0.7, 0.8][index / 2]);
        assert_eq!(
            cli(&["validate", path(&first.join(&name))]).status.code(),
            Some(0)
        );
    }
    assert_eq!(
        std::fs::read(first.join("sweep-manifest.json")).unwrap(),
        std::fs::read(second.join("sweep-manifest.json")).unwrap()
    );
    let rerun = cli(&[
        "sweep",
        path(&base),
        "--grid",
        path(&grid),
        "--output-dir",
        path(&first),
    ]);
    assert_eq!(rerun.status.code(), Some(2));
    assert_eq!(std::fs::read(&base).unwrap(), base_bytes);
    assert_eq!(std::fs::read(&grid).unwrap(), grid_bytes);
    let invalid_grid = root.path().join("invalid.json");
    write(
        &invalid_grid,
        br#"{"axes":[{"id":"cutoff","values":[0.6,1.2]}]}"#,
    );
    let absent = root.path().join("absent");
    assert_eq!(
        cli(&[
            "sweep",
            path(&base),
            "--grid",
            path(&invalid_grid),
            "--output-dir",
            path(&absent)
        ])
        .status
        .code(),
        Some(2)
    );
    assert!(!absent.exists());
    let occupied = root.path().join("occupied");
    std::fs::create_dir(&occupied).unwrap();
    write(&occupied.join("candidate-0000.json"), b"sentinel");
    assert_eq!(
        cli(&[
            "sweep",
            path(&base),
            "--grid",
            path(&grid),
            "--output-dir",
            path(&occupied)
        ])
        .status
        .code(),
        Some(2)
    );
    assert_eq!(
        std::fs::read(occupied.join("candidate-0000.json")).unwrap(),
        b"sentinel"
    );
}

/// Trace: FR-080-AC-3, FR-080-AC-4, IT-016-SC-05, IT-016-SC-06
#[test]
fn generated_candidates_tune_like_handwritten_and_replay_unchanged_requests() {
    let root = tempfile::tempdir().unwrap();
    let graph = source_graph();
    let base = root.path().join("base.json");
    write(&base, &serde_json::to_vec(&graph).unwrap());
    let grid = root.path().join("grid.json");
    write(
        &grid,
        br#"{"axes":[{"id":"cutoff","values":[0.6,0.7,0.8]},{"id":"weights","values":[[0.25,0.75],[0.5,0.5]]}]}"#,
    );
    let dest = root.path().join("generated");
    let generated = sweep(&base, &grid, &dest, 16).unwrap();
    assert_eq!(generated.candidates, 6);
    let mut generated_paths = Vec::new();
    let mut handwritten_paths = Vec::new();
    for index in 0..6 {
        let cutoff = [0.6, 0.7, 0.8][index / 2];
        let weights = [[0.25, 0.75], [0.5, 0.5]][index % 2];
        generated_paths.push(dest.join(format!("candidate-{index:04}.json")));
        let mut handwritten = graph.clone();
        let node = handwritten
            .nodes
            .iter_mut()
            .find(|node| node.id.as_str() == "risky_contract")
            .unwrap();
        let Binding::Literal { value, .. } = node.inputs.get_mut("b").unwrap() else {
            panic!("literal")
        };
        value.value = Value::Probability(Probability::new(cutoff).unwrap());
        let metadata = handwritten
            .nodes
            .iter_mut()
            .find(|node| node.id.as_str() == "metadata")
            .unwrap();
        let Binding::Literal { value, .. } = metadata.inputs.get_mut("weights").unwrap() else {
            panic!("weights literal")
        };
        value.value = Value::List(
            weights
                .into_iter()
                .enumerate()
                .map(|(item, weight)| {
                    Datum::new(["contract", "testing"][item], Value::Number(weight)).unwrap()
                })
                .collect(),
        );
        let file = root.path().join(format!("handwritten-{index}.json"));
        write(&file, &serde_json::to_vec(&handwritten).unwrap());
        handwritten_paths.push(file);
    }
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dataset = repo.join("examples/data/code-review-dataset.json");
    let recording = repo.join("examples/recordings/code-review.json");
    let tune = |paths: &[PathBuf]| {
        let mut args = vec!["tune".to_owned()];
        for candidate in paths {
            args.push("--candidate".into());
            args.push(path(candidate).into());
        }
        args.extend([
            "--dataset".into(),
            path(&dataset).into(),
            "--output-name".into(),
            "needs_review".into(),
            "--metric".into(),
            "agreement".into(),
            "--replay".into(),
            path(&recording).into(),
        ]);
        let borrowed = args.iter().map(String::as_str).collect::<Vec<_>>();
        let output = cli(&borrowed);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        json(&output)
    };
    let a = tune(&generated_paths);
    let b = tune(&handwritten_paths);
    assert_eq!(a["ranking"].as_array().unwrap().len(), 6);
    for index in 0..6 {
        assert_eq!(
            a["candidates"][index]["measurement"],
            b["candidates"][index]["measurement"]
        );
        assert_eq!(a["candidates"][index]["runs"].as_array().unwrap().len(), 6);
        assert_eq!(a["ranking"][index]["index"], b["ranking"][index]["index"]);
    }
    let input = repo.join("examples/data/code-review-input.json");
    let mut requests = Vec::new();
    let mut metadata = Vec::new();
    for candidate in &generated_paths {
        let output = cli(&[
            "replay",
            path(candidate),
            "--recording",
            path(&recording),
            "--input",
            path(&input),
        ]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        let document = json(&output);
        metadata.push(document["outputs"]["metadata"].clone());
        let trace = document["trace"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|node| {
                node["model"]["request"]
                    .as_object()
                    .map(|_| node["model"]["request"].clone())
            })
            .collect::<Vec<_>>();
        assert_eq!(trace.len(), 1);
        requests.push(trace);
    }
    assert!(requests.windows(2).all(|pair| pair[0] == pair[1]));
    for index in (0..metadata.len()).step_by(2) {
        assert_ne!(metadata[index], metadata[index + 1]);
    }
}

struct Constant;
#[async_trait]
impl ModelBackend for Constant {
    async fn infer(&self, _: &ModelRequest) -> sapho_core::Result<ModelResponse> {
        Ok(ModelResponse {
            model: "jev-1.13.0".into(),
            raw: None,
            usage: None,
            answers: BTreeMap::from([
                (
                    "contract_risk".into(),
                    sapho_core::Answer::Boolean {
                        probability: Probability::new(0.8)?,
                    },
                ),
                (
                    "test_gap".into(),
                    sapho_core::Answer::Boolean {
                        probability: Probability::new(0.5)?,
                    },
                ),
            ]),
        })
    }
}

/// Trace: FR-080-AC-4, IT-016-SC-06
#[tokio::test]
async fn request_affecting_probability_literal_has_exact_replay_miss() {
    let root = tempfile::tempdir().unwrap();
    let mut graph = source_graph();
    let context = graph
        .nodes
        .iter_mut()
        .find(|node| node.id.as_str() == "context")
        .unwrap();
    context.inputs.insert(
        "feature".into(),
        Binding::Literal {
            value: Datum::new(
                "request_feature",
                Value::Probability(Probability::new(0.4).unwrap()),
            )
            .unwrap(),
            value_type: ValueType::Probability,
        },
    );
    let mut registry = BackendRegistry::default();
    let recorder = Arc::new(RecordingBackend::new(Arc::new(Constant), 1_000_000).unwrap());
    registry
        .register(
            BackendId::new("judge").unwrap(),
            BackendBinding {
                backend: recorder.clone(),
                model: "jev-1.13.0".into(),
                expected_model: Some("jev-1.13.0".into()),
                distribution_policy: DistributionPolicy::Strict {},
            },
        )
        .unwrap();
    let inputs = BTreeMap::from([
        (
            "change".into(),
            Datum::new("change", Value::Text("synthetic".into())).unwrap(),
        ),
        (
            "public_api_changed".into(),
            Datum::new("public_api_changed", Value::Boolean(true)).unwrap(),
        ),
    ]);
    let recorded = Runner::new(&graph, &PrimitiveRegistry::default(), registry)
        .unwrap()
        .run(&inputs, RunLimits::default(), None)
        .await;
    assert!(recorded.error.is_none(), "{:?}", recorded.error);
    let recording = root.path().join("recording.json");
    recorder
        .snapshot()
        .unwrap()
        .write_new(&recording, 1_000_000)
        .unwrap();
    let base = root.path().join("request.json");
    write(&base, &serde_json::to_vec(&graph).unwrap());
    let grid = root.path().join("grid.json");
    write(
        &grid,
        br#"{"axes":[{"id":"request_feature","values":[0.4,0.6]}]}"#,
    );
    let dest = root.path().join("candidate");
    sweep(&base, &grid, &dest, 16).unwrap();
    let input = root.path().join("input.json");
    write(
        &input,
        br#"{"change":"synthetic","public_api_changed":true}"#,
    );
    let first = cli(&[
        "replay",
        path(&dest.join("candidate-0000.json")),
        "--recording",
        path(&recording),
        "--input",
        path(&input),
    ]);
    assert_eq!(
        first.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&first.stdout)
    );
    let changed = cli(&[
        "replay",
        path(&dest.join("candidate-0001.json")),
        "--recording",
        path(&recording),
        "--input",
        path(&input),
    ]);
    assert_eq!(changed.status.code(), Some(2));
    assert_eq!(
        json(&changed)["error"]["code"],
        "replay_miss",
        "{}",
        String::from_utf8_lossy(&changed.stdout)
    );
}
