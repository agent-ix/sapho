// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Pure label-free confidence distribution comparison (FR-068).
use crate::EvidenceError;
use sapho_core::BackendId;
use serde::{Deserialize, Serialize};

/// Maximum UTF-8 byte length of each identity copied into a drift report.
pub const MAX_DRIFT_IDENTITY_BYTES: usize = 256;

/// Validate a caller or provider identity before it can enter a drift report.
pub fn validate_drift_identity(value: &str) -> Result<(), EvidenceError> {
    if value.trim().is_empty() || value.len() > MAX_DRIFT_IDENTITY_BYTES {
        return Err(EvidenceError::InvalidDriftInput);
    }
    Ok(())
}

/// Nonsecret identity of the selected recorded answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriftSelector {
    /// Exact backend binding.
    pub binding: BackendId,
    /// Exact question ID.
    pub question_id: String,
    /// Actual reported model identity.
    pub actual_model: String,
}
impl DriftSelector {
    /// Validate exact selector identities within the report byte ceiling.
    pub fn validate(&self) -> Result<(), EvidenceError> {
        validate_drift_identity(self.binding.as_str())?;
        validate_drift_identity(&self.question_id)?;
        validate_drift_identity(&self.actual_model)
    }
}
/// Bounded host projection of one validated recording set, without labels or raw bodies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfidenceWindow {
    /// Caller-supplied window name.
    pub name: String,
    /// Every retained successful exchange, including repeats.
    pub retained_exchanges: u32,
    /// Exchanges with another binding or actual model.
    pub skipped_nonmatching: u32,
    /// Matching exchanges without the selected question.
    pub skipped_missing_question: u32,
    /// One selected confidence per matching successful exchange.
    pub confidences: Vec<f64>,
}
/// Count and confidence statistics for one supplied recording set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriftWindow {
    /// Caller-supplied window name.
    pub name: String,
    /// Retained successful exchanges before selection.
    pub retained_exchanges: u32,
    /// Selected matching-question observations.
    pub matching_questions: u32,
    /// Skipped binding/model exchanges.
    pub skipped_nonmatching: u32,
    /// Skipped exchanges without the selected question.
    pub skipped_missing_question: u32,
    /// Mean predicted-label or reported confidence, absent when empty.
    pub mean_confidence: Option<f64>,
    /// Fraction at or above the inclusive threshold, absent when empty.
    pub high_confidence_share: Option<f64>,
}
/// Signed and absolute label-free confidence movement between two named sets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriftReport {
    /// Schema version.
    pub version: u32,
    /// Selected binding, question and actual model.
    pub selector: DriftSelector,
    /// Inclusive confidence threshold.
    pub threshold: f64,
    /// Reference statistics.
    pub reference: DriftWindow,
    /// Current statistics.
    pub current: DriftWindow,
    /// Current minus reference mean confidence.
    pub mean_delta: Option<f64>,
    /// Absolute mean-confidence distance.
    pub mean_distance: Option<f64>,
    /// Current minus reference high-confidence share.
    pub share_delta: Option<f64>,
    /// Absolute share distance.
    pub share_distance: Option<f64>,
    /// `insufficient_observations` when either selected set is empty.
    pub absent_reason: Option<String>,
}

fn summarize(mut input: ConfidenceWindow, threshold: f64) -> Result<DriftWindow, EvidenceError> {
    validate_drift_identity(&input.name)?;
    if input
        .confidences
        .iter()
        .any(|c| !c.is_finite() || !(0.0..=1.0).contains(c))
    {
        return Err(EvidenceError::InvalidDriftInput);
    }
    let matched =
        u32::try_from(input.confidences.len()).map_err(|_| EvidenceError::InvalidDriftInput)?;
    if input
        .skipped_nonmatching
        .checked_add(input.skipped_missing_question)
        .and_then(|n| n.checked_add(matched))
        != Some(input.retained_exchanges)
    {
        return Err(EvidenceError::InvalidDriftInput);
    }
    input.confidences.sort_by(f64::total_cmp);
    let mean_confidence =
        (matched > 0).then(|| input.confidences.iter().sum::<f64>() / f64::from(matched));
    let high_confidence_share = (matched > 0).then(|| {
        let count = input
            .confidences
            .iter()
            .filter(|&&c| c >= threshold)
            .count();
        f64::from(u32::try_from(count).unwrap_or(u32::MAX)) / f64::from(matched)
    });
    Ok(DriftWindow {
        name: input.name,
        retained_exchanges: input.retained_exchanges,
        matching_questions: matched,
        skipped_nonmatching: input.skipped_nonmatching,
        skipped_missing_question: input.skipped_missing_question,
        mean_confidence,
        high_confidence_share,
    })
}

/// Compare validated host-projected confidence sets without any label or clock input.
pub fn compare_confidence_windows(
    selector: DriftSelector,
    threshold: f64,
    reference: ConfidenceWindow,
    current: ConfidenceWindow,
) -> Result<DriftReport, EvidenceError> {
    selector.validate()?;
    if reference.name == current.name || !threshold.is_finite() || !(0.0..=1.0).contains(&threshold)
    {
        return Err(EvidenceError::InvalidDriftInput);
    }
    let reference = summarize(reference, threshold)?;
    let current = summarize(current, threshold)?;
    let mean_delta = current
        .mean_confidence
        .zip(reference.mean_confidence)
        .map(|(a, b)| a - b);
    let share_delta = current
        .high_confidence_share
        .zip(reference.high_confidence_share)
        .map(|(a, b)| a - b);
    Ok(DriftReport {
        version: 1,
        selector,
        threshold,
        reference,
        current,
        mean_delta,
        mean_distance: mean_delta.map(f64::abs),
        share_delta,
        share_distance: share_delta.map(f64::abs),
        absent_reason: mean_delta
            .is_none()
            .then(|| "insufficient_observations".into()),
    })
}
