// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Trusted compiled domain ports and the stock native graph adapter.
use crate::{
    Error, ErrorCode, Result,
    lifecycle::{Attempt, Job, StageId},
    storage::Ledger,
};
use async_trait::async_trait;
use sapho_core::{BackendRegistry, Inputs, PrimitiveRegistry};
use sapho_graph::{GraphSpec, compile};
use sapho_runtime::{Engine, RunLimits};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, time::Duration};
/// Serializable explicit engine limits; no domain-specific policy fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    /// Maximum expanded node instances.
    pub node_instances: usize,
    /// Maximum collected elements.
    pub collection_items: usize,
    /// Maximum model requests.
    pub model_requests: usize,
    /// Concurrent graph operations; default hosts dispatch jobs serially.
    pub concurrency: usize,
    /// Maximum engine data bytes.
    pub data_bytes: usize,
    /// Whole execution duration ceiling.
    pub seconds: u64,
}
impl Limits {
    /// Convert only a positive, finite budget.
    pub fn run(&self) -> Result<RunLimits> {
        if [
            self.node_instances,
            self.collection_items,
            self.model_requests,
            self.concurrency,
            self.data_bytes,
        ]
        .contains(&0)
            || self.seconds == 0
        {
            return Err(Error::new(
                ErrorCode::Invalid,
                "all execution limits must be positive",
            ));
        }
        Ok(RunLimits {
            node_instances: self.node_instances,
            collection_items: self.collection_items,
            model_requests: self.model_requests,
            concurrency: self.concurrency,
            data_bytes: self.data_bytes,
            duration: Duration::from_secs(self.seconds),
        })
    }
}
/// Exact prepared request persisted before execution. Adapter metadata is bounded data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedStage {
    /// Credential-free actual host binding metadata committed before dispatch.
    pub provenance: crate::execution::Provenance,
    /// Request schema version.
    pub schema: u32,
    /// Compiled adapter identity.
    pub adapter: String,
    /// Stage identity.
    pub stage: StageId,
    /// Exact adapter input, graph/policy or references needed to reproduce this request.
    pub payload: serde_json::Value,
    /// Explicit native execution limits.
    pub limits: Limits,
}
/// Execution outcome before domain acceptance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionOutcome {
    /// Captured output must be domain-validated and sealed.
    Executed,
    /// Failed execution with retained diagnostic evidence.
    Failed,
    /// Preflight/resource refusal.
    BudgetRejected,
    /// Domain uncertainty retained without acceptance.
    Abstained,
}
/// One bounded execution result, never a correctness claim.
pub struct Execution {
    /// Execution-only outcome.
    pub outcome: ExecutionOutcome,
    /// Typed adapter output serialized under the host artifact bound.
    pub evidence: Vec<u8>,
}
/// Domain-owned dashboard metric; core never derives semantic eligibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metric {
    /// Human-facing label.
    pub label: String,
    /// Observed count.
    pub value: u64,
    /// Optional domain target.
    pub target: Option<u64>,
}
/// Domain status projection, separate from successful graph execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainSnapshot {
    /// Named domain observations.
    pub metrics: BTreeMap<String, Metric>,
    /// Domain completion, absent if no domain predicate exists.
    pub complete: Option<bool>,
}
/// Trusted Rust adapter, not an untrusted plugin or security sandbox.
/// Preparation/storage are synchronous; execute must not borrow a ledger or transaction.
#[async_trait]
pub trait CampaignAdapter: Send + Sync {
    /// Stable compiled adapter identifier.
    fn id(&self) -> &str;
    /// Initialize adapter-owned tables under the existing writer fence.
    fn initialize(&self, _ledger: &mut Ledger) -> Result<()> {
        Ok(())
    }
    /// Validate a bounded job before generic admission.
    fn admit(&self, job: &Job) -> Result<()>;
    /// Choose eligible stages for this job under domain rules.
    fn stages(&self, job: &Job, ledger: &Ledger) -> Result<Vec<StageId>>;
    /// Produce exact isolated stage inputs and immutable execution metadata.
    fn prepare(&self, job: &Job, stage: &StageId, ledger: &Ledger) -> Result<PreparedStage>;
    /// Execute using authoritative domain/native Sapho engines and explicit bindings.
    async fn execute(
        &self,
        request: &PreparedStage,
        backends: BackendRegistry,
    ) -> Result<Execution>;
    /// Validate and atomically seal domain evidence; no network or inference here.
    fn seal(
        &self,
        tx: &rusqlite::Transaction<'_>,
        job: &Job,
        request: &PreparedStage,
        attempt: &Attempt,
        evidence: &[u8],
    ) -> Result<()>;
    /// Project domain counts without moving their meanings into Sapho.
    fn snapshot(&self, _ledger: &Ledger) -> Result<DomainSnapshot> {
        Ok(DomainSnapshot::default())
    }
}
/// Stock graph payload: native Inputs and native configurable graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphJob {
    /// Native graph loaded explicitly from YAML/JSON by the command host.
    pub graph: GraphSpec,
    /// Typed native graph inputs.
    pub inputs: Inputs,
    /// Positive execution limits.
    pub limits: Limits,
}
/// Default graph campaign adapter with no domain semantics.
pub struct GraphAdapter;
fn decode(value: &serde_json::Value) -> Result<GraphJob> {
    serde_json::from_value(value.clone()).map_err(|e| Error::new(ErrorCode::Invalid, e.to_string()))
}
fn report(value: &impl Serialize) -> Result<Vec<u8>> {
    sapho_core::bounded_json(value, crate::storage::DEFAULT_ARTIFACT_BYTES)
        .map_err(|e| Error::new(ErrorCode::Refused, e.to_string()))
}
#[async_trait]
impl CampaignAdapter for GraphAdapter {
    fn id(&self) -> &str {
        "sapho-graph/v1"
    }
    fn admit(&self, job: &Job) -> Result<()> {
        if job.adapter != self.id() || job.payload_schema != 1 {
            return Err(Error::new(
                ErrorCode::Invalid,
                "unsupported graph adapter job",
            ));
        }
        let input = decode(&job.payload)?;
        input.limits.run()?;
        compile(&input.graph, &PrimitiveRegistry::default())
            .map_err(|e| Error::new(ErrorCode::Invalid, e.to_string()))?;
        Ok(())
    }
    fn stages(&self, _job: &Job, _ledger: &Ledger) -> Result<Vec<StageId>> {
        Ok(vec![StageId::new("evaluate")?])
    }
    fn prepare(&self, job: &Job, stage: &StageId, _ledger: &Ledger) -> Result<PreparedStage> {
        self.admit(job)?;
        if stage.as_str() != "evaluate" {
            return Err(Error::new(ErrorCode::Invalid, "unsupported graph stage"));
        }
        let input = decode(&job.payload)?;
        Ok(PreparedStage {
            provenance: Default::default(),
            schema: 1,
            adapter: self.id().into(),
            stage: stage.clone(),
            payload: job.payload.clone(),
            limits: input.limits,
        })
    }
    async fn execute(
        &self,
        request: &PreparedStage,
        backends: BackendRegistry,
    ) -> Result<Execution> {
        let input = decode(&request.payload)?;
        let graph = compile(&input.graph, &PrimitiveRegistry::default())
            .map_err(|e| Error::new(ErrorCode::Invalid, e.to_string()))?;
        let engine = Engine::new(graph, backends)
            .map_err(|e| Error::new(ErrorCode::Invalid, e.to_string()))?;
        match engine.run(&input.inputs, request.limits.run()?).await {
            Ok(run) => Ok(Execution {
                outcome: ExecutionOutcome::Executed,
                evidence: report(
                    &serde_json::json!({"schema":1,"outputs":run.outputs,"trace":run.trace}),
                )?,
            }),
            Err(failure) => Ok(Execution {
                outcome: ExecutionOutcome::Failed,
                evidence: report(
                    &serde_json::json!({"schema":1,"error":failure.error,"trace":failure.trace}),
                )?,
            }),
        }
    }
    fn seal(
        &self,
        _tx: &rusqlite::Transaction<'_>,
        job: &Job,
        request: &PreparedStage,
        _attempt: &Attempt,
        evidence: &[u8],
    ) -> Result<()> {
        self.admit(job)?;
        if request.adapter != self.id()
            || request.stage.as_str() != "evaluate"
            || request.schema != 1
            || request.payload != job.payload
        {
            return Err(Error::new(
                ErrorCode::Refused,
                "graph request does not bind the admitted job",
            ));
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Output {
            schema: u32,
            outputs: Inputs,
            trace: sapho_runtime::Trace,
        }
        let result: Output = serde_json::from_slice(evidence)
            .map_err(|e| Error::new(ErrorCode::Refused, e.to_string()))?;
        if result.schema != 1 {
            return Err(Error::new(ErrorCode::Refused, "unsupported graph result"));
        }
        let _ = (result.outputs, result.trace);
        Ok(())
    }
}
