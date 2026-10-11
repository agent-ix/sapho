// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Behavior at native HTTP and recording seams, without a model or credential store.
use super::*;
use sapho_core::{
    Answer, BackendId, ChoiceOption, DistributionPolicy, NamedQuestion, Question, Usage, Value,
    validate_response,
};
use sapho_recording::{Recording, RecordingBackend, ReplayBackend};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicUsize, Ordering},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Notify,
};
fn request() -> ModelRequest {
    ModelRequest {
        backend: BackendId::new("decision").unwrap(),
        model: DEFAULT_MODEL.into(),
        expected_model: Some(DEFAULT_MODEL.into()),
        distribution_policy: DistributionPolicy::Strict {},
        state: Value::Record(BTreeMap::from([(
            "context".into(),
            Value::Text("public synthetic example".into()),
        )])),
        questions: vec![
            NamedQuestion {
                id: "z_boolean".into(),
                question: Question::Boolean {
                    instructions: "True?".into(),
                    yes: "Affirmative".into(),
                    no: "Negative".into(),
                },
            },
            NamedQuestion {
                id: "a_choice".into(),
                question: Question::Choice {
                    instructions: "Which?".into(),
                    options: vec![
                        ChoiceOption {
                            label: "z".into(),
                            description: "Last".into(),
                        },
                        ChoiceOption {
                            label: "a".into(),
                            description: "First".into(),
                        },
                        ChoiceOption {
                            label: "middle".into(),
                            description: "Middle".into(),
                        },
                    ],
                },
            },
            NamedQuestion {
                id: "m_score".into(),
                question: Question::Score {
                    instructions: "How much?".into(),
                    levels: vec!["Low".into(), "High".into()],
                },
            },
        ],
    }
}
fn response() -> Vec<u8> {
    br#"{"model":"clm-latest","answers":{"z_boolean":{"type":"noul","noul":0.7},"a_choice":{"type":"choice","choice":"z","confidence":0.4,"probabilities":{"z":0.6,"a":0.2,"middle":0.2}},"m_score":{"type":"score","score":0.75,"confidence":0.5,"legend":{"0":"Low","1":"High"},"probabilities":{"0":0.25,"1":0.75}}},"usage":{"input_tokens":38,"output_tokens":0,"billing_units":3}}"#.to_vec()
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
async fn capture_server(
    status: u16,
    body: Vec<u8>,
    chunked: bool,
) -> (String, tokio::task::JoinHandle<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut bytes = Vec::new();
        let mut buffer = [0; 1024];
        loop {
            let n = stream.read(&mut buffer).await.unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&buffer[..n]);
            if let Some(index) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..index]);
                let size: usize = headers
                    .lines()
                    .find_map(|line| {
                        line.to_lowercase()
                            .strip_prefix("content-length:")
                            .map(|n| n.trim().parse().unwrap())
                    })
                    .unwrap();
                if bytes.len() >= index + 4 + size {
                    break;
                }
            }
        }
        let header = if chunked {
            format!(
                "HTTP/1.1 {status} OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
            )
        } else {
            format!(
                "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
        };
        stream.write_all(header.as_bytes()).await.unwrap();
        if chunked {
            stream
                .write_all(format!("{:x}\r\n", body.len()).as_bytes())
                .await
                .unwrap();
            stream.write_all(&body).await.unwrap();
            stream.write_all(b"\r\n0\r\n\r\n").await.unwrap();
        } else {
            stream.write_all(&body).await.unwrap();
        }
        bytes
    });
    (format!("http://{address}"), task)
}
/// Trace: NFR-004-M-1, FR-043-AC-1, FR-043-AC-2, FR-044-AC-1
#[tokio::test]
async fn real_http_preserves_order_true_false_confidence_and_usage() {
    let (url, server) = capture_server(200, response(), false).await;
    let secret = SecretValue::new("synthetic-test-credential");
    let backend = ClmBackend::new(&url, Some(&secret), Limits::default()).unwrap();
    let raw = backend.infer(&request()).await.unwrap();
    let captured = String::from_utf8(server.await.unwrap()).unwrap();
    let (headers, body) = captured.split_once("\r\n\r\n").unwrap();
    assert!(headers.starts_with("POST /v1/systemone HTTP/1.1"));
    assert!(
        headers
            .to_lowercase()
            .contains("authorization: bearer synthetic-test-credential")
    );
    let decoded: serde_json::Value = serde_json::from_str(body).unwrap();
    assert_eq!(
        decoded["state"],
        serde_json::json!({"context":"public synthetic example"})
    );
    assert_eq!(decoded["model"], "clm-latest");
    assert_eq!(
        decoded["questions"]["z_boolean"],
        serde_json::json!({"type":"noul","instructions":"True?","criteria":{"true":"Affirmative","false":"Negative"}})
    );
    assert_eq!(
        decoded["questions"]["a_choice"]["criteria"],
        serde_json::json!({"z":"Last","a":"First","middle":"Middle"})
    );
    assert_eq!(
        decoded["questions"]["m_score"]["criteria"],
        serde_json::json!(["Low", "High"])
    );
    assert!(body.find("z_boolean").unwrap() < body.find("a_choice").unwrap());
    assert!(body.find("a_choice").unwrap() < body.find("m_score").unwrap());
    assert!(body.find("\"z\":\"Last\"").unwrap() < body.find("\"a\":\"First\"").unwrap());
    let Answer::Choice {
        confidence,
        probabilities,
        ..
    } = &raw.answers["a_choice"]
    else {
        panic!("choice")
    };
    assert_eq!(confidence.get(), 0.4);
    assert_eq!(probabilities.as_ref().unwrap()["z"].get(), 0.6);
    let Answer::Score {
        expected,
        confidence,
        ..
    } = &raw.answers["m_score"]
    else {
        panic!("score")
    };
    assert_eq!((*expected, confidence.get()), (0.75, 0.5));
    assert_eq!(
        raw.usage,
        Some(Usage {
            input_tokens: 38,
            output_tokens: 0,
            billing_units: Some(3)
        })
    );
    validate_response(&request(), &raw).unwrap();
}
/// Trace: FR-043-AC-3
#[tokio::test]
async fn invalid_wire_and_empty_questions_refuse_without_private_errors() {
    let original = String::from_utf8(response()).unwrap();
    for body in [
        original.replace(
            "\"model\":\"clm-latest\"",
            "\"model\":\"clm-latest\",\"model\":\"sentinel\"",
        ),
        original.replace("\"input_tokens\":38", "\"input_tokens\":\"sentinel\""),
        original.replace("\"billing_units\":3", "\"billing_units\":-1"),
        original.replace(
            "\"output_tokens\":0",
            "\"output_tokens\":0,\"unknown\":\"sentinel\"",
        ),
        original.replace("\"noul\":0.7", "\"noul\":1.1"),
        original.replace("\"confidence\":0.4", "\"confidence\":\"sentinel\""),
        original.replace("\"score\":0.75", "\"score\":\"sentinel\""),
        original.replace("\"choice\":\"z\"", "\"choice\":false"),
        original.replace("\"1\":\"High\"", "\"1\":\"sentinel\""),
        original.replace("\"z\":0.6,\"a\":0.2,\"middle\":0.2", "\"z\":0.6"),
        original.replace("\"type\":\"noul\"", "\"type\":\"sentinel\""),
        original.replace("z_boolean", "unknown_boolean"),
        original.replace("\"model\":\"clm-latest\"", "\"model\":\"\""),
        "sentinel: not JSON".into(),
    ] {
        let backend =
            ClmBackend::with_transport(fixed(200, body.into_bytes()), Limits::default()).unwrap();
        let error = backend.infer(&request()).await.unwrap_err();
        assert!(matches!(
            error.code,
            ErrorCode::InvalidAnswer | ErrorCode::MissingAnswer
        ));
        assert!(!format!("{error:?}").contains("sentinel"));
    }
    let seam = fixed(200, response());
    let backend = ClmBackend::with_transport(seam.clone(), Limits::default()).unwrap();
    let mut empty = request();
    empty.questions.clear();
    assert_eq!(
        backend.infer(&empty).await.unwrap_err().code,
        ErrorCode::InvalidValue
    );
    assert_eq!(seam.calls.load(Ordering::SeqCst), 0);
}
/// Trace: FR-043-AC-4, FR-045-AC-3
#[tokio::test]
async fn approximate_raw_mass_survives_recording_and_policy_sensitive_replay() {
    let bytes = String::from_utf8(response())
        .unwrap()
        .replace("\"a\":0.2", "\"a\":0.19")
        .into_bytes();
    let mut req = request();
    let backend =
        Arc::new(ClmBackend::with_transport(fixed(200, bytes), Limits::default()).unwrap());
    let raw = backend.infer(&req).await.unwrap();
    assert_eq!(
        validate_response(&req, &raw).unwrap_err().code,
        ErrorCode::InvalidAnswer
    );
    req.distribution_policy = DistributionPolicy::approximate(0.02).unwrap();
    let accepted = validate_response(&req, &raw).unwrap();
    assert!(
        (accepted
            .distribution_adjustment("a_choice")
            .unwrap()
            .unwrap()
            .raw_mass
            - 0.99)
            .abs()
            < 1e-12
    );
    let recorder = RecordingBackend::new(backend, 1_048_576).unwrap();
    assert_eq!(recorder.infer(&req).await.unwrap(), raw);
    let saved = recorder.snapshot().unwrap().to_json(1_048_576).unwrap();
    let recording = Recording::from_json(&saved, 1_048_576).unwrap();
    let replay = ReplayBackend::new(&recording, 1_048_576).unwrap();
    assert_eq!(replay.infer(&req).await.unwrap(), raw);
    req.distribution_policy = DistributionPolicy::Strict {};
    assert_eq!(
        replay.infer(&req).await.unwrap_err().code,
        ErrorCode::ReplayMiss
    );
}
/// Trace: FR-044-AC-2
#[test]
fn endpoints_and_limits_refuse_unsafe_or_unbounded_configuration() {
    for url in [
        "http://example.com",
        "https://user:sentinel@example.com",
        "https://example.com?sentinel",
        "https://example.com#sentinel",
        "file:///tmp/service",
        "relative",
    ] {
        assert!(matches!(
            ClmBackend::new(url, None, Limits::default()),
            Err(ConfigurationError::InvalidEndpoint)
        ));
    }
    assert_eq!(
        endpoint("http://[::1]:8700").unwrap().path(),
        "/v1/systemone"
    );
    assert_eq!(
        endpoint("https://example.com/prefix/").unwrap().path(),
        "/prefix/v1/systemone"
    );
    for limits in [
        Limits {
            timeout: Duration::ZERO,
            ..Limits::default()
        },
        Limits {
            request_bytes: 0,
            ..Limits::default()
        },
        Limits {
            response_bytes: 0,
            ..Limits::default()
        },
        Limits {
            in_flight: 0,
            ..Limits::default()
        },
        Limits {
            in_flight: usize::MAX,
            ..Limits::default()
        },
    ] {
        assert!(matches!(
            ClmBackend::new(DEFAULT_BASE_URL, None, limits),
            Err(ConfigurationError::InvalidLimits)
        ));
    }
    assert!(matches!(
        ClmBackend::new(
            DEFAULT_BASE_URL,
            Some(&SecretValue::new("sentinel\n")),
            Limits::default()
        ),
        Err(ConfigurationError::InvalidCredential)
    ));
}
/// Trace: FR-044-AC-3, FR-072-AC-5, IT-013-SC-05
#[tokio::test]
async fn byte_limits_accept_exact_boundary_and_reject_overflow_before_send_or_decode() {
    let body = response();
    let req = request();
    let encoded = wire::encode(&req, 1_048_576).unwrap();
    let seam = fixed(200, body.clone());
    let exact = Limits {
        request_bytes: encoded.len(),
        response_bytes: body.len(),
        ..Limits::default()
    };
    ClmBackend::with_transport(seam.clone(), exact)
        .unwrap()
        .infer(&req)
        .await
        .unwrap();
    let backend = ClmBackend::with_transport(
        seam.clone(),
        Limits {
            request_bytes: encoded.len() - 1,
            ..exact
        },
    )
    .unwrap();
    assert_eq!(
        backend.infer(&req).await.unwrap_err().code,
        ErrorCode::LimitExceeded
    );
    assert_eq!(seam.calls.load(Ordering::SeqCst), 1);
    for chunked in [false, true] {
        let (url, task) = capture_server(200, body.clone(), chunked).await;
        let backend = ClmBackend::new(
            &url,
            None,
            Limits {
                response_bytes: body.len() - 1,
                ..exact
            },
        )
        .unwrap();
        assert_eq!(
            backend.infer(&req).await.unwrap_err().code,
            ErrorCode::LimitExceeded
        );
        task.await.unwrap();
    }
}
/// Trace: FR-044-AC-4
#[tokio::test]
async fn status_categories_are_single_attempt_and_never_echo_error_bodies() {
    for (status, expected) in [
        (401, ErrorCode::Unauthorized),
        (403, ErrorCode::Unauthorized),
        (429, ErrorCode::RateLimited),
        (422, ErrorCode::ServiceValidation),
        (400, ErrorCode::ServiceValidation),
        (502, ErrorCode::BackendFailed),
    ] {
        let seam = fixed(status, b"sentinel-secret-and-input".to_vec());
        let backend = ClmBackend::with_transport(seam.clone(), Limits::default()).unwrap();
        let error = backend.infer(&request()).await.unwrap_err();
        assert_eq!(error.code, expected);
        assert!(!format!("{error:?}").contains("sentinel"));
        assert_eq!(seam.calls.load(Ordering::SeqCst), 1);
    }
}
struct Waiting {
    calls: AtomicUsize,
    started: Notify,
    release: Notify,
}
#[async_trait]
impl Transport for Waiting {
    async fn post(&self, _: &[u8], _: usize) -> Result<HttpResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.started.notify_one();
        self.release.notified().await;
        Ok(HttpResponse {
            status: 200,
            body: response(),
        })
    }
}
/// Trace: FR-044-AC-3, FR-072-AC-5, IT-013-SC-05
#[tokio::test(start_paused = true)]
async fn queued_timeout_and_cancellation_release_concurrency_capacity() {
    let seam = Arc::new(Waiting {
        calls: AtomicUsize::new(0),
        started: Notify::new(),
        release: Notify::new(),
    });
    let backend = Arc::new(
        ClmBackend::with_transport(
            seam.clone(),
            Limits {
                in_flight: 1,
                ..Limits::default()
            },
        )
        .unwrap(),
    );
    let first = {
        let backend = backend.clone();
        tokio::spawn(async move { backend.infer(&request()).await })
    };
    seam.started.notified().await;
    let second = {
        let backend = backend.clone();
        tokio::spawn(async move { backend.infer(&request()).await })
    };
    tokio::task::yield_now().await;
    assert_eq!(seam.calls.load(Ordering::SeqCst), 1);
    tokio::time::advance(Duration::from_secs(31)).await;
    assert_eq!(
        first.await.unwrap().unwrap_err().code,
        ErrorCode::DeadlineExceeded
    );
    assert_eq!(
        second.await.unwrap().unwrap_err().code,
        ErrorCode::DeadlineExceeded
    );
    assert_eq!(seam.calls.load(Ordering::SeqCst), 1);
    let first = {
        let backend = backend.clone();
        tokio::spawn(async move { backend.infer(&request()).await })
    };
    seam.started.notified().await;
    let second = {
        let backend = backend.clone();
        tokio::spawn(async move { backend.infer(&request()).await })
    };
    tokio::task::yield_now().await;
    second.abort();
    assert!(second.await.unwrap_err().is_cancelled());
    first.abort();
    assert!(first.await.unwrap_err().is_cancelled());
    let third = {
        let backend = backend.clone();
        tokio::spawn(async move { backend.infer(&request()).await })
    };
    seam.started.notified().await;
    assert_eq!(seam.calls.load(Ordering::SeqCst), 3);
    tokio::time::advance(Duration::from_secs(31)).await;
    assert_eq!(
        third.await.unwrap().unwrap_err().code,
        ErrorCode::DeadlineExceeded
    );
    let fourth = {
        let backend = backend.clone();
        tokio::spawn(async move { backend.infer(&request()).await })
    };
    seam.started.notified().await;
    seam.release.notify_one();
    fourth.await.unwrap().unwrap();
}
/// Trace: FR-044-AC-1, FR-044-AC-4
#[tokio::test]
async fn production_redirect_is_refused_without_following_location() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut bytes = [0; 4096];
        assert!(stream.read(&mut bytes).await.unwrap() > 0);
        stream.write_all(b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/sentinel\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap();
    });
    let backend = ClmBackend::new(&url, None, Limits::default()).unwrap();
    let error = backend.infer(&request()).await.unwrap_err();
    assert_eq!(error.code, ErrorCode::BackendFailed);
    assert_eq!(
        error.context.get("http_status").map(String::as_str),
        Some("302")
    );
    task.await.unwrap();
}
/// Trace: FR-044-AC-3, FR-044-AC-4
#[tokio::test]
async fn production_http_stall_times_out_without_echoing_transport_text() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let started = Arc::new(Notify::new());
    let server_started = started.clone();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut byte = [0];
        stream.read_exact(&mut byte).await.unwrap();
        server_started.notify_one();
        std::future::pending::<()>().await;
    });
    let backend = ClmBackend::new(
        &url,
        None,
        Limits {
            timeout: Duration::from_millis(100),
            ..Limits::default()
        },
    )
    .unwrap();
    let call = tokio::spawn(async move { backend.infer(&request()).await });
    started.notified().await;
    let error = call.await.unwrap().unwrap_err();
    assert_eq!(error.code, ErrorCode::DeadlineExceeded);
    assert_eq!(error.message.as_ref(), "CLM request refused");
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
}
