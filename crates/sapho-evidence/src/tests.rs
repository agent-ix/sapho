// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Evidence acceptance tests with independently specified expected counts.
use super::*;
use sapho_core::{Datum, Degree, Probability};
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
