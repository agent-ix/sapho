// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Live check against a running Ollama; not part of the default tests.
//!
//! `cargo run -p sapho-ollama --example live_smoke` sends about nine calls (each reads the
//! model description first) to `OLLAMA_URL` (default `http://127.0.0.1:11434`) using
//! `OLLAMA_MODEL` (default `qwen3:30b`) and `OLLAMA_EMBED_MODEL` (default `qwen3-embedding:4b`),
//! then prints seconds and tokens per call.
use sapho_core::{
    BackendId, ChoiceOption, DistributionPolicy, ExtractRequest, ModelBackend, ModelRequest,
    NamedQuestion, Question, Value, extract,
};
use sapho_ollama::{DEFAULT_BASE_URL, Limits, OllamaBackend, OllamaEmbedder, Server, Settings};
use serde_json::json;
use std::{collections::BTreeMap, env, error::Error, time::Instant};

fn settings(model: &str, think: bool, num_ctx: u64) -> Settings {
    Settings {
        model: model.into(),
        think,
        num_ctx,
        num_predict: 512,
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let url = env::var("OLLAMA_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.into());
    let model = env::var("OLLAMA_MODEL").unwrap_or_else(|_| "qwen3:30b".into());
    let embed_model =
        env::var("OLLAMA_EMBED_MODEL").unwrap_or_else(|_| "qwen3-embedding:4b".into());
    let server = Server::new(&url, Limits::default())?;
    let schema = json!({
        "type": "object",
        "properties": {"fruit": {"type": "boolean"}, "colour": {"type": "string"}},
        "required": ["fruit", "colour"],
        "additionalProperties": false
    });
    let extract_request = |input: &str| ExtractRequest {
        model: model.clone(),
        instructions: "Say whether the item is a fruit and name its usual colour.".into(),
        input: input.into(),
        schema: schema.clone(),
    };

    let backend = OllamaBackend::new(server.clone(), settings(&model, false, 8192))?;
    for item in ["banana", "carrot", "strawberry"] {
        let started = Instant::now();
        match extract(&backend, &extract_request(item)).await {
            Ok(r) => println!(
                "extract {item}: {:.2}s wall, {:?} ms server, in {:?} out {:?} tokens, model {}, value {}",
                started.elapsed().as_secs_f64(),
                r.usage.elapsed_ms,
                r.usage.input_tokens,
                r.usage.output_tokens,
                r.model.name,
                r.value
            ),
            Err(e) => println!(
                "extract {item}: {:?} {:?} {:?}",
                e.code, e.reason, e.too_large
            ),
        }
    }

    let thinking = OllamaBackend::new(server.clone(), settings(&model, true, 8192))?;
    let started = Instant::now();
    match extract(&thinking, &extract_request("banana")).await {
        Ok(r) => println!(
            "extract think=true: ok {} in {:.2}s",
            r.value,
            started.elapsed().as_secs_f64()
        ),
        Err(e) => println!(
            "extract think=true: {:?} {:?} in {:.2}s",
            e.code,
            e.reason,
            started.elapsed().as_secs_f64()
        ),
    }

    let small = OllamaBackend::new(server.clone(), settings(&model, false, 1024))?;
    let started = Instant::now();
    let big = extract_request(&"word ".repeat(20_000));
    match extract(&small, &big).await {
        Ok(_) => println!("oversize: unexpectedly accepted"),
        Err(e) => println!(
            "oversize: {:?} {:?} in {:.2}s",
            e.code,
            e.too_large,
            started.elapsed().as_secs_f64()
        ),
    }

    let questions = vec![
        NamedQuestion {
            id: "fruit".into(),
            question: Question::Boolean {
                instructions: "Is the item a fruit?".into(),
                yes: "it is a fruit".into(),
                no: "it is not a fruit".into(),
            },
        },
        NamedQuestion {
            id: "colour".into(),
            question: Question::Choice {
                instructions: "What is the item's usual colour?".into(),
                options: ["yellow", "orange", "red", "green"]
                    .iter()
                    .map(|c| ChoiceOption {
                        label: (*c).into(),
                        description: format!("usually {c}"),
                    })
                    .collect(),
            },
        },
    ];
    for item in ["banana", "carrot"] {
        let request = ModelRequest {
            distribution_policy: DistributionPolicy::approximate(0.05)?,
            backend: BackendId::new("ollama")?,
            model: model.clone(),
            expected_model: None,
            state: Value::Record(BTreeMap::from([("item".into(), Value::Text(item.into()))])),
            questions: questions.clone(),
        };
        let started = Instant::now();
        match backend.infer(&request).await {
            Ok(r) => println!(
                "ask {item}: {:.2}s wall, usage {:?}, answers {}",
                started.elapsed().as_secs_f64(),
                r.usage,
                serde_json::to_string(&r.answers)?
            ),
            Err(e) => println!("ask {item}: {:?} {:?}", e.code, e.context),
        }
    }

    let embedder = OllamaEmbedder::new(server, embed_model, 32)?;
    let started = Instant::now();
    let inputs = vec!["a ripe banana".to_string(), "a raw carrot".to_string()];
    let embedded = embedder.embed(&inputs).await?;
    println!(
        "embed: {:.2}s, {} vectors of {}, in {:?} tokens",
        started.elapsed().as_secs_f64(),
        embedded.vectors.len(),
        embedded.vectors[0].len(),
        embedded.input_tokens
    );
    let started = Instant::now();
    match embedder.embed(&["word ".repeat(400_000)]).await {
        Ok(_) => println!("embed oversize: accepted"),
        Err(e) => println!(
            "embed oversize: {:?} {:?} in {:.2}s",
            e.code,
            e.reason,
            started.elapsed().as_secs_f64()
        ),
    }
    Ok(())
}
