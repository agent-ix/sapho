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
        |request, _attempt| {
            Ok((
                runtime.block_on(adapter.execute(request, Default::default(), _attempt))?,
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
        |_, _| panic!("sealed work cannot redispatch"),
    )
    .unwrap();
    assert_eq!(repeat, StageOutcome::Retained(id));
}

struct ProjectingAdapter {
    reject_dispatch: bool,
    reject_outcome: bool,
}
#[async_trait::async_trait]
impl CampaignAdapter for ProjectingAdapter {
    fn id(&self) -> &str {
        "sapho-graph/v1"
    }
    fn admit(&self, job: &Job) -> sapho_campaign::Result<()> {
        GraphAdapter.admit(job)
    }
    fn stages(
        &self,
        job: &Job,
        ledger: &sapho_campaign::storage::Ledger,
    ) -> sapho_campaign::Result<Vec<StageId>> {
        GraphAdapter.stages(job, ledger)
    }
    fn prepare(
        &self,
        job: &Job,
        stage: &StageId,
        ledger: &sapho_campaign::storage::Ledger,
    ) -> sapho_campaign::Result<sapho_campaign::adapter::PreparedStage> {
        GraphAdapter.prepare(job, stage, ledger)
    }
    fn dispatch(
        &self,
        tx: &rusqlite::Transaction<'_>,
        _job: &Job,
        _request: &sapho_campaign::adapter::PreparedStage,
        _attempt: sapho_campaign::lifecycle::AttemptId,
    ) -> sapho_campaign::Result<()> {
        tx.execute("INSERT INTO projections VALUES('dispatch')", [])?;
        if self.reject_dispatch {
            return Err(sapho_campaign::Error::new(
                sapho_campaign::ErrorCode::Refused,
                "dispatch projection refused",
            ));
        }
        Ok(())
    }
    fn outcome(
        &self,
        tx: &rusqlite::Transaction<'_>,
        _attempt: &sapho_campaign::lifecycle::Attempt,
        _state: AttemptState,
    ) -> sapho_campaign::Result<()> {
        tx.execute("INSERT INTO projections VALUES('outcome')", [])?;
        if self.reject_outcome {
            return Err(sapho_campaign::Error::new(
                sapho_campaign::ErrorCode::Refused,
                "outcome projection refused",
            ));
        }
        Ok(())
    }
    async fn execute(
        &self,
        request: &sapho_campaign::adapter::PreparedStage,
        backends: sapho_core::BackendRegistry,
        attempt: sapho_campaign::lifecycle::AttemptId,
    ) -> sapho_campaign::Result<sapho_campaign::adapter::Execution> {
        GraphAdapter.execute(request, backends, attempt).await
    }
    fn seal(
        &self,
        tx: &rusqlite::Transaction<'_>,
        job: &Job,
        request: &sapho_campaign::adapter::PreparedStage,
        attempt: &sapho_campaign::lifecycle::Attempt,
        evidence: &[u8],
    ) -> sapho_campaign::Result<()> {
        GraphAdapter.seal(tx, job, request, attempt, evidence)
    }
}
#[test]
fn runner_adapter_projection_refusals_leave_no_partial_domain_transitions() {
    // Trace: FR-049-AC-3, FR-050-AC-1, FR-050-AC-3, IT-007-SC-04
    let dir = tempfile::tempdir().unwrap();
    let mut campaign = Campaign::open(dir.path(), true).unwrap();
    campaign
        .ledger_mut()
        .connection_mut()
        .unwrap()
        .execute_batch("CREATE TABLE projections(kind TEXT)")
        .unwrap();
    let payload = GraphJob {
        graph: sapho_graph::GraphSpec::parse_with_format(
            include_str!("../../../examples/reference/facts.json"),
            sapho_graph::GraphFormat::Json,
        )
        .unwrap(),
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
        id: JobId::new("projection-fault").unwrap(),
        adapter: "sapho-graph/v1".into(),
        payload_schema: 1,
        payload: serde_json::to_value(payload).unwrap(),
    };
    campaign.admit(&job).unwrap();
    let stage = StageId::new("evaluate").unwrap();
    let refused = ProjectingAdapter {
        reject_dispatch: true,
        reject_outcome: false,
    };
    assert!(
        run_stage(
            &mut campaign,
            &refused,
            &job.id,
            &stage,
            &Default::default(),
            |_, _| panic!("refused dispatch cannot execute")
        )
        .is_err()
    );
    assert!(campaign.attempts().unwrap().is_empty());
    assert_eq!(
        campaign
            .ledger()
            .connection()
            .query_row("SELECT count(*) FROM projections", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let adapter = ProjectingAdapter {
        reject_dispatch: false,
        reject_outcome: true,
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    assert!(
        run_stage(
            &mut campaign,
            &adapter,
            &job.id,
            &stage,
            &Default::default(),
            |request, _attempt| Ok((
                runtime.block_on(adapter.execute(request, Default::default(), _attempt))?,
                vec![]
            ))
        )
        .is_err()
    );
    let attempts = campaign.attempts().unwrap();
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].state, AttemptState::DispatchIntent);
    assert!(attempts[0].response.is_none());
    assert_eq!(
        campaign
            .ledger()
            .connection()
            .query_row(
                "SELECT count(*) FROM projections WHERE kind='outcome'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}
