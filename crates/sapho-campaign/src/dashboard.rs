// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Bounded machine projections independent of terminal rendering and domain payloads.
use crate::{
    Error, ErrorCode, Result,
    lifecycle::{Attempt, AttemptId, AttemptState, Campaign, CampaignSnapshot},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Maximum serialized projection size, separate from inference artifact bounds.
pub const MAX_BYTES: usize = 256 * 1024;
/// Explicit metric units; no inference of meaning from a rendered label.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    /// Imported job envelopes.
    Jobs,
    /// Retained stage attempts.
    Attempts,
    /// Domain passages.
    Passages,
    /// Distinct source repositories.
    Repositories,
    /// Distinct domain groups.
    Domains,
    /// Completed readings.
    Readings,
    /// Prepared review packets.
    Packets,
    /// Explicit human responses.
    Judgments,
    /// HTTP send attempts, not correctness.
    HttpDispatches,
}
/// Authority of a count, separate from its unit.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Authority {
    /// Durable lifecycle counts.
    Execution,
    /// Downstream domain-assessment projection.
    DomainAssessment,
    /// Explicit supplied human responses.
    HumanResponse,
    /// Ephemeral provider observation.
    ProviderObservation,
}
/// One bounded named metric.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metric {
    /// Observed count.
    pub value: u64,
    /// Optional goal; does not itself establish completion.
    pub target: Option<u64>,
    /// Physical/domain count unit.
    pub unit: Unit,
    /// Stable host-defined semantic category.
    pub meaning: String,
    /// Evidence authority.
    pub authority: Authority,
    /// Actual scoring denominator and assessment provenance; a goal is never this value.
    #[serde(default)]
    pub denominator: Option<Denominator>,
}
/// Explicit quality/ratio assessment scope, separate from campaign targets.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Denominator {
    /// Number of actually assessed applicable cells/records.
    pub value: u64,
    /// Actual evaluated partition or frozen cohort identity.
    pub split: String,
    /// Immutable assessment evidence identity.
    pub assessment_sha256: String,
    /// Host-defined assessment meaning, including agreement vs source correctness.
    pub meaning: String,
}
/// Selected immutable attempt references, excluding private retry text and payloads.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedAttempt {
    /// Real retained identifier.
    pub id: AttemptId,
    /// Compiled stage identity.
    pub stage: String,
    /// Durable lifecycle state.
    pub state: AttemptState,
    /// Explicit retry lineage.
    pub parent: Option<AttemptId>,
    /// Retained immutable request provenance.
    pub request_sha256: String,
    /// Retained outcome evidence, not an implied successful answer.
    pub response_sha256: Option<String>,
}
impl From<Attempt> for SelectedAttempt {
    fn from(attempt: Attempt) -> Self {
        Self {
            id: attempt.id,
            stage: attempt.stage.as_str().into(),
            state: attempt.state,
            parent: attempt.parent,
            request_sha256: attempt.request,
            response_sha256: attempt.response,
        }
    }
}
/// Optional current activity; absence means unknown/idle, never correctness.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Live {
    /// Associated current attempt.
    pub attempt: AttemptId,
    /// Explicit selected model, not a source excerpt.
    pub model: String,
    /// Actual observation time.
    pub observed_unix_ms: u64,
    /// Host's explicit freshness bound.
    pub max_age_ms: u64,
    /// Native HTTP send attempts.
    pub dispatch_attempts: u64,
    /// Captured transport receipts.
    pub receipts: u64,
    /// Host-observed provider activity.
    pub in_flight: bool,
}
/// Versioned projection built by a synchronous read-only host snapshot.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Projection {
    /// Currently supported schema version.
    pub schema: u32,
    /// Generation time, not durable event time.
    pub generated_unix_ms: u64,
    /// Authoritative last durable event time; absent for ledgers without event timestamps.
    #[serde(default)]
    pub last_event_unix_ms: Option<u64>,
    /// Counts from one durable read snapshot.
    pub campaign: CampaignSnapshot,
    /// Bounded host-owned metric records.
    pub metrics: BTreeMap<String, Metric>,
    /// Domain predicate, absent when unspecified.
    pub complete: Option<bool>,
    /// Explicitly ephemeral observation outside the durable snapshot.
    pub live: Option<Live>,
    /// Requested retained attempt only; no automatic selection.
    pub selected_attempt: Option<SelectedAttempt>,
}
impl Projection {
    /// Validate bounds/version/freshness before emission or after decoding.
    pub fn validate(&self) -> Result<()> {
        if self.schema != 1
            || self.metrics.len() > 256
            || self.metrics.iter().any(|(k, m)| {
                k.is_empty()
                    || k.len() > 128
                    || m.meaning.is_empty()
                    || m.meaning.len() > 128
                    || m.denominator.as_ref().is_some_and(|d| {
                        d.value == 0
                            || m.value > d.value
                            || d.split.is_empty()
                            || d.split.len() > 128
                            || d.meaning.is_empty()
                            || d.meaning.len() > 128
                            || d.assessment_sha256.len() != 64
                            || !d.assessment_sha256.bytes().all(|b| b.is_ascii_hexdigit())
                    })
            })
            || self
                .last_event_unix_ms
                .is_some_and(|time| time > self.generated_unix_ms)
            || self
                .selected_attempt
                .as_ref()
                .is_some_and(|a| a.stage.len() > 256)
            || self.live.as_ref().is_some_and(|p| {
                p.model.len() > 256
                    || p.max_age_ms == 0
                    || p.observed_unix_ms > self.generated_unix_ms
                    || self.generated_unix_ms - p.observed_unix_ms > p.max_age_ms
            })
        {
            return Err(Error::new(
                ErrorCode::Refused,
                "dashboard schema, bounds or freshness invalid",
            ));
        }
        Ok(())
    }
    /// Bounded canonical JSON; never truncates a projection.
    pub fn to_json(&self) -> Result<Vec<u8>> {
        self.validate()?;
        sapho_core::bounded_json(self, MAX_BYTES)
            .map_err(|e| Error::new(ErrorCode::Refused, e.to_string()))
    }
}

/// Read generic counts and exact selection in one SQLite snapshot; no blob scan.
pub fn snapshot(
    campaign: &Campaign,
    selected: Option<AttemptId>,
    generated_unix_ms: u64,
) -> Result<Projection> {
    let transaction = campaign.ledger().connection().unchecked_transaction()?;
    let durable = campaign.snapshot()?;
    let selected_attempt = selected
        .map(|id| campaign.attempt(id).map(SelectedAttempt::from))
        .transpose()?;
    let metrics = BTreeMap::from([(
        "jobs".into(),
        Metric {
            value: durable.jobs,
            target: None,
            unit: Unit::Jobs,
            meaning: "imported_jobs".into(),
            authority: Authority::Execution,
            denominator: None,
        },
    )]);
    transaction.commit()?;
    let projection = Projection {
        schema: 1,
        generated_unix_ms,
        last_event_unix_ms: None,
        campaign: durable,
        metrics,
        complete: None,
        live: None,
        selected_attempt,
    };
    projection.validate()?;
    Ok(projection)
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Trace: FR-052-AC-6
    #[test]
    fn read_projection_is_bounded_versioned_and_does_not_mutate() {
        let dir = tempfile::tempdir().unwrap();
        let campaign = Campaign::open(dir.path(), true).unwrap();
        let before = campaign.snapshot().unwrap().sequence;
        let mut view = snapshot(&campaign, None, 100).unwrap();
        assert_eq!(view.schema, 1);
        assert_eq!(view.campaign.sequence, before);
        assert!(view.complete.is_none());
        assert!(view.live.is_none());
        assert!(view.last_event_unix_ms.is_none());
        assert!(view.selected_attempt.is_none());
        assert_eq!(campaign.snapshot().unwrap().sequence, before);
        let bytes = view.to_json().unwrap();
        let decoded: Projection = serde_json::from_slice(&bytes).unwrap();
        decoded.validate().unwrap();
        assert!(snapshot(&campaign, Some(AttemptId::new(999).unwrap()), 100).is_err());
        view.schema = 2;
        assert!(view.to_json().is_err());
        view.schema = 1;
        view.metrics.get_mut("jobs").unwrap().meaning = "x".repeat(129);
        assert!(view.to_json().is_err());
    }
    /// Trace: FR-052-AC-6
    #[test]
    fn durable_freshness_and_quality_denominators_are_explicit_and_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let campaign = Campaign::open(dir.path(), true).unwrap();
        let mut view = snapshot(&campaign, None, 100).unwrap();
        let mut old: serde_json::Value = serde_json::from_slice(&view.to_json().unwrap()).unwrap();
        old.as_object_mut().unwrap().remove("last_event_unix_ms");
        old["metrics"]["jobs"]
            .as_object_mut()
            .unwrap()
            .remove("denominator");
        let legacy: Projection = serde_json::from_value(old).unwrap();
        assert!(legacy.last_event_unix_ms.is_none());
        assert!(legacy.metrics["jobs"].denominator.is_none());
        view.last_event_unix_ms = Some(101);
        assert!(view.to_json().is_err());
        view.last_event_unix_ms = Some(90);
        let metric = view.metrics.get_mut("jobs").unwrap();
        metric.value = 2;
        metric.target = Some(1000);
        metric.denominator = Some(Denominator {
            value: 3,
            split: "frozen-dev".into(),
            assessment_sha256: "a".repeat(64),
            meaning: "source_reviewed_correctness".into(),
        });
        view.to_json().unwrap();
        view.metrics
            .get_mut("jobs")
            .unwrap()
            .denominator
            .as_mut()
            .unwrap()
            .value = 1;
        assert!(view.to_json().is_err());
        view.metrics
            .get_mut("jobs")
            .unwrap()
            .denominator
            .as_mut()
            .unwrap()
            .value = 0;
        assert!(view.to_json().is_err());
    }
    /// Trace: FR-052-AC-6
    #[test]
    fn selection_excludes_private_retry_reason_and_observations_require_freshness() {
        let attempt = Attempt {
            id: AttemptId::new(2).unwrap(),
            job: crate::lifecycle::JobId::new("opaque").unwrap(),
            stage: crate::lifecycle::StageId::new("classify").unwrap(),
            parent: Some(AttemptId::new(1).unwrap()),
            reason: Some("PRIVATE SOURCE".into()),
            state: AttemptState::Failed,
            request: "a".repeat(64),
            response: Some("b".repeat(64)),
        };
        let selected = SelectedAttempt::from(attempt);
        assert_eq!(selected.parent.unwrap().get(), 1);
        assert_eq!(selected.request_sha256, "a".repeat(64));
        assert!(
            !serde_json::to_string(&selected)
                .unwrap()
                .contains("PRIVATE")
        );
        let dir = tempfile::tempdir().unwrap();
        let campaign = Campaign::open(dir.path(), true).unwrap();
        let mut view = snapshot(&campaign, None, 100).unwrap();
        view.live = Some(Live {
            attempt: AttemptId::new(1).unwrap(),
            model: "model".into(),
            observed_unix_ms: 101,
            max_age_ms: 3,
            dispatch_attempts: 1,
            receipts: 0,
            in_flight: true,
        });
        assert!(view.validate().is_err());
        view.live.as_mut().unwrap().observed_unix_ms = 96;
        assert!(view.validate().is_err());
        view.live.as_mut().unwrap().observed_unix_ms = 97;
        view.validate().unwrap();
    }
}
