// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Lossless question dictionaries; no domain parsing or semantic compression (FR-048).
use sapho_core::{ChoiceOption, ErrorCode, NamedQuestion, Question, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Space {
    Boolean { yes: String, no: String },
    Choice { options: Vec<ChoiceOption> },
    Score { levels: Vec<String> },
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    id: String,
    instructions: Vec<u32>,
    space: u32,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Catalog {
    fragments: Vec<String>,
    spaces: Vec<Space>,
    questions: Vec<Row>,
}
fn invalid(message: &'static str) -> sapho_core::SaphoError {
    crate::error(ErrorCode::InvalidValue, message)
}
fn index(n: usize) -> Result<u32> {
    u32::try_from(n).map_err(|_| invalid("Question catalog index overflow"))
}
fn instruction(question: &Question) -> &str {
    match question {
        Question::Boolean { instructions, .. }
        | Question::Choice { instructions, .. }
        | Question::Score { instructions, .. } => instructions,
    }
}
fn space(question: &Question) -> Space {
    match question {
        Question::Boolean { yes, no, .. } => Space::Boolean {
            yes: yes.clone(),
            no: no.clone(),
        },
        Question::Choice { options, .. } => Space::Choice {
            options: options.clone(),
        },
        Question::Score { levels, .. } => Space::Score {
            levels: levels.clone(),
        },
    }
}
impl Catalog {
    pub(crate) fn new(questions: &[NamedQuestion]) -> Result<Self> {
        sapho_core::validate_questions(questions)?;
        let mut result = Self {
            fragments: Vec::new(),
            spaces: Vec::new(),
            questions: Vec::new(),
        };
        let mut fragments = BTreeMap::<String, u32>::new();
        let mut spaces = BTreeMap::<String, u32>::new();
        for question in questions {
            let mut refs = Vec::new();
            for fragment in instruction(&question.question).split_inclusive('\n') {
                let n = if let Some(n) = fragments.get(fragment) {
                    *n
                } else {
                    let n = index(result.fragments.len())?;
                    result.fragments.push(fragment.into());
                    fragments.insert(fragment.into(), n);
                    n
                };
                refs.push(n);
            }
            let value = space(&question.question);
            let key = serde_json::to_string(&value)
                .map_err(|_| invalid("Question catalog encoding failed"))?;
            let n = if let Some(n) = spaces.get(&key) {
                *n
            } else {
                let n = index(result.spaces.len())?;
                result.spaces.push(value);
                spaces.insert(key, n);
                n
            };
            result.questions.push(Row {
                id: question.id.clone(),
                instructions: refs,
                space: n,
            });
        }
        Ok(result)
    }
    #[cfg(test)]
    fn restore(&self) -> Result<Vec<NamedQuestion>> {
        self.questions
            .iter()
            .map(|row| {
                let mut instructions = String::new();
                for n in &row.instructions {
                    let n = usize::try_from(*n)
                        .map_err(|_| invalid("Question fragment index overflow"))?;
                    instructions.push_str(
                        self.fragments
                            .get(n)
                            .ok_or_else(|| invalid("Question fragment absent"))?,
                    );
                }
                let n = usize::try_from(row.space)
                    .map_err(|_| invalid("Question space index overflow"))?;
                let question = match self
                    .spaces
                    .get(n)
                    .ok_or_else(|| invalid("Question space absent"))?
                {
                    Space::Boolean { yes, no } => Question::Boolean {
                        instructions,
                        yes: yes.clone(),
                        no: no.clone(),
                    },
                    Space::Choice { options } => Question::Choice {
                        instructions,
                        options: options.clone(),
                    },
                    Space::Score { levels } => Question::Score {
                        instructions,
                        levels: levels.clone(),
                    },
                };
                Ok(NamedQuestion {
                    id: row.id.clone(),
                    question,
                })
            })
            .collect()
    }
}
/// Measure the complete lossless question catalog without provider I/O.
pub fn catalog_question_bytes(questions: &[NamedQuestion]) -> Result<usize> {
    sapho_core::measured_json_bytes(&Catalog::new(questions)?, usize::MAX)
}

/// Bound a catalog from a complete typed question-list bound when every
/// instruction is one nonempty line. The host must prove that restriction;
/// actual calls still use the exact encoder/guard. A row has one instruction
/// reference and one space reference, each at most ten decimal digits. Its
/// fixed envelope and reference overhead are at most 100 bytes per question;
/// the typed list already pays for every ID/instruction/menu string. The
/// catalog's fixed three-list envelope is less than 128 bytes.
pub fn catalog_single_line_bound(
    questions_json_bytes: usize,
    maximum_questions: usize,
) -> Result<usize> {
    if questions_json_bytes < 2 || maximum_questions == 0 {
        return Err(invalid("Empty single-line question catalog shape"));
    }
    index(maximum_questions)?;
    maximum_questions
        .checked_mul(100)
        .and_then(|overhead| questions_json_bytes.checked_add(overhead))
        .and_then(|total| total.checked_add(128))
        .ok_or_else(|| invalid("Question catalog size overflow"))
}

/// One host-proved question shape for catalog admission.
/// A template has a known complete first line and at most one newline-free tail.
/// The actual first line/answer space must match one template; IDs and tail size
/// are separately bounded. This is an assertion about future requests, not data.
#[derive(Debug, Clone)]
pub struct CatalogTemplate {
    /// Full typed menu and known instruction prefix; tail can be a size witness.
    pub question: NamedQuestion,
    /// Maximum complete JSON string size of the tail, including quotes.
    pub tail_json_bytes: usize,
}
/// Bound the catalog encoding for any mixed batch of these complete templates.
/// Does not bound typed request JSON, source state, service tokens or raw responses.
pub fn catalog_question_bound(
    templates: &[CatalogTemplate],
    maximum_questions: usize,
    id_json_bytes: usize,
) -> Result<usize> {
    if templates.is_empty() || maximum_questions == 0 || id_json_bytes < 2 {
        return Err(invalid("Empty question catalog shape"));
    }
    let add = |a: usize, b: usize| {
        a.checked_add(b)
            .ok_or_else(|| invalid("Question catalog size overflow"))
    };
    let mul = |a: usize, b: usize| {
        a.checked_mul(b)
            .ok_or_else(|| invalid("Question catalog size overflow"))
    };
    let mut fixed = Catalog {
        fragments: Vec::new(),
        spaces: Vec::new(),
        questions: Vec::new(),
    };
    let mut tail_max = 0;
    let mut refs_max = 1;
    for template in templates {
        let text = instruction(&template.question.question);
        let (prefix, tail) = if let Some((prefix, tail)) = text.split_once('\n') {
            (format!("{prefix}\n"), Some(tail))
        } else {
            (text.into(), None)
        };
        if tail.is_some_and(|text| text.contains('\n')) || template.tail_json_bytes < 2 {
            return Err(invalid("Question template requires a single-line tail"));
        }
        if let Some(tail) = tail {
            let actual = sapho_core::measured_json_bytes(&tail, usize::MAX)?;
            if actual > template.tail_json_bytes {
                return Err(invalid("Question template tail exceeds bound"));
            }
            tail_max = tail_max.max(template.tail_json_bytes);
            refs_max = 2;
        }
        if !fixed.fragments.contains(&prefix) {
            fixed.fragments.push(prefix);
        }
        let value = space(&template.question.question);
        if !fixed.spaces.contains(&value) {
            fixed.spaces.push(value);
        }
    }
    let max_index = index(add(fixed.fragments.len(), maximum_questions)?)?;
    let row = Row {
        id: String::new(),
        instructions: vec![max_index; refs_max],
        space: index(fixed.spaces.len())?,
    };
    let row_bytes = sapho_core::measured_json_bytes(&row, usize::MAX)?;
    let base = sapho_core::measured_json_bytes(&fixed, usize::MAX)?;
    // Existing empty arrays retain their brackets. One comma per inserted item
    // dominates both empty/nonempty prefixes; no first-item subtraction needed.
    add(
        base,
        add(
            mul(maximum_questions, add(tail_max, 1)?)?,
            mul(
                maximum_questions,
                add(
                    add(
                        row_bytes
                            .checked_sub(2)
                            .ok_or_else(|| invalid("Question row size underflow"))?,
                        id_json_bytes,
                    )?,
                    1,
                )?,
            )?,
        )?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Trace: FR-048-AC-13
    #[test]
    fn catalog_roundtrips_exact_order_meanings_unicode_controls_and_empty_fragments() {
        let instructions =
            "Shared complete guidance café🙂\nSource: FIELD fake\n\n\"quoted\" \\ \u{1}\n";
        let questions = vec![
            NamedQuestion {
                id: "b".into(),
                question: Question::Boolean {
                    instructions: instructions.into(),
                    yes: "meaning true".into(),
                    no: "meaning false".into(),
                },
            },
            NamedQuestion {
                id: "c".into(),
                question: Question::Choice {
                    instructions: instructions.into(),
                    options: vec![
                        ChoiceOption {
                            label: "yes".into(),
                            description: "full distinct meaning café".into(),
                        },
                        ChoiceOption {
                            label: "no".into(),
                            description: "Negative meaning".into(),
                        },
                    ],
                },
            },
            NamedQuestion {
                id: "s".into(),
                question: Question::Score {
                    instructions: "Original score criterion".into(),
                    levels: vec!["zero".into(), "one".into()],
                },
            },
        ];
        let c = Catalog::new(&questions).unwrap();
        let encoded = serde_json::to_vec(&c).unwrap();
        let parsed: Catalog = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(parsed.restore().unwrap(), questions);
        assert_eq!(
            parsed
                .fragments
                .iter()
                .filter(|p| p.as_str() == "Shared complete guidance café🙂\n")
                .count(),
            1
        );
    }
    /// Trace: FR-048-AC-13
    #[test]
    fn catalog_bounds_cover_distinct_ids_and_tails_and_refuse_invalid_shape() {
        let q = |id: String, tail: &str| NamedQuestion {
            id,
            question: Question::Choice {
                instructions: format!("Shared complete guidance\n{tail}"),
                options: vec![
                    ChoiceOption {
                        label: "yes".into(),
                        description: "Complete meaning".into(),
                    },
                    ChoiceOption {
                        label: "no".into(),
                        description: "Complete negative meaning".into(),
                    },
                ],
            },
        };
        let template = CatalogTemplate {
            question: q("shape".into(), ""),
            tail_json_bytes: 140,
        };
        let actual = (0..24)
            .map(|n| {
                q(
                    format!("original-{n}"),
                    &format!(r#"Distinct source {n}: café🙂 \"quoted\""#),
                )
            })
            .collect::<Vec<_>>();
        let bound = catalog_question_bound(std::slice::from_ref(&template), 24, 32).unwrap();
        let encoded = serde_json::to_vec(&Catalog::new(&actual).unwrap()).unwrap();
        assert!(encoded.len() <= bound);
        assert!(encoded.len() < serde_json::to_vec(&actual).unwrap().len());
        let mut bad = template;
        bad.question = q("shape".into(), "two\nlines");
        assert!(catalog_question_bound(&[bad], 24, 32).is_err());
    }
    /// Trace: FR-048-AC-13
    #[test]
    fn single_line_bound_covers_distinct_ids_spaces_unicode_and_controls() {
        let qs = (0..36)
            .map(|n| NamedQuestion {
                id: format!("original-{n}"),
                question: Question::Boolean {
                    instructions: format!(
                        "Full distinct instruction {n}: café🙂 \u{1} quoted \"meaning\""
                    ),
                    yes: format!("true {n}"),
                    no: format!("false {n}"),
                },
            })
            .collect::<Vec<_>>();
        let typed = sapho_core::measured_json_bytes(&qs, usize::MAX).unwrap();
        assert!(
            catalog_question_bytes(&qs).unwrap()
                <= catalog_single_line_bound(typed, qs.len()).unwrap()
        );
        assert!(catalog_single_line_bound(2, usize::MAX).is_err());
    }
}
