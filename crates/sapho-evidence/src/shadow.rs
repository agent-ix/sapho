// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Pure, label-kind separated comparison of decision and observational outcomes.
use crate::{CaseOutcome, Dataset, EvidenceError, LabelKind, Measurement, Metrics, Split, measure};
use sapho_core::{ItemId, SourceId, ValueType};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Explicit score direction for a shadow comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShadowMetric {
    /// Higher Boolean agreement wins.
    Agreement,
    /// Lower probability Brier loss wins.
    Brier,
    /// Shared SAPHO-21 ECE, unavailable until its metric is implemented.
    Ece,
}
/// Caller-selected comparison, without any promotion action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShadowComparisonRequest {
    /// Public decision output.
    pub output: String,
    /// Shadow ask ID.
    pub shadow: String,
    /// Metric and score direction.
    pub metric: ShadowMetric,
    /// Positive scored-case floor for each role.
    pub min_scored: u32,
    /// Finite nonnegative required gain.
    pub margin: f64,
}
/// One role's actual coverage and metric for one label kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShadowRoleScore {
    /// Selected cases carrying the named label.
    pub selected: u32,
    /// Cases scored successfully.
    pub scored: u32,
    /// Cases with unsupported or missing predictions.
    pub unscored: u32,
    /// Cases with execution errors.
    pub failed: u32,
    /// Cases excluded because the role's model made the label.
    pub self_source: Vec<ItemId>,
    /// Exact scored case identities in dataset order.
    pub scored_cases: Vec<ItemId>,
    /// Boolean agreement, if computed.
    pub agreement: Option<f64>,
    /// Probability Brier, if computed.
    pub brier: Option<f64>,
    /// Shared ECE, absent until SAPHO-21 supplies it.
    pub ece: Option<f64>,
    /// Why ECE is absent.
    pub ece_reason: Option<String>,
}
/// Comparison for one provenance kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShadowKindComparison {
    /// Origin of the supplied labels.
    pub label_kind: LabelKind,
    /// Ordinary decision role.
    pub champion: ShadowRoleScore,
    /// Observational role.
    pub challenger: ShadowRoleScore,
    /// Scored case IDs shared by both roles.
    pub matched_cases: Vec<ItemId>,
    /// True, false, or unavailable when comparability fails.
    pub beat_champion: Option<bool>,
    /// Reason for an unavailable verdict.
    pub reason: Option<String>,
}
/// Side-by-side scores over the selected split, never a promotion command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShadowComparison {
    /// Existing dataset identity.
    pub dataset: SourceId,
    /// Explicitly selected partition.
    pub split: Split,
    /// Public output being compared.
    pub output: String,
    /// Shadow ask ID.
    pub shadow: String,
    /// Selected score metric.
    pub metric: ShadowMetric,
    /// Requested scored-case floor.
    pub min_scored: u32,
    /// Requested improvement margin.
    pub margin: f64,
    /// Split-specific status; neither value is a promotion authorization.
    pub promotion_status: ShadowPromotionStatus,
    /// One report per label provenance kind present for this output.
    pub kinds: Vec<ShadowKindComparison>,
}
/// Comparison status, without a deployment or promotion action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShadowPromotionStatus {
    /// Development labels cannot support a promotion verdict.
    NotPromotable,
    /// Held-out evidence remains an evaluation only.
    HeldOutEvaluation,
}

/// Compare two role-specific outcome maps against the same curated labels.
pub fn compare_shadow(
    dataset: &Dataset,
    split: Split,
    schemas: &BTreeMap<String, ValueType>,
    champion: &BTreeMap<ItemId, CaseOutcome>,
    challenger: &BTreeMap<ItemId, CaseOutcome>,
    request: &ShadowComparisonRequest,
    max_cases: usize,
) -> Result<ShadowComparison, EvidenceError> {
    dataset.validate(max_cases)?;
    if request.output.is_empty()
        || request.shadow.is_empty()
        || request.min_scored == 0
        || !request.margin.is_finite()
        || request.margin < 0.0
    {
        return Err(sapho_core::SaphoError::new(
            sapho_core::ErrorCode::InvalidValue,
            "Invalid shadow comparison request",
        )
        .into());
    }
    let schema = schemas.get(&request.output).ok_or_else(|| {
        sapho_core::SaphoError::new(
            sapho_core::ErrorCode::UnknownReference,
            "Shadow comparison output absent",
        )
    })?;
    let selected_schema = BTreeMap::from([(request.output.clone(), schema.clone())]);
    let mut kinds = Vec::new();
    for kind in [
        LabelKind::Human,
        LabelKind::Agent,
        LabelKind::DeterministicCheck,
        LabelKind::Model,
    ] {
        let mut subset = dataset.clone();
        subset.cases.retain_mut(|case| {
            if case.split != split || case.label_provenance.kind != kind {
                return false;
            }
            case.labels.retain(|name, _| name == &request.output);
            !case.labels.is_empty()
        });
        if subset.cases.is_empty() {
            continue;
        }
        let champion_measure = measure(&subset, split, &selected_schema, champion, max_cases)?;
        let challenger_measure = measure(&subset, split, &selected_schema, challenger, max_cases)?;
        let champion_score = role(&champion_measure, &request.output);
        let challenger_score = role(&challenger_measure, &request.output);
        let challenger_ids = challenger_score
            .scored_cases
            .iter()
            .collect::<BTreeSet<_>>();
        let matched_cases = champion_score
            .scored_cases
            .iter()
            .filter(|id| challenger_ids.contains(id))
            .cloned()
            .collect::<Vec<_>>();
        let (beat_champion, reason) =
            verdict(&champion_score, &challenger_score, &matched_cases, request);
        kinds.push(ShadowKindComparison {
            label_kind: kind,
            champion: champion_score,
            challenger: challenger_score,
            matched_cases,
            beat_champion,
            reason,
        });
    }
    Ok(ShadowComparison {
        dataset: dataset.id.clone(),
        split,
        output: request.output.clone(),
        shadow: request.shadow.clone(),
        metric: request.metric,
        min_scored: request.min_scored,
        margin: request.margin,
        promotion_status: if split == Split::Development {
            ShadowPromotionStatus::NotPromotable
        } else {
            ShadowPromotionStatus::HeldOutEvaluation
        },
        kinds,
    })
}
fn role(measurement: &Measurement, output: &str) -> ShadowRoleScore {
    let coverage = measurement.outputs.get(output);
    let (agreement, brier) = match coverage.map(|c| &c.metrics) {
        Some(Metrics::Boolean { agreement, .. }) => (*agreement, None),
        Some(Metrics::Probability { brier }) => (None, *brier),
        _ => (None, None),
    };
    ShadowRoleScore {
        selected: measurement.selected_cases,
        scored: coverage.map_or(0, |c| c.scored),
        unscored: coverage.map_or(0, |c| c.unscored),
        failed: coverage.map_or(0, |c| c.failed),
        self_source: measurement.self_source.clone(),
        scored_cases: measurement
            .predictions
            .iter()
            .filter(|p| {
                p.output == output
                    && p.predicted.is_some()
                    && p.unscored.is_none()
                    && p.error.is_none()
            })
            .map(|p| p.case.clone())
            .collect(),
        agreement,
        brier,
        ece: None,
        ece_reason: Some("shared_sapho_21_ece_unavailable".into()),
    }
}
fn verdict(
    champion: &ShadowRoleScore,
    challenger: &ShadowRoleScore,
    matched: &[ItemId],
    request: &ShadowComparisonRequest,
) -> (Option<bool>, Option<String>) {
    if champion.scored_cases != matched || challenger.scored_cases != matched {
        return (None, Some("unequal_scored_cases".into()));
    }
    if matched.len() < request.min_scored as usize {
        return (None, Some("insufficient_scored_cases".into()));
    }
    let gain = match request.metric {
        ShadowMetric::Agreement => champion
            .agreement
            .zip(challenger.agreement)
            .map(|(a, b)| b - a),
        ShadowMetric::Brier => champion.brier.zip(challenger.brier).map(|(a, b)| a - b),
        ShadowMetric::Ece => None,
    };
    match gain {
        Some(gain) => (Some(gain > 0.0 && gain >= request.margin), None),
        None => (None, Some("metric_not_computed".into())),
    }
}
