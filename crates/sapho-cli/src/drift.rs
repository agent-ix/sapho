// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Bounded recording-to-confidence projection; no model execution (FR-068).
use crate::CliError;
use sapho_core::{Answer, ErrorCode, Question, SaphoError};
use sapho_evidence::{ConfidenceWindow, DriftReport, DriftSelector, compare_confidence_windows};
use sapho_recording::Recording;

fn mismatch(detail: &'static str) -> CliError {
    SaphoError::new(ErrorCode::RecordingMismatch, detail).into()
}

/// Validate bounded recording bytes and extract one exact question's confidences.
pub fn project_recording_confidences(
    bytes: &[u8],
    max_bytes: usize,
    name: &str,
    selector: &DriftSelector,
    question: &Question,
) -> Result<ConfidenceWindow, CliError> {
    if selector.question_id.trim().is_empty() || name.trim().is_empty() {
        return Err(mismatch("Missing drift selector or window name"));
    }
    selector.binding.validate()?;
    let recording = Recording::from_json(bytes, max_bytes)?;
    let retained_exchanges = u32::try_from(recording.exchanges.len())
        .map_err(|_| mismatch("Recording exchange count exceeds supported range"))?;
    let mut skipped_nonmatching = 0u32;
    let mut skipped_missing_question = 0u32;
    let mut confidences = Vec::new();
    for exchange in recording.exchanges {
        if exchange.request.backend != selector.binding
            || exchange.response.model != selector.actual_model
        {
            skipped_nonmatching += 1;
            continue;
        }
        let Some(named) = exchange
            .request
            .questions
            .iter()
            .find(|named| named.id == selector.question_id)
        else {
            skipped_missing_question += 1;
            continue;
        };
        if &named.question != question {
            return Err(mismatch("Selected recorded Question definition differs"));
        }
        let Some(answer) = exchange.response.answers.get(&selector.question_id) else {
            skipped_missing_question += 1;
            continue;
        };
        let confidence = match (question, answer) {
            (Question::Boolean { .. }, Answer::Boolean { probability }) => {
                let p = probability.get();
                p.max(1.0 - p)
            }
            (Question::Choice { .. }, Answer::Choice { confidence, .. })
            | (Question::Score { .. }, Answer::Score { confidence, .. }) => confidence.get(),
            _ => return Err(mismatch("Selected recorded answer kind differs")),
        };
        confidences.push(confidence);
    }
    Ok(ConfidenceWindow {
        name: name.into(),
        retained_exchanges,
        skipped_nonmatching,
        skipped_missing_question,
        confidences,
    })
}

/// Compare two bounded validated recording exports using pure evidence statistics.
pub fn compare_recordings(
    reference_bytes: &[u8],
    current_bytes: &[u8],
    max_bytes: usize,
    reference_name: &str,
    current_name: &str,
    selector: DriftSelector,
    question: &Question,
    threshold: f64,
) -> Result<DriftReport, CliError> {
    let reference = project_recording_confidences(
        reference_bytes,
        max_bytes,
        reference_name,
        &selector,
        question,
    )?;
    let current =
        project_recording_confidences(current_bytes, max_bytes, current_name, &selector, question)?;
    Ok(compare_confidence_windows(
        selector, threshold, reference, current,
    )?)
}
