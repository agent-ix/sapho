// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Run every graph reference example offline through real public APIs.
//! `cargo run --example reference` checks both encodings and their expected results.
use sapho::{core::*, graph::*, recording::*, runtime::*};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    path::Path,
    sync::Arc,
};

type ExampleResult<T> = std::result::Result<T, Box<dyn Error>>;

/// One entry of `examples/reference/cases.json`. Input files sit beside the graphs.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CaseEntry {
    /// Graph file stem: `NAME.yaml` and `NAME.json`.
    name: String,
    /// Plain JSON input file.
    input: String,
    /// Plain JSON outputs.
    expected: serde_json::Value,
    /// Trace paths of guarded nodes that skip; every other node completes.
    #[serde(default)]
    skipped: Vec<Vec<String>>,
    /// The graph asks a model, so the stock CLI needs a live binding to run it.
    #[serde(default)]
    backend: bool,
    /// The graph calls registered Rust code, so the stock CLI cannot run it.
    #[serde(default)]
    native: bool,
    /// A second input that takes the other guard branch.
    alternate: Option<AlternateEntry>,
}
/// A further run of the same graph.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AlternateEntry {
    input: String,
    expected: serde_json::Value,
    #[serde(default)]
    skipped: Vec<Vec<String>>,
}
/// A reference case with its graph and inputs already read from disk.
struct Case {
    name: String,
    spec: GraphSpec,
    backend: bool,
    /// The first run is the case's own input; it is also recorded and replayed.
    runs: Vec<Run>,
}
struct Run {
    input: serde_json::Value,
    expected: serde_json::Value,
    skipped: Vec<Vec<String>>,
}

/// The reference case that runs each operation, comparator and reducer. No arm
/// is a wildcard, so a new variant does not compile until it names a case, and
/// `check_coverage` confirms that case's graph uses it.
fn operation_case(operation: &Operation) -> &'static str {
    match operation {
        Operation::Record {}
        | Operation::List { .. }
        | Operation::And
        | Operation::Or
        | Operation::Not => "facts",
        Operation::Compare { comparator } => match comparator {
            Comparator::Equal
            | Comparator::Less
            | Comparator::LessEqual
            | Comparator::Greater
            | Comparator::GreaterEqual => "facts",
        },
        Operation::Degree | Operation::Complement => "strengths",
        Operation::Reduce { reducer, .. } => match reducer {
            Reducer::Min | Reducer::Max | Reducer::WeightedMean => "strengths",
        },
        Operation::Map { .. }
        | Operation::Filter
        | Operation::Pairs
        | Operation::Join { .. }
        | Operation::Collect => "collections",
        Operation::Coalesce => "guards",
        Operation::Code { .. } => "native",
        Operation::Questions { .. } | Operation::Ask { .. } | Operation::Probability { .. } => {
            "questions"
        }
    }
}
/// The reference case that asks each question kind, exhaustive like `operation_case`.
fn question_case(question: &Question) -> &'static str {
    match question {
        Question::Boolean { .. } | Question::Choice { .. } | Question::Score { .. } => "questions",
    }
}
/// Serialized identity of a configured feature, such as `compare less` or `boolean`.
fn wire_name(value: &impl serde::Serialize) -> ExampleResult<String> {
    let wire = serde_json::to_value(value)?;
    Ok(["kind", "comparator", "reducer"]
        .iter()
        .filter_map(|field| wire.get(field)?.as_str())
        .collect::<Vec<_>>()
        .join(" "))
}
/// Root and reusable-definition nodes of a graph.
fn nodes(spec: &GraphSpec) -> impl Iterator<Item = &NodeSpec> {
    spec.nodes
        .iter()
        .chain(spec.subgraphs.values().flat_map(|body| &body.nodes))
}
/// Check that each feature a reference graph uses appears in the case named for it.
fn check_coverage(cases: &[Case]) -> ExampleResult<()> {
    let mut used = BTreeMap::<&str, BTreeSet<String>>::new();
    let mut claims = BTreeSet::new();
    for case in cases {
        for node in nodes(&case.spec) {
            let mut features = vec![(wire_name(&node.operation)?, operation_case(&node.operation))];
            if let Operation::Questions { questions } = &node.operation {
                for named in questions {
                    features.push((
                        format!("question {}", wire_name(&named.question)?),
                        question_case(&named.question),
                    ));
                }
            }
            for (feature, claimed) in features {
                used.entry(&case.name).or_default().insert(feature.clone());
                claims.insert((feature, claimed));
            }
        }
    }
    for (feature, case) in claims {
        if !used
            .get(case)
            .is_some_and(|features| features.contains(&feature))
        {
            return Err(
                format!("`{feature}` names reference case {case}, which does not use it").into(),
            );
        }
    }
    Ok(())
}
/// Read every reference graph and input file before evaluation starts.
fn load(directory: &Path) -> ExampleResult<Vec<Case>> {
    let read = |file: &str| std::fs::read_to_string(directory.join(file));
    let read_json = |file: &str| -> ExampleResult<serde_json::Value> {
        Ok(serde_json::from_str(&read(file)?)?)
    };
    let entries: Vec<CaseEntry> = serde_json::from_str(&read("cases.json")?)?;
    entries
        .into_iter()
        .map(|entry| {
            let spec = GraphSpec::parse(&read(&format!("{}.yaml", entry.name))?)?;
            let json = read(&format!("{}.json", entry.name))?;
            assert_eq!(
                spec,
                GraphSpec::parse_with_format(&json, GraphFormat::Json)?
            );
            // The CLI recipe checker skips flagged cases, so the flags must match the graph.
            assert_eq!(
                entry.backend,
                nodes(&spec).any(|n| matches!(n.operation, Operation::Ask { .. }))
            );
            assert_eq!(
                entry.native,
                nodes(&spec).any(|n| matches!(n.operation, Operation::Code { .. }))
            );
            let mut runs = vec![Run {
                input: read_json(&entry.input)?,
                expected: entry.expected,
                skipped: entry.skipped,
            }];
            if let Some(alternate) = entry.alternate {
                runs.push(Run {
                    input: read_json(&alternate.input)?,
                    expected: alternate.expected,
                    skipped: alternate.skipped,
                });
            }
            Ok(Case {
                name: entry.name,
                spec,
                backend: entry.backend,
                runs,
            })
        })
        .collect()
}

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
            raw: None,
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
/// Every traced node completes except the listed guarded nodes, which skip.
fn check_statuses(trace: &Trace, skipped: &[Vec<String>]) {
    for node in &trace.nodes {
        let expected = if skipped.contains(&node.path) {
            NodeStatus::Skipped
        } else {
            NodeStatus::Completed
        };
        assert_eq!(node.status, expected, "{:?}", node.path);
    }
    assert!(
        skipped
            .iter()
            .all(|path| trace.nodes.iter().any(|n| &n.path == path))
    );
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

/// Evaluate every case through the engine, recording and replaying its model calls.
async fn evaluate(cases: &[Case]) -> ExampleResult<()> {
    let mut primitives = PrimitiveRegistry::default();
    primitives.register(PrimitiveId::new("text_length")?, Arc::new(TextLength))?;
    for case in cases {
        let (name, spec) = (case.name.as_str(), &case.spec);
        let recorder = Arc::new(RecordingBackend::new(Arc::new(TutorialBackend), 4_000_000)?);
        let engine = Engine::new(compile(spec, &primitives)?, backends(recorder.clone())?)?;
        let first = case.runs.first().ok_or("Case has no run")?;
        let supplied = inputs(spec, &first.input)?;
        let mut outputs = Vec::new();
        for (index, run) in case.runs.iter().enumerate() {
            let result = engine
                .run(&inputs(spec, &run.input)?, RunLimits::default())
                .await?;
            check_outputs(&result, &run.expected)?;
            check_statuses(&result.trace, &run.skipped);
            if name == "guarded-model" && index == 0 {
                // The disabled guard skips the ask, so no model call is made.
                assert!(recorder.snapshot()?.exchanges.is_empty());
            }
            outputs.push(result.outputs);
        }
        let recording = recorder.snapshot()?;
        let bytes = recording.to_json(4_000_000)?;
        let loaded = Recording::from_json(&bytes, 4_000_000)?;
        let replay = Engine::new(
            compile(spec, &primitives)?,
            backends(Arc::new(ReplayBackend::new(&loaded, 4_000_000)?))?,
        )?;
        let replayed = replay.run(&supplied, RunLimits::default()).await?;
        assert_eq!(Some(&replayed.outputs), outputs.first());
        if case.backend {
            assert!(!recording.exchanges.is_empty());
            let empty = ReplayBackend::new(&Recording::default(), 4_000_000)?;
            let failure = empty
                .infer(&recording.exchanges.first().ok_or("No exchange")?.request)
                .await;
            assert_eq!(failure.err().map(|e| e.code), Some(ErrorCode::ReplayMiss));
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
    Ok(())
}

fn main() -> ExampleResult<()> {
    // Acquisition is explicit and synchronous: every file is read here, before
    // the async runtime starts, so no blocking I/O runs on a Tokio worker.
    let cases = load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/reference"))?;
    check_coverage(&cases)?;
    distributions()?;
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(evaluate(&cases))?;
    println!("feature coverage, distribution policies and structured failures verified");
    Ok(())
}
