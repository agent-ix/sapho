// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Evidence acceptance tests with independently specified expected counts.
use super::*;
use sapho_core::{BackendId, Datum, Degree, Probability, Usage};
fn provenance(kind: LabelKind, source: &str) -> LabelProvenance {
    LabelProvenance {
        kind,
        source: source.into(),
        reference: "original source".into(),
    }
}

#[test]
fn current_evidence_has_no_model_digest_member() {
    let current = provenance(LabelKind::Model, "reported-model");
    let value = serde_json::to_value(&current).unwrap();
    assert!(value.get("model_digest").is_none());
    assert_eq!(
        serde_json::from_value::<LabelProvenance>(value).unwrap(),
        current
    );
    let with_digest = serde_json::json!({
        "kind": "model", "source": "reported-model", "model_digest": "sha256:old",
        "reference": "original source"
    });
    assert!(serde_json::from_value::<LabelProvenance>(with_digest).is_err());
}
fn dataset(labels: &[bool]) -> Dataset {
    Dataset {
        id: SourceId::new("curated").unwrap(),
        cases: labels
            .iter()
            .enumerate()
            .map(|(i, label)| Case {
                id: ItemId::new(format!("case-{i}")).unwrap(),
                split: Split::Development,
                inputs: Inputs::new(),
                labels: BTreeMap::from([("result".into(), *label)]),
                label_provenance: provenance(LabelKind::Human, "reviewer-1"),
            })
            .collect(),
    }
}
fn outcomes(values: &[Value]) -> BTreeMap<ItemId, CaseOutcome> {
    values
        .iter()
        .enumerate()
        .map(|(i, value)| {
            (
                ItemId::new(format!("case-{i}")).unwrap(),
                CaseOutcome::Completed {
                    outputs: Inputs::from([(
                        "result".into(),
                        Datum::new("prediction", value.clone()).unwrap(),
                    )]),
                    models: Vec::new(),
                },
            )
        })
        .collect()
}
/// Trace: FR-062-AC-1, FR-062-AC-2, FR-062-AC-3, FR-062-AC-4,
/// FR-062-AC-5, FR-062-AC-6, FR-062-AC-7, FR-063-AC-3, FR-063-AC-4,
/// IT-009-SC-01, IT-009-SC-02, IT-009-SC-03, IT-009-SC-04
#[test]
fn roster_partitions_actual_models_and_projects_only_measured_fields() {
    let mut data = dataset(&[true, false, true]);
    for (index, case) in data.cases.iter_mut().enumerate() {
        case.labels.insert("flag".into(), index != 1);
        case.label_provenance.kind = if index == 1 {
            LabelKind::Agent
        } else {
            LabelKind::Human
        };
    }
    let judge = BackendId::new("judge").unwrap();
    let other = BackendId::new("custom").unwrap();
    let outputs = BTreeMap::from([
        ("result".into(), ValueType::Probability),
        ("flag".into(), ValueType::Boolean),
    ]);
    let mappings = BTreeMap::from([
        (
            "result".into(),
            RosterMapping {
                binding: judge.clone(),
                question_kind: RosterQuestionKind::Boolean,
            },
        ),
        (
            "flag".into(),
            RosterMapping {
                binding: other.clone(),
                question_kind: RosterQuestionKind::Boolean,
            },
        ),
    ]);
    let make_call =
        |binding: BackendId, model: Option<&str>, duration: u64, usage: Option<Usage>| RosterCall {
            path: vec!["root".into(), binding.to_string()],
            binding,
            actual_model: model.map(str::to_owned),
            completed: model.is_some(),
            usage,
            elapsed_micros: Some(duration),
            descriptor: None,
            mode: RosterMode::Live,
        };
    let mut cases = BTreeMap::new();
    for (index, model, probability) in [(0, Some("A"), 0.8), (1, Some("B"), 0.2), (2, None, 0.4)] {
        let id = data.cases[index].id.clone();
        let outcome = if let Some(model) = model {
            CaseOutcome::Completed {
                outputs: Inputs::from([
                    (
                        "result".into(),
                        Datum::new(
                            "p",
                            Value::Probability(Probability::new(probability).unwrap()),
                        )
                        .unwrap(),
                    ),
                    (
                        "flag".into(),
                        Datum::new("b", Value::Boolean(index == 0)).unwrap(),
                    ),
                ]),
                models: vec![
                    ModelIdentity { name: model.into() },
                    ModelIdentity {
                        name: "custom-model".into(),
                    },
                ],
            }
        } else {
            CaseOutcome::Failed {
                error: SaphoError::new(ErrorCode::BackendFailed, "scripted"),
                models: Vec::new(),
            }
        };
        let mut calls = vec![make_call(
            judge.clone(),
            model,
            10 + index as u64 * 10,
            if index == 0 {
                Some(Usage {
                    billing_units: Some(2),
                    input_tokens: 10,
                    output_tokens: 4,
                })
            } else {
                None
            },
        )];
        if index < 2 {
            calls.push(make_call(other.clone(), Some("custom-model"), 5, None));
        }
        let mut flag_lineage = vec![RosterContributor {
            binding: other.clone(),
            actual_model: Some("custom-model".into()),
            question_kind: Some(RosterQuestionKind::Boolean),
        }];
        if index == 1 {
            flag_lineage.push(RosterContributor {
                binding: judge.clone(),
                actual_model: Some("B".into()),
                question_kind: Some(RosterQuestionKind::Boolean),
            });
        }
        cases.insert(
            id,
            RosterCase {
                outcome,
                calls,
                lineage: BTreeMap::from([
                    (
                        "result".into(),
                        model
                            .map(|name| {
                                vec![RosterContributor {
                                    binding: judge.clone(),
                                    actual_model: Some(name.into()),
                                    question_kind: Some(RosterQuestionKind::Boolean),
                                }]
                            })
                            .unwrap_or_default(),
                    ),
                    (
                        "flag".into(),
                        if index == 2 { Vec::new() } else { flag_lineage },
                    ),
                ]),
            },
        );
    }
    let identity = format!("graph-v1:sha256:{}", "a".repeat(64));
    let report = roster(
        &data,
        Split::Development,
        &identity,
        "0.1",
        &outputs,
        &mappings,
        &cases,
        10,
    )
    .unwrap();
    let mut wrong_kind = mappings.clone();
    wrong_kind.get_mut("result").unwrap().question_kind = RosterQuestionKind::Choice;
    assert!(matches!(
        roster(&data, Split::Development, &identity, "0.1", &outputs, &wrong_kind, &cases, 10),
        Err(EvidenceError::InvalidRoster(message)) if message.contains("Question kind mismatch")
    ));
    assert_eq!(report.entries.len(), 4);
    let a = report
        .entries
        .iter()
        .find(|entry| entry.binding == judge && entry.actual_model.as_deref() == Some("A"))
        .unwrap();
    let b = report
        .entries
        .iter()
        .find(|entry| entry.binding == judge && entry.actual_model.as_deref() == Some("B"))
        .unwrap();
    let unknown = report
        .entries
        .iter()
        .find(|entry| entry.binding == judge && entry.actual_model.is_none())
        .unwrap();
    assert_eq!((a.calls, b.calls, unknown.calls), (1, 1, 1));
    assert_eq!(a.input_tokens.as_ref().unwrap().total, 10);
    assert_eq!(a.input_tokens.as_ref().unwrap().count, 1);
    assert!(b.input_tokens.is_none());
    assert_eq!(a.latency.p50_micros, Some(10));
    assert_eq!(b.latency.p95_micros, Some(20));
    assert!(unknown.outputs.is_empty());
    let a_metric = &a.outputs["result"][&LabelKind::Human];
    let b_metric = &b.outputs["result"][&LabelKind::Agent];
    assert_eq!(a_metric.case_ids, vec![data.cases[0].id.clone()]);
    assert_eq!(b_metric.case_ids, vec![data.cases[1].id.clone()]);
    assert_eq!(a_metric.measurement.scored, 1);
    assert_eq!(b_metric.measurement.scored, 1);
    assert!(matches!(
        a_metric.measurement.metrics,
        Metrics::Probability { brier: Some(value), .. } if (value - 0.04).abs() < 1e-12
    ));
    assert_eq!(a_metric.ece, None);
    assert_eq!(a_metric.ece_absent_reason.as_deref(), Some("not_computed"));
    assert_eq!(
        report
            .unattributed
            .iter()
            .filter(|row| row.reason == UnattributedReason::MultipleResponses)
            .count(),
        1
    );
    let projected = project_roster_literal(
        &report,
        &[
            RosterSelection {
                field: "first".into(),
                binding: judge.clone(),
                actual_model: Some("A".into()),
            },
            RosterSelection {
                field: "second".into(),
                binding: judge.clone(),
                actual_model: Some("B".into()),
            },
        ],
        100_000,
    )
    .unwrap();
    projected.value_type.check(&projected.value.value).unwrap();
    let Value::Record(fields) = &projected.value.value else {
        panic!("expected Record")
    };
    let Value::Record(first) = &fields["first"] else {
        panic!("expected profile Record")
    };
    assert_eq!(first["calls"], Value::Number(1.0));
    assert_eq!(first["provider"], Value::Optional(None));
    assert_eq!(
        first["latency_p50_micros"],
        Value::Optional(Some(Box::new(Value::Number(10.0))))
    );
    assert!(
        !serde_json::to_string(&projected)
            .unwrap()
            .contains("endpoint")
    );
    assert!(
        project_roster_literal(
            &report,
            &[RosterSelection {
                field: "first".into(),
                binding: judge,
                actual_model: Some("missing".into())
            }],
            100_000
        )
        .is_err()
    );
    assert!(
        project_roster_literal(
            &report,
            &[
                RosterSelection {
                    field: "duplicate".into(),
                    binding: BackendId::new("judge").unwrap(),
                    actual_model: Some("A".into())
                },
                RosterSelection {
                    field: "duplicate".into(),
                    binding: BackendId::new("judge").unwrap(),
                    actual_model: Some("B".into())
                },
            ],
            100_000
        )
        .is_err()
    );
    assert!(
        project_roster_literal(
            &report,
            &[RosterSelection {
                field: "small".into(),
                binding: BackendId::new("judge").unwrap(),
                actual_model: Some("A".into())
            }],
            16
        )
        .is_err()
    );
    let mut mismatched = projected.clone();
    mismatched.value_type = ValueType::Boolean;
    assert!(
        mismatched
            .value_type
            .check(&mismatched.value.value)
            .is_err()
    );
    let mut replay = cases;
    for case in replay.values_mut() {
        for call in &mut case.calls {
            call.mode = RosterMode::Replay;
            call.elapsed_micros = None;
        }
    }
    let replayed = roster(
        &data,
        Split::Development,
        &identity,
        "0.1",
        &outputs,
        &mappings,
        &replay,
        10,
    )
    .unwrap();
    assert_eq!(replayed.entries[0].latency.count, 0);
    assert_eq!(
        replayed.entries[0].latency.absent_reason.as_deref(),
        Some("replay_only")
    );
    let replay_literal = project_roster_literal(
        &replayed,
        &[RosterSelection {
            field: "first".into(),
            binding: BackendId::new("judge").unwrap(),
            actual_model: Some("A".into()),
        }],
        100_000,
    )
    .unwrap();
    let Value::Record(replay_fields) = &replay_literal.value.value else {
        panic!("Record")
    };
    let Value::Record(replay_first) = &replay_fields["first"] else {
        panic!("profile")
    };
    assert_eq!(replay_first["latency_p50_micros"], Value::Optional(None));
    let Value::Record(replay_outputs) = &replay_first["outputs"] else {
        panic!("outputs")
    };
    let Value::Record(replay_result) = &replay_outputs["result"] else {
        panic!("result")
    };
    let Value::Record(replay_human) = &replay_result["human"] else {
        panic!("human")
    };
    assert_eq!(replay_human["ece"], Value::Optional(None));
    replay.values_mut().next().unwrap().calls[0].mode = RosterMode::Live;
    assert!(
        roster(
            &data,
            Split::Development,
            &identity,
            "0.1",
            &outputs,
            &mappings,
            &replay,
            10
        )
        .is_err()
    );
}
/// Trace: FR-036-AC-1, FR-036-AC-3, TC-036
#[test]
fn boolean_counts_have_exact_denominators_and_case_evidence() {
    let data = dataset(&[true, false, false, true]);
    let report = measure(
        &data,
        Split::Development,
        &BTreeMap::from([("result".into(), ValueType::Boolean)]),
        &outcomes(&[
            Value::Boolean(true),
            Value::Boolean(false),
            Value::Boolean(true),
            Value::Boolean(false),
        ]),
        100,
    )
    .unwrap();
    assert_eq!(
        report.outputs["result"].metrics,
        Metrics::Boolean {
            confusion: Confusion {
                true_positive: 1,
                true_negative: 1,
                false_positive: 1,
                false_negative: 1
            },
            agreement: Some(0.5)
        }
    );
    assert_eq!(report.outputs["result"].scored, 4);
    assert_eq!(
        report.predictions[0].label_provenance,
        provenance(LabelKind::Human, "reviewer-1")
    );
    assert!(report.complete());
    let mut duplicate = data.clone();
    duplicate.cases.push(data.cases[0].clone());
    assert!(matches!(
        duplicate.validate(100),
        Err(EvidenceError::DuplicateCase(_))
    ));
    let mut unlabelled = data.clone();
    unlabelled.cases[0].labels.clear();
    assert!(matches!(
        unlabelled.validate(100),
        Err(EvidenceError::MissingLabels(_))
    ));
    let mut no_origin = data;
    no_origin.cases[0].label_provenance.source = " ".into();
    assert!(matches!(
        no_origin.validate(100),
        Err(EvidenceError::MissingProvenance(_))
    ));
    no_origin.cases[0].label_provenance.source = "reviewer-1".into();
    no_origin.cases[0].label_provenance.reference = "".into();
    assert!(matches!(
        no_origin.validate(100),
        Err(EvidenceError::MissingProvenance(_))
    ));
    let unknown_kind = serde_json::json!({
        "kind": "oracle", "source": "x", "reference": "y"
    });
    assert!(serde_json::from_value::<LabelProvenance>(unknown_kind).is_err());
}
/// Trace: FR-064-AC-3, FR-062-AC-4
#[test]
fn ordinary_measure_per_kind_keeps_top_level_totals() {
    let mut data = dataset(&[true, false]);
    data.cases[1].label_provenance.kind = LabelKind::Agent;
    let report = measure(
        &data,
        Split::Development,
        &BTreeMap::from([("result".into(), ValueType::Boolean)]),
        &outcomes(&[Value::Boolean(true), Value::Boolean(true)]),
        10,
    )
    .unwrap();
    assert_eq!(report.outputs["result"].scored, 2);
    assert_eq!(report.per_kind["result"][&LabelKind::Human].scored, 1);
    assert_eq!(report.per_kind["result"][&LabelKind::Agent].scored, 1);
    assert_eq!(
        report.per_kind["result"][&LabelKind::Human].metrics,
        Metrics::Boolean {
            confusion: Confusion {
                true_positive: 1,
                ..Confusion::default()
            },
            agreement: Some(1.0)
        }
    );
    assert_eq!(
        report.per_kind["result"][&LabelKind::Agent].metrics,
        Metrics::Boolean {
            confusion: Confusion {
                false_positive: 1,
                ..Confusion::default()
            },
            agreement: Some(0.0)
        }
    );
}
/// Trace: FR-036-AC-2, FR-036-AC-3
#[test]
fn probability_scores_and_missing_degree_failures_remain_separate() {
    let mut data = dataset(&[true, false, true]);
    data.cases[2].split = Split::HeldOut;
    let report = measure(
        &data,
        Split::Development,
        &BTreeMap::from([("result".into(), ValueType::Probability)]),
        &outcomes(&[
            Value::Probability(Probability::new(0.8).unwrap()),
            Value::Probability(Probability::new(0.4).unwrap()),
        ]),
        100,
    )
    .unwrap();
    let Metrics::Probability {
        brier: Some(score), ..
    } = report.outputs["result"].metrics
    else {
        panic!("Brier expected")
    };
    assert!((score - 0.1).abs() < 1e-12);
    assert_eq!(report.selected_cases, 2);
    assert_eq!(report.predictions.len(), 2);
    let mut actual = outcomes(&[Value::Degree(Degree::new(0.8).unwrap())]);
    actual.insert(
        ItemId::new("case-1").unwrap(),
        CaseOutcome::Failed {
            error: SaphoError::new(ErrorCode::ReplayMiss, "missing recording"),
            models: Vec::new(),
        },
    );
    let degree = measure(
        &data,
        Split::Development,
        &BTreeMap::from([("result".into(), ValueType::Degree)]),
        &actual,
        100,
    )
    .unwrap();
    assert_eq!(degree.outputs["result"].scored, 0);
    assert_eq!(degree.outputs["result"].unscored, 1);
    assert_eq!(degree.outputs["result"].failed, 1);
    assert!(!degree.complete());
    let missing = measure(
        &dataset(&[true]),
        Split::Development,
        &BTreeMap::new(),
        &BTreeMap::from([(
            ItemId::new("case-0").unwrap(),
            CaseOutcome::Completed {
                outputs: Inputs::new(),
                models: Vec::new(),
            },
        )]),
        100,
    )
    .unwrap();
    assert_eq!(
        missing.predictions[0].unscored,
        Some(UnscoredReason::MissingOutput)
    );
}

/// Trace: FR-065-AC-1, FR-065-AC-2, FR-066-AC-1, IT-010-SC-01, IT-010-SC-02
#[test]
fn sapho_21_ten_case_ece_parity() {
    let rows = [
        (0.05, false),
        (0.15, false),
        (0.25, true),
        (0.35, false),
        (0.45, true),
        (0.55, true),
        (0.65, true),
        (0.75, false),
        (0.9, true),
        (1.0, false),
    ];
    let data = dataset(&rows.map(|(_, label)| label));
    let values = rows.map(|(p, _)| Value::Probability(Probability::new(p).unwrap()));
    let report = measure(
        &data,
        Split::Development,
        &BTreeMap::from([("result".into(), ValueType::Probability)]),
        &outcomes(&values),
        100,
    )
    .unwrap();
    let Metrics::Probability {
        ece,
        calibration,
        risk_coverage,
        ..
    } = &report.outputs["result"].metrics
    else {
        panic!("Probability metrics expected");
    };
    assert!((ece.unwrap() - 0.33).abs() < 1e-9);
    assert_eq!(calibration.len(), 10);
    assert_eq!((calibration[9].count, calibration[9].correct), (3, 2));
    assert!((calibration[9].mean_confidence.unwrap() - 0.95).abs() < 1e-9);
    assert!((calibration[9].accuracy.unwrap() - 2.0 / 3.0).abs() < 1e-9);
    assert_eq!((calibration[8].count, calibration[8].correct), (1, 1));
    assert!((calibration[8].mean_confidence.unwrap() - 0.85).abs() < 1e-9);
    assert_eq!(
        risk_coverage
            .iter()
            .map(|row| (row.answered, row.correct, row.total))
            .collect::<Vec<_>>(),
        [
            (10, 6, 10),
            (8, 5, 10),
            (6, 3, 10),
            (4, 3, 10),
            (3, 2, 10),
            (2, 1, 10)
        ]
    );
    for row in risk_coverage {
        assert!((row.coverage - f64::from(row.answered) / 10.0).abs() < 1e-12);
        assert!(
            (row.accuracy.unwrap() - f64::from(row.correct) / f64::from(row.answered)).abs()
                < 1e-12
        );
        assert!((row.risk.unwrap() - (1.0 - row.accuracy.unwrap())).abs() < 1e-12);
    }
}

/// Trace: FR-065-AC-2, FR-066-AC-2, FR-066-AC-3, IT-010-SC-03
#[test]
fn calibration_boundaries_and_threshold_overrides_are_checked() {
    let data = dataset(&[true, true, true]);
    let values = [0.899, 0.9, 1.0].map(|p| Value::Probability(Probability::new(p).unwrap()));
    let schema = BTreeMap::from([("result".into(), ValueType::Probability)]);
    let cases = outcomes(&values);
    let default = measure(&data, Split::Development, &schema, &cases, 10).unwrap();
    let Metrics::Probability {
        calibration,
        brier,
        ece,
        ..
    } = &default.outputs["result"].metrics
    else {
        panic!("Probability metrics expected");
    };
    assert_eq!(calibration[8].count, 1);
    assert_eq!(calibration[9].count, 2);
    assert!((calibration[9].mean_confidence.unwrap() - 0.95).abs() < 1e-12);
    assert_eq!(calibration[9].accuracy, Some(1.0));
    let selected = measure_with_thresholds(
        &data,
        Split::Development,
        &schema,
        &cases,
        10,
        &[0.899, 0.9, 1.0],
    )
    .unwrap();
    let Metrics::Probability {
        risk_coverage,
        brier: selected_brier,
        ece: selected_ece,
        ..
    } = &selected.outputs["result"].metrics
    else {
        panic!("Probability metrics expected");
    };
    assert_eq!((selected_brier, selected_ece), (brier, ece));
    assert_eq!(
        risk_coverage
            .iter()
            .map(|row| row.answered)
            .collect::<Vec<_>>(),
        [3, 2, 1]
    );
    let never = measure_with_thresholds(
        &data,
        Split::Development,
        &schema,
        &outcomes(&[Value::Probability(Probability::new(0.5).unwrap())]),
        10,
        &[0.9],
    )
    .unwrap();
    let Metrics::Probability { risk_coverage, .. } = &never.outputs["result"].metrics else {
        panic!("Probability metrics expected");
    };
    assert_eq!(risk_coverage[0].answered, 0);
    assert_eq!(risk_coverage[0].coverage, 0.0);
    assert_eq!(
        (risk_coverage[0].accuracy, risk_coverage[0].risk),
        (None, None)
    );
    for invalid in [
        vec![],
        vec![0.9, 0.9],
        vec![0.9, 0.8],
        vec![f64::NAN],
        vec![f64::INFINITY],
        vec![0.49],
        vec![1.01],
        (0..33).map(|i| 0.5 + f64::from(i) / 100.0).collect(),
    ] {
        assert!(matches!(
            measure_with_thresholds(&data, Split::Development, &schema, &cases, 10, &invalid),
            Err(EvidenceError::InvalidThresholds)
        ));
    }
}

/// Trace: FR-065-AC-3, FR-065-AC-4, FR-065-AC-5, FR-066-AC-4, FR-066-AC-5, IT-010-SC-04, IT-010-SC-05
#[test]
fn empty_probability_and_exclusions_preserve_measurement_contract() {
    let mut data = dataset(&[true, false, true, false]);
    data.cases[0].label_provenance = provenance(LabelKind::Model, "answering-model");
    data.cases[3].split = Split::HeldOut;
    let encoded = serde_json::to_vec(&data).unwrap();
    let decoded: Dataset = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(decoded, data);
    decoded.validate(10).unwrap();
    let cases = BTreeMap::from([
        (
            data.cases[0].id.clone(),
            CaseOutcome::Completed {
                outputs: Inputs::from([(
                    "result".into(),
                    Datum::new("p", Value::Probability(Probability::new(1.0).unwrap())).unwrap(),
                )]),
                models: vec![ModelIdentity {
                    name: "answering-model".into(),
                }],
            },
        ),
        (
            data.cases[1].id.clone(),
            CaseOutcome::Completed {
                outputs: Inputs::from([(
                    "result".into(),
                    Datum::new("n", Value::Number(0.8)).unwrap(),
                )]),
                models: Vec::new(),
            },
        ),
        (
            data.cases[2].id.clone(),
            CaseOutcome::Failed {
                error: SaphoError::new(ErrorCode::ReplayMiss, "fixture"),
                models: Vec::new(),
            },
        ),
        (
            data.cases[3].id.clone(),
            CaseOutcome::Completed {
                outputs: Inputs::from([(
                    "result".into(),
                    Datum::new("p", Value::Probability(Probability::new(0.9).unwrap())).unwrap(),
                )]),
                models: Vec::new(),
            },
        ),
    ]);
    let report = measure(
        &data,
        Split::Development,
        &BTreeMap::from([("result".into(), ValueType::Probability)]),
        &cases,
        10,
    )
    .unwrap();
    assert_eq!(report.self_source, vec![data.cases[0].id.clone()]);
    assert_eq!(
        (
            report.selected_cases,
            report.outputs["result"].labelled,
            report.outputs["result"].scored,
            report.outputs["result"].unscored,
            report.outputs["result"].failed
        ),
        (3, 2, 0, 1, 1)
    );
    let Metrics::Probability {
        brier,
        ece,
        calibration,
        risk_coverage,
    } = &report.outputs["result"].metrics
    else {
        panic!("Probability metrics expected");
    };
    assert_eq!((*brier, *ece), (None, None));
    assert!(calibration.iter().all(|bucket| bucket.count == 0
        && bucket.mean_confidence.is_none()
        && bucket.accuracy.is_none()));
    assert!(risk_coverage.is_empty());
    let mut one_scored = cases.clone();
    one_scored.insert(
        data.cases[1].id.clone(),
        CaseOutcome::Completed {
            outputs: Inputs::from([(
                "result".into(),
                Datum::new("p", Value::Probability(Probability::new(0.2).unwrap())).unwrap(),
            )]),
            models: Vec::new(),
        },
    );
    let isolated = measure(
        &data,
        Split::Development,
        &BTreeMap::from([("result".into(), ValueType::Probability)]),
        &one_scored,
        10,
    )
    .unwrap();
    let Metrics::Probability {
        calibration,
        risk_coverage,
        ..
    } = &isolated.outputs["result"].metrics
    else {
        panic!("Probability metrics expected");
    };
    assert_eq!(isolated.outputs["result"].scored, 1);
    assert_eq!(
        calibration.iter().map(|bucket| bucket.count).sum::<u32>(),
        1
    );
    assert!(risk_coverage.iter().all(|row| row.total == 1));
    for value_type in [ValueType::Degree, ValueType::Number] {
        let unsupported = measure(
            &data,
            Split::Development,
            &BTreeMap::from([("result".into(), value_type.clone())]),
            &cases,
            10,
        )
        .unwrap();
        assert!(matches!(
            unsupported.outputs["result"].metrics,
            Metrics::Unsupported { .. }
        ));
        assert_eq!(unsupported.outputs["result"].scored, 0);
    }
}
/// Trace: FR-037-AC-1, FR-037-AC-2, FR-037-AC-3, TC-037
#[test]
fn ranking_uses_complete_development_results_and_stable_ties() {
    let data = dataset(&[true, false]);
    let schema = BTreeMap::from([("result".into(), ValueType::Boolean)]);
    let correct = measure(
        &data,
        Split::Development,
        &schema,
        &outcomes(&[Value::Boolean(true), Value::Boolean(false)]),
        100,
    )
    .unwrap();
    let partial = measure(
        &data,
        Split::Development,
        &schema,
        &outcomes(&[Value::Boolean(true)]),
        100,
    )
    .unwrap();
    let candidates = vec![
        Candidate {
            id: SourceId::new("partial").unwrap(),
            measurement: partial,
        },
        Candidate {
            id: SourceId::new("first").unwrap(),
            measurement: correct.clone(),
        },
        Candidate {
            id: SourceId::new("tie").unwrap(),
            measurement: correct,
        },
    ];
    let ranked = rank(&candidates, "result", Metric::Agreement, 3).unwrap();
    assert_eq!(
        ranked.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        vec!["first", "tie"]
    );
    assert!(matches!(
        rank(&candidates, "result", Metric::Agreement, 2),
        Err(EvidenceError::CandidateLimit { .. })
    ));
    assert!(matches!(
        rank(&candidates[..1], "result", Metric::Agreement, 3),
        Err(EvidenceError::NoRankableCandidate { .. })
    ));
    assert!(matches!(
        rank(&[], "result", Metric::Agreement, 3),
        Err(EvidenceError::NoCandidates)
    ));
}
/// Trace: FR-038-AC-1, FR-038-AC-2, FR-038-AC-3, TC-038
#[test]
fn training_export_keeps_only_curated_development_rows_and_refuses_limits() {
    let mut data = dataset(&[true, false]);
    data.cases[1].split = Split::HeldOut;
    let bytes = export_training(&data, 100, 10000).unwrap();
    let rows = std::str::from_utf8(&bytes)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<TrainingRow>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, data.cases[0].id);
    assert_eq!(rows[0].labels, data.cases[0].labels);
    assert_eq!(rows[0].label_provenance, data.cases[0].label_provenance);
    assert!(matches!(
        export_training(&data, 1, 10000),
        Err(EvidenceError::CaseLimit { .. })
    ));
    assert!(matches!(
        export_training(&data, 100, 1),
        Err(EvidenceError::Core(SaphoError {
            code: ErrorCode::LimitExceeded,
            ..
        }))
    ));
    data.cases[0].split = Split::HeldOut;
    assert!(matches!(
        export_training(&data, 100, 10000),
        Err(EvidenceError::EmptySplit(Split::Development))
    ));
    assert!(serde_json::from_str::<Dataset>("{\"exchanges\":[]}").is_err());
}

fn answered_by(name: &str, values: &[Value]) -> BTreeMap<ItemId, CaseOutcome> {
    let mut outcomes = outcomes(values);
    for outcome in outcomes.values_mut() {
        if let CaseOutcome::Completed { models, .. } = outcome {
            models.push(ModelIdentity { name: name.into() });
        }
    }
    outcomes
}
fn measure_boolean(data: &Dataset, outcomes: &BTreeMap<ItemId, CaseOutcome>) -> Measurement {
    measure(
        data,
        Split::Development,
        &BTreeMap::from([("result".into(), ValueType::Boolean)]),
        outcomes,
        100,
    )
    .unwrap()
}
/// Trace: FR-036-AC-4
#[test]
fn model_labels_are_scored_when_another_model_answers() {
    let mut data = dataset(&[true, false]);
    for case in &mut data.cases {
        case.label_provenance = provenance(LabelKind::Model, "labeler:1b");
    }
    data.validate(100).unwrap();
    let report = measure_boolean(
        &data,
        &answered_by("judge:30b", &[Value::Boolean(true), Value::Boolean(true)]),
    );
    assert!(report.self_source.is_empty());
    assert_eq!(report.outputs["result"].labelled, 2);
    assert_eq!(report.outputs["result"].scored, 2);
    assert_eq!(
        report.predictions[0].label_provenance.kind,
        LabelKind::Model
    );
}
/// Trace: FR-036-AC-5
#[test]
fn a_model_is_never_scored_against_its_own_labels() {
    let mut data = dataset(&[true, false, true]);
    data.cases[0].label_provenance = provenance(LabelKind::Model, "judge:30b");
    data.cases[1].label_provenance = provenance(LabelKind::Model, "labeler:renamed");
    let values = [
        Value::Boolean(true),
        Value::Boolean(false),
        Value::Boolean(true),
    ];
    let report = measure_boolean(&data, &answered_by("judge:30b", &values));
    assert_eq!(report.self_source, [data.cases[0].id.clone()]);
    let result = &report.outputs["result"];
    assert_eq!(
        (
            result.labelled,
            result.scored,
            result.unscored,
            result.failed
        ),
        (2, 2, 0, 0)
    );
    assert_eq!(report.predictions.len(), 2);
    assert_eq!(report.predictions[0].case, data.cases[1].id);
    assert_eq!(report.selected_cases, 3);
    assert!(report.complete());

    // A human label is never self-sourced, whichever model answered.
    let human_only = dataset(&[true]);
    let report = measure_boolean(
        &human_only,
        &answered_by("reviewer-1", &[Value::Boolean(true)]),
    );
    assert!(report.self_source.is_empty());
    assert_eq!(report.outputs["result"].scored, 1);

    // A different model name does not make the failed case self-sourced.
    let mut failed = outcomes(&values[..1]);
    failed.insert(
        data.cases[1].id.clone(),
        CaseOutcome::Failed {
            error: SaphoError::new(ErrorCode::BackendFailed, "down"),
            models: vec![ModelIdentity {
                name: "judge:other-tag".into(),
            }],
        },
    );
    let report = measure_boolean(&data, &failed);
    assert!(report.self_source.is_empty());
    assert!(report.outputs["result"].failed >= 1);
}
