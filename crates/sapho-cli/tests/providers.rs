// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Shared credential and provider-independent replay boundaries; never touch real storage.
use ix_cli_kit::secrets::*;
use sapho_cli::*;
use sapho_core::*;
use sapho_recording::*;
use std::{
    collections::BTreeMap,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
struct Store {
    calls: AtomicUsize,
    result: std::result::Result<Option<SecretValue>, SecretError>,
}
impl CredentialBackend for &Store {
    fn get(
        &self,
        scope: &AppScope,
        key: &SecretKey,
    ) -> std::result::Result<Option<SecretValue>, SecretError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert_eq!(scope.as_str(), "agent-ix/sapho");
        assert_eq!(key.as_str(), "clm-api-key");
        self.result.clone()
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
/// Trace: FR-045-AC-1
#[test]
fn shared_credentials_enforce_precedence_identity_and_typed_redacted_errors() {
    let native = Store {
        calls: AtomicUsize::new(0),
        result: Ok(Some(SecretValue::new("synthetic-store"))),
    };
    let store = SecretStore::new(&native);
    let selected = resolve_credential(
        Provider::Clm,
        &store,
        Some(SecretValue::new("synthetic-explicit")),
        Some("synthetic-env".into()),
    )
    .unwrap()
    .unwrap();
    assert_eq!(selected.expose_secret(), "synthetic-explicit");
    let selected = resolve_credential(Provider::Clm, &store, None, Some("synthetic-env".into()))
        .unwrap()
        .unwrap();
    assert_eq!(selected.expose_secret(), "synthetic-env");
    assert_eq!(native.calls.load(Ordering::SeqCst), 0);
    let selected = resolve_credential(Provider::Clm, &store, None, None)
        .unwrap()
        .unwrap();
    assert_eq!(selected.expose_secret(), "synthetic-store");
    assert_eq!(native.calls.load(Ordering::SeqCst), 1);
    assert!(matches!(
        resolve_credential(Provider::Clm, &store, None, Some("".into())),
        Err(CliError::Credential(SecretError::InvalidEnvironment))
    ));
    assert_eq!(native.calls.load(Ordering::SeqCst), 1);
    for category in [
        SecretError::Locked,
        SecretError::Unavailable,
        SecretError::BackendFailure,
    ] {
        let native = Store {
            calls: AtomicUsize::new(0),
            result: Err(category),
        };
        let error =
            resolve_credential(Provider::Clm, &SecretStore::new(&native), None, None).unwrap_err();
        assert!(matches!(error, CliError::Credential(actual) if actual == category));
        assert!(!format!("{error:?}").contains("synthetic"));
        assert_ne!(
            serde_json::to_value(&error).unwrap()["detail"],
            serde_json::Value::Null
        );
    }
    let native = Store {
        calls: AtomicUsize::new(0),
        result: Ok(None),
    };
    assert!(
        resolve_credential(Provider::Clm, &SecretStore::new(&native), None, None)
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        resolve_credential(Provider::Jev, &store, Some(SecretValue::new(" ")), None),
        Err(CliError::CredentialInvalid)
    ));
}
/// Trace: FR-045-AC-2
#[test]
fn provider_models_are_explicit_except_the_clm_alias() {
    let binding: BindingConfig =
        serde_json::from_str(r#"{"provider":"clm","distribution_policy":{"kind":"strict"}}"#)
            .unwrap();
    assert_eq!(binding.provider, Provider::Clm);
    assert_eq!(binding.model, "clm-latest");
    assert!(
        serde_json::from_str::<BindingConfig>(
            r#"{"provider":"jev","distribution_policy":{"kind":"strict"}}"#
        )
        .is_err()
    );
    assert!(
        serde_json::from_str::<BindingConfig>(
            r#"{"provider":"clm","model":"","distribution_policy":{"kind":"strict"}}"#
        )
        .is_err()
    );
    for malformed in [
        serde_json::json!({"provider":false,"model":"m","distribution_policy":{"kind":"strict"}}),
        serde_json::json!({"provider":"clm","model":false,"distribution_policy":{"kind":"strict"}}),
        serde_json::json!({"provider":"clm","expected_model":false,"distribution_policy":{"kind":"strict"}}),
        serde_json::json!({"provider":"clm","distribution_policy":false}),
    ] {
        assert!(serde_json::from_value::<BindingConfig>(malformed).is_err());
    }
    for key in ["api_key", "endpoint"] {
        let value = serde_json::json!({"provider":"clm","distribution_policy":{"kind":"strict"},key:"synthetic"});
        assert!(serde_json::from_value::<BindingConfig>(value).is_err());
    }
}
/// Trace: FR-045-AC-2
#[test]
fn unused_live_metadata_does_not_prepare_any_provider() {
    let bindings = BTreeMap::from([(
        BackendId::new("unused").unwrap(),
        BindingConfig {
            provider: Provider::Clm,
            model: "clm-latest".into(),
            expected_model: None,
            distribution_policy: DistributionPolicy::Strict {},
        },
    )]);
    live_bindings(&[], &bindings).unwrap();
}
/// Trace: FR-045-AC-2
#[cfg(not(feature = "clm"))]
#[test]
fn missing_clm_feature_refuses_without_credentials() {
    let id = BackendId::new("decision").unwrap();
    let bindings = BTreeMap::from([(
        id.clone(),
        BindingConfig {
            provider: Provider::Clm,
            model: "clm-latest".into(),
            expected_model: None,
            distribution_policy: DistributionPolicy::Strict {},
        },
    )]);
    assert!(matches!(
        live_bindings(&[id], &bindings),
        Err(CliError::Feature(Provider::Clm))
    ));
}
/// Trace: FR-045-AC-3
#[tokio::test]
async fn explicit_clm_replay_checks_only_model_and_policy_without_guessing_provider() {
    let request = ModelRequest {
        backend: BackendId::new("decision").unwrap(),
        model: "clm-latest".into(),
        expected_model: None,
        distribution_policy: DistributionPolicy::Strict {},
        state: Value::Record(BTreeMap::new()),
        questions: vec![NamedQuestion {
            id: "q".into(),
            question: Question::Boolean {
                instructions: "Synthetic?".into(),
                yes: "True".into(),
                no: "False".into(),
            },
        }],
    };
    let response = ModelResponse {
        model: "clm-latest".into(),
        answers: BTreeMap::from([(
            "q".into(),
            Answer::Boolean {
                probability: Probability::new(0.7).unwrap(),
            },
        )]),
        usage: Some(Usage {
            input_tokens: 38,
            output_tokens: 0,
            billing_units: Some(1),
        }),
    };
    let recording = Recording {
        exchanges: vec![Exchange {
            request: request.clone(),
            response: response.clone(),
        }],
    };
    let mut bindings = BTreeMap::from([(
        request.backend.clone(),
        BindingConfig {
            provider: Provider::Clm,
            model: request.model.clone(),
            expected_model: None,
            distribution_policy: request.distribution_policy,
        },
    )]);
    for explicit in [None, Some(&bindings)] {
        let registry = replay_bindings(&recording, explicit, 1_048_576).unwrap();
        assert_eq!(
            registry
                .get(&request.backend)
                .unwrap()
                .backend
                .infer(&request)
                .await
                .unwrap(),
            response
        );
    }
    bindings.get_mut(&request.backend).unwrap().provider = Provider::Jev;
    replay_bindings(&recording, Some(&bindings), 1_048_576).unwrap();
    bindings.get_mut(&request.backend).unwrap().model = "different".into();
    assert!(
        matches!(replay_bindings(&recording, Some(&bindings), 1_048_576), Err(CliError::Engine(error)) if error.code == ErrorCode::RecordingMismatch)
    );
    bindings.get_mut(&request.backend).unwrap().model = request.model.clone();
    bindings.get_mut(&request.backend).unwrap().expected_model = Some("different".into());
    assert!(
        matches!(replay_bindings(&recording, Some(&bindings), 1_048_576), Err(CliError::Engine(error)) if error.code == ErrorCode::RecordingMismatch)
    );
    bindings.get_mut(&request.backend).unwrap().expected_model = None;
    bindings
        .get_mut(&request.backend)
        .unwrap()
        .distribution_policy = DistributionPolicy::approximate(0.01).unwrap();
    assert!(
        matches!(replay_bindings(&recording, Some(&bindings), 1_048_576), Err(CliError::Engine(error)) if error.code == ErrorCode::RecordingMismatch)
    );
}
/// Trace: FR-046-AC-2
#[test]
fn built_version_agrees_and_host_endpoint_uses_shared_precedence() {
    let report = ix_cli_kit::version::Agreement::new()
        .surface("manifest", env!("CARGO_PKG_VERSION"))
        .command(
            "binary",
            std::path::Path::new(env!("CARGO_BIN_EXE_sapho")),
            &["--version"],
        )
        .check()
        .unwrap();
    assert_eq!(report.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(
        resolve_endpoint(
            Some("explicit".into()),
            Some("environment".into()),
            "default"
        ),
        "explicit"
    );
    assert_eq!(
        resolve_endpoint(None, Some("environment".into()), "default"),
        "environment"
    );
    assert_eq!(resolve_endpoint(None, None, "default"), "default");
    let output = Command::new(env!("CARGO_BIN_EXE_sapho"))
        .arg("--version")
        .env_clear()
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("sapho {}\n", env!("CARGO_PKG_VERSION"))
    );
}
#[cfg(any(feature = "clm", feature = "jev"))]
fn production_roundtrip(provider: Provider) {
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    let (model, provider_name, endpoint_env) = match provider {
        Provider::Clm => ("clm-latest", "clm", "CLM_BASE_URL"),
        Provider::Jev => ("jev-latest", "jev", "TYPESAFE_BASE_URL"),
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                .unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut buffer = [0; 1024];
                let n = stream.read(&mut buffer).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buffer[..n]);
                if let Some(index) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..index]);
                    let length: usize = headers
                        .lines()
                        .find_map(|line| {
                            line.to_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse().unwrap())
                        })
                        .unwrap();
                    if bytes.len() >= index + 4 + length {
                        break;
                    }
                }
            }
            let captured = String::from_utf8(bytes).unwrap();
            assert!(captured.starts_with("POST /v1/systemone HTTP/1.1"));
            let body: serde_json::Value =
                serde_json::from_str(captured.split_once("\r\n\r\n").unwrap().1).unwrap();
            assert_eq!(body["model"], model);
            assert_eq!(body["questions"]["q"]["type"], "noul");
            let body = serde_json::to_vec(&serde_json::json!({"model": model, "answers": {"q": {"type":"noul", "noul":0.8}}, "usage": {"input_tokens":38, "output_tokens":0, "billing_units":1}})).unwrap();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(&body).unwrap();
        }
    });
    let root = tempfile::tempdir().unwrap();
    let graph = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/graphs/multilayer.yaml");
    let input = root.path().join("input.json");
    std::fs::write(&input, br#"{"text":"Synthetic context"}"#).unwrap();
    let bindings = root.path().join("bindings.yaml");
    std::fs::write(
        &bindings,
        format!("judge:\n  provider: {provider_name}\n  model: {model}\n  distribution_policy: {{kind: strict}}\n"),
    )
    .unwrap();
    let recording = root.path().join("recording.json");
    let recorded = Command::new(env!("CARGO_BIN_EXE_sapho"))
        .env_clear()
        .env(endpoint_env, endpoint)
        .env(provider.credential_environment(), "synthetic-credential")
        .env("TYPESAFE_LOG_LEVEL", "debug")
        .arg("record")
        .arg(&graph)
        .arg("--input")
        .arg(&input)
        .arg("--bindings")
        .arg(&bindings)
        .arg("--recording")
        .arg(&recording)
        .output()
        .unwrap();
    assert!(
        recorded.status.success(),
        "{}",
        String::from_utf8_lossy(&recorded.stderr)
    );
    server.join().unwrap();
    assert!(
        recorded.stderr.is_empty(),
        "SDK debug logging must be disabled"
    );
    assert!(!String::from_utf8_lossy(&recorded.stdout).contains("synthetic-credential"));
    let saved = Recording::from_json(&std::fs::read(&recording).unwrap(), 1_048_576).unwrap();
    assert_eq!(saved.exchanges.len(), 2);
    assert_eq!(
        saved.exchanges[0]
            .response
            .usage
            .as_ref()
            .unwrap()
            .billing_units,
        if provider == Provider::Clm {
            Some(1)
        } else {
            None
        }
    );
    let replayed = Command::new(env!("CARGO_BIN_EXE_sapho"))
        .env_clear()
        .env("CLM_BASE_URL", "not-a-url")
        .env("CLM_API_KEY", "")
        .env("TYPESAFE_API_KEY", "")
        .arg("replay")
        .arg(&graph)
        .arg("--input")
        .arg(&input)
        .arg("--bindings")
        .arg(&bindings)
        .arg("--recording")
        .arg(&recording)
        .output()
        .unwrap();
    assert!(
        replayed.status.success(),
        "{}",
        String::from_utf8_lossy(&replayed.stderr)
    );
    let original: RunReport = serde_json::from_slice(&recorded.stdout).unwrap();
    let replay: RunReport = serde_json::from_slice(&replayed.stdout).unwrap();
    assert_eq!(original.outputs, replay.outputs);
    assert_eq!(original.exit, replay.exit);
}

/// Trace: NFR-003-M-1, FR-045-AC-2, FR-045-AC-3, FR-043-AC-1, FR-046-AC-1
#[cfg(feature = "clm")]
#[test]
fn production_cli_clm_records_then_replays_after_service_shutdown_with_poisoned_credentials() {
    production_roundtrip(Provider::Clm);
}
/// Trace: NFR-003-M-1, FR-045-AC-2, FR-045-AC-3, FR-046-AC-1
#[cfg(feature = "jev")]
#[test]
fn production_cli_jev_uses_shared_credentials_and_suppresses_sdk_debug_bodies() {
    production_roundtrip(Provider::Jev);
}
