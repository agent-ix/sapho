// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Exact Decisions request and answer codec; provider values are never normalized.
use base64::Engine;
use sapho_core::{
    Answer, ErrorCode, ModelRequest, ModelResponse, Probability, Question, Result, SaphoError,
    Usage, Value, bounded_json, decode_json,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

fn invalid(message: &'static str) -> SaphoError {
    SaphoError::new(ErrorCode::InvalidValue, message)
}
fn answer_error(reason: &'static str) -> SaphoError {
    SaphoError::new(ErrorCode::InvalidAnswer, "Invalid Decisions answer")
        .with_context("reason", reason)
}
fn envelope_error() -> SaphoError {
    invalid("Invalid Decisions input envelope")
        .with_context("reason", "invalid_decisions_input_envelope")
}
#[derive(Serialize)]
struct Request {
    model: &'static str,
    input: Input,
    questions: Vec<WireQuestion>,
}
#[derive(Serialize)]
#[serde(untagged)]
enum Input {
    Text(String),
    Messages(Vec<Message>),
}
#[derive(Serialize)]
struct Message {
    role: &'static str,
    content: Vec<Part>,
}
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Part {
    InputText { text: String },
    InputImage { image_url: String },
}
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WireQuestion {
    Predicate {
        name: String,
        instructions: String,
    },
    Choice {
        name: String,
        instructions: String,
        choices: Vec<WireChoice>,
    },
    Score {
        name: String,
        instructions: String,
        levels: Vec<WireLevel>,
    },
}
#[derive(Serialize)]
struct WireChoice {
    value: String,
    description: String,
}
#[derive(Serialize)]
struct WireLevel {
    label: String,
    description: String,
}
fn input(request: &ModelRequest, ceiling: usize) -> Result<Input> {
    let Value::Record(state) = &request.state else {
        return Err(invalid("Decisions state must be a record"));
    };
    let Some(envelope) = state.get("decisions_input") else {
        return serde_json::to_string(&request.state.to_plain_json()?)
            .map(Input::Text)
            .map_err(|_| invalid("Cannot serialize Decisions state"));
    };
    let Value::Record(fields) = envelope else {
        return Err(envelope_error());
    };
    if state.len() != 1 || fields.len() != 2 {
        return Err(envelope_error());
    }
    let Some(Value::Text(text)) = fields.get("text") else {
        return Err(envelope_error());
    };
    let Some(Value::List(images)) = fields.get("images") else {
        return Err(envelope_error());
    };
    if images.len() > 8 {
        return Err(envelope_error());
    }
    if images.is_empty() {
        return Ok(Input::Text(text.clone()));
    }
    let mut content = vec![Part::InputText { text: text.clone() }];
    for image in images {
        let Value::Text(url) = &image.value else {
            return Err(envelope_error());
        };
        if url.len() > ceiling {
            return Err(envelope_error());
        }
        let Some((prefix, payload)) = url.split_once(",") else {
            return Err(envelope_error());
        };
        if !matches!(
            prefix,
            "data:image/png;base64"
                | "data:image/jpeg;base64"
                | "data:image/webp;base64"
                | "data:image/gif;base64"
        ) || payload.is_empty()
            || base64::engine::general_purpose::STANDARD
                .decode(payload)
                .is_err()
        {
            return Err(envelope_error());
        }
        content.push(Part::InputImage {
            image_url: url.clone(),
        });
    }
    Ok(Input::Messages(vec![Message {
        role: "user",
        content,
    }]))
}
/// Serialize one checked, bounded Decisions request before transport.
pub(crate) fn encode(request: &ModelRequest, ceiling: usize) -> Result<Vec<u8>> {
    request.validate()?;
    if request.model != super::MODEL {
        return Err(invalid("Unsupported Decisions model"));
    }
    if !(1..=32).contains(&request.questions.len()) {
        return Err(invalid("Decisions question count must be 1 through 32"));
    }
    let input = input(request, ceiling)?;
    let mut questions = Vec::with_capacity(request.questions.len());
    for named in &request.questions {
        let name = named.id.clone();
        let wire = match &named.question {
            Question::Boolean {
                instructions,
                yes,
                no,
            } => {
                if yes.trim().is_empty() || no.trim().is_empty() {
                    return Err(invalid("Boolean criteria must be nonempty"));
                }
                WireQuestion::Predicate {
                    name,
                    instructions: format!(
                        "{instructions}\nTrue criterion: {yes}\nFalse criterion: {no}"
                    ),
                }
            }
            Question::Choice {
                instructions,
                options,
            } => {
                if options.len() > 32
                    || options
                        .iter()
                        .any(|item| item.description.trim().is_empty())
                {
                    return Err(invalid("Invalid Decisions choice options"));
                }
                WireQuestion::Choice {
                    name,
                    instructions: instructions.clone(),
                    choices: options
                        .iter()
                        .map(|item| WireChoice {
                            value: item.label.clone(),
                            description: item.description.clone(),
                        })
                        .collect(),
                }
            }
            Question::Score {
                instructions,
                levels,
            } => {
                if levels.len() > 32 || levels.iter().any(|item| item.trim().is_empty()) {
                    return Err(invalid("Invalid Decisions score levels"));
                }
                WireQuestion::Score {
                    name,
                    instructions: instructions.clone(),
                    levels: levels
                        .iter()
                        .enumerate()
                        .map(|(index, description)| WireLevel {
                            label: index.to_string(),
                            description: description.clone(),
                        })
                        .collect(),
                }
            }
        };
        questions.push(wire);
    }
    bounded_json(
        &Request {
            model: super::MODEL,
            input,
            questions,
        },
        ceiling,
    )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    model: String,
    answers: Vec<WireAnswer>,
    usage: WireUsage,
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum WireAnswer {
    Predicate {
        name: Option<String>,
        probability: Probability,
    },
    Choice {
        name: Option<String>,
        choice: String,
        confidence: Probability,
        probabilities: Vec<ChoiceProbability>,
    },
    Score {
        name: Option<String>,
        score: f64,
        confidence: Probability,
        probabilities: Vec<ScoreProbability>,
    },
    Refusal {
        name: Option<String>,
    },
}
impl WireAnswer {
    fn name(&self) -> Option<&str> {
        match self {
            Self::Predicate { name, .. }
            | Self::Choice { name, .. }
            | Self::Score { name, .. }
            | Self::Refusal { name } => name.as_deref(),
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChoiceProbability {
    value: String,
    probability: Probability,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScoreProbability {
    value: i64,
    label: String,
    probability: Probability,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireUsage {
    input_tokens: u64,
    output_tokens: u64,
    total_tokens: u64,
    input_tokens_details: Option<InputTokenDetails>,
    output_tokens_details: Option<OutputTokenDetails>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InputTokenDetails {
    cached_tokens: u64,
    cache_write_tokens: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OutputTokenDetails {
    reasoning_tokens: u64,
}
fn distribution(
    labels: &[String],
    pairs: impl IntoIterator<Item = (String, Probability)>,
) -> Result<BTreeMap<String, Probability>> {
    let mut probabilities = BTreeMap::new();
    for (label, probability) in pairs {
        if !labels.contains(&label) || probabilities.insert(label, probability).is_some() {
            return Err(answer_error("invalid_distribution_entry"));
        }
    }
    if probabilities.len() != labels.len() {
        return Err(answer_error("incomplete_distribution"));
    }
    Ok(probabilities)
}
/// Decode exact typed answers, leaving distribution mass interpretation to core validation.
pub(crate) fn decode(
    request: &ModelRequest,
    bytes: &[u8],
    ceiling: usize,
) -> Result<ModelResponse> {
    let response: Response = decode_json(bytes, ceiling).map_err(|error| {
        if error.code == ErrorCode::LimitExceeded {
            error
        } else {
            answer_error("invalid_success_schema")
        }
    })?;
    if response.model.is_empty() {
        return Err(answer_error("missing_model"));
    }
    let _ = response.usage.total_tokens;
    let _ = response
        .usage
        .input_tokens_details
        .as_ref()
        .map(|details| (details.cached_tokens, details.cache_write_tokens));
    let _ = response
        .usage
        .output_tokens_details
        .as_ref()
        .map(|details| details.reasoning_tokens);
    if response.answers.len() < request.questions.len() {
        return Err(SaphoError::new(
            ErrorCode::MissingAnswer,
            "Decisions answer absent",
        ));
    }
    if response.answers.len() > request.questions.len() {
        return Err(answer_error("extra_answer"));
    }
    let mut answers = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for (question, wire) in request.questions.iter().zip(response.answers) {
        let name = wire.name().ok_or_else(|| answer_error("unnamed_answer"))?;
        if !seen.insert(name.to_owned()) {
            return Err(answer_error("duplicate_answer"));
        }
        if name != question.id {
            return Err(answer_error("answer_order_or_name"));
        }
        let answer = match (&question.question, wire) {
            (_, WireAnswer::Refusal { .. }) => {
                return Err(
                    answer_error("provider_refusal").with_context("question", question.id.clone())
                );
            }
            (Question::Boolean { .. }, WireAnswer::Predicate { probability, .. }) => {
                Answer::Boolean { probability }
            }
            (
                Question::Choice { options, .. },
                WireAnswer::Choice {
                    choice,
                    confidence,
                    probabilities,
                    ..
                },
            ) => {
                let labels = options
                    .iter()
                    .map(|item| item.label.clone())
                    .collect::<Vec<_>>();
                if !labels.contains(&choice) {
                    return Err(answer_error("unknown_choice"));
                }
                let probabilities = distribution(
                    &labels,
                    probabilities
                        .into_iter()
                        .map(|item| (item.value, item.probability)),
                )?;
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
                    probabilities,
                    ..
                },
            ) => {
                if !score.is_finite() || score < 0.0 || score > (levels.len() - 1) as f64 {
                    return Err(answer_error("score_out_of_range"));
                }
                let labels = (0..levels.len())
                    .map(|index| index.to_string())
                    .collect::<Vec<_>>();
                let mut pairs = Vec::new();
                for item in probabilities {
                    if item.value < 0 || item.label != item.value.to_string() {
                        return Err(answer_error("score_label_mismatch"));
                    }
                    pairs.push((item.label, item.probability));
                }
                let probabilities = distribution(&labels, pairs)?;
                Answer::Score {
                    expected: score,
                    confidence,
                    probabilities: Some(probabilities),
                }
            }
            _ => return Err(answer_error("answer_kind_mismatch")),
        };
        answers.insert(question.id.clone(), answer);
    }
    Ok(ModelResponse {
        model: response.model,
        raw: None,
        answers,
        usage: Some(Usage {
            billing_units: None,
            input_tokens: response.usage.input_tokens,
            output_tokens: response.usage.output_tokens,
        }),
    })
}
