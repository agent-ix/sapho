// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Stock offline Decisions metadata and replay without credentials or provider feature.
use async_trait::async_trait;
use sapho_cli::{Runner, replay_bindings_json};
use sapho_core::{
    BackendBinding, BackendId, BackendRegistry, Datum, DistributionPolicy, PrimitiveRegistry, Value,
};
use sapho_decisions::{DecisionsBackend, HttpResponse, Limits, Transport};
use sapho_graph::GraphSpec;
use sapho_recording::RecordingBackend;
use sapho_runtime::RunLimits;
use std::{collections::BTreeMap, path::Path, process::Command, sync::Arc};

struct SyntheticTransport;
#[async_trait]
impl Transport for SyntheticTransport {
    async fn post(&self, body: &[u8], _: usize) -> sapho_core::Result<HttpResponse> {
        let request: serde_json::Value = serde_json::from_slice(body).unwrap();
        assert_eq!(request["model"], "gpt-6-luna");
        assert_eq!(request["questions"][0]["name"], "contract_risk");
        assert_eq!(request["questions"][1]["name"], "test_gap");
        Ok(HttpResponse {
            status: 200,
            body: br#"{"model":"gpt-6-luna","answers":[{"type":"predicate","name":"contract_risk","probability":0.8},{"type":"predicate","name":"test_gap","probability":0.8}],"usage":{"input_tokens":42,"input_tokens_details":{"cached_tokens":0,"cache_write_tokens":0},"output_tokens":0,"output_tokens_details":{"reasoning_tokens":0},"total_tokens":42}}"#.to_vec(),
        })
    }
}
fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_sapho"))
        .args(args)
        .env_clear()
        .output()
        .unwrap()
}
fn path(path: &Path) -> &str {
    path.to_str().unwrap()
}
/// Trace: FR-047-AC-2, FR-087-AC-1, FR-087-AC-2, FR-087-AC-3, IT-018-SC-04
#[tokio::test]
async fn stock_replay_uses_recorded_identity_without_decisions_feature_or_key() {
    let root = tempfile::tempdir().unwrap();
    let graph =
        GraphSpec::parse(include_str!("../../../examples/graphs/code-review.yaml")).unwrap();
    let graph_path = root.path().join("graph.json");
    std::fs::write(&graph_path, serde_json::to_vec(&graph).unwrap()).unwrap();
    let id = BackendId::new("judge").unwrap();
    let backend = Arc::new(
        DecisionsBackend::with_transport(Arc::new(SyntheticTransport), Limits::default()).unwrap(),
    );
    let recorder = Arc::new(RecordingBackend::new(backend, 1_048_576).unwrap());
    let mut registry = BackendRegistry::default();
    registry
        .register(
            id,
            BackendBinding {
                backend: recorder.clone(),
                model: "gpt-6-luna".into(),
                expected_model: Some("gpt-6-luna".into()),
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
    let original = Runner::new(&graph, &PrimitiveRegistry::default(), registry)
        .unwrap()
        .run(&inputs, RunLimits::default(), None)
        .await;
    assert!(original.error.is_none(), "{:?}", original.error);
    let recording_path = root.path().join("recording.json");
    recorder
        .snapshot()
        .unwrap()
        .write_new(&recording_path, 1_048_576)
        .unwrap();
    let bytes = std::fs::read(&recording_path).unwrap();
    let registry = replay_bindings_json(&bytes, None, 1_048_576).unwrap();
    assert!(registry.get(&BackendId::new("judge").unwrap()).is_ok());
    let metadata = root.path().join("bindings.json");
    std::fs::write(&metadata, br#"{"judge":{"provider":"decisions","model":"gpt-6-luna","expected_model":"gpt-6-luna","distribution_policy":{"kind":"strict"}}}"#).unwrap();
    let input_path = root.path().join("input.json");
    std::fs::write(
        &input_path,
        br#"{"change":"synthetic","public_api_changed":true}"#,
    )
    .unwrap();
    let replay = cli(&[
        "replay",
        path(&graph_path),
        "--recording",
        path(&recording_path),
        "--bindings",
        path(&metadata),
        "--input",
        path(&input_path),
    ]);
    assert_eq!(
        replay.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&replay.stdout)
    );
    let result: serde_json::Value = serde_json::from_slice(&replay.stdout).unwrap();
    assert_eq!(result["error"], serde_json::Value::Null);
    assert_eq!(
        result["outputs"],
        serde_json::to_value(original.outputs).unwrap()
    );
    assert!(String::from_utf8_lossy(&replay.stdout).contains("input_tokens"));
    let validate = cli(&["validate", path(&graph_path)]);
    assert_eq!(validate.status.code(), Some(0));
    let inspect = cli(&["inspect", path(&graph_path)]);
    assert_eq!(inspect.status.code(), Some(0));
    let mismatch = root.path().join("mismatch.json");
    std::fs::write(&mismatch, br#"{"judge":{"provider":"decisions","model":"other","distribution_policy":{"kind":"strict"}}}"#).unwrap();
    let refused = cli(&[
        "replay",
        path(&graph_path),
        "--recording",
        path(&recording_path),
        "--bindings",
        path(&mismatch),
        "--input",
        path(&input_path),
    ]);
    assert_eq!(refused.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&refused.stdout).contains("recording_mismatch"));
}
