// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Synchronous durable orchestration around an injected async execution host.
use crate::{
    Error, ErrorCode, Result,
    adapter::{CampaignAdapter, Execution, ExecutionOutcome, PreparedStage},
    lifecycle::{AttemptId, AttemptState, Campaign, JobId, StageId},
    storage::DEFAULT_ARTIFACT_BYTES,
};
/// One explicit stage result. Skipping is not fabricated success.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageOutcome {
    /// No pending work; existing attempt retained.
    Retained(AttemptId),
    /// Operator pause prevented dispatch.
    Paused,
    /// New execution or recovered sealing completed.
    Attempt(AttemptId),
}
fn encode(value: &impl serde::Serialize) -> Result<Vec<u8>> {
    sapho_core::bounded_json(value, DEFAULT_ARTIFACT_BYTES)
        .map_err(|e| Error::new(ErrorCode::Refused, e.to_string()))
}
fn prepared(bytes: &[u8]) -> Result<PreparedStage> {
    serde_json::from_slice(bytes).map_err(|e| Error::new(ErrorCode::Storage, e.to_string()))
}
fn seal(campaign: &mut Campaign, adapter: &dyn CampaignAdapter, id: AttemptId) -> Result<()> {
    let attempt = campaign.attempt(id)?;
    let job = campaign.job(&attempt.job)?;
    let request = prepared(&campaign.ledger().load(&attempt.request)?)?;
    if request.schema != 1
        || request.adapter != adapter.id()
        || request.stage != attempt.stage
        || job.adapter != adapter.id()
    {
        return Err(Error::new(
            ErrorCode::Refused,
            "persisted request does not bind adapter stage",
        ));
    }
    let response = attempt
        .response
        .as_deref()
        .ok_or_else(|| Error::new(ErrorCode::Storage, "unsealed execution lacks response"))?;
    let bytes = campaign.ledger().load(response)?;
    campaign.seal(id, |tx, attempt| {
        adapter.seal(tx, &job, &request, attempt, &bytes)
    })
}
/// Execute at most one job/stage. The host closure can block on its owned runtime,
/// then snapshot exact recordings and raw provider receipts into the returned captures.
/// Storage never runs on a Tokio worker or holds a transaction across inference.
pub fn run_stage<F>(
    campaign: &mut Campaign,
    adapter: &dyn CampaignAdapter,
    job: &JobId,
    stage: &StageId,
    provenance: &crate::execution::Provenance,
    execute: F,
) -> Result<StageOutcome>
where
    F: FnOnce(&PreparedStage, AttemptId) -> Result<(Execution, Vec<Capture>)>,
{
    let input = campaign.job(job)?;
    if input.adapter != adapter.id() {
        return Err(Error::new(
            ErrorCode::Invalid,
            "compiled adapter does not own job",
        ));
    }
    let previous = campaign
        .attempts()?
        .into_iter()
        .rev()
        .find(|a| &a.job == job && &a.stage == stage);
    if let Some(attempt) = &previous {
        if attempt.state == AttemptState::Executed {
            seal(campaign, adapter, attempt.id)?;
            return Ok(StageOutcome::Attempt(attempt.id));
        }
        if campaign.retry_authorization(attempt.id)?.is_none() {
            return Ok(StageOutcome::Retained(attempt.id));
        }
    }
    if campaign.snapshot()?.paused {
        return Ok(StageOutcome::Paused);
    }
    let mut request = adapter.prepare(&input, stage, campaign.ledger())?;
    if request.schema != 1 || request.adapter != adapter.id() || &request.stage != stage {
        return Err(Error::new(
            ErrorCode::Invalid,
            "adapter prepared a mismatched request",
        ));
    }
    request.limits.run()?;
    request.provenance = provenance.clone();
    let bytes = encode(&request)?;
    let reason = previous
        .as_ref()
        .map(|p| campaign.retry_authorization(p.id))
        .transpose()?
        .flatten();
    let retry = previous
        .as_ref()
        .zip(reason.as_deref())
        .map(|(a, reason)| (a.id, reason));
    let id = campaign.start_with(job, stage, &bytes, retry, |tx, id| {
        adapter.dispatch(tx, &input, &request, id)
    })?;
    let (mut execution, mut captures) = match execute(&request, id) {
        Ok(result) => result,
        Err(error) => {
            let bytes = encode(
                &serde_json::json!({"schema":1,"code":format!("{:?}",error.code),"message":error.message}),
            )?;
            campaign.finish_with(id, AttemptState::Failed, &bytes, |tx, attempt, state| {
                adapter.outcome(tx, attempt, state)
            })?;
            return Ok(StageOutcome::Attempt(id));
        }
    };
    captures.append(&mut execution.captures);
    for capture in captures {
        campaign.capture(id, &capture.kind, &capture.bytes)?;
    }
    if execution.evidence.len() > DEFAULT_ARTIFACT_BYTES {
        campaign.finish_with(id,AttemptState::Failed,&encode(&serde_json::json!({"schema":1,"code":"result_serialization_limit","result_bytes":execution.evidence.len(),"limit":DEFAULT_ARTIFACT_BYTES}))?, |tx, attempt, state| adapter.outcome(tx, attempt, state))?;
        return Ok(StageOutcome::Attempt(id));
    }
    let outcome = match execution.outcome {
        ExecutionOutcome::Executed => AttemptState::Executed,
        ExecutionOutcome::Failed => AttemptState::Failed,
        ExecutionOutcome::BudgetRejected => AttemptState::BudgetRejected,
        ExecutionOutcome::Abstained => AttemptState::Abstained,
    };
    campaign.finish_with(id, outcome, &execution.evidence, |tx, attempt, state| {
        adapter.outcome(tx, attempt, state)
    })?;
    if outcome == AttemptState::Executed {
        seal(campaign, adapter, id)?;
    }
    Ok(StageOutcome::Attempt(id))
}
/// Independent raw or exact-recording artifact captured by the execution host.
pub struct Capture {
    /// Stable evidence type; not a lifecycle outcome.
    pub kind: String,
    /// Bounded immutable bytes.
    pub bytes: Vec<u8>,
}
