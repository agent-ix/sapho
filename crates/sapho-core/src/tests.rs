// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Offline acceptance tests for the core value and extension contracts.
use super::*;
use std::{
    collections::BTreeMap,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

fn p(v: f64) -> Probability {
    Probability::new(v).unwrap()
}
fn choice() -> NamedQuestion {
    NamedQuestion {
        id: "q".into(),
        question: Question::Choice {
            instructions: "Select the described class".into(),
            options: vec![
                ChoiceOption {
                    label: "z".into(),
                    description: "z class".into(),
                },
                ChoiceOption {
                    label: "a".into(),
                    description: "a class".into(),
                },
            ],
        },
    }
}
fn request(q: Vec<NamedQuestion>) -> ModelRequest {
    ModelRequest {
        distribution_policy: DistributionPolicy::Strict {},
        backend: BackendId::new("judge").unwrap(),
        model: "model-1".into(),
        expected_model: None,
        state: Value::Record(BTreeMap::new()),
        questions: q,
    }
}
fn response(a: Answer) -> ModelResponse {
    ModelResponse {
        model: "model-1".into(),
        digest: None,
        raw: None,
        answers: BTreeMap::from([("q".into(), a)]),
        usage: None,
    }
}

/// Trace: FR-001-AC-1, FR-001-AC-2, FR-001-AC-3
#[test]
fn value_boundaries_preserve_types_and_refuse_invalid_data() {
    let d = Datum::new(
        "one",
        Value::Record(BTreeMap::from([(
            "text".into(),
            Value::Text("same".into()),
        )])),
    )
    .unwrap();
    let v = Value::List(vec![d.clone()]);
    let bytes = serde_json::to_vec(&v).unwrap();
    assert_eq!(serde_json::from_slice::<Value>(&bytes).unwrap(), v);
    let ty = ValueType::list(ValueType::Record {
        fields: BTreeMap::from([("text".into(), ValueType::Text)]),
    });
    ty.check(&v).unwrap();
    assert_eq!(
        Value::List(vec![d.clone(), d]).validate().unwrap_err().code,
        ErrorCode::DuplicateId
    );
    for n in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            Value::Number(n).validate().unwrap_err().code,
            ErrorCode::InvalidValue
        );
    }
    for n in [-0.1, 1.1, f64::NAN] {
        assert!(Probability::new(n).is_err());
        assert!(Degree::new(n).is_err());
    }
    assert!(serde_json::from_str::<Probability>("1.1").is_err());
    assert_eq!(
        ValueType::Number
            .check(&Value::Optional(None))
            .unwrap_err()
            .code,
        ErrorCode::TypeMismatch
    );
    ValueType::optional(ValueType::Number)
        .check(&Value::Optional(None))
        .unwrap();
    let mut nested = Value::Boolean(false);
    for _ in 0..32 {
        nested = Value::Optional(Some(Box::new(nested)));
    }
    assert_eq!(
        nested.validate().unwrap_err().code,
        ErrorCode::LimitExceeded
    );
    let source = SourceRef {
        source: SourceId::new("synthetic").unwrap(),
        start: Some(5),
        end: Some(4),
    };
    assert_eq!(source.validate().unwrap_err().code, ErrorCode::InvalidValue);
    let wrong = Value::Record(BTreeMap::from([("extra".into(), Value::Text("x".into()))]));
    assert_eq!(
        ValueType::Record {
            fields: BTreeMap::new()
        }
        .check(&wrong)
        .unwrap_err()
        .code,
        ErrorCode::TypeMismatch
    );
}
/// Trace: FR-002-AC-1, FR-002-AC-2, FR-002-AC-3
#[test]
fn repeated_text_keeps_occurrences_and_source_unions() {
    let first = SourceRef {
        source: SourceId::new("sentence").unwrap(),
        start: Some(0),
        end: Some(3),
    };
    let second = SourceRef {
        source: SourceId::new("sentence").unwrap(),
        start: Some(8),
        end: Some(11),
    };
    let mut a = Datum::new("first", Value::Text("API".into())).unwrap();
    a.sources.push(first.clone());
    let mut b = Datum::new("second", Value::Text("API".into())).unwrap();
    b.sources.push(second.clone());
    Value::List(vec![a.clone(), b.clone()]).validate().unwrap();
    assert_ne!(a.id, b.id);
    a.inherit_sources([first.clone(), second.clone()]);
    assert_eq!(a.sources, vec![first, second]);
    assert_eq!(b.sources.len(), 1);
}
struct Identity;
impl Primitive for Identity {
    fn signature(&self) -> Signature {
        Signature {
            inputs: BTreeMap::from([("value".into(), ValueType::Text)]),
            outputs: BTreeMap::from([("result".into(), ValueType::Text)]),
        }
    }
    fn execute(
        &self,
        ctx: &PrimitiveContext,
        inputs: &Inputs,
        _: &BTreeMap<String, Value>,
    ) -> Result<Inputs> {
        ctx.check_cancelled()?;
        Ok(Inputs::from([(
            "result".into(),
            inputs.get("value").unwrap().clone(),
        )]))
    }
}
/// Trace: FR-003-AC-1, FR-003-AC-2, FR-006-AC-2
#[test]
fn registry_collisions_never_replace_the_first_binding() {
    let id = PrimitiveId::new("identity").unwrap();
    let mut registry = PrimitiveRegistry::default();
    registry.register(id.clone(), Arc::new(Identity)).unwrap();
    assert_eq!(
        registry
            .register(id.clone(), Arc::new(Identity))
            .unwrap_err()
            .code,
        ErrorCode::DuplicateId
    );
    let implementation = registry.get(&id).unwrap();
    let context = PrimitiveContext::new(
        Instant::now() + Duration::from_secs(5),
        Arc::new(AtomicBool::new(false)),
    );
    let inputs = Inputs::from([(
        "value".into(),
        Datum::new("x", Value::Text("exact".into())).unwrap(),
    )]);
    let out = implementation
        .execute(&context, &inputs, &BTreeMap::new())
        .unwrap();
    assert_eq!(out["result"].value, Value::Text("exact".into()));
    let mut backends = BackendRegistry::default();
    let bid = BackendId::new("judge").unwrap();
    let b = BackendBinding {
        distribution_policy: DistributionPolicy::Strict {},
        backend: Arc::new(Refusing),
        model: "model-1".into(),
        expected_model: None,
    };
    backends.register(bid.clone(), b.clone()).unwrap();
    assert_eq!(
        backends.register(bid, b).unwrap_err().code,
        ErrorCode::DuplicateId
    );
}
/// Trace: FR-004-AC-1, FR-004-AC-2
#[test]
fn typed_question_spaces_keep_order_and_validate_cardinality() {
    let q = vec![
        choice(),
        NamedQuestion {
            id: "b".into(),
            question: Question::Boolean {
                instructions: "Is the condition true?".into(),
                yes: "true".into(),
                no: "false".into(),
            },
        },
        NamedQuestion {
            id: "s".into(),
            question: Question::Score {
                instructions: "Rate the severity".into(),
                levels: vec!["none".into(), "high".into()],
            },
        },
    ];
    validate_questions(&q).unwrap();
    let encoded = serde_json::to_vec(&q).unwrap();
    assert_eq!(
        serde_json::from_slice::<Vec<NamedQuestion>>(&encoded).unwrap(),
        q
    );
    assert_eq!(q[0].question.labels(), vec!["z", "a"]);
    assert!(validate_questions(&[choice(), choice()]).is_err());
    let mut bad = choice();
    if let Question::Choice { options, .. } = &mut bad.question {
        options[1].label = "z".into();
    }
    assert!(validate_questions(&[bad]).is_err());
    for labels in [vec!["only"], vec![" ", "valid"]] {
        let bad = NamedQuestion {
            id: "bad".into(),
            question: Question::Choice {
                instructions: "classify".into(),
                options: labels
                    .into_iter()
                    .map(|label| ChoiceOption {
                        label: label.into(),
                        description: "synthetic option".into(),
                    })
                    .collect(),
            },
        };
        assert_eq!(
            validate_questions(&[bad]).unwrap_err().code,
            ErrorCode::InvalidValue
        );
    }
    assert!(
        validate_questions(&[NamedQuestion {
            id: "x".into(),
            question: Question::Score {
                instructions: "rate".into(),
                levels: vec!["one".into()]
            }
        }])
        .is_err()
    );
}
/// Trace: FR-005-AC-1, FR-005-AC-2, FR-005-AC-3, FR-020-AC-1, FR-020-AC-2, FR-020-AC-3
#[test]
fn answers_distinguish_complete_partial_unavailable_and_expected_score() {
    let req = request(vec![choice()]);
    let complete = response(Answer::Choice {
        selected: "z".into(),
        confidence: p(0.7),
        probabilities: Some(BTreeMap::from([("z".into(), p(0.7)), ("a".into(), p(0.3))])),
    });
    let answers = validate_response(&req, &complete).unwrap();
    assert_eq!(
        answers.distribution_state("q").unwrap(),
        DistributionState::Complete
    );
    assert_eq!(
        answers
            .probability("q", &["z".into(), "a".into()])
            .unwrap()
            .get(),
        1.0
    );
    let partial = response(Answer::Choice {
        selected: "z".into(),
        confidence: p(0.7),
        probabilities: Some(BTreeMap::from([("z".into(), p(0.7))])),
    });
    let answers = validate_response(&req, &partial).unwrap();
    assert_eq!(
        answers.distribution_state("q").unwrap(),
        DistributionState::Partial
    );
    assert_eq!(
        answers.probability("q", &["a".into()]).unwrap_err().code,
        ErrorCode::UnsupportedDistribution
    );
    let absent = response(Answer::Choice {
        selected: "z".into(),
        confidence: p(0.7),
        probabilities: None,
    });
    assert_eq!(
        validate_response(&req, &absent)
            .unwrap()
            .distribution_state("q")
            .unwrap(),
        DistributionState::Unavailable
    );
    let mut bad = complete.clone();
    bad.answers.insert(
        "extra".into(),
        Answer::Boolean {
            probability: p(0.2),
        },
    );
    assert!(validate_response(&req, &bad).is_err());
    bad = complete.clone();
    bad.answers.insert(
        "q".into(),
        Answer::Boolean {
            probability: p(0.2),
        },
    );
    assert_eq!(
        validate_response(&req, &bad).unwrap_err().code,
        ErrorCode::InvalidAnswer
    );
    bad = response(Answer::Choice {
        selected: "unknown".into(),
        confidence: p(0.7),
        probabilities: None,
    });
    assert!(validate_response(&req, &bad).is_err());
    bad = response(Answer::Choice {
        selected: "z".into(),
        confidence: p(0.7),
        probabilities: Some(BTreeMap::from([("z".into(), p(0.7)), ("a".into(), p(0.7))])),
    });
    assert!(validate_response(&req, &bad).is_err());
    let invalid_answers = Answers {
        distribution_policy: DistributionPolicy::Strict {},
        questions: req.questions.clone(),
        values: bad.answers,
    };
    assert_eq!(
        invalid_answers.distribution_state("q").unwrap_err().code,
        ErrorCode::InvalidAnswer
    );
    let qr = request(vec![NamedQuestion {
        id: "q".into(),
        question: Question::Score {
            instructions: "rate".into(),
            levels: vec!["none".into(), "low".into(), "high".into()],
        },
    }]);
    let score = response(Answer::Score {
        expected: 1.4,
        confidence: p(0.6),
        probabilities: None,
    });
    assert_eq!(
        validate_response(&qr, &score).unwrap().values["q"],
        score.answers["q"]
    );
    let qr = request(vec![NamedQuestion {
        id: "q".into(),
        question: Question::Boolean {
            instructions: "true?".into(),
            yes: "yes".into(),
            no: "no".into(),
        },
    }]);
    let boolean = response(Answer::Boolean {
        probability: p(0.7),
    });
    let answers = validate_response(&qr, &boolean).unwrap();
    assert!((answers.probability("q", &["false".into()]).unwrap().get() - 0.3).abs() < 1e-12);
    let mut strict = req;
    strict.expected_model = Some("other".into());
    assert_eq!(
        validate_response(&strict, &complete).unwrap_err().code,
        ErrorCode::ModelMismatch
    );
}
struct Refusing;
#[async_trait::async_trait]
impl ModelBackend for Refusing {
    async fn infer(&self, _: &ModelRequest) -> Result<ModelResponse> {
        Err(SaphoError::new(ErrorCode::BackendFailed, "offline refusal"))
    }
}
/// Trace: FR-006-AC-1, FR-006-AC-3
#[tokio::test]
async fn custom_backend_errors_cross_the_shared_port() {
    let backend: Arc<dyn ModelBackend> = Arc::new(Refusing);
    assert_eq!(
        backend
            .infer(&request(vec![choice()]))
            .await
            .unwrap_err()
            .code,
        ErrorCode::BackendFailed
    );
}
/// Trace: FR-011-AC-1, FR-027-AC-3
#[test]
fn byte_accounting_refuses_before_growing_a_serialized_buffer() {
    let value = Value::Text("x".repeat(100));
    let actual = serde_json::to_vec(&value).unwrap();
    assert_eq!(bounded_json(&value, actual.len()).unwrap(), actual);
    assert_eq!(
        bounded_json(&value, actual.len() - 1).unwrap_err().code,
        ErrorCode::LimitExceeded
    );
    assert_eq!(
        measured_json_bytes(&value, actual.len()).unwrap(),
        actual.len()
    );
}

/// Trace: FR-005-AC-4, FR-020-AC-4
#[test]
fn approximate_complete_choice_and_score_preserve_raw_values_and_derive_projections() {
    for total in [0.99, 1.01] {
        for score in [false, true] {
            let (question, answer, labels) = if score {
                (
                    NamedQuestion {
                        id: "q".into(),
                        question: Question::Score {
                            instructions: "Rate synthetic strength".into(),
                            levels: vec!["low".into(), "middle".into(), "high".into()],
                        },
                    },
                    Answer::Score {
                        expected: 1.4,
                        confidence: p(0.31),
                        probabilities: Some(BTreeMap::from([
                            ("0".into(), p(0.2)),
                            ("1".into(), p(0.3)),
                            ("2".into(), p(total - 0.5)),
                        ])),
                    },
                    vec!["0".into(), "1".into(), "2".into()],
                )
            } else {
                (
                    choice(),
                    Answer::Choice {
                        selected: "z".into(),
                        confidence: p(0.31),
                        probabilities: Some(BTreeMap::from([
                            ("z".into(), p(0.6)),
                            ("a".into(), p(total - 0.6)),
                        ])),
                    },
                    vec!["z".into(), "a".into()],
                )
            };
            let mut req = request(vec![question]);
            let raw = response(answer.clone());
            assert_eq!(
                validate_response(&req, &raw).unwrap_err().code,
                ErrorCode::InvalidAnswer
            );
            req.distribution_policy = DistributionPolicy::approximate(0.01).unwrap();
            let answers = validate_response(&req, &raw).unwrap();
            assert_eq!(answers.values["q"], answer);
            assert_eq!(raw.answers["q"], answer);
            assert_eq!(
                answers.distribution_state("q").unwrap(),
                DistributionState::Approximate
            );
            let adjustment = answers.distribution_adjustment("q").unwrap().unwrap();
            assert!((adjustment.raw_mass - total).abs() < 1e-12);
            assert!((adjustment.scale - 1.0 / total).abs() < 1e-12);
            assert!((answers.probability("q", &labels).unwrap().get() - 1.0).abs() < 1e-12);
            let focal = if score { "0" } else { "z" };
            let mass = if score { 0.2 } else { 0.6 };
            assert!(
                (answers.probability("q", &[focal.into()]).unwrap().get() - mass / total).abs()
                    < 1e-12
            );
            let loaded: Answers =
                serde_json::from_str(&serde_json::to_string(&answers).unwrap()).unwrap();
            loaded.validate().unwrap();
            assert_eq!(loaded, answers);
            assert_eq!(
                loaded.probability("q", &labels).unwrap(),
                answers.probability("q", &labels).unwrap()
            );
        }
    }
}

/// Trace: FR-005-AC-4
#[test]
fn approximation_limits_and_mass_diagnostics_refuse_bad_inputs() {
    for bound in [0.0, 0.050001, f64::NAN, f64::INFINITY, -0.01] {
        assert_eq!(
            DistributionPolicy::approximate(bound).unwrap_err().code,
            ErrorCode::InvalidValue
        );
    }
    DistributionPolicy::approximate(0.05).unwrap();
    let invalid = DistributionPolicy::Approximate {
        max_mass_error: p(0.0),
    };
    let mut req = request(vec![choice()]);
    req.distribution_policy = invalid;
    assert_eq!(req.validate().unwrap_err().code, ErrorCode::InvalidValue);
    assert!(
        serde_json::from_str::<DistributionPolicy>(r#"{"kind":"strict","unknown":1}"#).is_err()
    );
    for wire in [
        r#"{"kind":"approximate","max_mass_error":0.01,"unknown":1}"#,
        r#"{"kind":"approximate"}"#,
        r#"{"kind":"approximate","max_mass_error":"not-a-number"}"#,
    ] {
        assert!(serde_json::from_str::<DistributionPolicy>(wire).is_err());
    }
    req.distribution_policy = DistributionPolicy::approximate(0.01).unwrap();
    for total in [0.0, 0.5, 0.989998, 1.010002] {
        let raw = response(Answer::Choice {
            selected: "z".into(),
            confidence: p(0.7),
            probabilities: Some(BTreeMap::from([
                ("z".into(), p(total / 2.0)),
                ("a".into(), p(total / 2.0)),
            ])),
        });
        let error = validate_response(&req, &raw).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidAnswer);
        assert_eq!(error.context["question"], "q");
        assert_eq!(error.context["coverage"], "complete");
        assert!((error.context["mass"].parse::<f64>().unwrap() - total).abs() < 1e-12);
        assert!((error.context["allowed_error"].parse::<f64>().unwrap() - 0.010001).abs() < 1e-12);
    }
    let raw = response(Answer::Choice {
        selected: "z".into(),
        confidence: p(0.7),
        probabilities: Some(BTreeMap::from([
            ("z".into(), p(0.6)),
            ("unknown".into(), p(0.39)),
        ])),
    });
    assert_eq!(
        validate_response(&req, &raw).unwrap_err().code,
        ErrorCode::InvalidAnswer
    );
    assert_eq!(
        validate_response(
            &req,
            &response(Answer::Boolean {
                probability: p(0.7)
            })
        )
        .unwrap_err()
        .code,
        ErrorCode::InvalidAnswer
    );
    let mut score_request = request(vec![NamedQuestion {
        id: "q".into(),
        question: Question::Score {
            instructions: "Rate synthetic strength".into(),
            levels: vec!["low".into(), "mid".into(), "high".into()],
        },
    }]);
    score_request.distribution_policy = DistributionPolicy::approximate(0.01).unwrap();
    for expected in [f64::NAN, f64::INFINITY, -0.01, 2.01] {
        let raw = response(Answer::Score {
            expected,
            confidence: p(0.7),
            probabilities: Some(BTreeMap::from([
                ("0".into(), p(0.33)),
                ("1".into(), p(0.33)),
                ("2".into(), p(0.33)),
            ])),
        });
        assert_eq!(
            validate_response(&score_request, &raw).unwrap_err().code,
            ErrorCode::InvalidAnswer
        );
    }
    let mut registry = BackendRegistry::default();
    let backend = Arc::new(Refusing);
    assert_eq!(
        registry
            .register(
                BackendId::new("invalid-policy").unwrap(),
                BackendBinding {
                    backend,
                    model: "model-1".into(),
                    expected_model: None,
                    distribution_policy: invalid,
                }
            )
            .unwrap_err()
            .code,
        ErrorCode::InvalidValue
    );
}

/// Trace: FR-005-AC-3, FR-005-AC-4, FR-020-AC-2, FR-020-AC-4
#[test]
fn approximation_never_normalizes_partial_or_unavailable_distributions() {
    let mut req = request(vec![choice()]);
    req.distribution_policy = DistributionPolicy::approximate(0.01).unwrap();
    let partial = response(Answer::Choice {
        selected: "z".into(),
        confidence: p(0.4),
        probabilities: Some(BTreeMap::from([("z".into(), p(0.7))])),
    });
    let answers = validate_response(&req, &partial).unwrap();
    assert_eq!(
        answers.distribution_state("q").unwrap(),
        DistributionState::Partial
    );
    assert_eq!(answers.distribution_adjustment("q").unwrap(), None);
    assert_eq!(answers.probability("q", &["z".into()]).unwrap().get(), 0.7);
    assert_eq!(
        answers.probability("q", &["a".into()]).unwrap_err().code,
        ErrorCode::UnsupportedDistribution
    );
    let unavailable = response(Answer::Choice {
        selected: "z".into(),
        confidence: p(0.4),
        probabilities: None,
    });
    let answers = validate_response(&req, &unavailable).unwrap();
    assert_eq!(
        answers.distribution_state("q").unwrap(),
        DistributionState::Unavailable
    );
    assert_eq!(answers.distribution_adjustment("q").unwrap(), None);
    assert_eq!(
        answers.probability("q", &["z".into()]).unwrap_err().code,
        ErrorCode::UnsupportedDistribution
    );
    let Question::Choice { options, .. } = &mut req.questions[0].question else {
        panic!("choice")
    };
    options.push(ChoiceOption {
        label: "extra".into(),
        description: "additional synthetic class".into(),
    });
    let excess = response(Answer::Choice {
        selected: "z".into(),
        confidence: p(0.4),
        probabilities: Some(BTreeMap::from([
            ("z".into(), p(0.7)),
            ("a".into(), p(0.31)),
        ])),
    });
    let error = validate_response(&req, &excess).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidAnswer);
    assert_eq!(error.context["coverage"], "partial");
}

/// Trace: FR-032-AC-1, FR-032-AC-2, TC-032
#[test]
fn plain_json_conversion_is_schema_directed_and_checked() {
    let number = serde_json::json!(0.75);
    for (ty, expected) in [
        (ValueType::Number, Value::Number(0.75)),
        (
            ValueType::Probability,
            Value::Probability(Probability::new(0.75).unwrap()),
        ),
        (ValueType::Degree, Value::Degree(Degree::new(0.75).unwrap())),
    ] {
        assert_eq!(
            decode_plain("root", &ty, &number, &[]).unwrap().value,
            expected
        );
    }
    assert_eq!(
        decode_plain(
            "root",
            &ValueType::optional(ValueType::Text),
            &serde_json::Value::Null,
            &[]
        )
        .unwrap()
        .value,
        Value::Optional(None)
    );
    assert_eq!(
        decode_plain(
            "root",
            &ValueType::optional(ValueType::Text),
            &serde_json::json!("hello"),
            &[]
        )
        .unwrap()
        .value,
        Value::Optional(Some(Box::new(Value::Text("hello".into()))))
    );
    let record = ValueType::Record {
        fields: BTreeMap::from([("flag".into(), ValueType::Boolean)]),
    };
    for value in [
        serde_json::json!({}),
        serde_json::json!({"flag":true,"extra":false}),
        serde_json::json!({"flag":0}),
    ] {
        assert_eq!(
            decode_plain("root", &record, &value, &[]).unwrap_err().code,
            ErrorCode::TypeMismatch
        );
    }
    assert_eq!(
        decode_plain(
            "root",
            &ValueType::Probability,
            &serde_json::json!(1.1),
            &[]
        )
        .unwrap_err()
        .code,
        ErrorCode::InvalidValue
    );
    assert_eq!(
        decode_plain("root", &ValueType::Boolean, &number, &[])
            .unwrap_err()
            .code,
        ErrorCode::TypeMismatch
    );
    let mut nested = ValueType::Number;
    for _ in 0..32 {
        nested = ValueType::optional(nested);
    }
    assert_eq!(
        decode_plain("root", &nested, &number, &[])
            .unwrap_err()
            .code,
        ErrorCode::LimitExceeded
    );
    assert_eq!(
        decode_json::<serde_json::Value>(b"1e999", 10)
            .unwrap_err()
            .code,
        ErrorCode::Config
    );
}
/// Trace: FR-032-AC-3, FR-032-AC-2
#[test]
fn strict_json_and_nested_occurrences_retain_identity_and_sources() {
    let json = serde_json::json!([["same", "same"], ["same"]]);
    let source = SourceRef {
        source: SourceId::new("local").unwrap(),
        start: None,
        end: None,
    };
    let ty = ValueType::list(ValueType::list(ValueType::Text));
    let a = decode_plain("root:/[]", &ty, &json, std::slice::from_ref(&source)).unwrap();
    assert_eq!(
        decode_plain("root:/[]", &ty, &json, std::slice::from_ref(&source)).unwrap(),
        a
    );
    let Value::List(outer) = a.value else {
        panic!("list expected")
    };
    let Value::List(first) = &outer[0].value else {
        panic!("nested list expected")
    };
    assert_ne!(first[0].id, first[1].id);
    assert_ne!(outer[0].id, first[0].id);
    assert_eq!(first[0].sources, vec![source]);
    assert_eq!(
        decode_json::<serde_json::Value>(br#"{"a":{"b":1,"b":2}}"#, 100)
            .unwrap_err()
            .code,
        ErrorCode::Config
    );
    let depth = "[".repeat(128) + "0" + &"]".repeat(128);
    assert_eq!(
        decode_json::<serde_json::Value>(depth.as_bytes(), 1000)
            .unwrap_err()
            .code,
        ErrorCode::LimitExceeded
    );
    assert_eq!(
        decode_json::<serde_json::Value>(b"{}", 1).unwrap_err().code,
        ErrorCode::LimitExceeded
    );
}

/// Trace: FR-032-AC-3, FR-056-AC-5
#[test]
fn typed_json_decoding_is_strict_without_building_a_generic_tree() {
    #[derive(Debug, PartialEq, serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Shape {
        name: String,
        items: Vec<Inner>,
    }
    #[derive(Debug, PartialEq, serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Inner {
        weight: f64,
    }
    let decoded: Shape = decode_json(
        br#" {"name":"a","items":[{"weight":0.25},{"weight":1e2}]} "#,
        1000,
    )
    .unwrap();
    assert_eq!(
        decoded,
        Shape {
            name: "a".into(),
            items: vec![Inner { weight: 0.25 }, Inner { weight: 100.0 }],
        }
    );
    let refused = |bytes: &[u8]| decode_json::<Shape>(bytes, 1000).unwrap_err().code;
    // A duplicate key anywhere is refused, even inside a nested typed member.
    assert_eq!(
        refused(br#"{"name":"a","items":[{"weight":1,"weight":2}]}"#),
        ErrorCode::Config
    );
    assert_eq!(
        refused(br#"{"name":"a","name":"b","items":[]}"#),
        ErrorCode::Config
    );
    // Type, unknown-member and trailing-content refusals keep their code.
    assert_eq!(refused(br#"{"name":1,"items":[]}"#), ErrorCode::Config);
    assert_eq!(
        refused(br#"{"name":"a","items":[],"extra":1}"#),
        ErrorCode::Config
    );
    assert_eq!(refused(br#"{"name":"a","items":[]} {}"#), ErrorCode::Config);
    assert_eq!(refused(br#"{"name":"a","items":[]"#), ErrorCode::Config);
    assert_eq!(
        refused(br#"{"name":"a","items":[{"weight":1e999}]}"#),
        ErrorCode::Config
    );
    // Containers nested 128 deep pass and 129 is a limit refusal, for ignored members too.
    let nested = |levels: usize| "[".repeat(levels) + &"]".repeat(levels);
    assert!(decode_json::<serde_json::Value>(nested(128).as_bytes(), 1000).is_ok());
    assert_eq!(
        decode_json::<serde::de::IgnoredAny>(nested(129).as_bytes(), 1000)
            .unwrap_err()
            .code,
        ErrorCode::LimitExceeded
    );
    let mapped = |levels: usize| r#"{"a":"#.repeat(levels) + "0" + &"}".repeat(levels);
    assert!(decode_json::<serde_json::Value>(mapped(127).as_bytes(), 10_000).is_ok());
    assert_eq!(
        decode_json::<serde_json::Value>(mapped(128).as_bytes(), 10_000)
            .unwrap_err()
            .code,
        ErrorCode::LimitExceeded
    );
    assert!(decode_json::<serde_json::Value>(b"{}", 2).is_ok());
    let hidden = format!(r#"{{"name":"a","items":[],"extra":{}}}"#, nested(200));
    assert_eq!(
        decode_json::<serde_json::Value>(hidden.as_bytes(), 10_000)
            .unwrap_err()
            .code,
        ErrorCode::LimitExceeded
    );
}

/// Trace: FR-032-AC-1, FR-041-AC-2
#[test]
fn plain_question_and_answer_inputs_validate_their_full_typed_contract() {
    let questions = vec![choice()];
    let q = decode_plain(
        "q",
        &ValueType::Questions,
        &serde_json::to_value(&questions).unwrap(),
        &[],
    )
    .unwrap();
    assert_eq!(q.value, Value::Questions(questions.clone()));
    let answers = validate_response(
        &request(questions.clone()),
        &response(Answer::Choice {
            selected: "z".into(),
            confidence: p(0.75),
            probabilities: Some(BTreeMap::from([
                ("z".into(), p(0.75)),
                ("a".into(), p(0.25)),
            ])),
        }),
    )
    .unwrap();
    assert_eq!(
        decode_plain(
            "a",
            &ValueType::Answers,
            &serde_json::to_value(&answers).unwrap(),
            &[]
        )
        .unwrap()
        .value,
        Value::Answers(answers.clone())
    );
    for (schema, value) in [
        (ValueType::Questions, serde_json::json!({})),
        (ValueType::Answers, serde_json::json!([])),
    ] {
        assert_eq!(
            decode_plain("bad", &schema, &value, &[]).unwrap_err().code,
            ErrorCode::TypeMismatch
        );
    }
    let mut invalid = serde_json::to_value(answers).unwrap();
    invalid["values"]["q"]["probabilities"]["a"] = serde_json::json!(0.1);
    assert_eq!(
        decode_plain("bad", &ValueType::Answers, &invalid, &[])
            .unwrap_err()
            .code,
        ErrorCode::InvalidAnswer
    );
    let mut invalid = serde_json::to_value(questions).unwrap();
    invalid[0]["question"]["extra"] = serde_json::json!(true);
    assert_eq!(
        decode_plain("bad", &ValueType::Questions, &invalid, &[])
            .unwrap_err()
            .code,
        ErrorCode::TypeMismatch
    );
}
/// Legacy serialized response compatibility; no source identity is inferred.
#[test]
fn legacy_provider_metadata_remains_optional_in_serialized_responses() {
    let mut with_digest = response(Answer::Choice {
        selected: "z".into(),
        confidence: p(0.7),
        probabilities: Some(BTreeMap::from([("z".into(), p(0.7)), ("a".into(), p(0.3))])),
    });
    with_digest.digest = Some("sha256:58574f".into());
    validate_response(&request(vec![choice()]), &with_digest).unwrap();
    let json = serde_json::to_value(&with_digest).unwrap();
    assert_eq!(json["digest"], "sha256:58574f");
    assert_eq!(
        serde_json::from_value::<ModelResponse>(json).unwrap(),
        with_digest
    );
    with_digest.digest = None;
    let json = serde_json::to_value(&with_digest).unwrap();
    assert!(json.get("digest").is_none());
    assert_eq!(
        serde_json::from_value::<ModelResponse>(json)
            .unwrap()
            .digest,
        None
    );
}
/// Trace: FR-006-AC-4, FR-048-AC-7
#[test]
fn raw_exchange_round_trips_and_is_absent_from_json_when_not_kept() {
    let mut kept = response(Answer::Boolean {
        probability: p(0.5),
    });
    kept.raw = Some(RawExchange {
        request: "{\"q\":\"caf\u{e9}\"}".into(),
        response: "{\"x\":1}".into(),
    });
    let json = serde_json::to_value(&kept).unwrap();
    assert_eq!(
        json["raw"],
        serde_json::json!({"request": "{\"q\":\"caf\u{e9}\"}", "response": "{\"x\":1}"})
    );
    assert_eq!(serde_json::from_value::<ModelResponse>(json).unwrap(), kept);
    kept.raw = None;
    let json = serde_json::to_value(&kept).unwrap();
    assert!(json.get("raw").is_none());
    assert_eq!(
        serde_json::from_value::<ModelResponse>(json).unwrap().raw,
        None
    );
}
