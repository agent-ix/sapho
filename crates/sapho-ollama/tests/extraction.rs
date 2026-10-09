// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Extraction through the core port against a loopback Ollama (FR-049 to FR-052, IT-007).
mod common;
use common::*;
use sapho_core::{ErrorCode, ExtractError, ExtractRequest, TooLarge, extract};
use sapho_ollama::{Limits, OllamaBackend, Server};
use serde_json::{Value, json};
use std::{collections::BTreeSet, sync::Arc, time::Duration};
use tokio::sync::Notify;

const MODEL: &str = "qwen3:30b";

fn schema() -> Value {
    json!({"type": "object", "properties": {"label": {"type": "string"}}, "required": ["label"]})
}
fn request(input: &str) -> ExtractRequest {
    ExtractRequest {
        model: MODEL.into(),
        instructions: "Label the item.".into(),
        input: input.into(),
        schema: schema(),
    }
}
fn context_refusal(kind: &str) -> Value {
    // Ollama 0.32 nests the typed error as a JSON document inside the `error` string.
    let inner = json!({"error": {"code": 400, "message": "too big", "type": kind, "n_prompt_tokens": 4112, "n_ctx": 1024}});
    json!({"error": inner.to_string()})
}
async fn failing(fake: &Fake, input: &str) -> ExtractError {
    extract(&backend(fake, MODEL), &request(input))
        .await
        .unwrap_err()
}

/// Trace: FR-049-AC-1
#[tokio::test]
async fn generate_request_has_exactly_the_listed_members() {
    let _serial = serial().await;
    let fake = serving(generated(MODEL, r#"{"label":"ok"}"#)).await;
    for think in [true, false] {
        let binding = OllamaBackend::new(
            Server::new(&fake.url, limits()).unwrap(),
            settings(MODEL, think, 4096, 512),
        )
        .unwrap();
        extract(&binding, &request("the item")).await.unwrap();
        let sent = fake.generates().pop().unwrap().json();
        let members: BTreeSet<_> = sent
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            members,
            BTreeSet::from([
                "model", "system", "prompt", "format", "think", "stream", "truncate", "shift",
                "options"
            ])
        );
        assert_eq!(sent["model"], MODEL);
        assert_eq!(sent["system"], "Label the item.");
        assert_eq!(sent["prompt"], "the item");
        assert_eq!(sent["format"], schema());
        assert_eq!(sent["think"], think);
        assert_eq!(sent["stream"], false);
        assert_eq!(sent["truncate"], false);
        assert_eq!(sent["shift"], false);
        assert_eq!(
            sent["options"],
            json!({"temperature": 0, "num_ctx": 4096, "num_predict": 512})
        );
    }
    assert!(
        fake.requests("/api/generate")
            .iter()
            .all(|r| r.path == "/api/generate")
    );
}

/// Trace: FR-049-AC-2
#[tokio::test]
async fn no_conversation_state_reaches_the_second_request() {
    let _serial = serial().await;
    let mut first = generated(MODEL, r#"{"label":"S2-sentinel"}"#);
    first["context"] = json!([91234, 91235]);
    let fake = serving(first).await;
    let binding = backend(&fake, MODEL);
    extract(&binding, &request("S1-sentinel")).await.unwrap();
    extract(&binding, &request("second item")).await.unwrap();
    let second = fake.generates().pop().unwrap();
    let text = String::from_utf8(second.body.clone()).unwrap();
    for leaked in ["S1-sentinel", "S2-sentinel", "91234", "91235"] {
        assert!(!text.contains(leaked), "{leaked}");
    }
    let members = second.json();
    for forbidden in ["context", "messages", "images", "template", "raw", "suffix"] {
        assert!(members.get(forbidden).is_none(), "{forbidden}");
    }
}

/// Trace: FR-049-AC-3
#[tokio::test]
async fn unusable_answers_have_their_own_reasons_and_keep_the_exchange() {
    let _serial = serial().await;
    let mut cut = generated(MODEL, r#"{"label":"par"#);
    cut["done_reason"] = json!("length");
    let mut empty = generated(MODEL, "");
    empty["thinking"] = json!(r#"{"label":"hidden"}"#);
    let garbage = generated(MODEL, "plain words");
    for (answer, reason) in [
        (cut, "output_truncated"),
        (empty, "empty_response"),
        (garbage, "not_json"),
    ] {
        let sent = serde_json::to_string(&answer).unwrap();
        let fake = serving(answer).await;
        let error = failing(&fake, "item").await;
        assert_eq!(error.code, ErrorCode::InvalidAnswer, "{reason}");
        assert_eq!(error.reason, Some(reason));
        let raw = error.raw.unwrap();
        assert_eq!(raw.response, sent);
        assert_eq!(raw.request, fake.generates()[0].text());
    }
}

/// Trace: FR-049-AC-4
#[tokio::test]
async fn invalid_bindings_are_refused_and_nothing_is_sent() {
    let _serial = serial().await;
    let fake = serving(generated(MODEL, "{}")).await;
    let server = Server::new(&fake.url, limits()).unwrap();
    for bad in [
        settings(MODEL, false, 0, 10),
        settings(MODEL, false, 100, 0),
        settings(MODEL, false, 100, 100),
        settings(MODEL, false, 100, 200),
        settings("", false, 100, 10),
    ] {
        let error = OllamaBackend::new(server.clone(), bad).unwrap_err();
        assert_eq!(error.code, ErrorCode::Config);
    }
    assert_eq!(fake.total(), 0);
}

/// Trace: FR-049-AC-5
#[tokio::test]
async fn thinking_text_is_never_the_answer_but_stays_in_the_raw_bytes() {
    let _serial = serial().await;
    let mut answer = generated(MODEL, r#"{"label":"from-response"}"#);
    answer["thinking"] = json!("long reasoning text");
    let fake = serving(answer).await;
    let response = extract(&backend(&fake, MODEL), &request("item"))
        .await
        .unwrap();
    assert_eq!(response.value, json!({"label": "from-response"}));
    assert!(response.raw.response.contains("long reasoning text"));
}

/// Trace: FR-050-AC-1, IT-007-SC-03
#[tokio::test]
async fn server_context_refusal_is_too_large_with_the_servers_count() {
    let _serial = serial().await;
    let fake = Fake::start(|r| match r.path.as_str() {
        "/api/show" => description(BLOB),
        _ => Reply::status(400, context_refusal("exceed_context_size_error")),
    })
    .await;
    let error = failing(&fake, "item").await;
    assert_eq!(error.code, ErrorCode::TooLarge);
    assert_eq!(
        error.too_large,
        Some(TooLarge {
            reported_input_tokens: Some(4112),
            context_tokens: 4096,
            reserved_output_tokens: 512
        })
    );
    assert!(error.raw.is_none(), "a refusal carries no exchange");
    let binary = Fake::start(|r| match r.path.as_str() {
        "/api/show" => description(BLOB),
        _ => Reply::Status(400, vec![0xff, 0xfe]),
    })
    .await;
    let error = failing(&binary, "item").await;
    assert_eq!(
        (error.code, error.reason),
        (ErrorCode::BackendFailed, Some("http_status"))
    );

    let other = Fake::start(|r| match r.path.as_str() {
        "/api/show" => description(BLOB),
        _ => Reply::status(400, context_refusal("invalid_request_error")),
    })
    .await;
    let error = failing(&other, "item").await;
    assert_eq!(error.code, ErrorCode::BackendFailed);
    assert_eq!(error.too_large, None);
}

/// Trace: FR-050-AC-2
#[tokio::test]
async fn nothing_is_refused_for_size_before_the_server_counts_it() {
    let _serial = serial().await;
    let spaces = " ".repeat(20_000);
    let refusing = Fake::start(|r| match r.path.as_str() {
        "/api/show" => description(BLOB),
        _ => Reply::status(400, context_refusal("exceed_context_size_error")),
    })
    .await;
    let small = OllamaBackend::new(
        Server::new(&refusing.url, limits()).unwrap(),
        settings(MODEL, false, 1000, 100),
    )
    .unwrap();
    let error = extract(&small, &request(&spaces)).await.unwrap_err();
    assert_eq!(refusing.generates().len(), 1);
    assert_eq!(refusing.generates()[0].json()["prompt"], spaces);
    assert_eq!(error.too_large.unwrap().reported_input_tokens, Some(4112));

    let accepting = serving(generated(MODEL, r#"{"label":"ok"}"#)).await;
    let small = OllamaBackend::new(
        Server::new(&accepting.url, limits()).unwrap(),
        settings(MODEL, false, 1000, 100),
    )
    .unwrap();
    assert!(extract(&small, &request(&spaces)).await.is_ok());
}

/// Trace: FR-050-AC-3, IT-007-SC-04
#[tokio::test]
async fn reply_leaving_no_room_for_the_answer_is_too_large_not_truncated() {
    let _serial = serial().await;
    let mut crowded = generated(MODEL, r#"{"label":"par"#);
    crowded["prompt_eval_count"] = json!(900);
    crowded["done_reason"] = json!("length");
    let fake = Fake::start(move |r| match r.path.as_str() {
        "/api/show" => description(BLOB),
        _ if r.json()["prompt"] == "crowded" => Reply::ok(crowded.clone()),
        _ => Reply::ok(generated(MODEL, r#"{"label":"ok"}"#)),
    })
    .await;
    let binding = OllamaBackend::new(
        Server::new(&fake.url, limits()).unwrap(),
        settings(MODEL, false, 1000, 200),
    )
    .unwrap();
    let error = extract(&binding, &request("crowded")).await.unwrap_err();
    assert_eq!(error.code, ErrorCode::TooLarge);
    assert_eq!(error.too_large.unwrap().reported_input_tokens, Some(900));
    assert_eq!(error.usage.as_deref().unwrap().input_tokens, Some(900));
    assert!(error.raw.is_some());
    assert!(extract(&binding, &request("next item")).await.is_ok());
}

/// Trace: FR-050-AC-4
#[tokio::test]
async fn missing_counts_are_absent_data_not_errors() {
    let _serial = serial().await;
    let mut no_input = generated(MODEL, r#"{"label":"ok"}"#);
    no_input
        .as_object_mut()
        .unwrap()
        .remove("prompt_eval_count");
    let mut no_output = generated(MODEL, r#"{"label":"ok"}"#);
    no_output.as_object_mut().unwrap().remove("eval_count");
    for (answer, input, output) in [(no_input, None, Some(8)), (no_output, Some(48), None)] {
        let fake = serving(answer).await;
        let binding = backend(&fake, MODEL);
        for _ in 0..2 {
            let response = extract(&binding, &request("item")).await.unwrap();
            assert_eq!(response.usage.input_tokens, input);
            assert_eq!(response.usage.output_tokens, output);
        }
        assert_eq!(fake.generates().len(), 2);
    }
}

/// Trace: FR-050-AC-5
#[tokio::test]
async fn non_ascii_input_reaches_the_server_byte_for_byte() {
    let _serial = serial().await;
    let input = "caf\u{e9} \u{4e2d}\u{6587} \u{1f980}";
    let fake = serving(generated(MODEL, r#"{"label":"ok"}"#)).await;
    extract(&backend(&fake, MODEL), &request(input))
        .await
        .unwrap();
    let sent = fake.generates().pop().unwrap();
    assert_eq!(sent.json()["prompt"], input);
    assert!(
        sent.body
            .windows(input.len())
            .any(|w| w == input.as_bytes())
    );
}

fn tagged(models: [&'static str; 2]) -> impl Fn(&Recorded) -> Reply + Send + Sync {
    move |r| match r.path.as_str() {
        // The description request is held too, so an overlap with another call's request shows.
        "/api/show" => Reply::After(Duration::from_millis(60), Box::new(description(BLOB))),
        _ => {
            let model = models
                .iter()
                .find(|m| **m == r.json()["model"].as_str().unwrap_or(""))
                .unwrap();
            Reply::After(
                Duration::from_millis(60),
                Box::new(Reply::ok(generated(model, r#"{"label":"ok"}"#))),
            )
        }
    }
}

/// Trace: FR-051-AC-1, IT-007-SC-05
#[tokio::test]
async fn concurrent_callers_never_overlap_and_a_cancelled_waiter_frees_the_way() {
    let _serial = serial().await;
    let fake = Fake::start(tagged(["tag-a", "tag-b"])).await;
    let (a, b) = (backend(&fake, "tag-a"), backend(&fake, "tag-b"));
    let (ra, rb) = (
        ExtractRequest {
            model: "tag-a".into(),
            ..request("one")
        },
        ExtractRequest {
            model: "tag-b".into(),
            ..request("two")
        },
    );
    let (first, second) = tokio::join!(extract(&a, &ra), extract(&b, &rb));
    first.unwrap();
    second.unwrap();
    assert_eq!(fake.max_active.load(std::sync::atomic::Ordering::SeqCst), 1);

    let gate = Arc::new(Notify::new());
    let held = gate.clone();
    let held_fake = Fake::start(move |r| match r.path.as_str() {
        "/api/show" => description(BLOB),
        _ if r.json()["prompt"] == "holder" => Reply::Held(
            held.clone(),
            Box::new(Reply::ok(generated(MODEL, r#"{"label":"ok"}"#))),
        ),
        _ => Reply::ok(generated(MODEL, r#"{"label":"ok"}"#)),
    })
    .await;
    let binding = Arc::new(backend(&held_fake, MODEL));
    let holder = {
        let binding = binding.clone();
        tokio::spawn(async move { extract(&*binding, &request("holder")).await })
    };
    while held_fake.generates().is_empty() {
        tokio::task::yield_now().await;
    }
    let waiter = tokio::time::timeout(
        Duration::from_millis(100),
        extract(&*binding, &request("cancelled waiter")),
    )
    .await;
    assert!(
        waiter.is_err(),
        "the permit is held, so the waiter cannot finish"
    );
    gate.notify_one();
    holder.await.unwrap().unwrap();
    extract(&*binding, &request("after")).await.unwrap();
    let prompts: Vec<_> = held_fake
        .generates()
        .iter()
        .map(|r| r.json()["prompt"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(prompts, ["holder", "after"]);
}

/// Trace: FR-051-AC-2
#[test]
fn unsafe_endpoints_and_zero_limits_are_refused() {
    for url in [
        "ftp://127.0.0.1:11434",
        "http://user:pw@127.0.0.1:11434",
        "http://127.0.0.1:11434?x=1",
        "http://127.0.0.1:11434#frag",
        "not a url",
    ] {
        assert_eq!(
            Server::new(url, limits()).unwrap_err().code,
            ErrorCode::Config,
            "{url}"
        );
    }
    for zeroed in [
        Limits {
            timeout: Duration::ZERO,
            ..limits()
        },
        Limits {
            request_bytes: 0,
            ..limits()
        },
        Limits {
            response_bytes: 0,
            ..limits()
        },
    ] {
        assert_eq!(
            Server::new("http://127.0.0.1:11434", zeroed)
                .unwrap_err()
                .code,
            ErrorCode::Config
        );
    }
    assert!(Server::new("https://example.test/base", limits()).is_ok());
}

/// Trace: FR-051-AC-3
#[tokio::test]
async fn stalls_and_oversize_bodies_are_bounded() {
    let _serial = serial().await;
    let stalled = Fake::start(|r| match r.path.as_str() {
        "/api/show" => description(BLOB),
        _ => Reply::Stall,
    })
    .await;
    let quick = OllamaBackend::new(
        Server::new(
            &stalled.url,
            Limits {
                timeout: Duration::from_millis(300),
                ..limits()
            },
        )
        .unwrap(),
        settings(MODEL, false, 4096, 512),
    )
    .unwrap();
    let error = extract(&quick, &request("item")).await.unwrap_err();
    assert_eq!(error.code, ErrorCode::DeadlineExceeded);

    let tiny = OllamaBackend::new(
        Server::new(
            &stalled.url,
            Limits {
                request_bytes: 300,
                ..limits()
            },
        )
        .unwrap(),
        settings(MODEL, false, 4096, 512),
    )
    .unwrap();
    let before = stalled.total();
    let error = extract(&tiny, &request(&"x".repeat(500)))
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::LimitExceeded);
    assert_eq!(stalled.total(), before, "refused before sending");

    let big = serving(generated(
        MODEL,
        &format!("{{\"label\":\"{}\"}}", "y".repeat(5000)),
    ))
    .await;
    let capped = OllamaBackend::new(
        Server::new(
            &big.url,
            Limits {
                response_bytes: 2000,
                ..limits()
            },
        )
        .unwrap(),
        settings(MODEL, false, 4096, 512),
    )
    .unwrap();
    let error = extract(&capped, &request("item")).await.unwrap_err();
    assert_eq!(error.code, ErrorCode::LimitExceeded);
}

/// Trace: FR-051-AC-4
#[tokio::test]
async fn failures_are_one_attempt_and_messages_hold_no_bodies() {
    let _serial = serial().await;
    let sentinel = "SENTINEL-8841";
    for (reply, code, reason) in [
        (
            Reply::status(404, json!({"error": sentinel})),
            ErrorCode::BackendFailed,
            "model_not_found",
        ),
        (
            Reply::status(500, json!({"error": sentinel})),
            ErrorCode::BackendFailed,
            "http_status",
        ),
        (Reply::Redirect, ErrorCode::BackendFailed, "http_status"),
        (
            Reply::Torn(sentinel.as_bytes().to_vec()),
            ErrorCode::BackendFailed,
            "connection_failed",
        ),
    ] {
        let reply = std::sync::Mutex::new(Some(reply));
        let fake = Fake::start(move |r| match r.path.as_str() {
            "/api/show" => description(BLOB),
            _ => reply.lock().unwrap().take().unwrap_or(Reply::Redirect),
        })
        .await;
        let error = failing(&fake, sentinel).await;
        assert_eq!((error.code, error.reason), (code, Some(reason)));
        assert_eq!(fake.generates().len(), 1, "{reason}: exactly one attempt");
        assert!(!error.message.contains(sentinel));
        assert!(!error.to_string().contains(sentinel));
    }
}

/// Trace: FR-049-AC-3
#[tokio::test]
async fn a_body_that_is_not_a_generate_response_is_invalid_and_retained() {
    let _serial = serial().await;
    let fake = Fake::start(|r| match r.path.as_str() {
        "/api/show" => description(BLOB),
        _ => Reply::Status(200, b"{\"model\": ".to_vec()),
    })
    .await;
    let error = failing(&fake, "item").await;
    assert_eq!(error.code, ErrorCode::InvalidAnswer);
    assert_eq!(error.reason, Some("malformed_response"));
    assert_eq!(error.raw.unwrap().response, "{\"model\": ");
}

/// Trace: FR-052-AC-1, FR-052-AC-2, IT-007-SC-01
#[tokio::test]
async fn success_carries_identity_usage_and_the_exact_bytes() {
    let _serial = serial().await;
    let answer = generated(MODEL, r#"{"label":"ok"}"#);
    let sent = serde_json::to_string(&answer).unwrap();
    let fake = serving(answer).await;
    let response = extract(&backend(&fake, MODEL), &request("item"))
        .await
        .unwrap();
    assert_eq!(response.model.name, MODEL);
    assert_eq!(response.model.digest, None);
    assert_eq!(response.usage.input_tokens, Some(48));
    assert_eq!(response.usage.output_tokens, Some(8));
    assert_eq!(response.usage.elapsed_ms, Some(5990));
    assert_eq!(response.usage.load_ms, Some(5786));
    assert_eq!(response.usage.prompt_ms, Some(103));
    assert_eq!(response.usage.generation_ms, Some(95));
    assert_eq!(response.raw.request, fake.generates()[0].text());
    assert_eq!(response.raw.response, sent);
}

/// Trace: FR-052-AC-3
#[tokio::test]
async fn an_unknown_model_is_refused_before_any_generate() {
    let _serial = serial().await;
    let fake = Fake::start(|_| Reply::status(404, json!({"error": "model 'x' not found"}))).await;
    let error = failing(&fake, "item").await;
    assert_eq!(error.code, ErrorCode::Config);
    assert_eq!(error.reason, Some("model_not_installed"));
    assert!(fake.generates().is_empty());
}

/// Trace: FR-052-AC-4
#[tokio::test]
async fn changed_description_does_not_claim_identity_and_a_different_name_is_refused() {
    let _serial = serial().await;
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let fake = Fake::start(move |r| match r.path.as_str() {
        "/api/show" => {
            let n = calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            description(if n == 0 { BLOB } else { "aa11" })
        }
        _ => Reply::ok(generated(MODEL, r#"{"label":"ok"}"#)),
    })
    .await;
    let binding = backend(&fake, MODEL);
    extract(&binding, &request("one")).await.unwrap();
    let second = extract(&binding, &request("two")).await.unwrap();
    assert_eq!(second.model.name, MODEL);
    assert_eq!(second.model.digest, None);
    assert_eq!(fake.generates().len(), 2);

    let renamed = serving(generated("another:tag", r#"{"label":"ok"}"#)).await;
    let error = failing(&renamed, "item").await;
    assert_eq!(
        (error.code, error.reason),
        (ErrorCode::ModelMismatch, Some("name_mismatch"))
    );
    assert!(error.raw.is_some());
}

/// Trace: FR-052-AC-7
#[tokio::test]
async fn external_retag_between_show_and_generate_leaves_only_the_response_name() {
    let _serial = serial().await;
    let serving_other_weights = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let changed = serving_other_weights.clone();
    let fake = Fake::start(move |r| match r.path.as_str() {
        "/api/show" => description(if changed.load(std::sync::atomic::Ordering::SeqCst) {
            "bb22"
        } else {
            BLOB
        }),
        // The scripted server changes the tag to other weights before answering,
        // while the response still reports the original model name.
        "/api/generate" => {
            changed.store(true, std::sync::atomic::Ordering::SeqCst);
            Reply::ok(generated(MODEL, r#"{"label":"ok"}"#))
        }
        _ => panic!("unexpected path"),
    })
    .await;
    let response = extract(&backend(&fake, MODEL), &request("one"))
        .await
        .unwrap();
    assert_eq!(response.model.digest, None);
    assert_eq!(response.model.name, MODEL);
    assert!(serving_other_weights.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(fake.generates().len(), 1);
    assert!(response.raw.request.contains(MODEL));
    // The name-only reply cannot distinguish a stable tag from an external retag.
}

/// Trace: FR-052-AC-5
#[tokio::test]
async fn distinct_tags_keep_distinct_response_names() {
    let _serial = serial().await;
    let fake = Fake::start(tagged(["tag-a", "tag-b"])).await;
    let mut names = Vec::new();
    for tag in ["tag-a", "tag-b"] {
        let req = ExtractRequest {
            model: tag.into(),
            ..request("item")
        };
        names.push(
            extract(&backend(&fake, tag), &req)
                .await
                .unwrap()
                .model
                .name,
        );
    }
    assert_eq!(names, ["tag-a", "tag-b"]);
}

/// Trace: FR-049-AC-6
#[tokio::test]
async fn a_request_for_another_model_than_the_binding_is_refused() {
    let _serial = serial().await;
    let fake = serving(generated(MODEL, "{}")).await;
    let req = ExtractRequest {
        model: "other".into(),
        ..request("item")
    };
    let error = extract(&backend(&fake, MODEL), &req).await.unwrap_err();
    assert_eq!(error.code, ErrorCode::Config);
    assert_eq!(fake.total(), 0);
}

/// Trace: FR-048-AC-2, IT-007-SC-02
#[tokio::test]
async fn schema_violation_from_the_server_keeps_pointer_and_exchange() {
    let _serial = serial().await;
    let fake = serving(generated(MODEL, r#"{"label": 7}"#)).await;
    let error = failing(&fake, "item").await;
    assert_eq!(
        (error.code, error.reason),
        (ErrorCode::InvalidAnswer, Some("schema_violation"))
    );
    assert_eq!(error.pointer.as_deref(), Some("/label"));
    assert!(error.raw.is_some() && error.usage.is_some());
}

/// Trace: FR-050-AC-3
#[tokio::test]
async fn a_prompt_filling_the_context_exactly_fits_and_one_token_more_does_not() {
    let _serial = serial().await;
    // num_ctx 1000 and num_predict 200 leave room for exactly 800 prompt tokens.
    for (counted, fits) in [(800u64, true), (801, false)] {
        let mut reply = generated(MODEL, r#"{"label":"ok"}"#);
        reply["prompt_eval_count"] = json!(counted);
        let fake = serving(reply).await;
        let binding = OllamaBackend::new(
            Server::new(&fake.url, limits()).unwrap(),
            settings(MODEL, false, 1000, 200),
        )
        .unwrap();
        let outcome = extract(&binding, &request("item")).await;
        match (fits, outcome) {
            (true, Ok(_)) => {}
            (false, Err(error)) => assert_eq!(error.code, ErrorCode::TooLarge),
            (fits, other) => panic!("fits={fits}: {other:?}"),
        }
    }
}

/// Trace: FR-051-AC-3
#[tokio::test]
async fn the_response_ceiling_holds_for_a_chunked_body_with_no_content_length() {
    let _serial = serial().await;
    let body = serde_json::to_vec(&generated(
        MODEL,
        &format!("{{\"label\":\"{}\"}}", "y".repeat(5000)),
    ))
    .unwrap();
    let fake = Fake::start(move |r| match r.path.as_str() {
        "/api/show" => description(BLOB),
        _ => Reply::Chunked(body.clone()),
    })
    .await;
    let capped = OllamaBackend::new(
        Server::new(
            &fake.url,
            Limits {
                response_bytes: 2000,
                ..limits()
            },
        )
        .unwrap(),
        settings(MODEL, false, 4096, 512),
    )
    .unwrap();
    let error = extract(&capped, &request("item")).await.unwrap_err();
    assert_eq!(error.code, ErrorCode::LimitExceeded);
    assert_eq!(error.reason, Some("response_too_large"));
}

/// Trace: FR-048-AC-7, FR-052-AC-2
#[tokio::test]
async fn a_body_that_is_not_utf8_is_judged_by_its_status_and_has_no_raw_exchange() {
    let _serial = serial().await;
    for (status, code, reason) in [
        (200, ErrorCode::InvalidAnswer, "malformed_response"),
        (502, ErrorCode::BackendFailed, "http_status"),
        (404, ErrorCode::BackendFailed, "model_not_found"),
    ] {
        let fake = Fake::start(move |r| match r.path.as_str() {
            "/api/show" => description(BLOB),
            _ => Reply::Status(status, vec![b'{', 0xff, 0xfe, b'}']),
        })
        .await;
        let error = failing(&fake, "item").await;
        assert_eq!(
            (error.code, error.reason),
            (code, Some(reason)),
            "status {status}"
        );
        assert!(error.raw.is_none());
        let usage = error.usage.as_deref().unwrap();
        assert!(usage.elapsed_ms.is_some());
        assert_eq!(
            (usage.input_tokens, usage.output_tokens, usage.load_ms),
            (None, None, None)
        );
    }
}

/// Trace: FR-051-AC-4
#[tokio::test]
async fn failure_statuses_with_binary_bodies_keep_their_reasons_and_no_exchange() {
    let _serial = serial().await;
    for (status, reason) in [(404, "model_not_found"), (500, "http_status")] {
        let fake = Fake::start(move |r| match r.path.as_str() {
            "/api/show" => description(BLOB),
            _ => Reply::Status(status, vec![0xff, 0x00, 0xfe]),
        })
        .await;
        let error = failing(&fake, "item").await;
        assert_eq!(
            (error.code, error.reason),
            (ErrorCode::BackendFailed, Some(reason))
        );
        assert!(error.raw.is_none());
    }
    // The description request is judged the same way.
    let fake = Fake::start(|_| Reply::Status(500, vec![0xff, 0xfe])).await;
    let error = failing(&fake, "item").await;
    assert_eq!(
        (error.code, error.reason),
        (ErrorCode::BackendFailed, Some("http_status"))
    );
}

/// Trace: FR-051-AC-6
#[tokio::test]
async fn a_400_is_too_large_only_for_a_readable_size_refusal() {
    let _serial = serial().await;
    let cases: [(Vec<u8>, ErrorCode, Option<&str>); 3] = [
        (
            serde_json::to_vec(&context_refusal("exceed_context_size_error")).unwrap(),
            ErrorCode::TooLarge,
            None,
        ),
        (
            vec![0xff, 0xfe],
            ErrorCode::BackendFailed,
            Some("http_status"),
        ),
        (
            serde_json::to_vec(&context_refusal("invalid_request_error")).unwrap(),
            ErrorCode::BackendFailed,
            Some("http_status"),
        ),
    ];
    for (body, code, reason) in cases {
        let fake = Fake::start(move |r| match r.path.as_str() {
            "/api/show" => description(BLOB),
            _ => Reply::Status(400, body.clone()),
        })
        .await;
        let error = failing(&fake, "item").await;
        assert_eq!((error.code, error.reason), (code, reason));
        assert!(error.raw.is_none());
    }
}

/// Trace: FR-051-AC-5
#[tokio::test]
async fn a_configured_header_reaches_every_request_and_is_never_printed_or_retained() {
    let _serial = serial().await;
    let sentinel = "SENTINEL-HEADER-3318";
    let responses = std::sync::Mutex::new(vec![
        Reply::ok(generated(MODEL, r#"{"label":"ok"}"#)),
        Reply::status(500, json!({"error": "down"})),
    ]);
    let fake = Fake::start(move |r| match r.path.as_str() {
        "/api/show" => description(BLOB),
        _ => responses.lock().unwrap().remove(0),
    })
    .await;
    let server = Server::new(&fake.url, limits())
        .unwrap()
        .with_header("Authorization", &format!("Bearer {sentinel}"))
        .unwrap();
    let debug = format!("{server:?} {server:#?}");
    assert!(!debug.contains(sentinel), "{debug}");
    assert!(debug.contains("Sensitive"));
    let binding = OllamaBackend::new(server, settings(MODEL, false, 4096, 512)).unwrap();
    let ok = extract(&binding, &request("one")).await.unwrap();
    let error = extract(&binding, &request("two")).await.unwrap_err();
    let heads: Vec<_> = fake
        .log
        .lock()
        .unwrap()
        .iter()
        .map(|r| r.head.clone())
        .collect();
    assert_eq!(
        heads.len(),
        4,
        "a description and a generate request per call"
    );
    assert!(
        heads.iter().all(|h| h.contains(sentinel)),
        "every request carries the header"
    );
    let kept = serde_json::to_string(&ok.raw).unwrap();
    assert!(!kept.contains(sentinel));
    for text in [
        error.to_string(),
        format!("{error:?}"),
        serde_json::to_string(&error).unwrap(),
    ] {
        assert!(!text.contains(sentinel), "{text}");
    }
    assert!(!format!("{binding:?}").contains(sentinel));
    assert!(
        Server::new(&fake.url, limits())
            .unwrap()
            .with_header("bad name", "x")
            .is_err(),
        "an invalid header is refused as Config"
    );
}
