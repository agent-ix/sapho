// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Evidence acceptance tests with independently specified expected counts.
use super::*;
use sapho_core::{Datum, Degree, Probability};
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
                label_provenance: "human review / original source".into(),
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
                },
            )
        })
        .collect()
}
/// Trace: FR-036-AC-1, FR-036-AC-3
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
        "human review / original source"
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
    no_origin.cases[0].label_provenance = " ".into();
    assert!(matches!(
        no_origin.validate(100),
        Err(EvidenceError::MissingProvenance(_))
    ));
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
    let Metrics::Probability { brier: Some(score) } = report.outputs["result"].metrics else {
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
/// Trace: FR-037-AC-1, FR-037-AC-2, FR-037-AC-3
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
/// Trace: FR-038-AC-1, FR-038-AC-2, FR-038-AC-3
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
