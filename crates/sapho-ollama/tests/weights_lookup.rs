// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Public, unpinned weights identity lookup through the configured Server.
mod common;

use common::{Fake, Reply, description, limits, serial};
use sapho_core::ErrorCode;
use sapho_ollama::Server;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Trace: FR-052-AC-6; SAPHO-95.
#[tokio::test]
async fn public_lookup_uses_server_headers_and_reads_current_weights_each_time() {
    let _serial = serial().await;
    let calls = AtomicUsize::new(0);
    let fake = Fake::start(move |request| {
        assert_eq!(request.path, "/api/show");
        assert_eq!(request.json(), json!({"model": "model:1"}));
        assert!(request.head.contains("x-test-token: local-test"));
        let blob = if calls.fetch_add(1, Ordering::SeqCst) == 0 {
            "abcd"
        } else {
            "ef01"
        };
        description(blob)
    })
    .await;
    let server = Server::new(&fake.url, limits())
        .unwrap()
        .with_header("x-test-token", "local-test")
        .unwrap();
    assert_eq!(
        server.weights_digest("model:1").await.unwrap(),
        "sha256:abcd"
    );
    assert_eq!(
        server.weights_digest("model:1").await.unwrap(),
        "sha256:ef01"
    );
    assert_eq!(fake.total(), 2);
}

/// Trace: FR-052-AC-6; SAPHO-95.
#[tokio::test]
async fn public_lookup_reports_missing_model() {
    let _serial = serial().await;
    let fake = Fake::start(|_| Reply::status(404, json!({"error": "missing"}))).await;
    let server = Server::new(&fake.url, limits()).unwrap();
    let error = server.weights_digest("missing:1").await.unwrap_err();
    assert_eq!(error.code, ErrorCode::Config);
    assert_eq!(error.reason, Some("model_not_installed"));
    assert_eq!(fake.total(), 1);
}
