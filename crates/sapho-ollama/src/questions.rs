// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Typed questions answered from answer-token log-probabilities (FR-054).
//!
//! The reply is constrained by a JSON Schema enum, so the model must emit one allowed
//! value per question. The server's log-probabilities at the token where that value
//! starts say how much weight each allowed value had.
use crate::{
    backend::{Alternative, OllamaBackend, TokenLogprob},
    http::failure,
};
use async_trait::async_trait;
use sapho_core::{
    Answer, ErrorCode, ExtractError, ModelBackend, ModelRequest, ModelResponse, NamedQuestion,
    Probability, Question, Result, SaphoError, Usage,
};
use serde::{Serialize, Serializer, ser::SerializeMap};
use std::collections::BTreeMap;

const PREAMBLE: &str = "Answer every question about the input. Reply with JSON only.";
const MAX_SCORE_LEVELS: usize = 10;

/// A question's allowed values, in the order the question declares them.
struct Allowed {
    /// Answer-space key core uses for the value (`yes`/`no` map to `true`/`false`).
    key: String,
    /// The value the model must emit.
    value: String,
    /// The value as it is written inside the response JSON.
    written: Vec<u8>,
}
impl Allowed {
    fn new(key: &str, value: &str) -> Self {
        // A JSON string literal is the value between its two quotes.
        let literal = serde_json::Value::String(value.into()).to_string();
        let inner = literal
            .get(1..literal.len().saturating_sub(1))
            .unwrap_or("");
        Self {
            key: key.into(),
            value: value.into(),
            written: inner.as_bytes().to_vec(),
        }
    }
}

fn allowed_values(question: &Question) -> Vec<Allowed> {
    match question {
        Question::Boolean { .. } => vec![Allowed::new("true", "yes"), Allowed::new("false", "no")],
        Question::Choice { options, .. } => options
            .iter()
            .map(|o| Allowed::new(&o.label, &o.label))
            .collect(),
        Question::Score { levels, .. } => (0..levels.len())
            .map(|i| Allowed::new(&i.to_string(), &i.to_string()))
            .collect(),
    }
}

/// The reply schema: one required string member per question, in question order.
struct ReplyFormat<'a>(&'a [(&'a str, Vec<String>)]);
impl Serialize for ReplyFormat<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        struct Properties<'a>(&'a [(&'a str, Vec<String>)]);
        impl Serialize for Properties<'_> {
            fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
                let mut map = s.serialize_map(Some(self.0.len()))?;
                for (id, values) in self.0 {
                    map.serialize_entry(
                        id,
                        &serde_json::json!({"type": "string", "enum": values}),
                    )?;
                }
                map.end()
            }
        }
        let mut map = serializer.serialize_map(Some(4))?;
        map.serialize_entry("type", "object")?;
        map.serialize_entry("properties", &Properties(self.0))?;
        let required: Vec<&str> = self.0.iter().map(|(id, _)| *id).collect();
        map.serialize_entry("required", &required)?;
        map.serialize_entry("additionalProperties", &false)?;
        map.end()
    }
}

fn system_text(questions: &[NamedQuestion]) -> String {
    let mut text = PREAMBLE.to_string();
    for q in questions {
        text.push_str("\n\n");
        text.push_str(&q.id);
        text.push_str(": ");
        match &q.question {
            Question::Boolean {
                instructions,
                yes,
                no,
            } => text.push_str(&format!("{instructions}\nyes: {yes}\nno: {no}")),
            Question::Choice {
                instructions,
                options,
            } => {
                text.push_str(instructions);
                for o in options {
                    text.push_str(&format!("\n{}: {}", o.label, o.description));
                }
            }
            Question::Score {
                instructions,
                levels,
            } => {
                text.push_str(instructions);
                for (i, description) in levels.iter().enumerate() {
                    text.push_str(&format!("\n{i}: {description}"));
                }
            }
        }
    }
    text
}

/// Canonical JSON: keys sorted, no insignificant whitespace.
fn canonical(value: &serde_json::Value, out: &mut String) {
    match value {
        serde_json::Value::Object(members) => {
            let mut sorted: Vec<_> = members.iter().collect();
            sorted.sort_by(|a, b| a.0.cmp(b.0));
            out.push('{');
            for (i, (key, member)) in sorted.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::Value::String(key.clone()).to_string());
                out.push(':');
                canonical(member, out);
            }
            out.push('}');
        }
        serde_json::Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                canonical(item, out);
            }
            out.push(']');
        }
        scalar => out.push_str(&scalar.to_string()),
    }
}

/// One member of the response object: where its value starts and what it says.
struct Member {
    key: String,
    start: usize,
    value: String,
}

/// Read a flat object of string members, keeping the byte offset of each value's first byte.
fn members(text: &[u8]) -> Option<Vec<Member>> {
    fn skip(text: &[u8], mut i: usize) -> usize {
        while text.get(i).is_some_and(u8::is_ascii_whitespace) {
            i += 1;
        }
        i
    }
    /// A string literal starting at its opening quote: its decoded text and the index after it.
    fn string(text: &[u8], open: usize) -> Option<(String, usize)> {
        let mut i = open + 1;
        loop {
            match text.get(i)? {
                b'\\' => i += 2,
                b'"' => break,
                _ => i += 1,
            }
        }
        let decoded = serde_json::from_slice(text.get(open..=i)?).ok()?;
        Some((decoded, i + 1))
    }
    let mut i = skip(text, 0);
    if text.get(i) != Some(&b'{') {
        return None;
    }
    i = skip(text, i + 1);
    let mut found: Vec<Member> = Vec::new();
    if text.get(i) == Some(&b'}') {
        return (skip(text, i + 1) == text.len()).then_some(found);
    }
    loop {
        if text.get(i) != Some(&b'"') {
            return None;
        }
        let (key, after_key) = string(text, i)?;
        i = skip(text, after_key);
        if text.get(i) != Some(&b':') {
            return None;
        }
        i = skip(text, i + 1);
        if text.get(i) != Some(&b'"') {
            return None;
        }
        let start = i + 1;
        let (value, after_value) = string(text, i)?;
        if found.iter().any(|m| m.key == key) {
            return None;
        }
        found.push(Member { key, start, value });
        i = skip(text, after_value);
        match text.get(i)? {
            b',' => i = skip(text, i + 1),
            b'}' => return (skip(text, i + 1) == text.len()).then_some(found),
            _ => return None,
        }
    }
}

/// The token containing `offset` and the offset's position inside it.
fn locate(tokens: &[TokenLogprob], offset: usize) -> Option<(&TokenLogprob, usize)> {
    let mut start = 0usize;
    for token in tokens {
        let end = start.checked_add(token.bytes.len())?;
        if offset < end {
            return Some((token, offset - start));
        }
        start = end;
    }
    None
}

/// Mass of each allowed value at the answer position, plus the bound for unlisted ones.
struct Masses {
    by_value: Vec<Option<f64>>,
    bound: f64,
}

fn masses(token: &TokenLogprob, inside: usize, selected: usize, allowed: &[Allowed]) -> Masses {
    let mut by_value: Vec<Option<f64>> = vec![None; allowed.len()];
    let generated = token.logprob.exp();
    let mut add = |index: usize, mass: f64| {
        *by_value[index].get_or_insert(0.0) += mass;
    };
    add(selected, generated);
    let mut seen: Vec<&[u8]> = vec![&token.bytes];
    let mut lowest = f64::INFINITY;
    for Alternative { logprob, bytes } in &token.top_logprobs {
        let mass = logprob.exp();
        lowest = lowest.min(mass);
        if seen.contains(&bytes.as_slice()) {
            continue;
        }
        seen.push(bytes);
        // The candidate stands at the same position only if it shares the bytes before the
        // value starts; what follows must begin one allowed value.
        if bytes.get(..inside) != token.bytes.get(..inside) {
            continue;
        }
        let Some(rest) = bytes.get(inside..).filter(|rest| !rest.is_empty()) else {
            continue;
        };
        if let Some(index) = allowed.iter().position(|a| a.written.starts_with(rest)) {
            add(index, mass);
        }
    }
    let selected_mass = by_value[selected].unwrap_or(generated);
    Masses {
        by_value,
        bound: lowest.min(selected_mass),
    }
}

fn answer(question: &Question, allowed: &[Allowed], selected: usize, m: &Masses) -> Option<Answer> {
    let filled = |i: usize| m.by_value[i].unwrap_or(m.bound);
    let total: f64 = (0..allowed.len()).map(filled).sum();
    if !(total.is_finite() && total > 0.0) {
        return None;
    }
    if matches!(question, Question::Boolean { .. }) {
        // allowed = [yes, no]
        return Some(Answer::Boolean {
            probability: Probability::new(filled(0) / total).ok()?,
        });
    }
    let mut probabilities = BTreeMap::new();
    let mut known_mass = 0.0;
    let mut weighted = 0.0;
    for (i, value) in allowed.iter().enumerate() {
        if let Some(mass) = m.by_value[i] {
            let p = mass / total;
            known_mass += p;
            if matches!(question, Question::Score { .. }) {
                weighted += p * i as f64;
            }
            probabilities.insert(value.key.clone(), Probability::new(p).ok()?);
        }
    }
    let confidence = Probability::new(filled(selected) / total).ok()?;
    let probabilities = Some(probabilities);
    Some(match question {
        Question::Score { .. } => Answer::Score {
            expected: weighted / known_mass,
            confidence,
            probabilities,
        },
        _ => Answer::Choice {
            selected: allowed[selected].key.clone(),
            confidence,
            probabilities,
        },
    })
}

fn invalid(reason: &'static str, message: &'static str) -> ExtractError {
    failure(ErrorCode::InvalidAnswer, reason, message)
}

impl OllamaBackend {
    fn ask_checked(request: &ModelRequest) -> Result<Vec<(&NamedQuestion, Vec<Allowed>)>> {
        let mut plan = Vec::new();
        for q in &request.questions {
            if let Question::Score { levels, .. } = &q.question
                && levels.len() > MAX_SCORE_LEVELS
            {
                return Err(failure(
                    ErrorCode::Config,
                    "too_many_levels",
                    "A score question allows at most ten levels",
                )
                .into());
            }
            let allowed = allowed_values(&q.question);
            let mut firsts: Vec<_> = allowed.iter().map(|a| a.written.first()).collect();
            firsts.sort();
            if firsts.windows(2).any(|w| w[0] == w[1]) {
                return Err(failure(
                    ErrorCode::Config,
                    "answer_values_not_distinct",
                    "Allowed answer values must begin with different bytes",
                )
                .into());
            }
            plan.push((q, allowed));
        }
        Ok(plan)
    }
}

#[async_trait]
impl ModelBackend for OllamaBackend {
    async fn infer(&self, request: &ModelRequest) -> Result<ModelResponse> {
        if self.settings.think {
            return Err(failure(
                ErrorCode::Config,
                "think_unsupported_for_questions",
                "Typed questions need a binding with thinking off",
            )
            .into());
        }
        request.validate()?;
        if request.model != self.settings.model {
            return Err(failure(
                ErrorCode::Config,
                "request_model_differs",
                "The request names a model other than the binding's",
            )
            .into());
        }
        let plan = Self::ask_checked(request)?;
        let state = request.state.to_plain_json()?;
        let mut prompt = String::new();
        canonical(&state, &mut prompt);
        let system = system_text(&request.questions);
        let spaces: Vec<(&str, Vec<String>)> = plan
            .iter()
            .map(|(q, allowed)| {
                let values = allowed.iter().map(|a| a.value.clone()).collect();
                (q.id.as_str(), values)
            })
            .collect();
        let format = ReplyFormat(&spaces);
        let body = self.body(&system, &prompt, &format, true);
        let generated = self.generate(&body).await?;
        let from = |e: ExtractError| -> SaphoError {
            e.with_raw(generated.raw.clone())
                .with_usage(generated.usage.clone())
                .into()
        };
        let tokens = generated
            .logprobs
            .as_deref()
            .ok_or_else(|| {
                failure(
                    ErrorCode::Config,
                    sapho_core::reason::LOGPROBS_UNAVAILABLE,
                    "The server returned no log-probabilities",
                )
            })
            .map_err(from)?;
        let text = generated.response.as_bytes();
        if tokens.iter().flat_map(|t| t.bytes.iter()).ne(text.iter()) {
            return Err(from(invalid(
                sapho_core::reason::LOGPROBS_MISMATCH,
                "Token bytes differ from the response text",
            )));
        }
        let found = members(text)
            .filter(|found| found.len() == plan.len())
            .ok_or_else(|| {
                invalid(
                    sapho_core::reason::ANSWER_MALFORMED,
                    "The answer is not one string per question",
                )
            })
            .map_err(from)?;
        let mut answers = BTreeMap::new();
        for (q, allowed) in &plan {
            let member = found
                .iter()
                .find(|m| m.key == q.id)
                .ok_or_else(|| {
                    invalid(
                        sapho_core::reason::ANSWER_MALFORMED,
                        "A question has no answer",
                    )
                })
                .map_err(from)?;
            let selected = allowed
                .iter()
                .position(|a| a.value == member.value)
                .ok_or_else(|| {
                    invalid(
                        sapho_core::reason::ANSWER_NOT_ALLOWED,
                        "The answer is not an allowed value",
                    )
                })
                .map_err(from)?;
            let (token, inside) = locate(tokens, member.start)
                .ok_or_else(|| {
                    invalid(
                        sapho_core::reason::LOGPROBS_MISMATCH,
                        "No token holds the answer",
                    )
                })
                .map_err(from)?;
            let m = masses(token, inside, selected, allowed);
            let value = answer(&q.question, allowed, selected, &m)
                .ok_or_else(|| {
                    invalid(
                        sapho_core::reason::LOGPROBS_MISMATCH,
                        "Probabilities are not usable",
                    )
                })
                .map_err(from)?;
            answers.insert(q.id.clone(), value);
        }
        let usage = match (generated.usage.input_tokens, generated.usage.output_tokens) {
            (Some(input_tokens), Some(output_tokens)) => Some(Usage {
                billing_units: None,
                input_tokens,
                output_tokens,
            }),
            _ => None,
        };
        Ok(ModelResponse {
            model: generated.model.name,
            raw: Some(generated.raw),
            answers,
            usage,
        })
    }
}
