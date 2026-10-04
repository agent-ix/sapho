// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Typed job and attempt lifecycle, separate from domain acceptance.
use crate::{
    Error, ErrorCode, Result,
    storage::{DEFAULT_ARTIFACT_BYTES, Ledger},
};
use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};
macro_rules! identity {
    ($name:ident, $doc:literal) => {
        #[doc=$doc]
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(try_from = "String")]
        pub struct $name(String);
        impl $name {
            /// Validate an opaque bounded identity; never interpreted as a filesystem path.
            pub fn new(value: impl Into<String>) -> Result<Self> {
                let value = value.into();
                if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
                    return Err(Error::new(ErrorCode::Invalid, "invalid identity"));
                }
                Ok(Self(value))
            }
            /// Borrow the exact identity.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
        impl TryFrom<String> for $name {
            type Error = Error;
            fn try_from(s: String) -> Result<Self> {
                Self::new(s)
            }
        }
    };
}
identity!(JobId, "Domain-independent job identity.");
identity!(StageId, "Adapter-owned stage identity.");
/// Durable attempt identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "i64", into = "i64")]
pub struct AttemptId(i64);
impl AttemptId {
    /// Positive SQLite attempt identifier.
    pub fn new(value: i64) -> Result<Self> {
        if value <= 0 {
            Err(Error::new(ErrorCode::Invalid, "attempt must be positive"))
        } else {
            Ok(Self(value))
        }
    }
    /// Numeric durable identifier.
    pub fn get(self) -> i64 {
        self.0
    }
}
impl TryFrom<i64> for AttemptId {
    type Error = Error;
    fn try_from(n: i64) -> Result<Self> {
        Self::new(n)
    }
}
impl From<AttemptId> for i64 {
    fn from(id: AttemptId) -> Self {
        id.0
    }
}
/// Every dispatch and execution outcome; domain sealing is separately evidenced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptState {
    /// Persisted before possible network dispatch.
    DispatchIntent,
    /// Result captured, domain transaction not yet sealed.
    Executed,
    /// Result validated and domain completion transaction sealed.
    Completed,
    /// Execution or serialization failed.
    Failed,
    /// Explicit resource policy refusal.
    BudgetRejected,
    /// Dispatched work lost its durable result.
    Indeterminate,
    /// Domain validator retained uncertainty rather than acceptance.
    Abstained,
}
impl AttemptState {
    /// Stable database representation.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DispatchIntent => "dispatch_intent",
            Self::Executed => "executed",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::BudgetRejected => "budget_rejected",
            Self::Indeterminate => "indeterminate",
            Self::Abstained => "abstained",
        }
    }
    /// Parse persisted state without routing on diagnostic text.
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "dispatch_intent" => Ok(Self::DispatchIntent),
            "executed" => Ok(Self::Executed),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "budget_rejected" => Ok(Self::BudgetRejected),
            "indeterminate" => Ok(Self::Indeterminate),
            "abstained" => Ok(Self::Abstained),
            _ => Err(Error::new(
                ErrorCode::Storage,
                "unknown persisted attempt state",
            )),
        }
    }
    /// Explicit retry may follow only a retained non-success outcome.
    pub fn retryable(self) -> bool {
        matches!(
            self,
            Self::Failed | Self::BudgetRejected | Self::Indeterminate | Self::Abstained
        )
    }
}
/// Immutable generic job envelope; the compiled adapter interprets the payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Job {
    /// Envelope version.
    pub schema: u32,
    /// Stable identity.
    pub id: JobId,
    /// Compiled domain adapter identifier.
    pub adapter: String,
    /// Adapter payload version.
    pub payload_schema: u32,
    /// Bounded domain content, never dispatched without adapter preparation.
    pub payload: serde_json::Value,
}
/// Snapshot counts are execution observations, not semantic correctness claims.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignSnapshot {
    /// Highest event sequence.
    pub sequence: i64,
    /// Dedicated control revision, unaffected by inference progress.
    pub control_revision: i64,
    /// Operator pause state.
    pub paused: bool,
    /// Number of imported jobs.
    pub jobs: u64,
    /// Retained attempt counts by exhaustive state.
    pub attempts: BTreeMap<AttemptState, u64>,
}
/// Generic stage attempt with immutable request/result references.
#[derive(Debug, Clone)]
pub struct Attempt {
    /// Attempt identity.
    pub id: AttemptId,
    /// Job identity.
    pub job: JobId,
    /// Stage identity.
    pub stage: StageId,
    /// Exact persisted state.
    pub state: AttemptState,
    /// Immutable request hash.
    pub request: String,
    /// Optional captured result hash.
    pub response: Option<String>,
}
/// Campaign services over a single fenced ledger.
pub struct Campaign {
    ledger: Ledger,
}
impl Campaign {
    /// Open generic state. Creation requires a writer and an existing state directory.
    pub fn open(root: &Path, write: bool) -> Result<Self> {
        let ledger = Ledger::open(root, "campaign.sqlite", write, DEFAULT_ARTIFACT_BYTES)?;
        if write {
            ledger.connection().execute_batch("CREATE TABLE IF NOT EXISTS campaign_jobs(id TEXT PRIMARY KEY,adapter TEXT NOT NULL,evidence TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS campaign_attempts(id INTEGER PRIMARY KEY,job TEXT NOT NULL,stage TEXT NOT NULL,parent INTEGER,reason TEXT,state TEXT NOT NULL,request TEXT NOT NULL,response TEXT);
        CREATE TABLE IF NOT EXISTS campaign_control(id INTEGER PRIMARY KEY CHECK(id=1),paused INTEGER NOT NULL,revision INTEGER NOT NULL);
        INSERT OR IGNORE INTO campaign_control VALUES(1,0,0);")?;
        }
        Ok(Self { ledger })
    }
    /// Access immutable artifacts and trusted domain extension services.
    pub fn ledger(&self) -> &Ledger {
        &self.ledger
    }
    /// Access the fenced ledger for trusted compiled domain schema extensions.
    pub fn ledger_mut(&mut self) -> &mut Ledger {
        &mut self.ledger
    }
    /// Admit one adapter-validated envelope idempotently.
    pub fn admit(&mut self, job: &Job) -> Result<bool> {
        if job.schema != 1 || job.payload_schema == 0 || job.adapter.trim().is_empty() {
            return Err(Error::new(ErrorCode::Invalid, "unsupported job envelope"));
        }
        let bytes = sapho_core::bounded_json(job, DEFAULT_ARTIFACT_BYTES)
            .map_err(|e| Error::new(ErrorCode::Invalid, e.to_string()))?;
        let hash = self.ledger.blob(&bytes)?;
        let tx = self.ledger.connection_mut()?.transaction()?;
        let existing: Option<String> = tx
            .query_row(
                "SELECT evidence FROM campaign_jobs WHERE id=?",
                [job.id.as_str()],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(old) = existing {
            return if old == hash {
                Ok(false)
            } else {
                Err(Error::new(
                    ErrorCode::Conflict,
                    "job identity already binds different content",
                ))
            };
        }
        tx.execute(
            "INSERT INTO campaign_jobs VALUES(?,?,?)",
            params![job.id.as_str(), job.adapter, hash],
        )?;
        Ledger::event_tx(&tx, "job_admitted", job.id.as_str(), &hash)?;
        tx.commit()?;
        Ok(true)
    }
    /// Load the exact admitted envelope.
    pub fn job(&self, id: &JobId) -> Result<Job> {
        let hash: String = self.ledger.connection().query_row(
            "SELECT evidence FROM campaign_jobs WHERE id=?",
            [id.as_str()],
            |r| r.get(0),
        )?;
        serde_json::from_slice(&self.ledger.load(&hash)?)
            .map_err(|e| Error::new(ErrorCode::Storage, e.to_string()))
    }
    /// Select jobs in deterministic identity order.
    pub fn jobs(&self) -> Result<Vec<JobId>> {
        let mut q = self
            .ledger
            .connection()
            .prepare("SELECT id FROM campaign_jobs ORDER BY id")?;
        let rows = q
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows.into_iter().map(JobId::new).collect()
    }
    /// Commit dispatch intent. Repeated stage execution needs an explicit retry parent/reason.
    pub fn start(
        &mut self,
        job: &JobId,
        stage: &StageId,
        request: &[u8],
        retry: Option<(AttemptId, &str)>,
    ) -> Result<AttemptId> {
        self.job(job)?;
        let hash = self.ledger.blob(request)?;
        let tx = self.ledger.connection_mut()?.transaction()?;
        let previous:Option<(i64,String)>=tx.query_row("SELECT id,state FROM campaign_attempts WHERE job=? AND stage=? ORDER BY id DESC LIMIT 1",params![job.as_str(),stage.as_str()],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        match (previous, retry) {
            (None, None) => {}
            (Some((id, state)), Some((parent, reason)))
                if id == parent.get()
                    && !reason.trim().is_empty()
                    && AttemptState::parse(&state)?.retryable() => {}
            _ => {
                return Err(Error::new(
                    ErrorCode::Conflict,
                    "stage already attempted or invalid explicit retry",
                ));
            }
        }
        let (parent, reason) = retry
            .map(|(id, reason)| (Some(id.get()), Some(reason)))
            .unwrap_or((None, None));
        tx.execute("INSERT INTO campaign_attempts(job,stage,parent,reason,state,request) VALUES(?,?,?,?,?,?)",params![job.as_str(),stage.as_str(),parent,reason,AttemptState::DispatchIntent.as_str(),hash])?;
        let id = AttemptId::new(tx.last_insert_rowid())?;
        Ledger::event_tx(&tx, "dispatch_intent", &id.get().to_string(), &hash)?;
        tx.commit()?;
        Ok(id)
    }
    /// Capture raw evidence independently of final execution validity.
    pub fn capture(&mut self, id: AttemptId, kind: &str, bytes: &[u8]) -> Result<String> {
        self.attempt(id)?;
        let hash = self.ledger.blob(bytes)?;
        let tx = self.ledger.connection_mut()?.transaction()?;
        Ledger::event_tx(&tx, kind, &id.get().to_string(), &hash)?;
        tx.commit()?;
        Ok(hash)
    }
    /// Retain execution result before domain sealing. Terminal success requires `seal`.
    pub fn finish(&mut self, id: AttemptId, state: AttemptState, bytes: &[u8]) -> Result<()> {
        if !matches!(
            state,
            AttemptState::Executed
                | AttemptState::Failed
                | AttemptState::BudgetRejected
                | AttemptState::Abstained
        ) {
            return Err(Error::new(ErrorCode::Invalid, "invalid execution outcome"));
        }
        let hash = self.ledger.blob(bytes)?;
        let tx = self.ledger.connection_mut()?.transaction()?;
        let changed=tx.execute("UPDATE campaign_attempts SET state=?,response=? WHERE id=? AND state='dispatch_intent'",params![state.as_str(),hash,id.get()])?;
        if changed != 1 {
            return Err(Error::new(
                ErrorCode::Conflict,
                "attempt is no longer dispatching",
            ));
        }
        Ledger::event_tx(&tx, state.as_str(), &id.get().to_string(), &hash)?;
        tx.commit()?;
        Ok(())
    }
    /// Seal trusted domain artifacts and completion atomically after captured execution.
    /// The closure is synchronous and must not perform inference or blocking network I/O.
    pub fn seal<F>(&mut self, id: AttemptId, extension: F) -> Result<()>
    where
        F: FnOnce(&Transaction<'_>, &Attempt) -> Result<()>,
    {
        let attempt = self.attempt(id)?;
        if attempt.state != AttemptState::Executed {
            return Err(Error::new(
                ErrorCode::Conflict,
                "attempt has no unsealed execution",
            ));
        }
        let response = attempt
            .response
            .as_deref()
            .ok_or_else(|| Error::new(ErrorCode::Storage, "executed attempt lacks response"))?;
        self.ledger.load(response)?;
        let tx = self.ledger.connection_mut()?.transaction()?;
        extension(&tx, &attempt)?;
        tx.execute(
            "UPDATE campaign_attempts SET state='completed' WHERE id=? AND state='executed'",
            [id.get()],
        )?;
        Ledger::event_tx(&tx, "completed", &id.get().to_string(), response)?;
        tx.commit()?;
        Ok(())
    }
    /// Reopen dispatch intents as indeterminate, preserving successful unsealed results.
    pub fn recover(&mut self) -> Result<usize> {
        let ids = {
            let mut q=self.ledger.connection().prepare("SELECT id,request FROM campaign_attempts WHERE state='dispatch_intent' ORDER BY id")?;
            q.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        let tx = self.ledger.connection_mut()?.transaction()?;
        for (id, request) in &ids {
            tx.execute(
                "UPDATE campaign_attempts SET state='indeterminate' WHERE id=?",
                [id],
            )?;
            Ledger::event_tx(&tx, "indeterminate", &id.to_string(), request)?;
        }
        tx.commit()?;
        Ok(ids.len())
    }
    /// Inspect one retained attempt.
    pub fn attempt(&self, id: AttemptId) -> Result<Attempt> {
        let (job, stage, state, request, response): (
            String,
            String,
            String,
            String,
            Option<String>,
        ) = self.ledger.connection().query_row(
            "SELECT job,stage,state,request,response FROM campaign_attempts WHERE id=?",
            [id.get()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )?;
        Ok(Attempt {
            id,
            job: JobId::new(job)?,
            stage: StageId::new(stage)?,
            state: AttemptState::parse(&state)?,
            request,
            response,
        })
    }
    /// Inspect retained attempts, including executed results requiring sealing.
    pub fn attempts(&self) -> Result<Vec<Attempt>> {
        let mut q = self
            .ledger
            .connection()
            .prepare("SELECT id FROM campaign_attempts ORDER BY id")?;
        let ids = q
            .query_map([], |r| r.get::<_, i64>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|id| self.attempt(AttemptId::new(id)?))
            .collect()
    }
    /// Read authoritative generic counts.
    pub fn snapshot(&self) -> Result<CampaignSnapshot> {
        let c = self.ledger.connection();
        let (paused, revision): (bool, i64) = c.query_row(
            "SELECT paused,revision FROM campaign_control WHERE id=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let mut q = c.prepare("SELECT state,count(*) FROM campaign_attempts GROUP BY state")?;
        let counts = q
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(CampaignSnapshot {
            sequence: c.query_row("SELECT coalesce(max(seq),0) FROM events", [], |r| r.get(0))?,
            control_revision: revision,
            paused,
            jobs: count(c.query_row("SELECT count(*) FROM campaign_jobs", [], |r| {
                r.get::<_, i64>(0)
            })?)?,
            attempts: counts
                .into_iter()
                .map(|(s, n)| Ok((AttemptState::parse(&s)?, count(n)?)))
                .collect::<Result<_>>()?,
        })
    }
}

fn count(n: i64) -> Result<u64> {
    u64::try_from(n).map_err(|_| Error::new(ErrorCode::Storage, "negative ledger count"))
}
