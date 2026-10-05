// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Exact CLM wire types, preserving declaration order and all provider numeric evidence.
use sapho_core::{
    Answer, ErrorCode, ModelRequest, ModelResponse, Probability, Question, Result, Usage,
    bounded_json, decode_json,
};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    model: String,
    answers: BTreeMap<String, WireAnswer>,
    usage: WireUsage,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireUsage {
    input_tokens: u64,
    output_tokens: u64,
    billing_units: u64,
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum WireAnswer {
    Noul {
        noul: Probability,
    },
    Choice {
        choice: String,
        confidence: Probability,
        probabilities: BTreeMap<String, Probability>,
    },
    Score {
        score: f64,
        confidence: Probability,
        legend: BTreeMap<String, String>,
        probabilities: BTreeMap<String, Probability>,
    },
}
pub(crate) fn encode(request: &ModelRequest, ceiling: usize) -> Result<Vec<u8>> {
    if request.questions.is_empty() {
        return Err(super::failure(ErrorCode::InvalidValue));
    }
    bounded_json(&sapho_systemone::build_request(request)?, ceiling)
}

pub(crate) fn decode(
    request: &ModelRequest,
    bytes: &[u8],
    ceiling: usize,
) -> Result<ModelResponse> {
    let response: Response = decode_json(bytes, ceiling).map_err(|error| {
        super::failure(if error.code == ErrorCode::LimitExceeded {
            ErrorCode::LimitExceeded
        } else {
            ErrorCode::InvalidAnswer
        })
    })?;
    if response.model.is_empty() || response.answers.len() != request.questions.len() {
        return Err(super::failure(ErrorCode::InvalidAnswer));
    }
    let mut raw = response.answers;
    let mut answers = BTreeMap::new();
    for question in &request.questions {
        let answer = raw
            .remove(&question.id)
            .ok_or_else(|| super::failure(ErrorCode::MissingAnswer))?;
        let answer = match (&question.question, answer) {
            (Question::Boolean { .. }, WireAnswer::Noul { noul }) => {
                Answer::Boolean { probability: noul }
            }
            (
                Question::Choice { .. },
                WireAnswer::Choice {
                    choice,
                    confidence,
                    probabilities,
                },
            ) => {
                check_labels(&question.question, &probabilities)?;
                Answer::Choice {
                    selected: choice,
                    confidence,
                    probabilities: Some(probabilities),
                }
            }
            (
                Question::Score { levels, .. },
                WireAnswer::Score {
                    score,
                    confidence,
                    legend,
                    probabilities,
                },
            ) => {
                check_labels(&question.question, &probabilities)?;
                let expected = levels
                    .iter()
                    .enumerate()
                    .map(|(i, level)| (i.to_string(), level.clone()))
                    .collect::<BTreeMap<_, _>>();
                if legend != expected || !score.is_finite() {
                    return Err(super::failure(ErrorCode::InvalidAnswer));
                }
                Answer::Score {
                    expected: score,
                    confidence,
                    probabilities: Some(probabilities),
                }
            }
            _ => return Err(super::failure(ErrorCode::InvalidAnswer)),
        };
        answers.insert(question.id.clone(), answer);
    }
    Ok(ModelResponse {
        model: response.model,
        digest: None,
        answers,
        usage: Some(Usage {
            input_tokens: response.usage.input_tokens,
            output_tokens: response.usage.output_tokens,
            billing_units: Some(response.usage.billing_units),
        }),
    })
}
fn check_labels(question: &Question, probabilities: &BTreeMap<String, Probability>) -> Result<()> {
    let labels = question.labels();
    if probabilities.len() != labels.len()
        || labels
            .iter()
            .any(|label| !probabilities.contains_key(label))
    {
        return Err(super::failure(ErrorCode::InvalidAnswer));
    }
    Ok(())
}
