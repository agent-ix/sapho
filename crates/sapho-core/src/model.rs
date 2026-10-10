// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Typed System One contracts and answer validation (FR-004/005/006).
use crate::{BackendId, ErrorCode, Probability, RawExchange, Result, SaphoError, Value};
use crate::{DistributionAdjustment, DistributionPolicy, distribution::MASS_ROUNDOFF};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// One named choice, kept in caller-specified order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChoiceOption {
    /// Label returned by the model.
    pub label: String,
    /// Meaning of this option.
    pub description: String,
}
/// A closed typed question; textual criteria belong to the consumer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Question {
    /// Boolean question returning P(true).
    Boolean {
        /// Narrow question wording.
        instructions: String,
        /// Meaning of true.
        yes: String,
        /// Meaning of false.
        no: String,
    },
    /// Choice among ordered labels.
    Choice {
        /// Narrow question wording.
        instructions: String,
        /// Ordered alternatives.
        options: Vec<ChoiceOption>,
    },
    /// Expected ordinal score against ordered criteria.
    Score {
        /// Narrow question wording.
        instructions: String,
        /// Level descriptions indexed from zero.
        levels: Vec<String>,
    },
}
/// A question ID paired with its answer space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedQuestion {
    /// Unique answer key within a request.
    pub id: String,
    /// Typed definition.
    pub question: Question,
}
impl Question {
    /// Declared outcomes in their configured order.
    pub fn labels(&self) -> Vec<String> {
        match self {
            Self::Boolean { .. } => vec!["false".into(), "true".into()],
            Self::Choice { options, .. } => options.iter().map(|o| o.label.clone()).collect(),
            Self::Score { levels, .. } => (0..levels.len()).map(|i| i.to_string()).collect(),
        }
    }
}
/// Validate uniqueness, cardinality and question text without inference.
pub fn validate_questions(qs: &[NamedQuestion]) -> Result<()> {
    let mut seen = BTreeSet::new();
    for q in qs {
        if q.id.is_empty() || !seen.insert(&q.id) {
            return Err(SaphoError::new(
                ErrorCode::InvalidValue,
                "Empty or duplicate question ID",
            ));
        }
        let (text, labels) = match &q.question {
            Question::Boolean { instructions, .. } => (instructions, None),
            Question::Choice {
                instructions,
                options,
            } => (
                instructions,
                Some(options.iter().map(|o| o.label.as_str()).collect::<Vec<_>>()),
            ),
            Question::Score {
                instructions,
                levels,
            } => {
                if levels.len() < 2 {
                    return Err(SaphoError::new(
                        ErrorCode::InvalidValue,
                        "Score needs at least two levels",
                    ));
                }
                (instructions, None)
            }
        };
        if text.trim().is_empty() {
            return Err(SaphoError::new(
                ErrorCode::InvalidValue,
                "Empty question wording",
            ));
        }
        if let Some(labels) = labels {
            let unique = labels.iter().copied().collect::<BTreeSet<_>>();
            if labels.len() < 2
                || labels.len() != unique.len()
                || labels.iter().any(|s| s.trim().is_empty())
            {
                return Err(SaphoError::new(
                    ErrorCode::InvalidValue,
                    "Invalid choice labels",
                ));
            }
        }
    }
    Ok(())
}
/// One raw typed model answer, validated against its request before use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Answer {
    /// Probability of true.
    Boolean {
        /// Returned unit-range probability.
        probability: Probability,
    },
    /// Selected label plus independently retained probability evidence.
    Choice {
        /// Reported selected label.
        selected: String,
        /// Reported selection confidence.
        confidence: Probability,
        /// Known outcome probabilities; None means unavailable.
        probabilities: Option<BTreeMap<String, Probability>>,
    },
    /// Expected score, potentially fractional.
    Score {
        /// Expected rubric level.
        expected: f64,
        /// Reported score confidence.
        confidence: Probability,
        /// Known rubric-level probabilities.
        probabilities: Option<BTreeMap<String, Probability>>,
    },
}
/// Whether all declared outcomes have known probability mass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistributionState {
    /// Every requested outcome was supplied and masses sum to one.
    Complete,
    /// Every outcome was supplied with accepted nonunit mass; projection normalizes.
    Approximate,
    /// Only some outcomes were supplied.
    Partial,
    /// No probability distribution was supplied.
    Unavailable,
}
/// Validated answers retain their question spaces for later probability lookup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Answers {
    /// Explicit policy used to validate these unchanged raw answer values.
    pub distribution_policy: DistributionPolicy,
    /// Original ordered definitions.
    pub questions: Vec<NamedQuestion>,
    /// Answer values keyed by question ID.
    pub values: BTreeMap<String, Answer>,
}
impl Answers {
    /// Validate this answer block after loading or native construction.
    pub fn validate(&self) -> Result<()> {
        validate_questions(&self.questions)?;
        self.distribution_policy.validate()?;
        validate_answer_map(&self.questions, &self.values, self.distribution_policy)
    }
    /// Return completeness without manufacturing missing probabilities.
    pub fn distribution_state(&self, id: &str) -> Result<DistributionState> {
        self.validate()?;
        let (q, a) = self.lookup(id)?;
        match a {
            Answer::Boolean { .. } => Ok(DistributionState::Complete),
            Answer::Choice { probabilities, .. } | Answer::Score { probabilities, .. } => {
                Ok(match probabilities {
                    None => DistributionState::Unavailable,
                    Some(p) if p.len() == q.labels().len() => {
                        if adjustment(q, a).is_some() {
                            DistributionState::Approximate
                        } else {
                            DistributionState::Complete
                        }
                    }
                    Some(_) => DistributionState::Partial,
                })
            }
        }
    }
    /// Inspect the normalization used for approximate complete projections.
    /// Exact, partial and unavailable distributions have no adjustment.
    pub fn distribution_adjustment(&self, id: &str) -> Result<Option<DistributionAdjustment>> {
        self.validate()?;
        let (question, answer) = self.lookup(id)?;
        Ok(adjustment(question, answer))
    }
    fn lookup(&self, id: &str) -> Result<(&Question, &Answer)> {
        let q = self.questions.iter().find(|q| q.id == id).ok_or_else(|| {
            SaphoError::new(ErrorCode::MissingAnswer, "Question definition absent")
                .with_context("question", id)
        })?;
        let a = self.values.get(id).ok_or_else(|| {
            SaphoError::new(ErrorCode::MissingAnswer, "Answer absent").with_context("question", id)
        })?;
        Ok((&q.question, a))
    }
    /// Project known mass; normalize only explicitly accepted approximate full distributions (FR-020).
    pub fn probability(&self, id: &str, labels: &[String]) -> Result<Probability> {
        self.validate()?;
        let (q, a) = self.lookup(id)?;
        let valid = q.labels();
        if labels.is_empty() || labels.iter().collect::<BTreeSet<_>>().len() != labels.len() {
            return Err(SaphoError::new(
                ErrorCode::InvalidValue,
                "Probability labels must be non-empty and unique",
            ));
        }
        let mut sum = 0.0;
        for label in labels {
            if !valid.contains(label) {
                return Err(SaphoError::new(
                    ErrorCode::InvalidAnswer,
                    "Unknown probability label",
                ));
            }
            sum += match a {
                Answer::Boolean { probability } => {
                    if label == "true" {
                        probability.get()
                    } else {
                        1.0 - probability.get()
                    }
                }
                Answer::Choice { probabilities, .. } | Answer::Score { probabilities, .. } => {
                    probabilities
                        .as_ref()
                        .and_then(|p| p.get(label))
                        .ok_or_else(|| {
                            SaphoError::new(
                                ErrorCode::UnsupportedDistribution,
                                "Requested outcome mass unavailable",
                            )
                        })?
                        .get()
                }
            };
        }
        if let Some(derived) = adjustment(q, a) {
            sum /= derived.raw_mass;
        }
        if sum > 1.0 + MASS_ROUNDOFF {
            return Err(SaphoError::new(
                ErrorCode::InvalidAnswer,
                "Projected mass exceeds one",
            ));
        }
        Probability::new(sum.min(1.0))
    }
}
fn adjustment(question: &Question, answer: &Answer) -> Option<DistributionAdjustment> {
    let probabilities = match answer {
        Answer::Boolean { .. } => return None,
        Answer::Choice { probabilities, .. } | Answer::Score { probabilities, .. } => {
            probabilities.as_ref()?
        }
    };
    if probabilities.len() != question.labels().len() {
        return None;
    }
    let raw_mass = probabilities.values().map(|p| p.get()).sum::<f64>();
    if (raw_mass - 1.0).abs() <= MASS_ROUNDOFF {
        return None;
    }
    Some(DistributionAdjustment {
        raw_mass,
        scale: 1.0 / raw_mass,
    })
}
fn validate_answer_map(
    qs: &[NamedQuestion],
    answers: &BTreeMap<String, Answer>,
    policy: DistributionPolicy,
) -> Result<()> {
    if answers.len() != qs.len() {
        return Err(SaphoError::new(
            ErrorCode::MissingAnswer,
            "Answer IDs differ from request",
        ));
    }
    for q in qs {
        let a = answers.get(&q.id).ok_or_else(|| {
            SaphoError::new(ErrorCode::MissingAnswer, "Required answer absent")
                .with_context("question", &q.id)
        })?;
        let dist = match (&q.question, a) {
            (Question::Boolean { .. }, Answer::Boolean { .. }) => None,
            (
                Question::Choice { options, .. },
                Answer::Choice {
                    selected,
                    probabilities,
                    ..
                },
            ) if options.iter().any(|o| o.label == *selected) => Some(probabilities),
            (
                Question::Score { levels, .. },
                Answer::Score {
                    expected,
                    probabilities,
                    ..
                },
            ) => {
                let max = u32::try_from(levels.len() - 1).map_err(|_| {
                    SaphoError::new(ErrorCode::InvalidValue, "Too many rubric levels")
                })?;
                if !expected.is_finite() || *expected < 0.0 || *expected > f64::from(max) {
                    return Err(SaphoError::new(
                        ErrorCode::InvalidAnswer,
                        "Expected score outside rubric",
                    ));
                }
                Some(probabilities)
            }
            _ => {
                return Err(SaphoError::new(
                    ErrorCode::InvalidAnswer,
                    "Answer type or selected label differs from question",
                ));
            }
        };
        if let Some(Some(dist)) = dist {
            let labels = q.question.labels();
            if dist.keys().any(|k| !labels.contains(k)) {
                return Err(SaphoError::new(
                    ErrorCode::InvalidAnswer,
                    "Unknown distribution outcome",
                ));
            }
            let mass = dist.values().map(|p| p.get()).sum::<f64>();
            let complete = dist.len() == labels.len();
            let allowed_error = if complete {
                policy.allowed_error()
            } else {
                MASS_ROUNDOFF
            };
            let invalid = if complete {
                mass <= 0.0 || (mass - 1.0).abs() > allowed_error
            } else {
                mass > 1.0 + allowed_error
            };
            if invalid {
                return Err(
                    SaphoError::new(ErrorCode::InvalidAnswer, "Invalid distribution mass")
                        .with_context("question", &q.id)
                        .with_context("coverage", if complete { "complete" } else { "partial" })
                        .with_context("mass", mass.to_string())
                        .with_context("allowed_error", allowed_error.to_string()),
                );
            }
        }
    }
    Ok(())
}
/// Exact backend request, including binding and model identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelRequest {
    /// Explicit host distribution interpretation, included in exact replay matching.
    pub distribution_policy: DistributionPolicy,
    /// Host registry binding identity.
    pub backend: BackendId,
    /// Requested model name or alias.
    pub model: String,
    /// Optional caller-requested strict actual identity.
    pub expected_model: Option<String>,
    /// Structured context without credentials or source sidecars.
    pub state: Value,
    /// Ordered question block: one explicit batch.
    pub questions: Vec<NamedQuestion>,
}
impl ModelRequest {
    /// Validate the request before transport or recording.
    pub fn validate(&self) -> Result<()> {
        self.backend.validate()?;
        self.distribution_policy.validate()?;
        if self.model.is_empty() || self.expected_model.as_ref().is_some_and(|m| m.is_empty()) {
            return Err(SaphoError::new(
                ErrorCode::InvalidValue,
                "Empty model identity",
            ));
        }
        if !matches!(self.state, Value::Record(_)) {
            return Err(SaphoError::new(
                ErrorCode::TypeMismatch,
                "Model state must be a record",
            ));
        }
        self.state.validate()?;
        validate_questions(&self.questions)
    }
}
/// Available provider-reported token counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Usage {
    /// Provider-reported question charges, distinct from token counts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_units: Option<u64>,
    /// Input tokens charged or counted by the provider.
    pub input_tokens: u64,
    /// Output tokens reported by the provider.
    pub output_tokens: u64,
}
/// Raw response retained even when answer validation fails.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelResponse {
    /// Actual resolved model identity.
    pub model: String,
    /// The exact request and response bodies (never headers or URLs), when the provider exposes them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<RawExchange>,
    /// Raw typed answers.
    pub answers: BTreeMap<String, Answer>,
    /// Usage when the provider supplies it.
    pub usage: Option<Usage>,
}
/// Check all response semantics and return an answer block safe for projection.
pub fn validate_response(request: &ModelRequest, response: &ModelResponse) -> Result<Answers> {
    request.validate()?;
    if response.model.is_empty() {
        return Err(SaphoError::new(
            ErrorCode::InvalidAnswer,
            "Actual model is empty",
        ));
    }
    if request
        .expected_model
        .as_ref()
        .is_some_and(|m| *m != response.model)
    {
        return Err(SaphoError::new(
            ErrorCode::ModelMismatch,
            "Actual model differs from expectation",
        )
        .with_context("actual", &response.model));
    }
    validate_answer_map(
        &request.questions,
        &response.answers,
        request.distribution_policy,
    )?;
    Ok(Answers {
        distribution_policy: request.distribution_policy,
        questions: request.questions.clone(),
        values: response.answers.clone(),
    })
}
