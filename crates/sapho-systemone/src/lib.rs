// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! One source-free request translator for verified System One compatible providers.
use sapho_core::{ModelRequest, Question, Result};
use typesafe_sdk_client::SystemOneRequest;
use typesafe_sdk_questions::{Entry, NoulCriteria, Question as SdkQuestion, Questions};

/// Default model alias declared by the CLM System One provider.
pub const CLM_DEFAULT_MODEL: &str = "clm-latest";

/// Translate core state/questions into the exact SDK request, without transport work.
pub fn build_request(request: &ModelRequest) -> Result<SystemOneRequest> {
    request.validate()?;
    let mut questions = Questions::new();
    for q in &request.questions {
        let question = match &q.question {
            Question::Boolean {
                instructions,
                yes,
                no,
            } => SdkQuestion::Noul {
                instructions: Some(Entry::from(instructions.clone())),
                criteria: Some(NoulCriteria {
                    yes: Some(Entry::from(yes.clone())),
                    no: Some(Entry::from(no.clone())),
                }),
            },
            Question::Choice {
                instructions,
                options,
            } => SdkQuestion::Choice {
                instructions: Some(Entry::from(instructions.clone())),
                criteria: options
                    .iter()
                    .map(|o| (o.label.clone(), Entry::from(o.description.clone())))
                    .collect(),
            },
            Question::Score {
                instructions,
                levels,
            } => SdkQuestion::Score {
                instructions: Some(Entry::from(instructions.clone())),
                criteria: levels.iter().cloned().map(Entry::from).collect(),
            },
        };
        questions.insert(q.id.clone(), question);
    }
    Ok(
        SystemOneRequest::new(Entry::from(request.state.to_plain_json()?), questions)
            .model(&request.model),
    )
}
