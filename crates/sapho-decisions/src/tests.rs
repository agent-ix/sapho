// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Synthetic official wire, refusal, bounds and exact replay evidence.
use super::*;
use sapho_core::{
    Answer, BackendId, ChoiceOption, Datum, DistributionPolicy, NamedQuestion, Question, Value,
    validate_response,
};
use sapho_recording::{Recording, RecordingBackend, ReplayBackend};
use serde_json::{Value as Json, json};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicUsize, Ordering},
};

fn request() -> ModelRequest {
    ModelRequest {
        backend: BackendId::new("judge").unwrap(),
        model: MODEL.into(),
        expected_model: Some(MODEL.into()),
        distribution_policy: DistributionPolicy::Strict {},
        state: Value::Record(BTreeMap::from([
            (
                "nested".into(),
                Value::Record(BTreeMap::from([("z".into(), Value::Boolean(true))])),
            ),
            ("alpha".into(), Value::Text("synthetic".into())),
        ])),
        questions: vec![
            NamedQuestion {
                id: "relevant".into(),
                question: Question::Boolean {
                    instructions: "Relevant?".into(),
                    yes: "Applicable".into(),
                    no: "Not applicable".into(),
                },
            },
            NamedQuestion {
                id: "route".into(),
                question: Question::Choice {
                    instructions: "Which route?".into(),
                    options: vec![
                        ChoiceOption {
                            label: "true".into(),
                            description: "Direct".into(),
                        },
                        ChoiceOption {
                            label: "billing".into(),
                            description: "Billing".into(),
                        },
                    ],
                },
            },
            NamedQuestion {
                id: "severity".into(),
                question: Question::Score {
                    instructions: "Severity?".into(),
                    levels: vec!["Low".into(), "Medium".into(), "High".into()],
                },
            },
        ],
    }
}
fn response() -> Vec<u8> {
    serde_json::to_vec(&json!({
        "model": MODEL,
        "answers": [
            {"type":"predicate","name":"relevant","probability":0.8},
            {"type":"choice","name":"route","choice":"billing","confidence":0.4,
             "probabilities":[{"value":"true","probability":0.1},{"value":"billing","probability":0.9}]},
            {"type":"score","name":"severity","score":1.1,"confidence":0.55,
             "probabilities":[{"value":0,"label":"0","probability":0.1},
                              {"value":1,"label":"1","probability":0.7},
                              {"value":2,"label":"2","probability":0.2}]}
        ],
        "usage":{"input_tokens":42,"input_tokens_details":{"cached_tokens":0,"cache_write_tokens":0},
                 "output_tokens":0,"output_tokens_details":{"reasoning_tokens":0},"total_tokens":42}
    })).unwrap()
}
struct Fixed {
    status: u16,
    body: Vec<u8>,
    calls: AtomicUsize,
}
#[async_trait]
impl Transport for Fixed {
    async fn post(&self, _: &[u8], _: usize) -> Result<HttpResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(HttpResponse {
            status: self.status,
            body: self.body.clone(),
        })
    }
}
fn fixed(status: u16, body: Vec<u8>) -> Arc<Fixed> {
    Arc::new(Fixed {
        status,
        body,
        calls: AtomicUsize::new(0),
    })
}
/// Trace: FR-085-AC-1, FR-085-AC-2, FR-085-AC-3, FR-085-AC-6, IT-018-SC-01, IT-018-SC-02
#[tokio::test]
async fn official_three_question_codec_and_reserved_image_envelope() {
    let req = request();
    let body: Json = serde_json::from_slice(&wire::encode(&req, 1_048_576).unwrap()).unwrap();
    assert_eq!(body["model"], MODEL);
    assert_eq!(
        body["input"],
        r#"{"alpha":"synthetic","nested":{"z":true}}"#
    );
    assert_eq!(body["questions"][0]["type"], "predicate");
    assert_eq!(
        body["questions"][0]["instructions"],
        "Relevant?\nTrue criterion: Applicable\nFalse criterion: Not applicable"
    );
    assert_eq!(body["questions"][1]["choices"][0]["value"], "true");
    assert_eq!(body["questions"][1]["choices"][1]["value"], "billing");
    assert_eq!(body["questions"][2]["levels"][2]["label"], "2");
    assert!(body.get("distribution_policy").is_none());
    let seam = fixed(200, response());
    let backend = DecisionsBackend::with_transport(seam.clone(), Limits::default()).unwrap();
    let actual = backend.infer(&req).await.unwrap();
    assert_eq!(seam.calls.load(Ordering::SeqCst), 1);
    assert_eq!(actual.model, MODEL);
    assert_eq!(
        actual.answers["relevant"],
        Answer::Boolean {
            probability: sapho_core::Probability::new(0.8).unwrap()
        }
    );
    let Answer::Choice {
        selected,
        confidence,
        probabilities,
    } = &actual.answers["route"]
    else {
        panic!("choice")
    };
    assert_eq!(selected, "billing");
    assert_eq!(confidence.get(), 0.4);
    assert_eq!(probabilities.as_ref().unwrap()["billing"].get(), 0.9);
    let Answer::Score {
        expected,
        confidence,
        ..
    } = &actual.answers["severity"]
    else {
        panic!("score")
    };
    assert_eq!((*expected, confidence.get()), (1.1, 0.55));
    assert_eq!(actual.usage.as_ref().unwrap().input_tokens, 42);
    assert_eq!(actual.usage.as_ref().unwrap().billing_units, None);
    assert_eq!(actual.raw.as_ref().unwrap().response.as_bytes(), response());
    validate_response(&req, &actual).unwrap();

    let mut envelope = req.clone();
    envelope.state = Value::Record(BTreeMap::from([(
        "decisions_input".into(),
        Value::Record(BTreeMap::from([
            ("text".into(), Value::Text("Inspect".into())),
            (
                "images".into(),
                Value::List(vec![
                    Datum::new("0", Value::Text("data:image/png;base64,YWJj".into())).unwrap(),
                    Datum::new("1", Value::Text("data:image/jpeg;base64,YWJj".into())).unwrap(),
                ]),
            ),
        ])),
    )]));
    let encoded: Json =
        serde_json::from_slice(&wire::encode(&envelope, 1_048_576).unwrap()).unwrap();
    assert_eq!(encoded["input"][0]["role"], "user");
    assert_eq!(
        encoded["input"][0]["content"][0],
        json!({"type":"input_text","text":"Inspect"})
    );
    assert_eq!(
        encoded["input"][0]["content"][1]["image_url"],
        "data:image/png;base64,YWJj"
    );
    assert_eq!(
        encoded["input"][0]["content"][2]["image_url"],
        "data:image/jpeg;base64,YWJj"
    );
    let mut text_only = envelope.clone();
    if let Value::Record(state) = &mut text_only.state
        && let Some(Value::Record(fields)) = state.get_mut("decisions_input")
    {
        fields.insert("images".into(), Value::List(Vec::new()));
    }
    let text_body: Json =
        serde_json::from_slice(&wire::encode(&text_only, 1_048_576).unwrap()).unwrap();
    assert_eq!(text_body["input"], "Inspect");
    if let Value::Record(state) = &mut envelope.state {
        state.insert("other".into(), Value::Boolean(true));
    }
    let error = wire::encode(&envelope, 1_048_576).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidValue);
    assert_eq!(error.context["reason"], "invalid_decisions_input_envelope");
    if let Value::Record(state) = &mut envelope.state {
        state.remove("other");
        state.insert("decisions_input".into(), Value::Text("ordinary".into()));
    }
    assert_eq!(
        wire::encode(&envelope, 1_048_576).unwrap_err().code,
        ErrorCode::InvalidValue
    );
    if let Value::Record(state) = &mut envelope.state {
        state.insert(
            "decisions_input".into(),
            Value::Record(BTreeMap::from([
                ("text".into(), Value::Text("Inspect".into())),
                (
                    "images".into(),
                    Value::List(vec![
                        Datum::new("0", Value::Text("https://example.com/x.png".into())).unwrap(),
                    ]),
                ),
            ])),
        );
    }
    assert_eq!(
        wire::encode(&envelope, 1_048_576).unwrap_err().code,
        ErrorCode::InvalidValue
    );
}
/// Trace: FR-085-AC-4, FR-086-AC-4, IT-018-SC-03, IT-018-SC-05
#[tokio::test]
async fn wrong_success_shapes_and_statuses_refuse_without_diagnostic_leaks() {
    let req = request();
    let original: Json = serde_json::from_slice(&response()).unwrap();
    for mutate in [
        ("refusal", json!({"type":"refusal","name":"route"})),
        (
            "wrong_kind",
            json!({"type":"predicate","name":"route","probability":0.4}),
        ),
        (
            "boolean_choice",
            json!({"type":"choice","name":"route","choice":true,"confidence":0.4,"probabilities":[]}),
        ),
        (
            "null_name",
            json!({"type":"choice","name":null,"choice":"billing","confidence":0.4,"probabilities":[]}),
        ),
    ] {
        let mut changed = original.clone();
        changed["answers"][1] = mutate.1;
        let bytes = serde_json::to_vec(&changed).unwrap();
        let backend =
            DecisionsBackend::with_transport(fixed(200, bytes.clone()), Limits::default()).unwrap();
        let error = backend.infer(&req).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidAnswer, "{}", mutate.0);
        assert_eq!(error.raw.as_ref().unwrap().response.as_bytes(), bytes);
        assert!(!format!("{error:?}").contains("billing"));
    }
    for status in [400, 401, 403, 422, 429, 500] {
        let seam = fixed(status, b"sentinel private body".to_vec());
        let backend = DecisionsBackend::with_transport(seam, Limits::default()).unwrap();
        let error = backend.infer(&req).await.unwrap_err();
        assert_eq!(
            error.code,
            match status {
                400 | 422 => ErrorCode::ServiceValidation,
                401 | 403 => ErrorCode::Unauthorized,
                429 => ErrorCode::RateLimited,
                _ => ErrorCode::BackendFailed,
            }
        );
        assert!(error.raw.is_none());
        assert!(!format!("{error:?}").contains("sentinel"));
    }
}
/// Trace: FR-085-AC-5, FR-086-AC-2, FR-086-AC-5, IT-018-SC-03, IT-018-SC-04
#[tokio::test]
async fn limits_policy_and_recording_replay_preserve_provider_evidence() {
    let mut limits = Limits::default();
    for invalid in [0, 5] {
        limits.in_flight = invalid;
        assert!(DecisionsBackend::with_transport(fixed(200, response()), limits).is_err());
    }
    limits = Limits::default();
    limits.request_bytes = 0;
    assert!(DecisionsBackend::with_transport(fixed(200, response()), limits).is_err());
    limits = Limits::default();
    limits.timeout = Duration::from_secs(31);
    assert!(DecisionsBackend::with_transport(fixed(200, response()), limits).is_err());
    let mut req = request();
    req.model = "not-luna".into();
    let seam = fixed(200, response());
    let backend = DecisionsBackend::with_transport(seam.clone(), Limits::default()).unwrap();
    assert_eq!(
        backend.infer(&req).await.unwrap_err().code,
        ErrorCode::InvalidValue
    );
    assert_eq!(seam.calls.load(Ordering::SeqCst), 0);
    req.model = MODEL.into();
    let backend = Arc::new(backend);
    let recorder = RecordingBackend::new(backend, 1_048_576).unwrap();
    let actual = recorder.infer(&req).await.unwrap();
    let bytes = recorder.snapshot().unwrap().to_json(1_048_576).unwrap();
    let recording = Recording::from_json(&bytes, 1_048_576).unwrap();
    let replay = ReplayBackend::new(&recording, 1_048_576).unwrap();
    assert_eq!(replay.infer(&req).await.unwrap(), actual);
    req.distribution_policy = DistributionPolicy::approximate(0.02).unwrap();
    assert_eq!(
        replay.infer(&req).await.unwrap_err().code,
        ErrorCode::ReplayMiss
    );
}

/// Trace: FR-086-AC-1, FR-086-AC-4, FR-086-AC-5, IT-018-SC-05
#[tokio::test]
async fn loopback_http_capture_observes_one_bearer_post_and_no_redirect() {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    assert_eq!(ENDPOINT, "https://api.openai.com/v1/decisions");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = reqwest::Url::parse(&format!(
        "http://{}/v1/decisions",
        listener.local_addr().unwrap()
    ))
    .unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut bytes = Vec::new();
        let mut buffer = [0; 1024];
        loop {
            let n = stream.read(&mut buffer).await.unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&buffer[..n]);
            if let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&bytes[..header_end]).to_ascii_lowercase();
                let length = header
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length: "))
                    .unwrap()
                    .parse::<usize>()
                    .unwrap();
                if bytes.len() >= header_end + 4 + length {
                    break;
                }
            }
        }
        let body = response();
        stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n", body.len()).as_bytes()).await.unwrap();
        stream.write_all(&body).await.unwrap();
        bytes
    });
    let backend = DecisionsBackend::prepare_http(
        &SecretValue::new("secret-sentinel"),
        Limits::default(),
        endpoint,
    )
    .unwrap();
    let actual = backend.infer(&request()).await.unwrap();
    let capture = server.await.unwrap();
    let text = String::from_utf8(capture).unwrap();
    assert!(text.starts_with("POST /v1/decisions HTTP/1.1\r\n"));
    assert!(
        text.to_ascii_lowercase()
            .contains("authorization: bearer secret-sentinel\r\n")
    );
    assert!(
        text.to_ascii_lowercase()
            .contains("content-type: application/json\r\n")
    );
    assert!(
        !actual
            .raw
            .as_ref()
            .unwrap()
            .request
            .contains("secret-sentinel")
    );
    assert!(actual.raw.as_ref().unwrap().request.contains("gpt-6-luna"));
}

/// Trace: FR-085-AC-4, FR-085-AC-5, FR-086-AC-2, FR-086-AC-3, IT-018-SC-03, IT-018-SC-05
#[tokio::test]
async fn request_response_limits_and_core_mass_policy_stay_separate() {
    let req = request();
    let seam = fixed(200, response());
    let backend = DecisionsBackend::with_transport(
        seam.clone(),
        Limits {
            request_bytes: 8,
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!(
        backend.infer(&req).await.unwrap_err().code,
        ErrorCode::LimitExceeded
    );
    assert_eq!(seam.calls.load(Ordering::SeqCst), 0);
    let backend = DecisionsBackend::with_transport(
        seam.clone(),
        Limits {
            response_bytes: 8,
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!(
        backend.infer(&req).await.unwrap_err().code,
        ErrorCode::LimitExceeded
    );
    let mut mutated: Json = serde_json::from_slice(&response()).unwrap();
    mutated["answers"][1]["probabilities"][0]["probability"] = json!(0.09);
    let backend = DecisionsBackend::with_transport(
        fixed(200, serde_json::to_vec(&mutated).unwrap()),
        Limits::default(),
    )
    .unwrap();
    let raw = backend.infer(&req).await.unwrap();
    assert_eq!(
        validate_response(&req, &raw).unwrap_err().code,
        ErrorCode::InvalidAnswer
    );
    let mut approximate = req;
    approximate.distribution_policy = DistributionPolicy::approximate(0.02).unwrap();
    let accepted = validate_response(&approximate, &raw).unwrap();
    assert_eq!(accepted.values["route"], raw.answers["route"]);
    let mut limits = Limits::default();
    limits.response_bytes += 1;
    assert!(DecisionsBackend::with_transport(seam.clone(), limits).is_err());
    limits = Limits::default();
    limits.request_bytes += 1;
    assert!(DecisionsBackend::with_transport(seam.clone(), limits).is_err());
    assert!(
        DecisionsBackend::new(&SecretValue::new("\nheader-invalid"), Limits::default()).is_err()
    );
    assert!(DecisionsBackend::new(&SecretValue::new(" "), Limits::default()).is_err());
    assert!(DecisionsBackend::new(&SecretValue::new("bad token"), Limits::default()).is_err());
}
struct Concurrent {
    active: AtomicUsize,
    maximum: AtomicUsize,
    calls: AtomicUsize,
}
#[async_trait]
impl Transport for Concurrent {
    async fn post(&self, _: &[u8], _: usize) -> Result<HttpResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.maximum.fetch_max(active, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(15)).await;
        self.active.fetch_sub(1, Ordering::SeqCst);
        Ok(HttpResponse {
            status: 200,
            body: response(),
        })
    }
}
/// Trace: FR-086-AC-2, FR-086-AC-3, IT-018-SC-05
#[tokio::test]
async fn semaphore_admits_at_most_four_and_stalled_call_times_out() {
    let seam = Arc::new(Concurrent {
        active: AtomicUsize::new(0),
        maximum: AtomicUsize::new(0),
        calls: AtomicUsize::new(0),
    });
    let backend =
        Arc::new(DecisionsBackend::with_transport(seam.clone(), Limits::default()).unwrap());
    let tasks = (0..8)
        .map(|_| {
            let backend = backend.clone();
            tokio::spawn(async move { backend.infer(&request()).await })
        })
        .collect::<Vec<_>>();
    for task in tasks {
        task.await.unwrap().unwrap();
    }
    assert_eq!(seam.calls.load(Ordering::SeqCst), 8);
    assert!(seam.maximum.load(Ordering::SeqCst) <= 4);
    assert_eq!(seam.active.load(Ordering::SeqCst), 0);
    let limits = Limits {
        timeout: Duration::from_millis(1),
        ..Limits::default()
    };
    let slow = DecisionsBackend::with_transport(seam, limits).unwrap();
    assert_eq!(
        slow.infer(&request()).await.unwrap_err().code,
        ErrorCode::DeadlineExceeded
    );
}

/// Trace: FR-085-AC-4, IT-018-SC-03
#[tokio::test]
async fn malformed_answer_matrix_refuses_with_typed_errors_and_raw_exchange() {
    let base: Json = serde_json::from_slice(&response()).unwrap();
    let mut cases: Vec<(Json, ErrorCode)> = Vec::new();
    let mut missing = base.clone();
    missing["answers"].as_array_mut().unwrap().pop();
    cases.push((missing, ErrorCode::MissingAnswer));
    let mut extra = base.clone();
    extra["answers"]
        .as_array_mut()
        .unwrap()
        .push(json!({"type":"refusal","name":"extra"}));
    cases.push((extra, ErrorCode::InvalidAnswer));
    let mut reordered = base.clone();
    reordered["answers"].as_array_mut().unwrap().swap(0, 1);
    cases.push((reordered, ErrorCode::InvalidAnswer));
    let mut duplicate = base.clone();
    duplicate["answers"][1]["name"] = json!("relevant");
    cases.push((duplicate, ErrorCode::InvalidAnswer));
    let mut missing_mass = base.clone();
    missing_mass["answers"][1]["probabilities"]
        .as_array_mut()
        .unwrap()
        .pop();
    cases.push((missing_mass, ErrorCode::InvalidAnswer));
    let mut duplicate_mass = base.clone();
    duplicate_mass["answers"][1]["probabilities"][1]["value"] = json!("true");
    cases.push((duplicate_mass, ErrorCode::InvalidAnswer));
    let mut extra_mass = base.clone();
    extra_mass["answers"][1]["probabilities"]
        .as_array_mut()
        .unwrap()
        .push(json!({"value":"extra","probability":0.0}));
    cases.push((extra_mass, ErrorCode::InvalidAnswer));
    let mut score_label = base.clone();
    score_label["answers"][2]["probabilities"][1]["label"] = json!("two");
    cases.push((score_label, ErrorCode::InvalidAnswer));
    let mut score_index = base.clone();
    score_index["answers"][2]["probabilities"][1]["value"] = json!(4);
    cases.push((score_index, ErrorCode::InvalidAnswer));
    let mut score_range = base.clone();
    score_range["answers"][2]["score"] = json!(3.1);
    cases.push((score_range, ErrorCode::InvalidAnswer));
    let mut confidence = base.clone();
    confidence["answers"][1]["confidence"] = json!(1.1);
    cases.push((confidence, ErrorCode::InvalidAnswer));
    let mut negative_usage = base.clone();
    negative_usage["usage"]["input_tokens"] = json!(-1);
    cases.push((negative_usage, ErrorCode::InvalidAnswer));
    let mut absent_usage = base.clone();
    absent_usage.as_object_mut().unwrap().remove("usage");
    cases.push((absent_usage, ErrorCode::InvalidAnswer));
    let mut unknown = base.clone();
    unknown["answers"][0]["private"] = json!("sentinel");
    cases.push((unknown, ErrorCode::InvalidAnswer));
    for (case, expected) in cases {
        let bytes = serde_json::to_vec(&case).unwrap();
        let backend =
            DecisionsBackend::with_transport(fixed(200, bytes.clone()), Limits::default()).unwrap();
        let error = backend.infer(&request()).await.unwrap_err();
        assert_eq!(error.code, expected, "{case}");
        assert_eq!(error.raw.as_ref().unwrap().response.as_bytes(), bytes);
        assert!(!format!("{error:?}").contains("sentinel"));
        assert!(!serde_json::to_string(&error).unwrap().contains("sentinel"));
    }
    let raw = String::from_utf8(response()).unwrap().replacen(
        "\"model\":\"gpt-6-luna\"",
        "\"model\":\"gpt-6-luna\",\"model\":\"sentinel\"",
        1,
    );
    let backend =
        DecisionsBackend::with_transport(fixed(200, raw.into_bytes()), Limits::default()).unwrap();
    let error = backend.infer(&request()).await.unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidAnswer);
    assert!(error.raw.is_some());
    assert!(!format!("{error:?}").contains("sentinel"));
}

struct Waiting {
    calls: AtomicUsize,
    release: tokio::sync::Notify,
}
#[async_trait]
impl Transport for Waiting {
    async fn post(&self, _: &[u8], _: usize) -> Result<HttpResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.release.notified().await;
        Ok(HttpResponse {
            status: 200,
            body: response(),
        })
    }
}
/// Trace: FR-086-AC-3, IT-018-SC-05
#[tokio::test]
async fn cancelled_inference_releases_its_permit() {
    let seam = Arc::new(Waiting {
        calls: AtomicUsize::new(0),
        release: tokio::sync::Notify::new(),
    });
    let backend = Arc::new(
        DecisionsBackend::with_transport(
            seam.clone(),
            Limits {
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
        while seam.calls.load(Ordering::SeqCst) < 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    first.abort();
    let _ = first.await;
    let second = tokio::spawn({
        let backend = backend.clone();
        async move { backend.infer(&request()).await }
    });
    tokio::time::timeout(Duration::from_secs(1), async {
        while seam.calls.load(Ordering::SeqCst) < 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    seam.release.notify_one();
    second.await.unwrap().unwrap();
}
async fn http_fixture(reply: Vec<u8>) -> (reqwest::Url, tokio::task::JoinHandle<()>) {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = reqwest::Url::parse(&format!(
        "http://{}/v1/decisions",
        listener.local_addr().unwrap()
    ))
    .unwrap();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buffer = [0; 4096];
        let _ = stream.read(&mut buffer).await.unwrap();
        stream.write_all(&reply).await.unwrap();
    });
    (endpoint, task)
}
/// Trace: FR-086-AC-3, FR-086-AC-4, IT-018-SC-05
#[tokio::test]
async fn advertised_and_streamed_response_overflow_are_bounded() {
    let (url, task) = http_fixture(
        b"HTTP/1.1 200 OK\r\nContent-Length: 9000000\r\nConnection: close\r\n\r\n".to_vec(),
    )
    .await;
    let backend =
        DecisionsBackend::prepare_http(&SecretValue::new("test"), Limits::default(), url).unwrap();
    assert_eq!(
        backend.infer(&request()).await.unwrap_err().code,
        ErrorCode::LimitExceeded
    );
    task.await.unwrap();
    let body = "a".repeat(128);
    let reply = format!("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n{}\r\n0\r\n\r\n", body.len(), body).into_bytes();
    let (url, task) = http_fixture(reply).await;
    let backend = DecisionsBackend::prepare_http(
        &SecretValue::new("test"),
        Limits {
            response_bytes: 64,
            ..Limits::default()
        },
        url,
    )
    .unwrap();
    assert_eq!(
        backend.infer(&request()).await.unwrap_err().code,
        ErrorCode::LimitExceeded
    );
    task.await.unwrap();
}

/// Trace: FR-085-AC-2, FR-085-AC-5, FR-086-AC-2, IT-018-SC-02
#[tokio::test]
async fn invalid_envelopes_and_cardinalities_never_reach_transport() {
    let seam = fixed(200, response());
    let backend = DecisionsBackend::with_transport(seam.clone(), Limits::default()).unwrap();
    let mut cases = Vec::new();
    let mut wrong_model = request();
    wrong_model.model = "another-model".into();
    cases.push(wrong_model);
    let mut no_questions = request();
    no_questions.questions.clear();
    cases.push(no_questions);
    let mut many_questions = request();
    many_questions.questions = (0..33)
        .map(|index| NamedQuestion {
            id: format!("question-{index}"),
            question: Question::Boolean {
                instructions: "Check?".into(),
                yes: "Yes".into(),
                no: "No".into(),
            },
        })
        .collect();
    cases.push(many_questions);
    let mut many_choices = request();
    let Question::Choice { options, .. } = &mut many_choices.questions[1].question else {
        panic!("choice")
    };
    *options = (0..33)
        .map(|index| ChoiceOption {
            label: format!("label-{index}"),
            description: "Description".into(),
        })
        .collect();
    cases.push(many_choices);
    let mut many_levels = request();
    let Question::Score { levels, .. } = &mut many_levels.questions[2].question else {
        panic!("score")
    };
    *levels = (0..33).map(|index| format!("Level {index}")).collect();
    cases.push(many_levels);
    let mut empty_criterion = request();
    let Question::Boolean { yes, .. } = &mut empty_criterion.questions[0].question else {
        panic!("boolean")
    };
    yes.clear();
    cases.push(empty_criterion);
    for image in [
        "file-123",
        "data:image/png;base64,???",
        "https://example.com/photo.png",
    ] {
        let mut req = request();
        req.state = Value::Record(BTreeMap::from([(
            "decisions_input".into(),
            Value::Record(BTreeMap::from([
                ("text".into(), Value::Text("Inspect".into())),
                (
                    "images".into(),
                    Value::List(vec![Datum::new("0", Value::Text(image.into())).unwrap()]),
                ),
            ])),
        )]));
        cases.push(req);
    }
    let mut too_many_images = request();
    too_many_images.state = Value::Record(BTreeMap::from([(
        "decisions_input".into(),
        Value::Record(BTreeMap::from([
            ("text".into(), Value::Text("Inspect".into())),
            (
                "images".into(),
                Value::List(
                    (0..9)
                        .map(|index| {
                            Datum::new(
                                index.to_string(),
                                Value::Text("data:image/png;base64,YWJj".into()),
                            )
                            .unwrap()
                        })
                        .collect(),
                ),
            ),
        ])),
    )]));
    cases.push(too_many_images);
    for req in cases {
        let error = backend.infer(&req).await.unwrap_err();
        assert!(matches!(
            error.code,
            ErrorCode::InvalidValue | ErrorCode::LimitExceeded
        ));
    }
    assert_eq!(seam.calls.load(Ordering::SeqCst), 0);
    let mut exact_max = request();
    exact_max.questions = (0..32)
        .map(|index| NamedQuestion {
            id: format!("question-{index}"),
            question: Question::Boolean {
                instructions: "Check?".into(),
                yes: "Yes".into(),
                no: "No".into(),
            },
        })
        .collect();
    wire::encode(&exact_max, 1_048_576).unwrap();
}

/// Trace: FR-086-AC-1, FR-086-AC-4, IT-018-SC-05
#[tokio::test]
async fn redirect_and_transport_failure_are_single_attempt_redacted_errors() {
    let (url, task) = http_fixture(b"HTTP/1.1 302 Found\r\nLocation: https://example.invalid/other\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec()).await;
    let backend = DecisionsBackend::prepare_http(
        &SecretValue::new("secret-sentinel"),
        Limits::default(),
        url,
    )
    .unwrap();
    let error = backend.infer(&request()).await.unwrap_err();
    assert_eq!(error.code, ErrorCode::BackendFailed);
    assert_eq!(error.context["http_status"], "302");
    assert!(error.raw.is_none());
    assert!(!format!("{error:?}").contains("secret-sentinel"));
    task.await.unwrap();
    let unavailable = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = unavailable.local_addr().unwrap();
    drop(unavailable);
    let url = reqwest::Url::parse(&format!("http://{address}/v1/decisions")).unwrap();
    let backend = DecisionsBackend::prepare_http(
        &SecretValue::new("secret-sentinel"),
        Limits::default(),
        url,
    )
    .unwrap();
    let error = backend.infer(&request()).await.unwrap_err();
    assert_eq!(error.code, ErrorCode::BackendFailed);
    assert!(!format!("{error:?}").contains("secret-sentinel"));
    assert!(!format!("{error:?}").contains(&address.to_string()));
}
