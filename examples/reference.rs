// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Run every graph reference example offline through real public APIs.
//! `cargo run --example reference` checks both encodings and their expected results.
use sapho::{core::*, graph::*, recording::*, runtime::*};
use std::{collections::BTreeMap, error::Error, sync::Arc};

type ExampleResult<T> = std::result::Result<T, Box<dyn Error>>;

/// A deterministic tutorial model, not a semantic classifier.
struct TutorialBackend;
#[async_trait::async_trait]
impl ModelBackend for TutorialBackend {
    async fn infer(&self, request: &ModelRequest) -> Result<ModelResponse> {
        request.validate()?;
        let mut answers = BTreeMap::new();
        for q in &request.questions {
            let answer = match &q.question {
                Question::Boolean { .. } => Answer::Boolean {
                    probability: Probability::new(0.8)?,
                },
                Question::Choice { options, .. } => {
                    let first = options.first().ok_or_else(|| {
                        SaphoError::new(ErrorCode::InvalidValue, "No choice options")
                    })?;
                    let rest = u32::try_from(options.len() - 1).map_err(|_| {
                        SaphoError::new(ErrorCode::InvalidValue, "Too many options")
                    })?;
                    Answer::Choice {
                        selected: first.label.clone(),
                        confidence: Probability::new(0.5)?,
                        probabilities: Some(
                            options
                                .iter()
                                .enumerate()
                                .map(|(i, o)| {
                                    Ok((
                                        o.label.clone(),
                                        Probability::new(if i == 0 {
                                            0.75
                                        } else {
                                            0.25 / f64::from(rest)
                                        })?,
                                    ))
                                })
                                .collect::<Result<_>>()?,
                        ),
                    }
                }
                Question::Score { levels, .. } => {
                    let last = u32::try_from(levels.len() - 1)
                        .map_err(|_| SaphoError::new(ErrorCode::InvalidValue, "Too many levels"))?;
                    let probabilities = (0..=last)
                        .map(|i| {
                            Ok((
                                i.to_string(),
                                Probability::new(if i == last {
                                    0.75
                                } else if i == 0 {
                                    0.25
                                } else {
                                    0.0
                                })?,
                            ))
                        })
                        .collect::<Result<_>>()?;
                    Answer::Score {
                        expected: f64::from(last) * 0.75,
                        confidence: Probability::new(0.5)?,
                        probabilities: Some(probabilities),
                    }
                }
            };
            answers.insert(q.id.clone(), answer);
        }
        Ok(ModelResponse {
            model: request.model.clone(),
            answers,
            usage: None,
        })
    }
}

/// Registered host code with exact ports and application-owned parameters.
struct TextLength;
impl Primitive for TextLength {
    fn signature(&self) -> Signature {
        Signature {
            inputs: BTreeMap::from([("text".into(), ValueType::Text)]),
            outputs: BTreeMap::from([("result".into(), ValueType::Number)]),
        }
    }
    fn execute(
        &self,
        context: &PrimitiveContext,
        inputs: &Inputs,
        params: &BTreeMap<String, Value>,
    ) -> Result<Inputs> {
        context.check_cancelled()?;
        if params.get("unit") != Some(&Value::Text("bytes".into())) {
            return Err(SaphoError::new(ErrorCode::CodeFailed, "Use unit=bytes"));
        }
        let Some(Value::Text(text)) = inputs.get("text").map(|d| &d.value) else {
            return Err(SaphoError::new(ErrorCode::TypeMismatch, "Expected text"));
        };
        let length = u32::try_from(text.len())
            .map_err(|_| SaphoError::new(ErrorCode::LimitExceeded, "Text too long"))?;
        Ok(BTreeMap::from([(
            "result".into(),
            Datum::new("length", Value::Number(f64::from(length)))?,
        )]))
    }
}

fn backends(backend: Arc<dyn ModelBackend>) -> Result<BackendRegistry> {
    let mut registry = BackendRegistry::default();
    registry.register(
        BackendId::new("judge")?,
        BackendBinding {
            backend,
            model: "tutorial-1".into(),
            expected_model: Some("tutorial-1".into()),
            distribution_policy: DistributionPolicy::Strict {},
        },
    )?;
    Ok(registry)
}
fn inputs(spec: &GraphSpec, plain: &serde_json::Value) -> Result<Inputs> {
    spec.inputs
        .iter()
        .map(|(name, ty)| {
            let value = plain
                .get(name)
                .ok_or_else(|| SaphoError::new(ErrorCode::MissingInput, "Example input absent"))?;
            Ok((name.clone(), decode_plain(name, ty, value, &[])?))
        })
        .collect()
}
fn check_value(actual: &serde_json::Value, expected: &serde_json::Value) {
    match (actual, expected) {
        (serde_json::Value::Number(a), serde_json::Value::Number(b)) => {
            assert!(
                (a.as_f64().unwrap_or(f64::INFINITY) - b.as_f64().unwrap_or(f64::NEG_INFINITY))
                    .abs()
                    < 1e-12
            );
        }
        (serde_json::Value::Array(a), serde_json::Value::Array(b)) => {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                check_value(a, b);
            }
        }
        (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
            assert_eq!(a.len(), b.len());
            for (k, b) in b {
                check_value(a.get(k).unwrap_or(&serde_json::Value::Null), b);
            }
        }
        _ => assert_eq!(actual, expected),
    }
}
fn check_outputs(run: &RunResult, expected: &serde_json::Value) -> ExampleResult<()> {
    let plain = run
        .outputs
        .iter()
        .map(|(k, d)| Ok((k.clone(), d.value.to_plain_json()?)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    check_value(&serde_json::to_value(plain)?, expected);
    Ok(())
}

fn distributions() -> ExampleResult<()> {
    let questions = vec![NamedQuestion {
        id: "role".into(),
        question: Question::Choice {
            instructions: "Which role?".into(),
            options: vec![
                ChoiceOption {
                    label: "actor".into(),
                    description: "Actor".into(),
                },
                ChoiceOption {
                    label: "other".into(),
                    description: "Other".into(),
                },
            ],
        },
    }];
    let mut answers = Answers {
        questions,
        distribution_policy: DistributionPolicy::Strict {},
        values: BTreeMap::from([(
            "role".into(),
            Answer::Choice {
                selected: "actor".into(),
                confidence: Probability::new(0.6)?,
                probabilities: Some(BTreeMap::from([
                    ("actor".into(), Probability::new(0.7)?),
                    ("other".into(), Probability::new(0.29)?),
                ])),
            },
        )]),
    };
    assert_eq!(
        answers.validate().err().map(|e| e.code),
        Some(ErrorCode::InvalidAnswer)
    );
    answers.distribution_policy = DistributionPolicy::approximate(0.02)?;
    answers.validate()?;
    assert_eq!(
        answers.distribution_state("role")?,
        DistributionState::Approximate
    );
    assert!((answers.probability("role", &["actor".into()])?.get() - 0.7 / 0.99).abs() < 1e-12);
    assert!(answers.distribution_adjustment("role")?.is_some());
    if let Some(Answer::Choice { probabilities, .. }) = answers.values.get_mut("role") {
        *probabilities = Some(BTreeMap::from([("actor".into(), Probability::new(0.7)?)]));
    }
    assert_eq!(
        answers.distribution_state("role")?,
        DistributionState::Partial
    );
    assert_eq!(
        answers
            .probability("role", &["other".into()])
            .err()
            .map(|e| e.code),
        Some(ErrorCode::UnsupportedDistribution)
    );
    if let Some(Answer::Choice { probabilities, .. }) = answers.values.get_mut("role") {
        *probabilities = None;
    }
    assert_eq!(
        answers.distribution_state("role")?,
        DistributionState::Unavailable
    );
    Ok(())
}

#[tokio::main]
async fn main() -> ExampleResult<()> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!("reference/cases.json"))?;
    let mut primitives = PrimitiveRegistry::default();
    primitives.register(PrimitiveId::new("text_length")?, Arc::new(TextLength))?;
    distributions()?;
    for case in cases {
        let name = case
            .get("name")
            .and_then(serde_json::Value::as_str)
            .ok_or("Missing example name")?;
        // Acquisition is explicit; no filesystem I/O occurs inside model inference.
        let yaml = std::fs::read_to_string(root.join(format!("examples/reference/{name}.yaml")))?;
        let json = std::fs::read_to_string(root.join(format!("examples/reference/{name}.json")))?;
        let spec = GraphSpec::parse(&yaml)?;
        assert_eq!(
            spec,
            GraphSpec::parse_with_format(&json, GraphFormat::Json)?
        );
        let recorder = Arc::new(RecordingBackend::new(Arc::new(TutorialBackend), 4_000_000)?);
        let engine = Engine::new(compile(&spec, &primitives)?, backends(recorder.clone())?)?;
        let supplied = inputs(&spec, &case["input"])?;
        let run = engine.run(&supplied, RunLimits::default()).await?;
        check_outputs(&run, &case["expected"])?;
        if name == "guarded-model" {
            assert!(recorder.snapshot()?.exchanges.is_empty());
        }
        if let Some(alternate) = case.get("alternate") {
            let run = engine
                .run(&inputs(&spec, &alternate["input"])?, RunLimits::default())
                .await?;
            check_outputs(&run, &alternate["expected"])?;
            assert!(
                run.trace
                    .nodes
                    .iter()
                    .any(|n| n.status == NodeStatus::Completed)
            );
        }
        let recording = recorder.snapshot()?;
        let bytes = recording.to_json(4_000_000)?;
        let loaded = Recording::from_json(&bytes, 4_000_000)?;
        let replay = Engine::new(
            compile(&spec, &primitives)?,
            backends(Arc::new(ReplayBackend::new(&loaded, 4_000_000)?))?,
        )?;
        let replayed = replay.run(&supplied, RunLimits::default()).await?;
        assert_eq!(replayed.outputs, run.outputs);
        if case.get("backend").and_then(serde_json::Value::as_bool) == Some(true) {
            assert!(!recording.exchanges.is_empty());
            let empty = ReplayBackend::new(&Recording::default(), 4_000_000)?;
            let failure = empty
                .infer(&recording.exchanges.first().ok_or("No exchange")?.request)
                .await;
            assert_eq!(failure.err().map(|e| e.code), Some(ErrorCode::ReplayMiss));
        }
        if name == "guards" {
            assert!(
                run.trace
                    .nodes
                    .iter()
                    .any(|n| n.status == NodeStatus::Skipped)
            );
        }
        if name == "collections" {
            let limited = RunLimits {
                collection_items: 1,
                ..RunLimits::default()
            };
            let failure = engine
                .run(&supplied, limited)
                .await
                .err()
                .ok_or("Expected limit failure")?;
            assert_eq!(failure.error.code, ErrorCode::LimitExceeded);
            assert!(!failure.trace.nodes.is_empty());
            let mut mismatched = spec.clone();
            if let Some(n) = mismatched
                .nodes
                .iter_mut()
                .find(|n| n.id.as_str() == "selected")
            {
                n.inputs.insert(
                    "mask".into(),
                    Binding::Input {
                        name: "right".into(),
                        path: vec![],
                    },
                );
            }
            assert_eq!(
                compile(&mismatched, &primitives).err().map(|e| e.code),
                Some(ErrorCode::TypeMismatch)
            );
        }
        println!("{name}: expected outputs, YAML/JSON equivalence and replay verified");
    }
    println!("distribution policies and structured failures verified");
    Ok(())
}
