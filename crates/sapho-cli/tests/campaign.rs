// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Stock campaign command end-to-end behavior, no network or model dependency.
#![cfg(feature = "campaign")]
use std::{path::Path, process::Command};
fn call(root: &Path, args: &[&str]) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_sapho"))
        .arg("campaign")
        .arg("--state")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn headless_graph_campaign_exports_exact_evidence_and_queues_controls() {
    // Trace: FR-052-AC-1, FR-052-AC-4, FR-050-AC-4, IT-007-SC-01
    let dir = tempfile::tempdir().unwrap();
    let state = dir.path().join("state");
    call(&state, &["init"]);
    let graph: serde_json::Value =
        serde_json::from_str(include_str!("../../../examples/reference/facts.json")).unwrap();
    let envelope = serde_json::json!({"schema":1,"id":"cli-facts","adapter":"sapho-graph/v1","payload_schema":1,"payload":{"graph":graph,"inputs":{},"limits":{"node_instances":100,"collection_items":100,"model_requests":1,"concurrency":1,"data_bytes":1_048_576,"seconds":10}}});
    let job = dir.path().join("job.json");
    std::fs::write(&job, serde_json::to_vec(&envelope).unwrap()).unwrap();
    call(&state, &["import", "--job", job.to_str().unwrap()]);
    let run = call(&state, &["run"]);
    assert_eq!(run["attempts"]["completed"], 1);
    let again = call(&state, &["run"]);
    assert_eq!(again["attempts"]["completed"], 1);
    let pause = call(&state, &["pause"]);
    assert_eq!(pause["applied"], false);
    let paused = Command::new(env!("CARGO_BIN_EXE_sapho"))
        .arg("campaign")
        .arg("--state")
        .arg(&state)
        .arg("run")
        .output()
        .unwrap();
    assert_eq!(paused.status.code(), Some(1));
    assert_eq!(call(&state, &["status"])["paused"], true);
    call(&state, &["resume"]);
    call(&state, &["run"]);
    assert_eq!(call(&state, &["doctor"])["evidence_verified"], true);
    let bundle = dir.path().join("bundle");
    assert_eq!(
        call(&state, &["export", "--output", bundle.to_str().unwrap()])["publication"],
        "staged_only"
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(bundle.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["snapshot"]["attempts"]["completed"], 1);
    assert!(manifest["artifacts"].as_object().unwrap().len() >= 3);
}

#[test]
fn migration_command_preserves_stock_state_and_refuses_domain_tables() {
    // Trace: FR-049-AC-4
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source");
    call(&source, &["init"]);
    let target = dir.path().join("target");
    assert_eq!(
        call(&source, &["migrate", "--output", target.to_str().unwrap()])["verified"],
        true
    );
    assert_eq!(call(&source, &["status"]), call(&target, &["status"]));
    let mut campaign = sapho_campaign::lifecycle::Campaign::open(&source, true).unwrap();
    campaign
        .ledger_mut()
        .connection_mut()
        .unwrap()
        .execute_batch("CREATE TABLE domain_extension(value TEXT)")
        .unwrap();
    drop(campaign);
    let rejected = dir.path().join("rejected");
    let result = Command::new(env!("CARGO_BIN_EXE_sapho"))
        .arg("campaign")
        .arg("--state")
        .arg(&source)
        .args(["migrate", "--output"])
        .arg(&rejected)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(!rejected.exists());
}
