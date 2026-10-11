// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Feature-free Claude replay and stock feature refusal remain offline.
#![cfg(not(feature = "claude"))]
use async_trait::async_trait;
use sapho_cli::Runner;
use sapho_core::{
    Answer, BackendBinding, BackendId, BackendRegistry, Datum, DistributionPolicy, ModelBackend,
    ModelRequest, ModelResponse, PrimitiveRegistry, Probability, RawExchange, Result, Usage, Value,
};
use sapho_graph::GraphSpec;
use sapho_recording::RecordingBackend;
use sapho_runtime::RunLimits;
use std::{collections::BTreeMap, path::Path, process::Command, sync::Arc};

struct Synthetic;
#[async_trait]
impl ModelBackend for Synthetic {
    async fn infer(&self, _request: &ModelRequest) -> Result<ModelResponse> {
        Ok(ModelResponse {
            model: "actual-claude".into(),
            raw: Some(RawExchange {
                request: "{\"model\":\"requested-claude\"}".into(),
                response: "{\"model\":\"actual-claude\"}".into(),
            }),
            answers: BTreeMap::from([(
                "q".into(),
                Answer::Boolean {
                    probability: Probability::new(0.8)?,
                },
            )]),
            usage: Some(Usage {
                billing_units: None,
                input_tokens: 12,
                output_tokens: 3,
            }),
        })
    }
}
fn path(value: &Path) -> &str {
    value.to_str().unwrap()
}
fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_sapho"))
        .args(args)
        .env_clear()
        .env("ANTHROPIC_API_KEY", "poisoned-offline-key")
        .env("ANTHROPIC_WORKSPACE_ID", "invalid-workspace")
        .output()
        .unwrap()
}
/// Trace: FR-090-AC-2, FR-090-AC-3, IT-019-SC-04
#[tokio::test]
async fn feature_free_replay_uses_recorded_claude_identity_and_never_resolves_secret() {
    let root = tempfile::tempdir().unwrap();
    let graph = GraphSpec::parse(include_str!("../../../examples/graphs/multilayer.yaml")).unwrap();
    let graph_path = root.path().join("graph.json");
    std::fs::write(&graph_path, serde_json::to_vec(&graph).unwrap()).unwrap();
    let input_path = root.path().join("input.json");
    std::fs::write(&input_path, br#"{"text":"synthetic"}"#).unwrap();
    let metadata = root.path().join("bindings.yaml");
    std::fs::write(
        &metadata,
        "judge: {provider: claude, model: requested-claude, distribution_policy: {kind: strict}}\n",
    )
    .unwrap();
    let recorder = Arc::new(RecordingBackend::new(Arc::new(Synthetic), 1_048_576).unwrap());
    let mut registry = BackendRegistry::default();
    registry
        .register(
            BackendId::new("judge").unwrap(),
            BackendBinding {
                backend: recorder.clone(),
                model: "requested-claude".into(),
                expected_model: None,
                distribution_policy: DistributionPolicy::Strict {},
            },
        )
        .unwrap();
    let inputs = BTreeMap::from([(
        "text".into(),
        Datum::new("text", Value::Text("synthetic".into())).unwrap(),
    )]);
    let report = Runner::new(&graph, &PrimitiveRegistry::default(), registry)
        .unwrap()
        .run(&inputs, RunLimits::default(), None)
        .await;
    assert!(report.error.is_none(), "{:?}", report.error);
    let recording_path = root.path().join("recording.json");
    let recording = recorder.snapshot().unwrap();
    assert_eq!(recording.exchanges.len(), 2);
    assert_eq!(recording.exchanges[0].response.model, "actual-claude");
    assert_eq!(
        recording.exchanges[0]
            .response
            .usage
            .as_ref()
            .unwrap()
            .input_tokens,
        12
    );
    assert!(
        recording.exchanges[0]
            .response
            .raw
            .as_ref()
            .unwrap()
            .response
            .contains("actual-claude")
    );
    recording.write_new(&recording_path, 1_048_576).unwrap();
    let replay = cli(&[
        "replay",
        path(&graph_path),
        "--input",
        path(&input_path),
        "--bindings",
        path(&metadata),
        "--recording",
        path(&recording_path),
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
        serde_json::to_value(report.outputs).unwrap()
    );
    let refused = cli(&[
        "run",
        path(&graph_path),
        "--input",
        path(&input_path),
        "--bindings",
        path(&metadata),
    ]);
    assert_eq!(refused.status.code(), Some(2));
    let error: serde_json::Value = serde_json::from_slice(&refused.stdout).unwrap();
    assert_eq!(error["error"]["kind"], "feature");
    let printed = format!(
        "{}{}",
        String::from_utf8_lossy(&replay.stdout),
        String::from_utf8_lossy(&refused.stdout)
    );
    assert!(!printed.contains("poisoned-offline-key"));
    assert!(!printed.contains("invalid-workspace"));
}
