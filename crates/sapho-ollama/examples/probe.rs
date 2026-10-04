// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Explicit synthetic local-service probe; never runs during default tests.
use sapho_core::{
    BackendId, DistributionPolicy, ModelBackend, ModelRequest, NamedQuestion, Question, Value,
    validate_response,
};
use sapho_ollama::{Limits, OllamaBackend};
use std::collections::BTreeMap;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let model = std::env::args().nth(1).ok_or("explicit model required")?;
    let backend = OllamaBackend::new(
        "http://127.0.0.1:11434",
        Limits {
            think: Some(false),
            output_tokens: 512,
            ..Limits::default()
        },
    )?;
    let request = ModelRequest {
        backend: BackendId::new("judge")?,
        model: model.clone(),
        expected_model: Some(model),
        distribution_policy: DistributionPolicy::Strict {},
        state: Value::Record(BTreeMap::from([(
            "sentence".into(),
            Value::Text("The service shall retain the receipt.".into()),
        )])),
        questions: vec![
            NamedQuestion {
                id: "obligation".into(),
                question: Question::Boolean {
                    instructions: "Does the source sentence express an obligation?".into(),
                    yes: "Expresses obligation".into(),
                    no: "Does not express obligation".into(),
                },
            },
            NamedQuestion {
                id: "kind".into(),
                question: Question::Choice {
                    instructions: "Select the sentence kind.".into(),
                    options: vec![
                        sapho_core::ChoiceOption {
                            label: "obligation".into(),
                            description: "Requirement".into(),
                        },
                        sapho_core::ChoiceOption {
                            label: "informative".into(),
                            description: "Informative prose".into(),
                        },
                    ],
                },
            },
            NamedQuestion {
                id: "clarity".into(),
                question: Question::Score {
                    instructions: "Rate clarity from 0 to 2.".into(),
                    levels: vec!["unclear".into(), "partly clear".into(), "clear".into()],
                },
            },
        ],
    };
    let response = backend.infer(&request).await?;
    validate_response(&request, &response)?;
    println!("{}", serde_json::to_string(&response)?);
    println!("raw_receipts={}", backend.receipts()?.len());
    Ok(())
}
