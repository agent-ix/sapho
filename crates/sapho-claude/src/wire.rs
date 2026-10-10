// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Fixed JSON-output schema and checked answer translation for Messages.
use sapho_core::{
    Answer, ErrorCode, ModelRequest, ModelResponse, Probability, Question, Result, SaphoError,
    Usage, bounded_json, decode_json,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};
use std::collections::{BTreeMap, BTreeSet};

fn invalid(reason: &'static str) -> SaphoError {
    SaphoError::new(ErrorCode::InvalidAnswer, "Invalid Claude answer")
        .with_context("reason", reason)
}
fn input_invalid(reason: &'static str) -> SaphoError {
    SaphoError::new(ErrorCode::InvalidValue, "Invalid Claude request")
        .with_context("reason", reason)
}
#[derive(Serialize)]
struct Request<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: [Message; 1],
    stream: bool,
    output_config: OutputConfig,
}
#[derive(Serialize)]
struct Message {
    role: &'static str,
    content: String,
}
#[derive(Serialize)]
struct OutputConfig {
    format: OutputFormat,
}
#[derive(Serialize)]
struct OutputFormat {
    r#type: &'static str,
    schema: Json,
}
fn schema() -> Json {
    let pair = json!({"type":"object","properties":{"label":{"type":"string"},
        "probability":{"type":"number"}},"required":["label","probability"],
        "additionalProperties":false});
    json!({"type":"object","properties":{"answers":{"type":"array","items":{
        "anyOf":[
            {"type":"object","properties":{"id":{"type":"string"},"kind":{"const":"boolean"},
                "probability":{"type":"number"}},
                "required":["id","kind","probability"],"additionalProperties":false},
            {"type":"object","properties":{"id":{"type":"string"},"kind":{"const":"choice"},
                "selected":{"type":"string"},"confidence":{"type":"number"},
                "probabilities":{"type":"array","items":pair.clone()}},
                "required":["id","kind","selected","confidence","probabilities"],
                "additionalProperties":false},
            {"type":"object","properties":{"id":{"type":"string"},"kind":{"const":"score"},
                "expected":{"type":"number"},"confidence":{"type":"number"},
                "probabilities":{"type":"array","items":pair}},
                "required":["id","kind","expected","confidence","probabilities"],
                "additionalProperties":false}]} }},"required":["answers"],"additionalProperties":false})
}
/// Encode one complete checked core request, without source sidecars or policy metadata.
pub(crate) fn encode(request: &ModelRequest, max_tokens: u32, ceiling: usize) -> Result<Vec<u8>> {
    request.validate()?;
    if !(1..=16).contains(&request.questions.len()) {
        return Err(input_invalid("question_count"));
    }
    for named in &request.questions {
        match &named.question {
            Question::Boolean { yes, no, .. } if yes.trim().is_empty() || no.trim().is_empty() => {
                return Err(input_invalid("boolean_criteria"));
            }
            Question::Choice { options, .. }
                if options.len() > 16
                    || options.iter().any(|o| o.description.trim().is_empty()) =>
            {
                return Err(input_invalid("choice_options"));
            }
            Question::Score { levels, .. }
                if levels.len() > 16 || levels.iter().any(|v| v.trim().is_empty()) =>
            {
                return Err(input_invalid("score_levels"));
            }
            _ => {}
        }
    }
    let state = bounded_json(&request.state.to_plain_json()?, ceiling)?;
    let questions = bounded_json(&request.questions, ceiling)?;
    let state = std::str::from_utf8(&state).map_err(|_| input_invalid("state_utf8"))?;
    let questions = std::str::from_utf8(&questions).map_err(|_| input_invalid("question_utf8"))?;
    let content = format!(
        "State JSON: {state}\nQuestions JSON: {questions}\nReturn one answer per question ID. Probabilities and confidence are numbers in [0,1]. Score levels use zero-based decimal labels; return the expected ordinal score separately."
    );
    bounded_json(
        &Request {
            model: &request.model,
            max_tokens,
            messages: [Message {
                role: "user",
                content,
            }],
            stream: false,
            output_config: OutputConfig {
                format: OutputFormat {
                    r#type: "json_schema",
                    schema: schema(),
                },
            },
        },
        ceiling,
    )
}
#[derive(Deserialize)]
struct ProviderMessage {
    r#type: String,
    id: String,
    role: String,
    model: String,
    stop_reason: String,
    stop_details: Option<StopDetails>,
    content: Vec<ContentBlock>,
    usage: ProviderUsage,
}
#[derive(Deserialize)]
struct StopDetails {
    r#type: String,
}
#[derive(Deserialize)]
struct ContentBlock {
    r#type: String,
    text: Option<String>,
}
#[derive(Deserialize)]
struct ProviderUsage {
    input_tokens: u64,
    output_tokens: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Output {
    answers: Vec<WireAnswer>,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum WireAnswer {
    Boolean {
        id: String,
        probability: Probability,
    },
    Choice {
        id: String,
        selected: String,
        confidence: Probability,
        probabilities: Vec<Mass>,
    },
    Score {
        id: String,
        expected: f64,
        confidence: Probability,
        probabilities: Vec<Mass>,
    },
}
impl WireAnswer {
    fn id(&self) -> &str {
        match self {
            Self::Boolean { id, .. } | Self::Choice { id, .. } | Self::Score { id, .. } => id,
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Mass {
    label: String,
    probability: Probability,
}
fn distribution(labels: &[String], mass: Vec<Mass>) -> Result<BTreeMap<String, Probability>> {
    let mut result = BTreeMap::new();
    for item in mass {
        if !labels.contains(&item.label) || result.insert(item.label, item.probability).is_some() {
            return Err(invalid("distribution_entry"));
        }
    }
    if result.len() != labels.len() {
        return Err(invalid("incomplete_distribution"));
    }
    Ok(result)
}
/// Decode a provider Message; core mass and expected-model policy are checked by the caller.
pub(crate) fn decode(
    request: &ModelRequest,
    bytes: &[u8],
    ceiling: usize,
) -> Result<ModelResponse> {
    let response: ProviderMessage = decode_json(bytes, ceiling).map_err(|e| {
        if e.code == ErrorCode::LimitExceeded {
            e
        } else {
            invalid("message_schema")
        }
    })?;
    if response.r#type != "message"
        || response.id.is_empty()
        || response.role != "assistant"
        || response.model.is_empty()
    {
        return Err(invalid("message_identity"));
    }
    if response.stop_reason == "refusal"
        || response
            .stop_details
            .as_ref()
            .is_some_and(|d| d.r#type == "refusal")
    {
        return Err(invalid("provider_refusal"));
    }
    if response.stop_reason != "end_turn" {
        return Err(invalid("stop_reason"));
    }
    if response.content.len() != 1 || response.content[0].r#type != "text" {
        return Err(invalid("content_blocks"));
    }
    let text = response.content[0]
        .text
        .as_ref()
        .ok_or_else(|| invalid("text_absent"))?;
    let output: Output = decode_json(text.as_bytes(), ceiling).map_err(|e| {
        if e.code == ErrorCode::LimitExceeded {
            e
        } else {
            invalid("answer_schema")
        }
    })?;
    if output.answers.len() < request.questions.len() {
        return Err(SaphoError::new(
            ErrorCode::MissingAnswer,
            "Claude answer absent",
        ));
    }
    if output.answers.len() > request.questions.len() {
        return Err(invalid("extra_answer"));
    }
    let expected_ids = request
        .questions
        .iter()
        .map(|q| q.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut answers = BTreeMap::new();
    for wire in output.answers {
        let id = wire.id().to_owned();
        if !expected_ids.contains(id.as_str()) || answers.contains_key(&id) {
            return Err(invalid("answer_id"));
        }
        let question = request
            .questions
            .iter()
            .find(|q| q.id == id)
            .ok_or_else(|| invalid("answer_id"))?;
        let answer = match (&question.question, wire) {
            (Question::Boolean { .. }, WireAnswer::Boolean { probability, .. }) => {
                Answer::Boolean { probability }
            }
            (
                Question::Choice { options, .. },
                WireAnswer::Choice {
                    selected,
                    confidence,
                    probabilities,
                    ..
                },
            ) => {
                let labels = options.iter().map(|o| o.label.clone()).collect::<Vec<_>>();
                if !labels.contains(&selected) {
                    return Err(invalid("selected_label"));
                }
                Answer::Choice {
                    selected,
                    confidence,
                    probabilities: Some(distribution(&labels, probabilities)?),
                }
            }
            (
                Question::Score { levels, .. },
                WireAnswer::Score {
                    expected,
                    confidence,
                    probabilities,
                    ..
                },
            ) => {
                if !expected.is_finite() || expected < 0.0 || expected > (levels.len() - 1) as f64 {
                    return Err(invalid("expected_score"));
                }
                let labels = (0..levels.len()).map(|i| i.to_string()).collect::<Vec<_>>();
                Answer::Score {
                    expected,
                    confidence,
                    probabilities: Some(distribution(&labels, probabilities)?),
                }
            }
            _ => return Err(invalid("answer_kind")),
        };
        answers.insert(id, answer);
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
