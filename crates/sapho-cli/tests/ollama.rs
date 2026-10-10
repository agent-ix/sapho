// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! The `ollama` provider through the real executable against a loopback double of the server.
#![cfg(feature = "ollama")]
use sapho_cli::*;
use sapho_graph::parse_config;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Command, Output},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
};

const BLOB: &str = "58574f2e94b99fb9e4391408b57e5aeaaaec10f6384e9a699fc2cb43a5c8eabf";
const MODEL: &str = "synthetic-model:1b";
const GRAPH: &str = r#"
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
outputs: {result: {kind: node, node: probability, port: result}}
"#;
const STRICT: &str = "distribution_policy: {kind: strict}";

/// A loopback imitation of the server: it records request paths and answers one question.
struct Double {
    url: String,
    paths: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl Double {
    fn start(delay: Duration) -> Self {
        Self::on(TcpListener::bind("127.0.0.1:0").unwrap(), delay)
    }
    /// A double on the default loopback address, or `None` when something else holds it.
    fn on_default() -> Option<Self> {
        TcpListener::bind("127.0.0.1:11434")
            .ok()
            .map(|listener| Self::on(listener, Duration::ZERO))
    }
    fn on(listener: TcpListener, delay: Duration) -> Self {
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let paths = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (seen, halt) = (paths.clone(), stop.clone());
        let thread = std::thread::spawn(move || {
            while !halt.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let seen = seen.clone();
                        std::thread::spawn(move || serve(stream, &seen, delay));
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(5)),
                }
            }
        });
        Self {
            url,
            paths,
            stop,
            thread: Some(thread),
        }
    }
    fn requests(&self) -> Vec<String> {
        self.paths.lock().unwrap().clone()
    }
    fn generates(&self) -> usize {
        self.requests()
            .iter()
            .filter(|path| *path == "/api/generate")
            .count()
    }
}
impl Drop for Double {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}
fn serve(mut stream: TcpStream, seen: &Mutex<Vec<String>>, delay: Duration) {
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut bytes = Vec::new();
    let (head, length) = loop {
        let mut buffer = [0; 4096];
        let Ok(n) = stream.read(&mut buffer) else {
            return;
        };
        if n == 0 {
            return;
        }
        bytes.extend_from_slice(&buffer[..n]);
        if let Some(index) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&bytes[..index]).into_owned();
            let length: usize = head
                .lines()
                .find_map(|line| {
                    line.to_lowercase()
                        .strip_prefix("content-length:")
                        .map(|v| v.trim().parse().unwrap())
                })
                .unwrap_or(0);
            break (head, index + 4 + length);
        }
    };
    while bytes.len() < length {
        let mut buffer = [0; 4096];
        match stream.read(&mut buffer) {
            Ok(n) if n > 0 => bytes.extend_from_slice(&buffer[..n]),
            _ => return,
        }
    }
    let path = head.split_whitespace().nth(1).unwrap_or("").to_owned();
    seen.lock().unwrap().push(path.clone());
    let body = if path == "/api/show" {
        serde_json::json!({
            "modelfile": format!("# Modelfile\n\nFROM /models/blobs/sha256-{BLOB}\n"),
            "details": {}
        })
    } else {
        std::thread::sleep(delay);
        let token = |text: &str, logprob: f64, alternatives: &[(&str, f64)]| {
            serde_json::json!({
                "token": text,
                "logprob": logprob,
                "bytes": text.as_bytes(),
                "top_logprobs": alternatives
                    .iter()
                    .map(|(t, l)| serde_json::json!({"token": t, "logprob": l, "bytes": t.as_bytes()}))
                    .collect::<Vec<_>>()
            })
        };
        serde_json::json!({
            "model": MODEL,
            "response": r#"{"q": "yes"}"#,
            "done": true,
            "done_reason": "stop",
            "prompt_eval_count": 40,
            "eval_count": 8,
            "logprobs": [
                token("{\"", -0.01, &[]),
                token("q", -0.01, &[]),
                token("\": \"", -0.01, &[]),
                token("yes", -0.3, &[("yes", -0.3), ("no", -1.4)]),
                token("\"}", -0.01, &[]),
            ]
        })
    };
    let body = serde_json::to_vec(&body).unwrap();
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(&body);
}

struct Workspace {
    root: tempfile::TempDir,
}
impl Workspace {
    fn new(bindings_extra: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let write = |name: &str, text: &str| std::fs::write(root.path().join(name), text).unwrap();
        write("graph.yaml", GRAPH);
        write("input.json", r#"{"text":"synthetic context"}"#);
        write(
            "bindings.yaml",
            &format!("judge:\n  provider: ollama\n  model: {MODEL}\n  {STRICT}\n{bindings_extra}"),
        );
        let case = |id: &str, label: bool| {
            serde_json::json!({
                "id": id,
                "split": "development",
                "inputs": {"text": {"id": id, "value": {"kind": "text", "value": "synthetic context"}, "sources": []}},
                "labels": {"result": label},
                "label_provenance": {"kind": "human", "source": "test author", "reference": "synthetic"}
            })
        };
        write(
            "dataset.json",
            &serde_json::json!({"id": "synthetic", "cases": [case("one", true)]}).to_string(),
        );
        Self { root }
    }
    fn file(&self, name: &str) -> String {
        self.root.path().join(name).to_str().unwrap().to_owned()
    }
    fn sapho(&self, environment: &[(&str, &str)], args: &[&str]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_sapho"));
        command.args(args).env_clear();
        for (key, value) in environment {
            command.env(key, value);
        }
        command.output().unwrap()
    }
    fn record(&self, environment: &[(&str, &str)], extra: &[&str]) -> Output {
        let mut args = vec!["record", "--input", "", "--bindings", "", "--recording", ""];
        let (graph, input, bindings, recording) = (
            self.file("graph.yaml"),
            self.file("input.json"),
            self.file("bindings.yaml"),
            self.file("recording.json"),
        );
        args[2] = &input;
        args[4] = &bindings;
        args[6] = &recording;
        args.insert(1, &graph);
        args.extend_from_slice(extra);
        self.sapho(environment, &args)
    }
}
fn json(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "Invalid JSON: {e}; {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}
fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Trace: FR-055-AC-6
#[test]
fn the_endpoint_is_the_environment_value_or_the_loopback_default() {
    assert_eq!(ollama_base_url(None), "http://127.0.0.1:11434");
    assert_eq!(
        ollama_base_url(Some("http://example.invalid:9".into())),
        "http://example.invalid:9"
    );
}

/// Trace: FR-055-AC-6
#[test]
fn an_empty_endpoint_variable_counts_as_unset_and_a_live_binding_builds_without_one() {
    assert_eq!(
        ollama_base_url(Some(String::new())),
        "http://127.0.0.1:11434"
    );
    // Construction sends nothing, so the live path with the variable unset can be built here.
    if std::env::var_os("OLLAMA_BASE_URL").is_none() {
        let bindings = parse_config::<Bindings>(
            &format!("judge: {{provider: ollama, model: m, {STRICT}}}"),
            sapho_graph::GraphFormat::Yaml,
        )
        .unwrap();
        live_bindings(&[sapho_core::BackendId::new("judge").unwrap()], &bindings).unwrap();
    }
}

/// Trace: FR-055-AC-2, FR-055-AC-3, FR-055-AC-6, FR-045-AC-4
#[test]
fn record_against_the_double_then_replay_measure_offline_with_the_response_name() {
    let workspace = Workspace::new("");
    let double = Double::start(Duration::ZERO);
    let environment = [("OLLAMA_BASE_URL", double.url.as_str())];
    let recorded = workspace.record(&environment, &[]);
    assert!(recorded.status.success(), "{}", text(&recorded));
    assert!(double.generates() >= 1);
    let live = workspace.sapho(
        &environment,
        &[
            "measure",
            &workspace.file("graph.yaml"),
            "--dataset",
            &workspace.file("dataset.json"),
            "--bindings",
            &workspace.file("bindings.yaml"),
            "--split",
            "development",
        ],
    );
    assert!(live.status.success(), "{}", text(&live));
    let url = double.url.clone();
    drop(double);

    let recording = std::fs::read_to_string(workspace.file("recording.json")).unwrap();
    let saved: serde_json::Value = serde_json::from_str(&recording).unwrap();
    let exchanges = saved["exchanges"].as_array().unwrap();
    assert!(!exchanges.is_empty());
    for exchange in exchanges {
        assert_eq!(exchange["response"]["model"], MODEL);
        assert!(exchange["response"].get("digest").is_none());
        assert!(exchange["response"]["raw"].is_object());
    }
    // The endpoint is in no document and no diagnostic.
    let host = url.trim_start_matches("http://");
    for surface in [
        &recording,
        &text(&recorded),
        &std::fs::read_to_string(workspace.file("bindings.yaml")).unwrap(),
    ] {
        assert!(!surface.contains(host), "{host}");
    }

    let offline = workspace.sapho(
        &[],
        &[
            "measure",
            &workspace.file("graph.yaml"),
            "--dataset",
            &workspace.file("dataset.json"),
            "--bindings",
            &workspace.file("bindings.yaml"),
            "--replay",
            &workspace.file("recording.json"),
            "--split",
            "development",
        ],
    );
    assert!(offline.status.success(), "{}", text(&offline));
    let (live, offline) = (json(&live), json(&offline));
    assert_eq!(offline["measurement"], live["measurement"]);
    assert_eq!(offline["runs"], live["runs"]);
    let scored = &offline["measurement"];
    assert_eq!(scored["outputs"]["result"]["scored"], 1);
}

/// Trace: FR-055-AC-4, FR-045-AC-4
#[test]
fn validation_inspection_and_replay_construct_no_backend_and_read_no_environment() {
    let workspace = Workspace::new("");
    let recorder = Double::start(Duration::ZERO);
    let recorded = workspace.record(&[("OLLAMA_BASE_URL", recorder.url.as_str())], &[]);
    assert!(recorded.status.success(), "{}", text(&recorded));
    drop(recorder);

    let watcher = Double::start(Duration::ZERO);
    let (graph, bindings, recording, dataset, input) = (
        workspace.file("graph.yaml"),
        workspace.file("bindings.yaml"),
        workspace.file("recording.json"),
        workspace.file("dataset.json"),
        workspace.file("input.json"),
    );
    let tuned = workspace.file("tuned.json");
    let invocations: Vec<Vec<&str>> = vec![
        vec!["validate", &graph],
        vec!["inspect", &graph],
        vec![
            "replay",
            &graph,
            "--input",
            &input,
            "--bindings",
            &bindings,
            "--recording",
            &recording,
        ],
        vec![
            "measure",
            &graph,
            "--dataset",
            &dataset,
            "--bindings",
            &bindings,
            "--replay",
            &recording,
            "--split",
            "development",
        ],
        vec![
            "tune",
            "--candidate",
            &graph,
            "--dataset",
            &dataset,
            "--bindings",
            &bindings,
            "--replay",
            &recording,
            "--output-name",
            "result",
            "--metric",
            "brier",
        ],
    ];
    // A watched server and an unusable value: any use of the variable would show in one.
    for value in [watcher.url.as_str(), "not-a-url"] {
        for (index, args) in invocations.iter().enumerate() {
            let mut args = args.clone();
            if index == 4 {
                args.push("--output");
                args.push(&tuned);
            }
            let output = workspace.sapho(&[("OLLAMA_BASE_URL", value)], &args);
            assert!(output.status.success(), "{args:?}: {}", text(&output));
            if index == 4 {
                std::fs::remove_file(&tuned).unwrap();
            }
        }
    }
    assert!(watcher.requests().is_empty());
}

/// Trace: FR-055-AC-5
#[test]
fn backend_refusals_surface_before_any_request_reaches_the_server() {
    let double = Double::start(Duration::ZERO);
    let environment = [("OLLAMA_BASE_URL", double.url.as_str())];
    let thinking = Workspace::new("");
    std::fs::write(
        thinking.file("bindings.yaml"),
        format!("judge:\n  provider: ollama\n  model: {MODEL}\n  think: true\n  {STRICT}\n"),
    )
    .unwrap();
    let run = thinking.sapho(
        &environment,
        &[
            "run",
            &thinking.file("graph.yaml"),
            "--input",
            &thinking.file("input.json"),
            "--bindings",
            &thinking.file("bindings.yaml"),
        ],
    );
    assert_eq!(run.status.code(), Some(2), "{}", text(&run));
    let report = json(&run);
    assert_eq!(report["error"]["code"], "config");
    assert_eq!(
        report["error"]["context"]["reason"],
        "think_unsupported_for_questions"
    );

    let zero = Workspace::new("");
    std::fs::write(
        zero.file("bindings.yaml"),
        format!("judge:\n  provider: ollama\n  model: {MODEL}\n  num_ctx: 0\n  {STRICT}\n"),
    )
    .unwrap();
    let record = zero.record(&environment, &[]);
    assert_eq!(record.status.code(), Some(2), "{}", text(&record));
    let refusal = json(&record);
    assert_eq!(refusal["error"]["kind"], "engine");
    assert_eq!(refusal["error"]["detail"]["code"], "config");
    assert_eq!(
        refusal["error"]["detail"]["context"]["reason"],
        "zero_limit"
    );

    let large = Workspace::new("");
    std::fs::write(
        large.file("bindings.yaml"),
        format!("judge:\n  provider: ollama\n  model: {MODEL}\n  num_predict: 32768\n  {STRICT}\n"),
    )
    .unwrap();
    let record = large.record(&environment, &[]);
    assert_eq!(
        json(&record)["error"]["detail"]["context"]["reason"],
        "num_predict_not_below_num_ctx"
    );
    assert!(double.requests().is_empty());
}

/// Trace: FR-055-AC-6
#[test]
fn the_default_loopback_url_serves_an_absent_or_empty_variable_and_bad_values_send_nothing() {
    let Some(default) = Double::on_default() else {
        eprintln!("skipped: the default loopback port is in use");
        return;
    };
    let workspace = Workspace::new("");
    // Absent and empty both reach the double listening at the default address.
    let absent = workspace.record(&[], &[]);
    assert!(absent.status.success(), "{}", text(&absent));
    let reached = default.generates();
    assert_eq!(reached, 1);
    std::fs::remove_file(workspace.file("recording.json")).unwrap();
    let empty = workspace.record(&[("OLLAMA_BASE_URL", "")], &[]);
    assert!(empty.status.success(), "{}", text(&empty));
    assert_eq!(default.generates(), 2);
    std::fs::remove_file(workspace.file("recording.json")).unwrap();
    let before = default.requests().len();
    for value in ["not a url", "http://user:pw@host/"] {
        let output = workspace.record(&[("OLLAMA_BASE_URL", value)], &[]);
        assert_eq!(output.status.code(), Some(2), "{}", text(&output));
        let refusal = json(&output);
        assert_eq!(refusal["error"]["detail"]["code"], "config");
        assert_eq!(
            refusal["error"]["detail"]["context"]["reason"],
            "invalid_base_url"
        );
        let shown = text(&output);
        assert!(!shown.contains("pw@host") && !shown.contains("not a url"));
    }
    assert_eq!(default.requests().len(), before);
}

/// Trace: FR-055-AC-7
#[test]
fn the_earlier_of_the_binding_and_run_deadlines_ends_the_call_and_nothing_retries() {
    let slow = Duration::from_secs(2);
    // The binding bound is the earlier one.
    let workspace = Workspace::new("");
    std::fs::write(
        workspace.file("bindings.yaml"),
        format!("judge:\n  provider: ollama\n  model: {MODEL}\n  timeout_seconds: 1\n  {STRICT}\n"),
    )
    .unwrap();
    let double = Double::start(slow);
    let output = workspace.record(&[("OLLAMA_BASE_URL", double.url.as_str())], &[]);
    let report = json(&output);
    assert_eq!(output.status.code(), Some(2), "{}", text(&output));
    assert_eq!(report["error"]["code"], "deadline_exceeded");
    assert_eq!(report["error"]["context"]["reason"], "timeout");
    assert_eq!(double.generates(), 1);

    // The run bound is the earlier one.
    let workspace = Workspace::new("");
    let double = Double::start(slow);
    let output = workspace.record(
        &[("OLLAMA_BASE_URL", double.url.as_str())],
        &["--timeout-secs", "1"],
    );
    let report = json(&output);
    assert_eq!(output.status.code(), Some(2), "{}", text(&output));
    assert_eq!(report["error"]["code"], "deadline_exceeded");
    assert!(report["error"]["context"].get("reason").is_none());
    assert_eq!(double.generates(), 1);

    // A run bound above the delay succeeds.
    let workspace = Workspace::new("");
    let double = Double::start(slow);
    let output = workspace.record(
        &[("OLLAMA_BASE_URL", double.url.as_str())],
        &["--timeout-secs", "10"],
    );
    assert!(output.status.success(), "{}", text(&output));
    assert_eq!(double.generates(), 1);
}

/// A live check against a real server; it runs only when `SAPHO_LIVE_OLLAMA` is set.
///
/// Trace: FR-055-AC-8
#[test]
fn live_server_smoke_runs_only_when_asked_for() {
    let Some(model) = std::env::var_os("SAPHO_LIVE_OLLAMA") else {
        return;
    };
    let model = model.to_string_lossy().into_owned();
    let workspace = Workspace::new("");
    std::fs::write(
        workspace.file("bindings.yaml"),
        format!("judge:\n  provider: ollama\n  model: {model}\n  {STRICT}\n"),
    )
    .unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_sapho"));
    command.args([
        "run",
        &workspace.file("graph.yaml"),
        "--input",
        &workspace.file("input.json"),
        "--bindings",
        &workspace.file("bindings.yaml"),
        "--timeout-secs",
        "600",
    ]);
    let output = command.output().unwrap();
    assert!(output.status.success(), "{}", text(&output));
}
