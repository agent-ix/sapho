// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Named System One host configuration through stock CLI and fake contract services.
use ix_cli_kit::secrets::*;
#[cfg(not(feature = "clm"))]
use sapho_cli::CliError;
use sapho_cli::{ServiceConfig, live_bindings_with_services};
use sapho_core::BackendId;
#[cfg(feature = "clm")]
use sapho_recording::{Recording, RecordingBackend};
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
    delay_ms: u64,
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
        if delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        }
        let _ = stream.write_all(&body);
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
            let (endpoint, capture, task) = fake(model, 0.6 + index as f64 * 0.1, 0);
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
        let recorded = if count == 3 {
            Command::new(env!("CARGO_BIN_EXE_sapho"))
                .env_clear()
                .env("SAPHO_SERVICE_CONFIG", &services)
                .args([
                    "record",
                    path(&graph_path),
                    "--input",
                    path(&input),
                    "--bindings",
                    path(&bindings),
                    "--recording",
                    path(&recording),
                ])
                .output()
                .unwrap()
        } else {
            invoke(&[
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
            ])
        };
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
        let changed_input = root.path().join("changed-input.json");
        std::fs::write(&changed_input, br#"{"text":"different request"}"#).unwrap();
        let changed_request = invoke(&[
            "replay",
            path(&graph_path),
            "--input",
            path(&changed_input),
            "--bindings",
            path(&bindings),
            "--recording",
            path(&recording),
        ]);
        assert_eq!(changed_request.status.code(), Some(2));
        let changed: serde_json::Value = serde_json::from_slice(&changed_request.stdout).unwrap();
        assert_eq!(changed["error"]["code"], "replay_miss");
        let changed_graph = root.path().join("changed.yaml");
        std::fs::write(&changed_graph, graph(&["different"])).unwrap();
        let changed_bindings = root.path().join("changed-bindings.yaml");
        std::fs::write(&changed_bindings, format!("{}different: {{provider: systemone, model: model-a, distribution_policy: {{kind: strict}}}}\n", std::fs::read_to_string(&bindings).unwrap())).unwrap();
        let miss = invoke(&[
            "replay",
            path(&changed_graph),
            "--input",
            path(&input),
            "--bindings",
            path(&changed_bindings),
            "--service-config",
            path(&services),
            "--recording",
            path(&recording),
        ]);
        assert_eq!(miss.status.code(), Some(2));
        let missed: serde_json::Value = serde_json::from_slice(&miss.stdout).unwrap();
        assert_eq!(missed["error"]["code"], "replay_miss");
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

/// Trace: FR-072-AC-3, FR-073-AC-3, IT-013-SC-02, IT-013-SC-03
#[cfg(feature = "clm")]
#[tokio::test]
async fn credentialed_named_service_records_without_persisting_key_or_secret() {
    use sapho_core::{BackendRegistry, Datum, Inputs, PrimitiveRegistry, Value};
    use sapho_graph::GraphSpec;
    use sapho_runtime::RunLimits;
    use std::sync::Arc;

    let (endpoint, captured, server) = fake("model-a", 0.8, 0);
    let config = ServiceConfig::from_json(
        &serde_json::to_vec(&serde_json::json!({"services": {
            "fast_a": {"base_url": endpoint, "credential_key": "key-a"}
        }}))
        .unwrap(),
    )
    .unwrap();
    let bindings = sapho_graph::parse_config::<sapho_cli::Bindings>(
        "fast_a: {provider: systemone, model: model-a, distribution_policy: {kind: strict}}",
        sapho_graph::GraphFormat::Yaml,
    )
    .unwrap();
    let store = Store {
        calls: Mutex::new(Vec::new()),
    };
    let id = BackendId::new("fast_a").unwrap();
    let live = live_bindings_with_services(
        std::slice::from_ref(&id),
        &bindings,
        &config,
        &SecretStore::new(&store),
    )
    .unwrap();
    assert_eq!(*store.calls.lock().unwrap(), ["key-a"]);
    let binding = live.get(&id).unwrap();
    let recorder = Arc::new(RecordingBackend::new(binding.backend, 1_048_576).unwrap());
    let mut recorded = BackendRegistry::default();
    recorded
        .register(
            id,
            sapho_core::BackendBinding {
                backend: recorder.clone(),
                ..binding
            },
        )
        .unwrap();
    let graph = GraphSpec::parse(&graph(&["fast_a"])).unwrap();
    let runner = sapho_cli::Runner::new(&graph, &PrimitiveRegistry::default(), recorded).unwrap();
    let inputs = Inputs::from([(
        "text".into(),
        Datum::new("input", Value::Text("synthetic".into())).unwrap(),
    )]);
    let report = runner.run(&inputs, RunLimits::default(), None).await;
    assert_eq!(report.exit, sapho_cli::ExitStatus::Completed);
    server.join().unwrap();
    assert!(captured.lock().unwrap()[0].contains("credential-sentinel"));
    let recording = recorder.snapshot().unwrap().to_json(1_048_576).unwrap();
    let report = serde_json::to_vec(&report).unwrap();
    for bytes in [&recording[..], &report[..]] {
        let text = String::from_utf8_lossy(bytes);
        assert!(!text.contains("key-a"));
        assert!(!text.contains("credential-sentinel"));
        assert!(!text.contains(&endpoint));
    }
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

/// Trace: FR-072-AC-5, IT-013-SC-05
#[cfg(feature = "clm")]
#[test]
fn service_overrides_bound_response_and_deadline_without_changing_peer() {
    let root = tempfile::tempdir().unwrap();
    let input = root.path().join("input.json");
    std::fs::write(&input, br#"{"text":"synthetic"}"#).unwrap();
    let bindings = root.path().join("bindings.yaml");
    std::fs::write(&bindings, "fast_a: {provider: systemone, model: model-a, distribution_policy: {kind: strict}}\nfast_b: {provider: systemone, model: model-b, distribution_policy: {kind: strict}}\n").unwrap();
    let services = root.path().join("services.json");
    let graph_a = root.path().join("a.yaml");
    let graph_b = root.path().join("b.yaml");
    std::fs::write(&graph_a, graph(&["fast_a"])).unwrap();
    std::fs::write(&graph_b, graph(&["fast_b"])).unwrap();
    let (endpoint_a, _, task_a) = fake("model-a", 0.6, 0);
    let (endpoint_b, _, task_b) = fake("model-b", 0.7, 0);
    std::fs::write(&services, serde_json::to_vec(&serde_json::json!({"services":{
        "fast_a":{"base_url":endpoint_a,"limits":{"timeout_ms":30000,"request_bytes":1048576,"response_bytes":1,"in_flight":1}},
        "fast_b":{"base_url":endpoint_b}
    }})).unwrap()).unwrap();
    let run = |graph_path: &Path| {
        invoke(&[
            "run",
            path(graph_path),
            "--input",
            path(&input),
            "--bindings",
            path(&bindings),
            "--service-config",
            path(&services),
        ])
    };
    let request_limited = root.path().join("request-limited.json");
    std::fs::write(&request_limited, serde_json::to_vec(&serde_json::json!({"services":{
        "fast_a":{"base_url":"http://127.0.0.1:1","limits":{"timeout_ms":30000,"request_bytes":1,"response_bytes":8388608,"in_flight":1}}
    }})).unwrap()).unwrap();
    let no_send = invoke(&[
        "run",
        path(&graph_a),
        "--input",
        path(&input),
        "--bindings",
        path(&bindings),
        "--service-config",
        path(&request_limited),
    ]);
    assert_eq!(no_send.status.code(), Some(2));
    let report: serde_json::Value = serde_json::from_slice(&no_send.stdout).unwrap();
    assert_eq!(report["error"]["code"], "limit_exceeded");
    let refused = run(&graph_a);
    task_a.join().unwrap();
    assert_eq!(refused.status.code(), Some(2));
    let report: serde_json::Value = serde_json::from_slice(&refused.stdout).unwrap();
    assert_eq!(report["error"]["code"], "limit_exceeded");
    let completed = run(&graph_b);
    task_b.join().unwrap();
    assert!(
        completed.status.success(),
        "{}",
        String::from_utf8_lossy(&completed.stdout)
    );
    let (endpoint_a, _, task_a) = fake("model-a", 0.6, 80);
    let (endpoint_b, _, task_b) = fake("model-b", 0.7, 0);
    std::fs::write(&services, serde_json::to_vec(&serde_json::json!({"services":{
        "fast_a":{"base_url":endpoint_a,"limits":{"timeout_ms":1,"request_bytes":1048576,"response_bytes":8388608,"in_flight":1}},
        "fast_b":{"base_url":endpoint_b}
    }})).unwrap()).unwrap();
    let refused = run(&graph_a);
    task_a.join().unwrap();
    assert_eq!(refused.status.code(), Some(2));
    let report: serde_json::Value = serde_json::from_slice(&refused.stdout).unwrap();
    assert_eq!(report["error"]["code"], "deadline_exceeded");
    let completed = run(&graph_b);
    task_b.join().unwrap();
    assert!(
        completed.status.success(),
        "{}",
        String::from_utf8_lossy(&completed.stdout)
    );
}

/// Trace: FR-072-AC-7, IT-013-SC-04
#[test]
fn manually_constructed_service_limits_cannot_bypass_host_maxima() {
    let bindings = sapho_graph::parse_config::<sapho_cli::Bindings>(
        "fast_a: {provider: systemone, model: m, distribution_policy: {kind: strict}}",
        sapho_graph::GraphFormat::Yaml,
    )
    .unwrap();
    let store = Store {
        calls: Mutex::new(Vec::new()),
    };
    let config = ServiceConfig {
        services: std::collections::BTreeMap::from([(
            BackendId::new("fast_a").unwrap(),
            sapho_cli::ServiceEntry {
                base_url: "http://127.0.0.1:1".into(),
                credential_key: None,
                limits: Some(sapho_cli::ServiceLimits {
                    timeout_ms: 30_001,
                    request_bytes: 1,
                    response_bytes: 1,
                    in_flight: 1,
                }),
            },
        )]),
    };
    assert!(
        live_bindings_with_services(
            &[BackendId::new("fast_a").unwrap()],
            &bindings,
            &config,
            &SecretStore::new(&store)
        )
        .is_err()
    );
    assert!(store.calls.lock().unwrap().is_empty());
}

#[cfg(feature = "clm")]
fn held_service(
    model: &'static str,
) -> (
    String,
    std::sync::mpsc::Receiver<usize>,
    std::sync::mpsc::Sender<()>,
    Arc<std::sync::atomic::AtomicUsize>,
    std::thread::JoinHandle<usize>,
) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let release_rx = Arc::new(Mutex::new(release_rx));
    let active = Arc::new(AtomicUsize::new(0));
    let max_active = Arc::new(AtomicUsize::new(0));
    let active_for_assertion = Arc::clone(&active);
    let server = std::thread::spawn(move || {
        let mut handlers = Vec::new();
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            let release_rx = Arc::clone(&release_rx);
            let started_tx = started_tx.clone();
            let active = Arc::clone(&active);
            let max_active = Arc::clone(&max_active);
            handlers.push(std::thread::spawn(move || {
                stream.set_read_timeout(Some(std::time::Duration::from_secs(10))).unwrap();
                let mut bytes = Vec::new();
                loop {
                    let mut buffer = [0u8; 1024];
                    let n = stream.read(&mut buffer).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&buffer[..n]);
                    if let Some(index) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..index]);
                        let length: usize = headers.lines().find_map(|line| line.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap())).unwrap();
                        if bytes.len() >= index + 4 + length { break }
                    }
                }
                let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                max_active.fetch_max(now, Ordering::SeqCst);
                started_tx.send(now).unwrap();
                release_rx.lock().unwrap().recv_timeout(std::time::Duration::from_secs(10)).unwrap();
                let body = serde_json::to_vec(&serde_json::json!({"model":model,"answers":{"q":{"type":"noul","noul":0.7}},"usage":{"input_tokens":1,"output_tokens":0,"billing_units":1}})).unwrap();
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
                stream.write_all(&body).unwrap();
                active.fetch_sub(1, Ordering::SeqCst);
            }));
        }
        for handler in handlers {
            handler.join().unwrap();
        }
        max_active.load(Ordering::SeqCst)
    });
    (
        endpoint,
        started_rx,
        release_tx,
        active_for_assertion,
        server,
    )
}

/// Trace: FR-072-AC-5, IT-013-SC-05
#[cfg(feature = "clm")]
#[test]
fn two_configured_services_enforce_independent_concurrency_ceilings() {
    use sapho_core::{DistributionPolicy, ModelRequest, NamedQuestion, Question, Value};
    use std::{collections::BTreeMap, sync::atomic::Ordering};
    let (url_a, started_a, release_a, active_a, server_a) = held_service("model-a");
    let (url_b, started_b, release_b, active_b, server_b) = held_service("model-b");
    let config = ServiceConfig::from_json(&serde_json::to_vec(&serde_json::json!({"services":{
        "fast_a":{"base_url":url_a,"limits":{"timeout_ms":30000,"request_bytes":1048576,"response_bytes":8388608,"in_flight":1}},
        "fast_b":{"base_url":url_b,"limits":{"timeout_ms":30000,"request_bytes":1048576,"response_bytes":8388608,"in_flight":2}}
    }})).unwrap()).unwrap();
    let bindings = sapho_graph::parse_config::<sapho_cli::Bindings>("fast_a: {provider: systemone, model: model-a, distribution_policy: {kind: strict}}\nfast_b: {provider: systemone, model: model-b, distribution_policy: {kind: strict}}", sapho_graph::GraphFormat::Yaml).unwrap();
    let store = Store {
        calls: Mutex::new(Vec::new()),
    };
    let registry = live_bindings_with_services(
        &[
            BackendId::new("fast_a").unwrap(),
            BackendId::new("fast_b").unwrap(),
        ],
        &bindings,
        &config,
        &SecretStore::new(&store),
    )
    .unwrap();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    let mut calls = Vec::new();
    runtime.block_on(async {
        for name in ["fast_a", "fast_a", "fast_b", "fast_b"] {
            let backend_id = BackendId::new(name).unwrap();
            let binding = registry.get(&backend_id).unwrap();
            let request = ModelRequest {
                backend: backend_id,
                model: binding.model,
                expected_model: None,
                distribution_policy: DistributionPolicy::Strict {},
                state: Value::Record(BTreeMap::new()),
                questions: vec![NamedQuestion {
                    id: "q".into(),
                    question: Question::Boolean {
                        instructions: "Is it true?".into(),
                        yes: "Yes".into(),
                        no: "No".into(),
                    },
                }],
            };
            calls.push(tokio::spawn(async move {
                binding.backend.infer(&request).await
            }));
        }
    });
    let timeout = std::time::Duration::from_secs(10);
    assert_eq!(started_a.recv_timeout(timeout).unwrap(), 1);
    let mut peer_starts = [
        started_b.recv_timeout(timeout).unwrap(),
        started_b.recv_timeout(timeout).unwrap(),
    ];
    peer_starts.sort_unstable();
    assert_eq!(peer_starts, [1, 2]);
    assert_eq!(active_a.load(Ordering::SeqCst), 1);
    assert_eq!(active_b.load(Ordering::SeqCst), 2);
    release_b.send(()).unwrap();
    release_b.send(()).unwrap();
    release_a.send(()).unwrap();
    assert_eq!(started_a.recv_timeout(timeout).unwrap(), 1);
    release_a.send(()).unwrap();
    runtime.block_on(async {
        for call in calls {
            assert!(call.await.unwrap().is_ok());
        }
    });
    assert_eq!(server_a.join().unwrap(), 1);
    assert_eq!(server_b.join().unwrap(), 2);
    assert!(store.calls.lock().unwrap().is_empty());
}

struct UnavailableStore;
impl CredentialBackend for &UnavailableStore {
    fn get(&self, _: &AppScope, _: &SecretKey) -> Result<Option<SecretValue>, SecretError> {
        Err(SecretError::Unavailable)
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
/// Trace: FR-072-AC-4, IT-013-SC-04
#[cfg(feature = "clm")]
#[test]
fn unavailable_named_store_key_refuses_before_transport() {
    let config = ServiceConfig::from_json(br#"{"services":{"fast_a":{"base_url":"http://127.0.0.1:1","credential_key":"fast-a-key"}}}"#).unwrap();
    let bindings = sapho_graph::parse_config::<sapho_cli::Bindings>(
        "fast_a: {provider: systemone, model: m, distribution_policy: {kind: strict}}",
        sapho_graph::GraphFormat::Yaml,
    )
    .unwrap();
    let result = live_bindings_with_services(
        &[BackendId::new("fast_a").unwrap()],
        &bindings,
        &config,
        &SecretStore::new(&UnavailableStore),
    );
    assert!(matches!(
        result,
        Err(sapho_cli::CliError::Credential(SecretError::Unavailable))
    ));
}
