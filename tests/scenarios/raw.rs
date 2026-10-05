// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Raw exchanges through traces and recordings, using the loopback Ollama double.
use crate::common::*;
use sapho::{core::*, recording::*, runtime::*};
use sapho_ollama::{Limits, OllamaBackend, Server, Settings};
use serde_json::{Value as Json, json};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

#[path = "../../crates/sapho-ollama/tests/common/mod.rs"]
mod double;
use double::{Fake, Reply, description};

const SENTINEL: &str = "SENTINEL-BEARER-6642";

/// A generate answer for the `q` Boolean question: `yes` with listed alternatives, stamped
/// with a creation time and duration counters that differ from call to call.
fn answer(call: usize, value: &str) -> Json {
    let tok = |text: &str, lp: f64, alts: &[(&str, f64)]| {
        json!({"token": text, "logprob": lp, "bytes": text.as_bytes(),
               "top_logprobs": alts.iter().map(|(t, l)| json!({"token": t, "logprob": l, "bytes": t.as_bytes()})).collect::<Vec<_>>()})
    };
    json!({
        "model": "model-1",
        "created_at": format!("2026-10-05T19:58:{call:02}Z"),
        "response": format!("{{\"q\": \"{value}\"}}"),
        "done": true,
        "done_reason": "stop",
        "total_duration": 1_000_000 + call,
        "load_duration": 10 + call,
        "prompt_eval_count": 40,
        "prompt_eval_duration": 5 + call,
        "eval_count": 8,
        "eval_duration": 7 + call,
        "logprobs": [
            tok("{\"", -0.01, &[]),
            tok("q", -0.01, &[]),
            tok("\": \"", -0.01, &[]),
            tok(value, -0.1, &[(value, -0.1), ("other", -3.0)]),
            tok("\"}", -0.01, &[]),
        ]
    })
}
async fn fake(values: Vec<&'static str>) -> Fake {
    let calls = AtomicUsize::new(0);
    Fake::start(move |r| match r.path.as_str() {
        "/api/show" => description(double::BLOB),
        _ => {
            let call = calls.fetch_add(1, Ordering::SeqCst);
            Reply::ok(answer(call + 1, values[call.min(values.len() - 1)]))
        }
    })
    .await
}
fn ollama(fake: &Fake, credential: bool) -> Arc<dyn ModelBackend> {
    let mut server = Server::new(
        &fake.url,
        Limits {
            timeout: Duration::from_secs(20),
            request_bytes: 1 << 20,
            response_bytes: 1 << 20,
        },
    )
    .unwrap();
    if credential {
        server = server
            .with_header("Authorization", &format!("Bearer {SENTINEL}"))
            .unwrap();
    }
    Arc::new(
        OllamaBackend::new(
            server,
            Settings {
                model: "model-1".into(),
                think: false,
                num_ctx: 4096,
                num_predict: 512,
            },
        )
        .unwrap(),
    )
}
fn ask_evidence(trace: &Trace) -> &ModelEvidence {
    trace.nodes.iter().find_map(|n| n.model.as_ref()).unwrap()
}

/// Trace: FR-017-AC-4, FR-027-AC-4, FR-027-AC-5, FR-035-AC-4
#[tokio::test]
async fn traces_and_recordings_keep_the_raw_exchange_and_never_a_header_credential() {
    let _serial = double::serial().await;
    let fake = fake(vec!["yes"]).await;
    let recorder = Arc::new(RecordingBackend::new(ollama(&fake, true), 1_000_000).unwrap());
    let run = engine(
        &ask_graph(None),
        &PrimitiveRegistry::default(),
        bindings(recorder.clone()),
    )
    .run(&Inputs::new(), limits())
    .await
    .unwrap();
    let sent = fake.generates()[0].text();
    assert!(
        fake.generates()[0].head.contains(SENTINEL),
        "the double sends the header"
    );
    let raw = ask_evidence(&run.trace)
        .response
        .as_ref()
        .unwrap()
        .raw
        .clone()
        .unwrap();
    assert_eq!(raw.request, sent);
    assert!(raw.response.contains("\"created_at\""));
    let trace = serde_json::to_string(&run.trace).unwrap().to_lowercase();
    assert!(!trace.contains(&SENTINEL.to_lowercase()) && !trace.contains("authorization"));

    let saved = recorder.snapshot().unwrap();
    let bytes = saved.to_json(1_000_000).unwrap();
    let text = String::from_utf8(bytes.clone()).unwrap().to_lowercase();
    assert!(!text.contains(&SENTINEL.to_lowercase()) && !text.contains("authorization"));
    let replay = Arc::new(
        ReplayBackend::new(&Recording::from_json(&bytes, 1_000_000).unwrap(), 1_000_000).unwrap(),
    );
    let replayed = engine(
        &ask_graph(None),
        &PrimitiveRegistry::default(),
        bindings(replay),
    )
    .run(&Inputs::new(), limits())
    .await
    .unwrap();
    assert_eq!(
        ask_evidence(&replayed.trace)
            .response
            .as_ref()
            .unwrap()
            .raw
            .as_ref(),
        Some(&raw)
    );

    // A ceiling the recording fits without its raw exchange, but not with it, refuses.
    let mut bare = saved.clone();
    for exchange in &mut bare.exchanges {
        exchange.response.raw = None;
    }
    let without = bare.to_json(1_000_000).unwrap().len();
    let with = bytes.len();
    assert!(with > without);
    assert!(saved.to_json(without).is_err());
    assert!(saved.to_json(with).is_ok());
}

/// Trace: FR-017-AC-4
#[tokio::test]
async fn a_failed_ask_keeps_the_exchange_its_error_carries() {
    struct Failing;
    #[async_trait::async_trait]
    impl ModelBackend for Failing {
        async fn infer(&self, _: &ModelRequest) -> Result<ModelResponse> {
            Err(
                SaphoError::new(ErrorCode::InvalidAnswer, "Unusable answer").with_raw(
                    RawExchange {
                        request: "request body".into(),
                        response: "response body".into(),
                    },
                ),
            )
        }
    }
    let failure = engine(
        &ask_graph(None),
        &PrimitiveRegistry::default(),
        bindings(Arc::new(Failing)),
    )
    .run(&Inputs::new(), limits())
    .await
    .unwrap_err();
    let failed = failure
        .trace
        .nodes
        .iter()
        .find(|n| n.status == NodeStatus::Failed)
        .unwrap();
    assert_eq!(
        failed.model.as_ref().unwrap().raw,
        Some(RawExchange {
            request: "request body".into(),
            response: "response body".into()
        })
    );
}

/// Trace: FR-017-AC-5
#[tokio::test]
async fn trace_comparison_ignores_raw_timing_but_not_the_answer() {
    let _serial = double::serial().await;
    let fake = fake(vec!["yes", "yes", "no"]).await;
    let backend = ollama(&fake, false);
    let mut traces = Vec::new();
    for _ in 0..3 {
        let run = engine(
            &ask_graph(None),
            &PrimitiveRegistry::default(),
            bindings(backend.clone()),
        )
        .run(&Inputs::new(), limits())
        .await
        .unwrap();
        traces.push(run.trace);
    }
    let raw = |t: &Trace| {
        ask_evidence(t)
            .response
            .as_ref()
            .unwrap()
            .raw
            .clone()
            .unwrap()
    };
    assert_ne!(
        raw(&traces[0]).response,
        raw(&traces[1]).response,
        "bodies differ in timing"
    );
    assert_eq!(traces[0], traces[1]);
    assert_ne!(
        traces[0], traces[2],
        "a different answer is a different trace"
    );
}

/// Trace: FR-027-AC-4
#[tokio::test]
async fn identical_requests_whose_raw_bodies_differ_in_timing_are_not_a_conflict() {
    let _serial = double::serial().await;
    let fake = fake(vec!["yes"]).await;
    let recorder = Arc::new(RecordingBackend::new(ollama(&fake, false), 1_000_000).unwrap());
    for _ in 0..2 {
        engine(
            &ask_graph(None),
            &PrimitiveRegistry::default(),
            bindings(recorder.clone()),
        )
        .run(&Inputs::new(), limits())
        .await
        .unwrap();
    }
    let saved = recorder.snapshot().unwrap();
    assert_eq!(saved.exchanges.len(), 2);
    let (a, b) = (&saved.exchanges[0].response, &saved.exchanges[1].response);
    assert_ne!(a.raw, b.raw, "the bodies differ in timing");
    assert_eq!(a.answers, b.answers);
    ReplayBackend::new(&saved, 1_000_000).unwrap();
}
