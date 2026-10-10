// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Claude CLI metadata, private credential and replay seams.
#![cfg(feature = "claude")]
use async_trait::async_trait;
use ix_cli_kit::secrets::*;
use sapho_cli::*;
use sapho_core::*;
use sapho_recording::*;
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

struct Store {
    calls: AtomicUsize,
}
impl CredentialBackend for &Store {
    fn get(
        &self,
        scope: &AppScope,
        key: &SecretKey,
    ) -> std::result::Result<Option<SecretValue>, SecretError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert_eq!(scope.as_str(), "agent-ix/sapho");
        assert_eq!(key.as_str(), "anthropic-api-key");
        Ok(Some(SecretValue::new("store-key")))
    }
    fn set(
        &self,
        _: &AppScope,
        _: &SecretKey,
        _: &SecretValue,
    ) -> std::result::Result<(), SecretError> {
        panic!("no writes")
    }
    fn delete(
        &self,
        _: &AppScope,
        _: &SecretKey,
    ) -> std::result::Result<DeleteStatus, SecretError> {
        panic!("no deletes")
    }
    fn status(&self, _: &AppScope, _: &SecretKey) -> std::result::Result<Presence, SecretError> {
        panic!("no status")
    }
}
/// Trace: FR-090-AC-1, FR-090-AC-2
#[test]
fn claude_metadata_and_credential_sources_are_private() {
    let binding: BindingConfig = serde_json::from_value(serde_json::json!({
        "provider":"claude", "model":"claude-sonnet-4-6", "distribution_policy":{"kind":"strict"}
    }))
    .unwrap();
    assert_eq!(binding.provider, Provider::Claude);
    assert_eq!(
        Provider::Claude.credential_environment(),
        Some("ANTHROPIC_API_KEY")
    );
    for key in ["api_key", "credential", "endpoint", "workspace_id"] {
        let mut value = serde_json::to_value(&binding).unwrap();
        value[key] = serde_json::json!("private");
        assert!(
            serde_json::from_value::<BindingConfig>(value).is_err(),
            "{key}"
        );
    }
    let native = Store {
        calls: AtomicUsize::new(0),
    };
    let store = SecretStore::new(&native);
    let explicit = resolve_credential(
        Provider::Claude,
        &store,
        Some(SecretValue::new("explicit-key")),
        Some("environment-key".into()),
    )
    .unwrap()
    .unwrap();
    assert_eq!(explicit.expose_secret(), "explicit-key");
    assert_eq!(native.calls.load(Ordering::SeqCst), 0);
    let environment = resolve_credential(
        Provider::Claude,
        &store,
        None,
        Some("environment-key".into()),
    )
    .unwrap()
    .unwrap();
    assert_eq!(environment.expose_secret(), "environment-key");
    assert_eq!(native.calls.load(Ordering::SeqCst), 0);
    let stored = resolve_credential(Provider::Claude, &store, None, None)
        .unwrap()
        .unwrap();
    assert_eq!(stored.expose_secret(), "store-key");
    assert_eq!(native.calls.load(Ordering::SeqCst), 1);
    assert!(replay_bindings(&Recording::default(), None, 1024).is_ok());
}
struct Fake;
#[async_trait]
impl sapho_claude::Transport for Fake {
    async fn post(&self, _: &[u8], _: usize) -> Result<sapho_claude::HttpResponse> {
        let output = serde_json::json!({"answers":[{"id":"q","kind":"boolean","probability":0.8}]})
            .to_string();
        Ok(sapho_claude::HttpResponse { status: 200,
            body: serde_json::to_vec(&serde_json::json!({"type":"message","id":"msg_x",
                "role":"assistant","model":"actual-claude","stop_reason":"end_turn","content":[{"type":"text","text":output}],
                "usage":{"input_tokens":5,"output_tokens":2}})).unwrap() })
    }
}
/// Trace: FR-090-AC-2, FR-090-AC-3
#[tokio::test]
async fn cli_registry_records_and_replays_without_claude_transport() {
    let request = ModelRequest {
        backend: BackendId::new("reader").unwrap(),
        model: "requested-claude".into(),
        expected_model: None,
        distribution_policy: DistributionPolicy::Strict {},
        state: Value::Record(BTreeMap::new()),
        questions: vec![NamedQuestion {
            id: "q".into(),
            question: Question::Boolean {
                instructions: "Is it?".into(),
                yes: "Yes".into(),
                no: "No".into(),
            },
        }],
    };
    let backend = Arc::new(
        sapho_claude::ClaudeBackend::with_transport(
            Arc::new(Fake),
            sapho_claude::Limits::default(),
        )
        .unwrap(),
    );
    let mut live = BackendRegistry::default();
    live.register(
        request.backend.clone(),
        BackendBinding {
            backend,
            model: request.model.clone(),
            expected_model: None,
            distribution_policy: request.distribution_policy,
        },
    )
    .unwrap();
    let (registry, recorders) =
        recording_bindings(std::slice::from_ref(&request.backend), &live, 1_048_576).unwrap();
    let response = registry
        .get(&request.backend)
        .unwrap()
        .backend
        .infer(&request)
        .await
        .unwrap();
    let recording = recorders[0].snapshot().unwrap();
    assert_eq!(recording.exchanges.len(), 1);
    let bytes = recording.to_json(1_048_576).unwrap();
    let explicit = BTreeMap::from([(
        request.backend.clone(),
        BindingConfig {
            provider: Provider::Claude,
            model: request.model.clone(),
            expected_model: None,
            distribution_policy: request.distribution_policy,
            ollama: None,
        },
    )]);
    for metadata in [None, Some(&explicit)] {
        let replay = replay_bindings_json(&bytes, metadata, 1_048_576).unwrap();
        assert_eq!(
            replay
                .get(&request.backend)
                .unwrap()
                .backend
                .infer(&request)
                .await
                .unwrap(),
            response
        );
    }
    let mut changed = explicit;
    changed.get_mut(&request.backend).unwrap().model = "other".into();
    assert!(
        matches!(replay_bindings_json(&bytes, Some(&changed), 1_048_576), Err(CliError::Engine(e)) if e.code == ErrorCode::RecordingMismatch)
    );
}

/// Trace: FR-090-AC-4
#[test]
fn offline_metadata_and_dataset_contracts_stay_separate() {
    use sapho_evidence::Dataset;
    let dataset = Dataset {
        id: SourceId::new("curated").unwrap(),
        cases: Vec::new(),
    };
    dataset.validate(1).unwrap();
    let serialized = serde_json::to_vec(&dataset).unwrap();
    let loaded: Dataset = serde_json::from_slice(&serialized).unwrap();
    assert_eq!(loaded, dataset);
    let binding: Bindings = sapho_graph::parse_config(
        "reader:\n  provider: claude\n  model: claude-sonnet-4-6\n  distribution_policy: {kind: strict}\n",
        sapho_graph::GraphFormat::Yaml,
    ).unwrap();
    assert!(live_bindings(&[], &binding).is_ok());
    assert!(replay_bindings(&Recording::default(), Some(&binding), 4096).is_ok());
}
