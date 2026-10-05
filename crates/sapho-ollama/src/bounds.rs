// SPDX-License-Identifier: AGPL-3.0-or-later
//! Conservative text-request envelopes using the owning provider encoders.
use crate::{Limits, PromptBudget, PromptFormat, error, prompt, wire_request};
use sapho_core::{ErrorCode, ModelRequest, Result, Value};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Host-proved complete text-field sizes, before provider encoding.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextFieldSize {
    /// Maximum literal UTF8 body bytes.
    pub utf8_bytes: usize,
    /// Maximum complete JSON string bytes, including quotes/escapes.
    pub json_string_bytes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use sapho_core::{BackendId, ChoiceOption, DistributionPolicy, NamedQuestion, Question};
    /// Trace: FR-048-AC-11
    #[test]
    fn text_bounds_dominate_real_encoder_for_unicode_controls_and_each_format() {
        for text in ["plain", "\"\\\n\u{1} FIELD fake\n", "日本語🙂\u{7f}"] {
            for format in [
                PromptFormat::ModelRequestJson,
                PromptFormat::FramedStateV1,
                PromptFormat::FramedCatalogV1,
            ] {
                let question = NamedQuestion {
                    id: "question-9999".into(),
                    question: Question::Choice {
                        instructions: "Use the complete original owner".into(),
                        options: vec![
                            ChoiceOption {
                                label: "yes".into(),
                                description: "yes".into(),
                            },
                            ChoiceOption {
                                label: "no".into(),
                                description: "no".into(),
                            },
                        ],
                    },
                };
                let actual = ModelRequest {
                    backend: BackendId::new("judge").unwrap(),
                    model: "synthetic".into(),
                    expected_model: Some("synthetic".into()),
                    distribution_policy: DistributionPolicy::Strict {},
                    state: Value::Record(BTreeMap::from([(
                        "owner".into(),
                        Value::Text(text.into()),
                    )])),
                    questions: vec![question],
                };
                let mut shape = actual.clone();
                shape.state = Value::Record(BTreeMap::from([(
                    "owner".into(),
                    Value::Text(String::new()),
                )]));
                let fields = BTreeMap::from([(
                    "owner".into(),
                    TextFieldSize {
                        utf8_bytes: text.len(),
                        json_string_bytes: serde_json::to_vec(text).unwrap().len(),
                    },
                )]);
                let limits = Limits {
                    prompt_format: format,
                    ..Default::default()
                };
                let bound = RequestUpperBound::measure_text_shape(
                    &shape,
                    &fields,
                    serde_json::to_vec(&actual.questions).unwrap().len(),
                    Some(
                        serde_json::to_vec(
                            &crate::catalog::Catalog::new(&actual.questions).unwrap(),
                        )
                        .unwrap()
                        .len(),
                    ),
                    &limits,
                )
                .unwrap();
                let exact = crate::RequestBudget::measure(&actual, &limits).unwrap();
                assert!(exact.prompt.prompt_bytes <= bound.prompt_upper.prompt_bytes);
                assert!(exact.typed_request_bytes <= bound.typed_request_bytes_upper);
                assert!(exact.wire_request_bytes <= bound.wire_request_bytes_upper);
                assert!(bound.allowed());
            }
        }
    }

    /// Trace: FR-048-AC-11, FR-048-AC-13
    #[test]
    fn catalog_bounds_cannot_be_implicitly_inferred_from_a_typed_json_bound() {
        let limits = Limits {
            prompt_format: PromptFormat::FramedCatalogV1,
            ..Default::default()
        };
        let shape = ModelRequest {
            backend: BackendId::new("judge").unwrap(),
            model: "synthetic".into(),
            expected_model: None,
            distribution_policy: DistributionPolicy::Strict {},
            state: Value::Record(BTreeMap::new()),
            questions: vec![NamedQuestion {
                id: "q".into(),
                question: Question::Boolean {
                    instructions: "Complete criterion".into(),
                    yes: "true".into(),
                    no: "false".into(),
                },
            }],
        };
        assert!(
            RequestUpperBound::measure_text_shape(&shape, &BTreeMap::new(), 256, None, &limits)
                .is_err()
        );
    }
}
/// Conservative sizes, distinct from exact RequestBudget measurements.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestUpperBound {
    /// Prompt component ceilings under the unchanged provider limits.
    pub prompt_upper: PromptBudget,
    /// Maximum complete typed ModelRequest JSON bytes.
    pub typed_request_bytes_upper: usize,
    /// Maximum complete HTTP body JSON bytes.
    pub wire_request_bytes_upper: usize,
    /// Maximum bytes for each complete typed/wire body.
    pub request_bytes_limit: usize,
}
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .ok_or_else(|| error(ErrorCode::LimitExceeded, "Request bound overflow"))
}
fn subtract(a: usize, b: usize) -> Result<usize> {
    a.checked_sub(b)
        .ok_or_else(|| error(ErrorCode::InvalidValue, "Request bound shape mismatch"))
}
fn count(value: &impl Serialize) -> Result<usize> {
    sapho_core::measured_json_bytes(value, usize::MAX)
}
impl RequestUpperBound {
    /// Bound a host-proved text-state shape without source, provider I/O or model calls.
    /// `shape` contains empty text placeholders and complete output-schema menu
    /// witnesses. Menu witnesses must dominate every possible actual output schema;
    /// `questions_json_bytes` independently bounds complete actual instructions.
    pub fn measure_text_shape(
        shape: &ModelRequest,
        fields: &BTreeMap<String, TextFieldSize>,
        questions_json_bytes: usize,
        catalog_questions_bytes: Option<usize>,
        limits: &Limits,
    ) -> Result<Self> {
        limits.validate()?;
        shape.validate()?;
        let shape_bytes = sapho_core::measured_json_bytes(shape, limits.request_bytes)?;
        let Value::Record(values) = &shape.state else {
            return Err(error(
                ErrorCode::TypeMismatch,
                "Text request shape must be a record",
            ));
        };
        if values.len() != fields.len() || questions_json_bytes < 2 {
            return Err(error(
                ErrorCode::InvalidValue,
                "Text request shape fields differ",
            ));
        }
        let mut state_upper = count(&shape.state)?;
        let mut literal = 0usize;
        let mut headers = 20usize; // decimal QUESTIONS byte-length header ceiling
        for (name, size) in fields {
            if !matches!(values.get(name), Some(Value::Text(text)) if text.is_empty())
                || size.json_string_bytes < add(size.utf8_bytes, 2)?
            {
                return Err(error(
                    ErrorCode::InvalidValue,
                    "Invalid complete text field bound",
                ));
            }
            state_upper = add(subtract(state_upper, 2)?, size.json_string_bytes)?;
            literal = add(literal, size.utf8_bytes)?;
            headers = add(headers, 20)?; // usize decimal upper bound per FIELD length
        }
        let rendered = prompt::render(shape, limits.prompt_format)?;
        let old_questions = count(&shape.questions)?;
        let old_state = count(&shape.state)?;
        let typed = add(
            add(
                subtract(subtract(shape_bytes, old_state)?, old_questions)?,
                state_upper,
            )?,
            questions_json_bytes,
        )?;
        let prompt_questions_bytes = match limits.prompt_format {
            PromptFormat::FramedCatalogV1 => catalog_questions_bytes.ok_or_else(|| {
                error(
                    ErrorCode::InvalidValue,
                    "Catalog request requires a catalog bound",
                )
            })?,
            PromptFormat::ModelRequestJson | PromptFormat::FramedStateV1 => questions_json_bytes,
        };
        if prompt_questions_bytes < 2 {
            return Err(error(
                ErrorCode::InvalidValue,
                "Empty prompt question bound",
            ));
        }
        let (state_bytes, envelope_bytes) = match limits.prompt_format {
            PromptFormat::ModelRequestJson => (state_upper, rendered.envelope_bytes),
            PromptFormat::FramedStateV1 | PromptFormat::FramedCatalogV1 => {
                (literal, add(rendered.envelope_bytes, headers)?)
            }
        };
        let prompt_bytes = add(
            add(rendered.instruction_bytes, state_bytes)?,
            add(prompt_questions_bytes, envelope_bytes)?,
        )?;
        // Wire serialization escapes arbitrary complete UTF8 prompt text. Six
        // bytes per original byte covers every control/quote/backslash encoding.
        // The real schema/options/envelope come from the same dispatch encoder.
        let wire_base = count(&wire_request(shape, "", limits))?;
        let prompt_json = add(
            prompt_bytes
                .checked_mul(6)
                .ok_or_else(|| error(ErrorCode::LimitExceeded, "Wire bound overflow"))?,
            2,
        )?;
        let wire = add(subtract(wire_base, 2)?, prompt_json)?;
        Ok(Self {
            prompt_upper: PromptBudget {
                format: limits.prompt_format,
                instruction_bytes: rendered.instruction_bytes,
                state_bytes,
                question_bytes: prompt_questions_bytes,
                envelope_bytes,
                prompt_bytes,
                context_tokens: limits.context_tokens,
                output_tokens: limits.output_tokens,
                template_reserve: 1024,
            },
            typed_request_bytes_upper: typed,
            wire_request_bytes_upper: wire,
            request_bytes_limit: limits.request_bytes,
        })
    }
    /// Both complete-body caps and the conservative prompt cap must hold.
    pub fn allowed(&self) -> bool {
        self.prompt_upper.allowed()
            && self.typed_request_bytes_upper <= self.request_bytes_limit
            && self.wire_request_bytes_upper <= self.request_bytes_limit
    }
}
