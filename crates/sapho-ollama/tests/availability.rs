// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Model availability uses the configured server without inferring answering weights.
mod common;

use common::{Fake, Reply, description, generated, limits, serial, settings};
use sapho_core::{ErrorCode, ExtractRequest, extract};
use sapho_ollama::{OllamaBackend, Server};
use serde_json::json;

fn request(model: &str) -> ExtractRequest {
    ExtractRequest {
        model: model.into(),
        instructions: "Label".into(),
        input: "item".into(),
        schema: json!({"type":"object","properties":{"label":{"type":"string"}},"required":["label"]}),
    }
}

/// Trace: FR-052-AC-6
#[tokio::test]
async fn availability_check_uses_configured_headers_and_request_bounds() {
    let _serial = serial().await;
    let fake = Fake::start(|request| match request.path.as_str() {
        "/api/show" => {
            assert_eq!(request.json(), json!({"model": "model:1"}));
            assert!(request.head.contains("x-test-token: local-test"));
            description("abcd")
        }
        "/api/generate" => Reply::ok(generated("model:1", r#"{"label":"ok"}"#)),
        _ => panic!("unexpected route"),
    })
    .await;
    let server = Server::new(&fake.url, limits())
        .unwrap()
        .with_header("x-test-token", "local-test")
        .unwrap();
    let backend = OllamaBackend::new(server, settings("model:1", false, 4096, 512)).unwrap();
    let result = extract(&backend, &request("model:1")).await.unwrap();
    assert_eq!(result.model.name, "model:1");
    assert_eq!(fake.total(), 2);
}

/// Trace: FR-052-AC-3, FR-052-AC-6
#[tokio::test]
async fn availability_check_reports_missing_model_before_inference() {
    let _serial = serial().await;
    let fake = Fake::start(|_| Reply::status(404, json!({"error": "missing"}))).await;
    let backend = OllamaBackend::new(
        Server::new(&fake.url, limits()).unwrap(),
        settings("missing:1", false, 4096, 512),
    )
    .unwrap();
    let error = extract(&backend, &request("missing:1")).await.unwrap_err();
    assert_eq!(error.code, ErrorCode::Config);
    assert_eq!(error.reason, Some("model_not_installed"));
    assert_eq!(fake.total(), 1);
}
