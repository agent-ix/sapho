// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Synthetic Messages contract tests; no Claude account is used.
use super::*;
use sapho_core::{
    Answer, BackendId, ChoiceOption, DistributionPolicy, NamedQuestion, Question, Value,
    validate_response,
};
use serde_json::{Value as Json, json};
use std::{collections::BTreeMap, sync::Mutex};

fn request() -> ModelRequest {
    ModelRequest {
        backend: BackendId::new("reader").unwrap(),
        model: "claude-sonnet-4-6".into(),
        expected_model: None,
        distribution_policy: DistributionPolicy::Strict {},
        state: Value::Record(BTreeMap::from([(
            "passage".into(),
            Value::Text("sample text".into()),
        )])),
        questions: vec![
            NamedQuestion {
                id: "relevant".into(),
                question: Question::Boolean {
                    instructions: "Is it relevant?".into(),
                    yes: "Relevant".into(),
                    no: "Irrelevant".into(),
                },
            },
            NamedQuestion {
                id: "route".into(),
                question: Question::Choice {
                    instructions: "Choose route".into(),
                    options: vec![
                        ChoiceOption {
                            label: "one".into(),
                            description: "First".into(),
                        },
                        ChoiceOption {
                            label: "two".into(),
                            description: "Second".into(),
                        },
                    ],
                },
            },
            NamedQuestion {
                id: "severity".into(),
                question: Question::Score {
                    instructions: "Rate severity".into(),
                    levels: vec!["Low".into(), "Mid".into(), "High".into()],
                },
            },
        ],
    }
}
fn answers() -> Json {
    json!({"answers":[
        {"id":"relevant","kind":"boolean","probability":0.8},
        {"id":"route","kind":"choice","selected":"two","confidence":0.4,
            "probabilities":[{"label":"one","probability":0.1},{"label":"two","probability":0.9}]},
        {"id":"severity","kind":"score","expected":1.1,"confidence":0.55,
            "probabilities":[{"label":"0","probability":0.1},{"label":"1","probability":0.7},
                {"label":"2","probability":0.2}]}
    ]})
}
fn message(output: Json) -> Vec<u8> {
    serde_json::to_vec(&json!({"type":"message","id":"msg_test","role":"assistant",
        "model":"claude-sonnet-4-6-20260101","stop_reason":"end_turn","stop_details":null,
        "content":[{"type":"text","text":output.to_string()}],
        "usage":{"input_tokens":42,"output_tokens":7,"cache_read_input_tokens":2}}))
    .unwrap()
}
struct Fake {
    status: u16,
    response: Vec<u8>,
    calls: Mutex<Vec<Vec<u8>>>,
}
impl Fake {
    fn new(status: u16, response: Vec<u8>) -> Arc<Self> {
        Arc::new(Self {
            status,
            response,
            calls: Mutex::new(Vec::new()),
        })
    }
    fn calls(&self) -> Vec<Vec<u8>> {
        self.calls.lock().unwrap().clone()
    }
}
#[async_trait]
impl Transport for Fake {
    async fn post(&self, body: &[u8], _: usize) -> Result<HttpResponse> {
        self.calls.lock().unwrap().push(body.to_vec());
        Ok(HttpResponse {
            status: self.status,
            body: self.response.clone(),
        })
    }
}
/// Trace: FR-088-AC-1, FR-088-AC-2, FR-089-AC-4
#[tokio::test]
async fn three_kinds_roundtrip_exact_values_and_data_independent_schema() {
    let fake = Fake::new(200, message(answers()));
    let backend = ClaudeBackend::with_transport(fake.clone(), Limits::default()).unwrap();
    let request = request();
    let result = backend.infer(&request).await.unwrap();
    assert_eq!(fake.calls().len(), 1);
    let body: Json = serde_json::from_slice(&fake.calls()[0]).unwrap();
    assert_eq!(body["model"], request.model);
    assert_eq!(body["max_tokens"], 1024);
    assert_eq!(body["stream"], false);
    assert_eq!(body["output_config"]["format"]["type"], "json_schema");
    let schema = body["output_config"]["format"]["schema"].to_string();
    assert!(!schema.contains("sample text"));
    assert!(!schema.contains("relevant"));
    let prompt = body["messages"][0]["content"].as_str().unwrap();
    assert!(prompt.contains("sample text"));
    assert!(prompt.find("relevant").unwrap() < prompt.find("route").unwrap());
    assert!(!fake.calls()[0].windows(11).any(|x| x == b"source_ref:"));
    assert_eq!(result.model, "claude-sonnet-4-6-20260101");
    assert_eq!(result.usage.as_ref().unwrap().input_tokens, 42);
    assert_eq!(result.usage.as_ref().unwrap().output_tokens, 7);
    assert_eq!(result.usage.as_ref().unwrap().billing_units, None);
    assert_eq!(
        result.answers["relevant"],
        Answer::Boolean {
            probability: sapho_core::Probability::new(0.8).unwrap()
        }
    );
    let Answer::Choice {
        selected,
        confidence,
        probabilities,
    } = &result.answers["route"]
    else {
        panic!("choice")
    };
    assert_eq!(selected, "two");
    assert_eq!(confidence.get(), 0.4);
    assert_eq!(probabilities.as_ref().unwrap()["one"].get(), 0.1);
    let Answer::Score {
        expected,
        confidence,
        probabilities,
    } = &result.answers["severity"]
    else {
        panic!("score")
    };
    assert_eq!((*expected, confidence.get()), (1.1, 0.55));
    assert_eq!(probabilities.as_ref().unwrap()["1"].get(), 0.7);
    assert_eq!(
        result.raw.as_ref().unwrap().request.as_bytes(),
        fake.calls()[0]
    );
    assert_eq!(
        result.raw.as_ref().unwrap().response.as_bytes(),
        message(answers())
    );
    validate_response(&request, &result).unwrap();
}
/// Trace: FR-088-AC-3
#[tokio::test]
async fn malformed_refusal_cutoff_and_mass_are_whole_ask_failures_with_safe_raw() {
    type Mutation = (&'static str, Box<dyn Fn(&mut Json)>);
    let mutations: Vec<Mutation> = vec![
        ("refusal", Box::new(|v| v["stop_reason"] = json!("refusal"))),
        (
            "cutoff",
            Box::new(|v| v["stop_reason"] = json!("max_tokens")),
        ),
        (
            "extra_block",
            Box::new(|v| {
                v["content"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"type":"text","text":"x"}))
            }),
        ),
        (
            "wrong_usage",
            Box::new(|v| v["usage"]["input_tokens"] = json!(-1)),
        ),
        (
            "missing_answer",
            Box::new(|v| {
                let mut o: Json =
                    serde_json::from_str(v["content"][0]["text"].as_str().unwrap()).unwrap();
                o["answers"].as_array_mut().unwrap().remove(1);
                v["content"][0]["text"] = json!(o.to_string());
            }),
        ),
        (
            "wrong_kind",
            Box::new(|v| {
                let mut o: Json =
                    serde_json::from_str(v["content"][0]["text"].as_str().unwrap()).unwrap();
                o["answers"][0]["kind"] = json!("choice");
                v["content"][0]["text"] = json!(o.to_string());
            }),
        ),
        (
            "bad_label",
            Box::new(|v| {
                let mut o: Json =
                    serde_json::from_str(v["content"][0]["text"].as_str().unwrap()).unwrap();
                o["answers"][1]["selected"] = json!("other");
                v["content"][0]["text"] = json!(o.to_string());
            }),
        ),
        (
            "bad_mass",
            Box::new(|v| {
                let mut o: Json =
                    serde_json::from_str(v["content"][0]["text"].as_str().unwrap()).unwrap();
                o["answers"][1]["probabilities"][0]["probability"] = json!(0.3);
                v["content"][0]["text"] = json!(o.to_string());
            }),
        ),
    ];
    for (name, mutate) in mutations {
        let mut body: Json = serde_json::from_slice(&message(answers())).unwrap();
        mutate(&mut body);
        let fake = Fake::new(200, serde_json::to_vec(&body).unwrap());
        let backend = ClaudeBackend::with_transport(fake, Limits::default()).unwrap();
        let error = backend.infer(&request()).await.expect_err(name);
        assert!(
            matches!(
                error.code,
                ErrorCode::InvalidAnswer | ErrorCode::MissingAnswer
            ),
            "{name}: {:?}",
            error.code
        );
        assert!(error.raw.is_some(), "{name}");
        assert!(!format!("{error:?}").contains("sample text"));
        assert!(
            !serde_json::to_string(&error)
                .unwrap()
                .contains("sample text")
        );
    }
}
/// Trace: FR-088-AC-4, FR-088-AC-5, FR-089-AC-3
#[tokio::test]
async fn bounds_and_provider_400_refuse_without_fallback() {
    let fake = Fake::new(400, b"private request echoed".to_vec());
    let backend = ClaudeBackend::with_transport(fake.clone(), Limits::default()).unwrap();
    let error = backend.infer(&request()).await.err().unwrap();
    assert_eq!(error.code, ErrorCode::ServiceValidation);
    assert_eq!(fake.calls().len(), 1);
    assert!(error.raw.is_none());
    assert!(!format!("{error:?}").contains("private request echoed"));
    let mut too_many = request();
    for index in 0..14 {
        let mut question = too_many.questions[0].clone();
        question.id = format!("extra_{index}");
        too_many.questions.push(question);
    }
    assert_eq!(
        backend.infer(&too_many).await.err().unwrap().code,
        ErrorCode::InvalidValue
    );
    assert_eq!(fake.calls().len(), 1);
    let mut image_shaped = request();
    image_shaped.state = Value::Record(BTreeMap::from([(
        "image_url".into(),
        Value::Text("https://example.invalid/private.png".into()),
    )]));
    let encoded = super::wire::encode(&image_shaped, 1024, 1_048_576).unwrap();
    assert!(
        String::from_utf8(encoded)
            .unwrap()
            .contains("https://example.invalid/private.png")
    );
    let oversized = Limits {
        request_bytes: 10,
        ..Limits::default()
    };
    let small = ClaudeBackend::with_transport(fake.clone(), oversized).unwrap();
    assert_eq!(
        small.infer(&request()).await.err().unwrap().code,
        ErrorCode::LimitExceeded
    );
    assert_eq!(fake.calls().len(), 1);
}
/// Trace: FR-089-AC-1, FR-089-AC-2, FR-089-AC-4
#[test]
fn endpoint_headers_and_configuration_bounds_are_checked() {
    let secret = SecretValue::new("private-key");
    assert!(
        ClaudeBackend::at_endpoint(
            "http://127.0.0.1:99/v1/messages",
            &secret,
            Some("wrkspc_123"),
            Limits::default()
        )
        .is_ok()
    );
    for endpoint in [
        "http://remote.example/v1/messages",
        "https://a.example/elsewhere",
        "https://u:p@a.example/v1/messages",
        "https://a.example/v1/messages?q=x",
    ] {
        assert!(matches!(
            ClaudeBackend::at_endpoint(endpoint, &secret, None, Limits::default()),
            Err(ConfigurationError::InvalidEndpoint)
        ));
    }
    assert_eq!(
        validate_workspace_id(&format!("wrkspc_{}", "a".repeat(121))),
        Ok(())
    );
    assert_eq!(
        validate_workspace_id(&format!("wrkspc_{}", "a".repeat(122))),
        Err(ConfigurationError::InvalidWorkspace)
    );
    assert_eq!(
        validate_workspace_id("wrkspc_x\r\nX: y"),
        Err(ConfigurationError::InvalidWorkspace)
    );
    assert!(matches!(
        ClaudeBackend::new(&SecretValue::new("a".repeat(8193)), None, Limits::default()),
        Err(ConfigurationError::InvalidCredential)
    ));
    assert!(
        ClaudeBackend::new(&SecretValue::new("a".repeat(8192)), None, Limits::default()).is_ok()
    );
    for limits in [
        Limits {
            timeout: Duration::ZERO,
            ..Limits::default()
        },
        Limits {
            timeout: Duration::from_secs(31),
            ..Limits::default()
        },
        Limits {
            in_flight: 5,
            ..Limits::default()
        },
        Limits {
            in_flight: 0,
            ..Limits::default()
        },
        Limits {
            request_bytes: 0,
            ..Limits::default()
        },
        Limits {
            request_bytes: 1_048_577,
            ..Limits::default()
        },
        Limits {
            response_bytes: 0,
            ..Limits::default()
        },
        Limits {
            response_bytes: 8 * 1_048_576 + 1,
            ..Limits::default()
        },
        Limits {
            max_tokens: 0,
            ..Limits::default()
        },
        Limits {
            max_tokens: 4097,
            ..Limits::default()
        },
    ] {
        assert!(matches!(
            ClaudeBackend::new(&secret, None, limits),
            Err(ConfigurationError::InvalidLimits)
        ));
    }
    assert!(
        ClaudeBackend::new(
            &secret,
            None,
            Limits {
                max_tokens: 4096,
                ..Limits::default()
            }
        )
        .is_ok()
    );
}

/// Trace: FR-089-AC-1, FR-089-AC-4
#[tokio::test(flavor = "multi_thread")]
async fn loopback_sends_bearer_version_and_workspace_only_in_headers() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1/messages", listener.local_addr().unwrap());
    let response = message(answers());
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = Vec::new();
        loop {
            let mut buf = [0u8; 2048];
            let n = stream.read(&mut buf).unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&buf[..n]);
            if let Some(end) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&bytes[..end]);
                let length: usize = header
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|s| s.trim().parse().unwrap())
                    })
                    .unwrap();
                if bytes.len() >= end + 4 + length {
                    break;
                }
            }
        }
        let captured = String::from_utf8(bytes).unwrap();
        let wire = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n",
            response.len()
        );
        stream.write_all(wire.as_bytes()).unwrap();
        stream.write_all(&response).unwrap();
        captured
    });
    let backend = ClaudeBackend::at_endpoint(
        &url,
        &SecretValue::new("private-key"),
        Some("wrkspc_demo"),
        Limits::default(),
    )
    .unwrap();
    let result = backend.infer(&request()).await.unwrap();
    let captured = server.join().unwrap();
    assert!(captured.starts_with("POST /v1/messages HTTP/1.1"));
    let headers = captured
        .split("\r\n\r\n")
        .next()
        .unwrap()
        .to_ascii_lowercase();
    assert!(headers.contains("authorization: bearer private-key"));
    assert!(headers.contains("anthropic-version: 2023-06-01"));
    assert!(headers.contains("anthropic-workspace-id: wrkspc_demo"));
    assert!(headers.contains("content-type: application/json"));
    let raw = result.raw.unwrap();
    assert!(!raw.request.contains("private-key"));
    assert!(!raw.request.contains("wrkspc_demo"));
    assert!(!raw.response.contains("private-key"));
}

/// Trace: FR-089-AC-2
#[tokio::test]
async fn permit_ceiling_is_shared() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Slow {
        active: AtomicUsize,
        peak: AtomicUsize,
    }
    #[async_trait]
    impl Transport for Slow {
        async fn post(&self, _: &[u8], _: usize) -> Result<HttpResponse> {
            let now = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.peak.fetch_max(now, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(15)).await;
            self.active.fetch_sub(1, Ordering::SeqCst);
            Ok(HttpResponse {
                status: 200,
                body: message(answers()),
            })
        }
    }
    let slow = Arc::new(Slow {
        active: AtomicUsize::new(0),
        peak: AtomicUsize::new(0),
    });
    let backend = Arc::new(ClaudeBackend::with_transport(slow.clone(), Limits::default()).unwrap());
    let mut calls = Vec::new();
    for _ in 0..10 {
        let backend = backend.clone();
        calls.push(tokio::spawn(async move { backend.infer(&request()).await }));
    }
    for call in calls {
        call.await.unwrap().unwrap();
    }
    assert!(slow.peak.load(Ordering::SeqCst) <= 4);
}

/// Trace: FR-090-AC-3
#[tokio::test]
async fn successful_exchange_records_and_replays_without_provider() {
    use sapho_recording::{Recording, RecordingBackend, ReplayBackend};
    let request = request();
    let backend = Arc::new(
        ClaudeBackend::with_transport(Fake::new(200, message(answers())), Limits::default())
            .unwrap(),
    );
    let recorder = RecordingBackend::new(backend, 1_048_576).unwrap();
    let response = recorder.infer(&request).await.unwrap();
    let recording = recorder.snapshot().unwrap();
    assert_eq!(recording.exchanges.len(), 1);
    let bytes = recording.to_json(1_048_576).unwrap();
    let loaded = Recording::from_json(&bytes, 1_048_576).unwrap();
    let replay = ReplayBackend::new(&loaded, 1_048_576).unwrap();
    assert_eq!(replay.infer(&request).await.unwrap(), response);
    let mut changed = request;
    changed.model = "different".into();
    assert_eq!(
        replay.infer(&changed).await.err().unwrap().code,
        ErrorCode::ReplayMiss
    );
}

/// Trace: FR-088-AC-3
#[tokio::test]
async fn duplicate_ids_duplicate_keys_and_incomplete_distributions_refuse() {
    let mut variants = Vec::new();
    let mut duplicate = answers();
    duplicate["answers"][2]["id"] = json!("route");
    variants.push(message(duplicate));
    let mut incomplete = answers();
    incomplete["answers"][1]["probabilities"]
        .as_array_mut()
        .unwrap()
        .pop();
    variants.push(message(incomplete));
    let mut unknown = answers();
    unknown["answers"][1]["probabilities"][0]["label"] = json!("other");
    variants.push(message(unknown));
    let body = message(answers());
    let text = String::from_utf8(body).unwrap().replace(
        "\"input_tokens\":42",
        "\"input_tokens\":42,\"input_tokens\":43",
    );
    variants.push(text.into_bytes());
    for body in variants {
        let backend =
            ClaudeBackend::with_transport(Fake::new(200, body), Limits::default()).unwrap();
        let error = backend.infer(&request()).await.err().unwrap();
        assert!(matches!(
            error.code,
            ErrorCode::InvalidAnswer | ErrorCode::MissingAnswer
        ));
        assert!(error.raw.is_some());
    }
}

/// Trace: FR-088-AC-2
#[tokio::test]
async fn strict_and_approximate_mass_keep_provider_bytes_unchanged() {
    let mut output = answers();
    output["answers"][1]["probabilities"][0]["probability"] = json!(0.12);
    let body = message(output);
    let fake = Fake::new(200, body.clone());
    let backend = ClaudeBackend::with_transport(fake.clone(), Limits::default()).unwrap();
    let strict_error = backend.infer(&request()).await.err().unwrap();
    assert_eq!(strict_error.code, ErrorCode::InvalidAnswer);
    let mut approximate = request();
    approximate.distribution_policy = DistributionPolicy::approximate(0.03).unwrap();
    let result = backend.infer(&approximate).await.unwrap();
    assert_eq!(result.raw.as_ref().unwrap().response.as_bytes(), body);
    let Answer::Choice {
        probabilities: Some(values),
        ..
    } = &result.answers["route"]
    else {
        panic!("choice")
    };
    assert_eq!(values["one"].get(), 0.12);
    assert_eq!(fake.calls().len(), 2);
}

/// Trace: FR-089-AC-2
#[tokio::test]
async fn timeout_cancels_call_and_restores_permit() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct FirstHangs(AtomicUsize);
    #[async_trait]
    impl Transport for FirstHangs {
        async fn post(&self, _: &[u8], _: usize) -> Result<HttpResponse> {
            if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
                std::future::pending::<()>().await;
            }
            Ok(HttpResponse {
                status: 200,
                body: message(answers()),
            })
        }
    }
    let transport = Arc::new(FirstHangs(AtomicUsize::new(0)));
    let limits = Limits {
        timeout: Duration::from_millis(10),
        in_flight: 1,
        ..Limits::default()
    };
    let backend = ClaudeBackend::with_transport(transport.clone(), limits).unwrap();
    assert_eq!(
        backend.infer(&request()).await.err().unwrap().code,
        ErrorCode::DeadlineExceeded
    );
    assert!(backend.infer(&request()).await.is_ok());
    assert_eq!(transport.0.load(Ordering::SeqCst), 2);
}

/// Trace: FR-089-AC-3
#[tokio::test]
async fn http_statuses_and_oversized_success_map_without_body_disclosure() {
    for (status, expected) in [
        (400, ErrorCode::ServiceValidation),
        (401, ErrorCode::Unauthorized),
        (403, ErrorCode::Unauthorized),
        (422, ErrorCode::ServiceValidation),
        (429, ErrorCode::RateLimited),
        (500, ErrorCode::BackendFailed),
        (302, ErrorCode::BackendFailed),
    ] {
        let body = b"sentinel-secret private-content".to_vec();
        let backend =
            ClaudeBackend::with_transport(Fake::new(status, body), Limits::default()).unwrap();
        let error = backend
            .infer(&request())
            .await
            .expect_err("status must refuse");
        assert_eq!(error.code, expected, "status {status}");
        assert_eq!(error.context["http_status"], status.to_string());
        assert!(error.raw.is_none());
        assert!(!format!("{error:?}").contains("sentinel-secret"));
        assert!(
            !serde_json::to_string(&error)
                .unwrap()
                .contains("private-content")
        );
    }
    let limits = Limits {
        response_bytes: 16,
        ..Limits::default()
    };
    let backend =
        ClaudeBackend::with_transport(Fake::new(200, message(answers())), limits).unwrap();
    assert_eq!(
        backend.infer(&request()).await.expect_err("overflow").code,
        ErrorCode::LimitExceeded
    );
    let backend =
        ClaudeBackend::with_transport(Fake::new(200, b"invalid json".to_vec()), Limits::default())
            .unwrap();
    assert_eq!(
        backend.infer(&request()).await.expect_err("malformed").code,
        ErrorCode::InvalidAnswer
    );
}

/// Trace: FR-089-AC-2
#[tokio::test(flavor = "multi_thread")]
async fn declared_response_length_over_ceiling_refuses_before_collection() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1/messages", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = [0u8; 2048];
        let _ = stream.read(&mut bytes).unwrap();
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\n")
            .unwrap();
    });
    let backend = ClaudeBackend::at_endpoint(
        &url,
        &SecretValue::new("private"),
        None,
        Limits {
            response_bytes: 100,
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!(
        backend
            .infer(&request())
            .await
            .expect_err("declared overflow")
            .code,
        ErrorCode::LimitExceeded
    );
    server.join().unwrap();
}

/// Trace: FR-089-AC-2
#[tokio::test(flavor = "multi_thread")]
async fn underreported_content_length_cannot_yield_a_valid_answer() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1/messages", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = [0u8; 8192];
        let _ = stream.read(&mut bytes).unwrap();
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\n")
            .unwrap();
        stream.write_all(&message(answers())).unwrap();
    });
    let backend =
        ClaudeBackend::at_endpoint(&url, &SecretValue::new("private"), None, Limits::default())
            .unwrap();
    let error = backend
        .infer(&request())
        .await
        .expect_err("truncated message");
    assert!(matches!(
        error.code,
        ErrorCode::InvalidAnswer | ErrorCode::BackendFailed
    ));
    server.join().unwrap();
}

/// Trace: FR-088-AC-3, FR-089-AC-3
#[tokio::test]
async fn actual_model_mismatch_keeps_provider_text_only_in_private_raw_evidence() {
    let mut response: Json = serde_json::from_slice(&message(answers())).unwrap();
    response["model"] = json!("sentinel-private-provider-body");
    let backend = ClaudeBackend::with_transport(
        Fake::new(200, serde_json::to_vec(&response).unwrap()),
        Limits::default(),
    )
    .unwrap();
    let mut request = request();
    request.expected_model = Some("expected-model".into());
    let error = backend.infer(&request).await.expect_err("model mismatch");
    assert_eq!(error.code, ErrorCode::ModelMismatch);
    assert!(
        error
            .raw
            .as_ref()
            .unwrap()
            .response
            .contains("sentinel-private-provider-body")
    );
    assert!(!format!("{error:?}").contains("sentinel-private-provider-body"));
    assert!(!format!("{error}").contains("sentinel-private-provider-body"));
    assert!(
        !serde_json::to_string(&error)
            .unwrap()
            .contains("sentinel-private-provider-body")
    );
}

/// Trace: FR-089-AC-3
#[tokio::test]
async fn invalid_request_state_does_not_enter_printable_error() {
    let fake = Fake::new(200, message(answers()));
    let backend = ClaudeBackend::with_transport(fake.clone(), Limits::default()).unwrap();
    let mut request = request();
    request.state = Value::Record(BTreeMap::from([(
        "sentinel-private/name".into(),
        Value::Text("sentinel-private-value".into()),
    )]));
    let error = backend.infer(&request).await.expect_err("invalid state");
    assert_eq!(error.code, ErrorCode::InvalidValue);
    assert!(fake.calls().is_empty());
    for printable in [
        format!("{error:?}"),
        format!("{error}"),
        serde_json::to_string(&error).unwrap(),
    ] {
        assert!(!printable.contains("sentinel-private/name"));
        assert!(!printable.contains("sentinel-private-value"));
    }
}

/// Trace: FR-089-AC-2
#[tokio::test]
async fn exact_serialized_byte_boundaries_and_queued_deadline() {
    let input = request();
    let request_len = super::wire::encode(&input, 1024, 1_048_576).unwrap().len();
    let response = message(answers());
    let fake = Fake::new(200, response.clone());
    let exact = ClaudeBackend::with_transport(
        fake.clone(),
        Limits {
            request_bytes: request_len,
            response_bytes: response.len(),
            ..Limits::default()
        },
    )
    .unwrap();
    exact.infer(&input).await.unwrap();
    let request_short = ClaudeBackend::with_transport(
        fake.clone(),
        Limits {
            request_bytes: request_len - 1,
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!(
        request_short.infer(&input).await.unwrap_err().code,
        ErrorCode::LimitExceeded
    );
    let response_short = ClaudeBackend::with_transport(
        fake.clone(),
        Limits {
            response_bytes: response.len() - 1,
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!(
        response_short.infer(&input).await.unwrap_err().code,
        ErrorCode::LimitExceeded
    );
    assert_eq!(fake.calls().len(), 2);

    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Held(AtomicUsize);
    #[async_trait]
    impl Transport for Held {
        async fn post(&self, _: &[u8], _: usize) -> Result<HttpResponse> {
            self.0.fetch_add(1, Ordering::SeqCst);
            std::future::pending::<()>().await;
            unreachable!()
        }
    }
    let held = Arc::new(Held(AtomicUsize::new(0)));
    let backend = Arc::new(
        ClaudeBackend::with_transport(
            held.clone(),
            Limits {
                timeout: Duration::from_millis(100),
                in_flight: 1,
                ..Limits::default()
            },
        )
        .unwrap(),
    );
    let first = tokio::spawn({
        let backend = backend.clone();
        async move { backend.infer(&request()).await }
    });
    tokio::time::timeout(Duration::from_secs(1), async {
        while held.0.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let second = backend.infer(&input).await.unwrap_err();
    assert_eq!(second.code, ErrorCode::DeadlineExceeded);
    assert_eq!(
        first.await.unwrap().unwrap_err().code,
        ErrorCode::DeadlineExceeded
    );
    assert_eq!(held.0.load(Ordering::SeqCst), 1);
}
