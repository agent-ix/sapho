// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Non-domain native graph composition through durable campaign orchestration.
use sapho_campaign::{
    adapter::{CampaignAdapter, GraphAdapter, GraphJob, Limits},
    lifecycle::{AttemptState, Campaign, Job, JobId, StageId},
    runner::{StageOutcome, run_stage},
};
#[test]
fn stock_graph_adapter_runs_without_quire_and_never_repeats_sealed_work() {
    // Trace: FR-051-AC-1, FR-052-AC-4, IT-007-SC-01
    let dir = tempfile::tempdir().unwrap();
    let mut campaign = Campaign::open(dir.path(), true).unwrap();
    let graph = sapho_graph::GraphSpec::parse_with_format(
        include_str!("../../../examples/reference/facts.json"),
        sapho_graph::GraphFormat::Json,
    )
    .unwrap();
    let payload = GraphJob {
        graph,
        inputs: Default::default(),
        limits: Limits {
            node_instances: 100,
            collection_items: 100,
            model_requests: 1,
            concurrency: 1,
            data_bytes: 1_048_576,
            seconds: 10,
        },
    };
    let job = Job {
        schema: 1,
        id: JobId::new("non-domain-facts").unwrap(),
        adapter: "sapho-graph/v1".into(),
        payload_schema: 1,
        payload: serde_json::to_value(payload).unwrap(),
    };
    let adapter = GraphAdapter;
    adapter.admit(&job).unwrap();
    campaign.admit(&job).unwrap();
    let stage = StageId::new("evaluate").unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let outcome = run_stage(
        &mut campaign,
        &adapter,
        &job.id,
        &stage,
        &Default::default(),
        |request| {
            Ok((
                runtime.block_on(adapter.execute(request, Default::default()))?,
                vec![],
            ))
        },
    )
    .unwrap();
    let StageOutcome::Attempt(id) = outcome else {
        panic!("expected new attempt")
    };
    assert_eq!(campaign.attempt(id).unwrap().state, AttemptState::Completed);
    let hash = campaign.attempt(id).unwrap().response.unwrap();
    let result: serde_json::Value =
        serde_json::from_slice(&campaign.ledger().load(&hash).unwrap()).unwrap();
    assert_eq!(result["outputs"]["both"]["value"]["value"], false);
    let repeat = run_stage(
        &mut campaign,
        &adapter,
        &job.id,
        &stage,
        &Default::default(),
        |_| panic!("sealed work cannot redispatch"),
    )
    .unwrap();
    assert_eq!(repeat, StageOutcome::Retained(id));
}
