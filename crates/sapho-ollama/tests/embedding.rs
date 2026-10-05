// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Embeddings through the shared client (FR-053).
mod common;
use common::*;
use sapho_core::{ErrorCode, ExtractRequest, extract};
use sapho_ollama::{OllamaEmbedder, Server};
use serde_json::{Value, json};
use std::{collections::BTreeSet, sync::atomic::Ordering, time::Duration};

const MODEL: &str = "qwen3-embedding:4b";

fn embedder(fake: &Fake, max: usize) -> OllamaEmbedder {
    OllamaEmbedder::new(Server::new(&fake.url, limits()).unwrap(), MODEL, max).unwrap()
}
fn texts(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("text {i}")).collect()
}
fn embeddings(vectors: Value) -> Value {
    json!({"model": MODEL, "embeddings": vectors, "total_duration": 5, "prompt_eval_count": 11})
}
async fn answering(reply: Value) -> Fake {
    Fake::start(move |r| match r.path.as_str() {
        "/api/show" => description(BLOB),
        _ => Reply::ok(reply.clone()),
    })
    .await
}

/// Trace: FR-053-AC-1
#[tokio::test]
async fn embed_sends_ordered_inputs_without_truncation_and_returns_vectors_in_order() {
    let _serial = serial().await;
    let fake = answering(embeddings(json!([[1.0, 0.0], [0.0, 1.0], [0.5, 0.5]]))).await;
    let inputs = texts(3);
    let result = embedder(&fake, 32).embed(&inputs).await.unwrap();
    let sent = fake.requests("/api/embed").pop().unwrap();
    let members: BTreeSet<_> = sent.json().as_object().unwrap().keys().cloned().collect();
    assert_eq!(
        members,
        BTreeSet::from(["model".into(), "input".into(), "truncate".into()])
    );
    assert_eq!(sent.json()["input"], json!(inputs));
    assert_eq!(sent.json()["truncate"], false);
    assert_eq!(sent.json()["model"], MODEL);
    assert_eq!(result.vectors, [[1.0, 0.0], [0.0, 1.0], [0.5, 0.5]]);
    assert_eq!(
        result.model.digest.as_deref(),
        Some(format!("sha256:{BLOB}").as_str())
    );
    assert_eq!(result.input_tokens, Some(11));
    assert_eq!(result.raw.request, sent.body);
}

/// Trace: FR-053-AC-2
#[tokio::test]
async fn refusals_and_malformed_vectors_have_their_own_outcomes() {
    let _serial = serial().await;
    let refusal = |text: &'static str| async move {
        let fake = Fake::start(move |r| match r.path.as_str() {
            "/api/show" => description(BLOB),
            _ => Reply::status(400, json!({"error": text})),
        })
        .await;
        embedder(&fake, 32).embed(&texts(1)).await.unwrap_err()
    };
    let long = refusal("the input length exceeds the context length").await;
    assert_eq!(
        (long.code, long.reason),
        (ErrorCode::TooLarge, Some("input_exceeds_context"))
    );
    assert!(long.raw.is_some());
    let other = refusal("something else").await;
    assert_eq!(other.code, ErrorCode::BackendFailed);

    for vectors in [json!([[1.0, 0.0]]), json!([[1.0, 0.0], [1.0]])] {
        let fake = answering(embeddings(vectors)).await;
        let error = embedder(&fake, 32).embed(&texts(2)).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidAnswer);
        assert!(error.raw.is_some());
    }
    let fake = Fake::start(|r| match r.path.as_str() {
        "/api/show" => description(BLOB),
        _ => Reply::Status(
            200,
            br#"{"model":"qwen3-embedding:4b","embeddings":[[1e999]]}"#.to_vec(),
        ),
    })
    .await;
    let error = embedder(&fake, 32).embed(&texts(1)).await.unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidAnswer);
    assert!(error.raw.is_some());
}

/// Trace: FR-053-AC-3
#[tokio::test]
async fn an_empty_list_and_an_over_long_list_are_refused_before_sending() {
    let _serial = serial().await;
    let fake = answering(embeddings(json!([[1.0]]))).await;
    let bound = embedder(&fake, 2);
    for inputs in [texts(0), texts(3)] {
        assert_eq!(
            bound.embed(&inputs).await.unwrap_err().code,
            ErrorCode::Config
        );
    }
    assert_eq!(fake.total(), 0);
    assert_eq!(
        OllamaEmbedder::new(Server::new(&fake.url, limits()).unwrap(), "", 2)
            .unwrap_err()
            .code,
        ErrorCode::Config
    );
}

/// Trace: FR-053-AC-4
#[tokio::test]
async fn embedding_and_extraction_share_the_one_request_permit() {
    let _serial = serial().await;
    let fake = Fake::start(|r| match r.path.as_str() {
        // The description request is held too, so an overlap with another call's request shows.
        "/api/show" => Reply::After(Duration::from_millis(60), Box::new(description(BLOB))),
        "/api/embed" => Reply::After(
            Duration::from_millis(60),
            Box::new(Reply::ok(embeddings(json!([[1.0]])))),
        ),
        _ => Reply::After(
            Duration::from_millis(60),
            Box::new(Reply::ok(generated("qwen3:30b", r#"{"label":"ok"}"#))),
        ),
    })
    .await;
    let extractor = backend(&fake, "qwen3:30b");
    let request = ExtractRequest {
        model: "qwen3:30b".into(),
        instructions: "Label.".into(),
        input: "item".into(),
        schema: json!({"type": "object"}),
    };
    let embed = embedder(&fake, 4);
    let inputs = texts(1);
    let (extracted, embedded) = tokio::join!(extract(&extractor, &request), embed.embed(&inputs));
    extracted.unwrap();
    embedded.unwrap();
    assert_eq!(fake.max_active.load(Ordering::SeqCst), 1);
}
