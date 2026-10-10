// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Named System One host configuration through stock CLI and fake contract services.
use ix_cli_kit::secrets::*;
#[cfg(not(feature = "clm"))]
use sapho_cli::CliError;
use sapho_cli::{ServiceConfig, live_bindings_with_services};
use sapho_core::BackendId;
#[cfg(feature = "clm")]
use sapho_recording::Recording;
use std::sync::Mutex;
#[cfg(feature = "clm")]
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::Path,
    process::Command,
    sync::Arc,
};

#[cfg(feature = "clm")]
fn graph(backends: &[&str]) -> String {
    let mut nodes = String::from(
        "inputs: {text: {kind: text}}\nnodes:\n  - id: context\n    operation: {kind: record}\n    inputs: {text: {kind: input, name: text}}\n  - id: questions\n    operation:\n      kind: questions\n      questions:\n        - id: q\n          question: {kind: boolean, instructions: Is it true?, yes: Yes, no: No}\n",
    );
    let mut outputs = String::new();
    for (index, backend) in backends.iter().enumerate() {
        nodes.push_str(&format!("  - id: ask{index}\n    operation: {{kind: ask, backend: {backend}}}\n    inputs:\n      state: {{kind: node, node: context, port: result}}\n      questions: {{kind: node, node: questions, port: result}}\n  - id: probability{index}\n    operation: {{kind: probability, question: q, labels: [\"true\"]}}\n    inputs: {{answers: {{kind: node, node: ask{index}, port: answers}}}}\n"));
        outputs.push_str(&format!(
            "  result{index}: {{kind: node, node: probability{index}, port: result}}\n"
        ));
    }
    format!("{nodes}outputs:\n{outputs}")
}
#[cfg(feature = "clm")]
fn fake(
    model: &'static str,
    value: f64,
) -> (String, Arc<Mutex<Vec<String>>>, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let captures = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&captures);
    let task = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .unwrap();
        let mut bytes = Vec::new();
        loop {
            let mut buffer = [0u8; 1024];
            let n = stream.read(&mut buffer).unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&buffer[..n]);
            if let Some(index) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..index]);
                let length: usize = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse().unwrap())
                    })
                    .unwrap();
                if bytes.len() >= index + 4 + length {
                    break;
                }
            }
        }
        let request = String::from_utf8(bytes).unwrap();
        assert!(request.starts_with("POST /v1/systemone HTTP/1.1"));
        captured.lock().unwrap().push(request);
        let body = serde_json::to_vec(
            &serde_json::json!({"model":model,"answers":{"q":{"type":"noul","noul":value}},"usage":{"input_tokens":38,"output_tokens":0,"billing_units":1}}),
        )
        .unwrap();
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .unwrap();
        stream.write_all(&body).unwrap();
    });
    (endpoint, captures, task)
}
#[cfg(feature = "clm")]
fn path(path: &Path) -> &str {
    path.to_str().unwrap()
}
#[cfg(feature = "clm")]
fn invoke(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_sapho"))
        .env_clear()
        .args(args)
        .output()
        .unwrap()
}

/// Trace: FR-072-AC-1, FR-072-AC-2, FR-073-AC-1, FR-073-AC-2, FR-073-AC-3, IT-013-SC-01, IT-013-SC-02, IT-013-SC-03
#[cfg(feature = "clm")]
#[test]
fn two_named_ports_record_and_replay_offline_and_third_name_needs_only_files() {
    for count in [2, 3] {
        let root = tempfile::tempdir().unwrap();
        let names = ["fast_a", "fast_b", "fast_c"];
        let models = ["model-a", "model-b", "model-c"];
        let mut endpoints = Vec::new();
        let mut captures = Vec::new();
        let mut tasks = Vec::new();
        for (index, model) in models.iter().take(count).enumerate() {
            let (endpoint, capture, task) = fake(model, 0.6 + index as f64 * 0.1);
            endpoints.push(endpoint);
            captures.push(capture);
            tasks.push(task);
        }
        let graph_path = root.path().join("graph.yaml");
        std::fs::write(&graph_path, graph(&names[..count])).unwrap();
        let input = root.path().join("input.json");
        std::fs::write(&input, br#"{"text":"synthetic"}"#).unwrap();
        let bindings = root.path().join("bindings.yaml");
        let metadata = names.iter().zip(models).take(count).map(|(name,model)| format!("{name}: {{provider: systemone, model: {model}, distribution_policy: {{kind: strict}}}}\n")).collect::<String>();
        std::fs::write(&bindings, metadata).unwrap();
        let services = root.path().join("services.json");
        let entries = names
            .iter()
            .take(count)
            .zip(&endpoints)
            .map(|(name, url)| ((*name).to_owned(), serde_json::json!({"base_url":url})))
            .collect::<serde_json::Map<_, _>>();
        std::fs::write(
            &services,
            serde_json::to_vec(&serde_json::json!({"services":entries})).unwrap(),
        )
        .unwrap();
        let recording = root.path().join("recording.json");
        let recorded = invoke(&[
            "record",
            path(&graph_path),
            "--input",
            path(&input),
            "--bindings",
            path(&bindings),
            "--service-config",
            path(&services),
            "--recording",
            path(&recording),
        ]);
        assert!(
            recorded.status.success(),
            "{}",
            format!(
                "stderr={} stdout={}",
                String::from_utf8_lossy(&recorded.stderr),
                String::from_utf8_lossy(&recorded.stdout)
            )
        );
        for task in tasks {
            task.join().unwrap();
        }
        for (index, capture) in captures.iter().enumerate() {
            let captured = capture.lock().unwrap();
            assert_eq!(captured.len(), 1);
            let body: serde_json::Value =
                serde_json::from_str(captured[0].split_once("\r\n\r\n").unwrap().1).unwrap();
            assert_eq!(body["model"], models[index]);
        }
        let saved = std::fs::read(&recording).unwrap();
        let parsed = Recording::from_json(&saved, 1_048_576).unwrap();
        assert_eq!(parsed.exchanges.len(), count);
        let ids = parsed
            .exchanges
            .iter()
            .map(|exchange| exchange.request.backend.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(ids, names[..count].iter().copied().collect());
        let serialized = String::from_utf8(saved).unwrap();
        for endpoint in &endpoints {
            assert!(!serialized.contains(endpoint));
        }
        std::fs::remove_file(&services).unwrap();
        let replayed = invoke(&[
            "replay",
            path(&graph_path),
            "--input",
            path(&input),
            "--bindings",
            path(&bindings),
            "--service-config",
            path(&services),
            "--recording",
            path(&recording),
        ]);
        assert!(
            replayed.status.success(),
            "{}",
            String::from_utf8_lossy(&replayed.stderr)
        );
        let original: serde_json::Value = serde_json::from_slice(&recorded.stdout).unwrap();
        let replay: serde_json::Value = serde_json::from_slice(&replayed.stdout).unwrap();
        assert_eq!(original["outputs"], replay["outputs"]);
        let changed_graph = root.path().join("changed.yaml");
        std::fs::write(&changed_graph, graph(&["different"])).unwrap();
        let miss = invoke(&[
            "replay",
            path(&changed_graph),
            "--input",
            path(&input),
            "--bindings",
            path(&bindings),
            "--service-config",
            path(&services),
            "--recording",
            path(&recording),
        ]);
        assert!(!miss.status.success());
    }
}

struct Store {
    calls: Mutex<Vec<String>>,
}
impl CredentialBackend for &Store {
    fn get(&self, _: &AppScope, key: &SecretKey) -> Result<Option<SecretValue>, SecretError> {
        self.calls.lock().unwrap().push(key.as_str().into());
        Ok(Some(SecretValue::new("credential-sentinel")))
    }
    fn set(&self, _: &AppScope, _: &SecretKey, _: &SecretValue) -> Result<(), SecretError> {
        panic!("no write")
    }
    fn delete(&self, _: &AppScope, _: &SecretKey) -> Result<DeleteStatus, SecretError> {
        panic!("no delete")
    }
    fn status(&self, _: &AppScope, _: &SecretKey) -> Result<Presence, SecretError> {
        panic!("no status")
    }
}
/// Trace: FR-072-AC-3, FR-072-AC-6, IT-013-SC-05
#[test]
fn only_required_named_service_resolves_its_own_store_key() {
    let config = ServiceConfig::from_json(br#"{"services":{"fast_a":{"base_url":"http://127.0.0.1:1111","credential_key":"key-a"},"unused":{"base_url":"http://127.0.0.1:2222","credential_key":"key-unused"}}}"#).unwrap();
    let bindings = sapho_graph::parse_config::<sapho_cli::Bindings>(
        "fast_a: {provider: systemone, model: m, distribution_policy: {kind: strict}}",
        sapho_graph::GraphFormat::Yaml,
    )
    .unwrap();
    let store = Store {
        calls: Mutex::new(Vec::new()),
    };
    let resolved = live_bindings_with_services(
        &[BackendId::new("fast_a").unwrap()],
        &bindings,
        &config,
        &SecretStore::new(&store),
    );
    #[cfg(feature = "clm")]
    assert!(resolved.is_ok(), "{:?}", resolved.err());
    #[cfg(not(feature = "clm"))]
    assert!(matches!(
        resolved,
        Err(CliError::Feature(sapho_cli::Provider::Systemone))
    ));
    let calls = store.calls.lock().unwrap();
    #[cfg(feature = "clm")]
    assert_eq!(*calls, ["key-a"]);
    #[cfg(not(feature = "clm"))]
    assert!(calls.is_empty());
}

/// Trace: FR-072-AC-3, FR-072-AC-4, FR-072-AC-7, IT-013-SC-04
#[test]
fn service_schema_limits_and_endpoint_rules_refuse_before_transport() {
    for invalid in [
        r#"{"services":{"fast_a":{"base_url":"http://127.0.0.1:1","credential":"plaintext-sentinel"}}}"#,
        r#"{"services":{"fast_a":{"base_url":"http://127.0.0.1:1","api_key":"plaintext-sentinel"}}}"#,
        r#"{"services":{"fast_a":{"base_url":"http://127.0.0.1:1","unknown":1}}}"#,
        r#"{"services":{"fast_a":{"base_url":"http://127.0.0.1:1","credential_key":""}}}"#,
        r#"{"services":{"fast_a":{"base_url":"http://127.0.0.1:1"},"fast_a":{"base_url":"http://127.0.0.1:2"}}}"#,
    ] {
        assert!(
            ServiceConfig::from_json(invalid.as_bytes()).is_err(),
            "{invalid}"
        );
    }
    let maxima = [
        ("timeout_ms", 30_000),
        ("request_bytes", 1_048_576),
        ("response_bytes", 8_388_608),
        ("in_flight", 4),
    ];
    for (field, maximum) in maxima {
        for bad in [0, maximum + 1] {
            let mut limits = serde_json::json!({"timeout_ms":30_000,"request_bytes":1_048_576,"response_bytes":8_388_608,"in_flight":4});
            limits[field] = bad.into();
            let bytes = serde_json::to_vec(&serde_json::json!({"services":{"fast_a":{"base_url":"http://127.0.0.1:1","limits":limits}}})).unwrap();
            assert!(ServiceConfig::from_json(&bytes).is_err(), "{field}={bad}");
        }
    }
    let exact = serde_json::json!({"services":{"fast_a":{"base_url":"http://127.0.0.1:1","limits":{"timeout_ms":30_000,"request_bytes":1_048_576,"response_bytes":8_388_608,"in_flight":4}}}});
    assert!(ServiceConfig::from_json(&serde_json::to_vec(&exact).unwrap()).is_ok());
    #[cfg(feature = "clm")]
    {
        let bindings = sapho_graph::parse_config::<sapho_cli::Bindings>(
            "fast_a: {provider: systemone, model: m, distribution_policy: {kind: strict}}",
            sapho_graph::GraphFormat::Yaml,
        )
        .unwrap();
        let store = Store {
            calls: Mutex::new(Vec::new()),
        };
        for url in [
            "http://example.com",
            "http://user:pass@127.0.0.1:1",
            "http://127.0.0.1:1?q=1",
            "http://127.0.0.1:1/#frag",
        ] {
            let bytes =
                serde_json::to_vec(&serde_json::json!({"services":{"fast_a":{"base_url":url}}}))
                    .unwrap();
            let config = ServiceConfig::from_json(&bytes).unwrap();
            assert!(
                live_bindings_with_services(
                    &[BackendId::new("fast_a").unwrap()],
                    &bindings,
                    &config,
                    &SecretStore::new(&store)
                )
                .is_err(),
                "{url}"
            );
        }
        for url in ["http://127.0.0.1:1", "https://example.com"] {
            let bytes =
                serde_json::to_vec(&serde_json::json!({"services":{"fast_a":{"base_url":url}}}))
                    .unwrap();
            let config = ServiceConfig::from_json(&bytes).unwrap();
            assert!(
                live_bindings_with_services(
                    &[BackendId::new("fast_a").unwrap()],
                    &bindings,
                    &config,
                    &SecretStore::new(&store)
                )
                .is_ok(),
                "{url}"
            );
        }
        assert!(store.calls.lock().unwrap().is_empty());
    }
}

/// Trace: FR-072-AC-3, FR-072-AC-6, FR-073-AC-4, IT-013-SC-04, IT-013-SC-06
#[test]
fn generic_bindings_require_model_and_reject_provider_fields() {
    assert!(
        sapho_graph::parse_config::<sapho_cli::Bindings>(
            "fast_a: {provider: systemone, distribution_policy: {kind: strict}}",
            sapho_graph::GraphFormat::Yaml
        )
        .is_err()
    );
    assert!(
        sapho_graph::parse_config::<sapho_cli::Bindings>(
            "legacy: {provider: clm, distribution_policy: {kind: strict}}",
            sapho_graph::GraphFormat::Yaml
        )
        .is_ok()
    );
    for field in ["endpoint", "credential", "api_key"] {
        let text = format!(
            "fast_a: {{provider: systemone, model: m, distribution_policy: {{kind: strict}}, {field}: forbidden}}"
        );
        assert!(
            sapho_graph::parse_config::<sapho_cli::Bindings>(&text, sapho_graph::GraphFormat::Yaml)
                .is_err(),
            "{field}"
        );
    }
}
