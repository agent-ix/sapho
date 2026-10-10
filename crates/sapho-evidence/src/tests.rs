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
/// Trace: FR-059-AC-1, FR-059-AC-2, FR-059-AC-3, FR-059-AC-4,
/// FR-059-AC-5, FR-059-AC-6, IT-008-SC-05, IT-008-SC-08
#[test]
fn shadow_comparison_uses_role_specific_case_sets_and_strict_margins() {
    let mut data = dataset(&[true, false, true, false]);
    for case in &mut data.cases {
        case.split = Split::HeldOut;
    }
    data.cases[3].label_provenance = provenance(LabelKind::Model, "shadow_model");
    let schemas = BTreeMap::from([("result".into(), ValueType::Boolean)]);
    let champion = outcomes(&[
        Value::Boolean(true),
        Value::Boolean(false),
        Value::Boolean(false),
        Value::Boolean(false),
    ]);
    let mut challenger = outcomes(&[
        Value::Boolean(true),
        Value::Boolean(false),
        Value::Boolean(true),
        Value::Boolean(false),
    ]);
    if let CaseOutcome::Completed { models, .. } = challenger.get_mut(&data.cases[3].id).unwrap() {
        models.push(ModelIdentity {
            name: "shadow_model".into(),
        });
    }
    let ordinary = measure(&data, Split::HeldOut, &schemas, &champion, 10).unwrap();
    assert_eq!(ordinary.outputs["result"].scored, 4);
    let request = ShadowComparisonRequest {
        output: "result".into(),
        shadow: "candidate".into(),
        metric: ShadowMetric::Agreement,
        min_scored: 1,
        margin: 0.0,
    };
    let compared = compare_shadow(
        &data,
        Split::HeldOut,
        &schemas,
        &champion,
        &challenger,
        &request,
        10,
    )
    .unwrap();
    assert_eq!(compared.kinds.len(), 2);
    let human = compared
        .kinds
        .iter()
        .find(|k| k.label_kind == LabelKind::Human)
        .unwrap();
    assert_eq!(human.champion.scored, 3);
    assert_eq!(human.challenger.scored, 3);
    assert_eq!(human.beat_champion, Some(true));
    let model = compared
        .kinds
        .iter()
        .find(|k| k.label_kind == LabelKind::Model)
        .unwrap();
    assert_eq!(model.champion.scored, 1);
    assert_eq!(model.challenger.self_source, vec![data.cases[3].id.clone()]);
    assert_eq!(model.beat_champion, None);
    assert_eq!(
        measure(&data, Split::HeldOut, &schemas, &champion, 10).unwrap(),
        ordinary
    );
    let mut tie = challenger.clone();
    tie.insert(
        data.cases[2].id.clone(),
        champion[&data.cases[2].id].clone(),
    );
    let tie_report = compare_shadow(
        &data,
        Split::HeldOut,
        &schemas,
        &champion,
        &tie,
        &request,
        10,
    )
    .unwrap();
    assert_eq!(
        tie_report
            .kinds
            .iter()
            .find(|k| k.label_kind == LabelKind::Human)
            .unwrap()
            .beat_champion,
        Some(false)
    );
    let margin = ShadowComparisonRequest {
        margin: 1.0 / 3.0,
        ..request.clone()
    };
    assert_eq!(
        compare_shadow(
            &data,
            Split::HeldOut,
            &schemas,
            &champion,
            &challenger,
            &margin,
            10
        )
        .unwrap()
        .kinds[0]
            .beat_champion,
        Some(true)
    );
    let too_high = ShadowComparisonRequest {
        margin: 0.34,
        ..request.clone()
    };
    assert_eq!(
        compare_shadow(
            &data,
            Split::HeldOut,
            &schemas,
            &champion,
            &challenger,
            &too_high,
            10
        )
        .unwrap()
        .kinds[0]
            .beat_champion,
        Some(false)
    );
    let ece = ShadowComparisonRequest {
        metric: ShadowMetric::Ece,
        ..request
    };
    assert_eq!(
        compare_shadow(
            &data,
            Split::HeldOut,
            &schemas,
            &champion,
            &challenger,
            &ece,
            10
        )
        .unwrap()
        .kinds[0]
            .beat_champion,
        None
    );
    // The existing Dataset schema and validation contract remain usable unchanged.
    let old_json = serde_json::to_vec(&data).unwrap();
    let decoded: Dataset = serde_json::from_slice(&old_json).unwrap();
    decoded.validate(10).unwrap();
    let mut development = data.clone();
    for case in &mut development.cases {
        case.split = Split::Development;
    }
    let development_report = compare_shadow(
        &development,
        Split::Development,
        &schemas,
        &champion,
        &challenger,
        &ShadowComparisonRequest {
            output: "result".into(),
            shadow: "candidate".into(),
            metric: ShadowMetric::Agreement,
            min_scored: 1,
            margin: 0.0,
        },
        10,
    )
    .unwrap();
    assert_eq!(
        development_report.promotion_status,
        ShadowPromotionStatus::NotPromotable
    );
}
/// Trace: FR-059-AC-1, FR-059-AC-2, FR-059-AC-3, IT-008-SC-05
#[test]
fn shadow_probability_brier_and_missing_coverage_have_explicit_absence() {
    let mut data = dataset(&[true, false]);
    for case in &mut data.cases {
        case.split = Split::HeldOut;
    }
    let schemas = BTreeMap::from([("result".into(), ValueType::Probability)]);
    let champion = outcomes(&[
        Value::Probability(Probability::new(0.6).unwrap()),
        Value::Probability(Probability::new(0.4).unwrap()),
    ]);
    let challenger = outcomes(&[
        Value::Probability(Probability::new(0.9).unwrap()),
        Value::Probability(Probability::new(0.1).unwrap()),
    ]);
    let request = ShadowComparisonRequest {
        output: "result".into(),
        shadow: "candidate".into(),
        metric: ShadowMetric::Brier,
        min_scored: 2,
        margin: 0.1,
    };
    let report = compare_shadow(
        &data,
        Split::HeldOut,
        &schemas,
        &champion,
        &challenger,
        &request,
        10,
    )
    .unwrap();
    let kind = &report.kinds[0];
    assert_eq!(kind.champion.scored_cases, kind.challenger.scored_cases);
    assert_eq!(kind.matched_cases.len(), 2);
    assert!((kind.champion.brier.unwrap() - 0.16).abs() < 1e-12);
    assert!((kind.challenger.brier.unwrap() - 0.01).abs() < 1e-12);
    assert_eq!(kind.beat_champion, Some(true));
    assert_eq!(kind.champion.ece, None);
    assert_eq!(
        kind.champion.ece_reason.as_deref(),
        Some("shared_sapho_21_ece_unavailable")
    );
    let high_floor = ShadowComparisonRequest {
        min_scored: 3,
        ..request.clone()
    };
    assert_eq!(
        compare_shadow(
            &data,
            Split::HeldOut,
            &schemas,
            &champion,
            &challenger,
            &high_floor,
            10
        )
        .unwrap()
        .kinds[0]
            .beat_champion,
        None
    );
    let mut incomplete = challenger;
    incomplete.insert(
        data.cases[1].id.clone(),
        CaseOutcome::Failed {
            error: SaphoError::new(ErrorCode::ReplayMiss, "missing shadow"),
            models: Vec::new(),
        },
    );
    let report = compare_shadow(
        &data,
        Split::HeldOut,
        &schemas,
        &champion,
        &incomplete,
        &request,
        10,
    )
    .unwrap();
    assert_eq!(report.kinds[0].challenger.failed, 1);
    assert_eq!(report.kinds[0].beat_champion, None);
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
