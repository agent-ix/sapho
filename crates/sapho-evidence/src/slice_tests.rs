// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Synthetic slice and explicit-window acceptance fixtures.
use crate::*;
use sapho_core::{Datum, Degree, Probability};

fn fixture() -> (Dataset, BTreeMap<ItemId, CaseOutcome>) {
    let rows = [
        ("b2", "B", false, true),
        ("a2", "A", false, false),
        ("b1", "B", true, true),
        ("a1", "A", true, true),
    ];
    let cases = rows
        .iter()
        .map(|(id, slice, label, _)| Case {
            id: ItemId::new(*id).unwrap(),
            split: Split::Development,
            inputs: Inputs::from([(
                "repo".into(),
                Datum::new(*id, Value::Text((*slice).into())).unwrap(),
            )]),
            labels: BTreeMap::from([("support".into(), *label)]),
            label_provenance: LabelProvenance {
                kind: LabelKind::Human,
                source: "curator".into(),
                reference: "fixture".into(),
            },
        })
        .collect();
    let outcomes = rows
        .iter()
        .map(|(id, _, _, predicted)| {
            (
                ItemId::new(*id).unwrap(),
                CaseOutcome::Completed {
                    outputs: Inputs::from([(
                        "support".into(),
                        Datum::new(*id, Value::Boolean(*predicted)).unwrap(),
                    )]),
                    models: Vec::new(),
                },
            )
        })
        .collect();
    (
        Dataset {
            id: SourceId::new("synthetic-slices").unwrap(),
            cases,
        },
        outcomes,
    )
}
fn agreement(measurement: &Measurement) -> Option<f64> {
    match &measurement.outputs["support"].metrics {
        Metrics::Boolean { agreement, .. } => *agreement,
        _ => None,
    }
}

/// Trace: FR-067-AC-1, FR-067-AC-2, FR-067-AC-4, FR-067-AC-5, IT-011-SC-01, IT-011-SC-02
#[test]
fn slice_counts_deltas_windows_and_order_are_deterministic() {
    let (data, outcomes) = fixture();
    let schema = BTreeMap::from([("support".into(), ValueType::Boolean)]);
    let path = ["repo".into()];
    let report = report_slices(
        &data,
        Split::Development,
        &schema,
        &outcomes,
        &path,
        3,
        None,
        10,
    )
    .unwrap();
    assert_eq!(agreement(&report.overall), Some(0.75));
    assert_eq!(
        report
            .slices
            .iter()
            .map(|slice| (
                &slice.key,
                slice.selected_cases,
                slice.too_small,
                agreement(&slice.measurement),
                slice.deltas["support"].agreement.value
            ))
            .collect::<Vec<_>>(),
        [
            (&SliceKey::Value("A".into()), 2, true, Some(1.0), Some(0.25)),
            (
                &SliceKey::Value("B".into()),
                2,
                true,
                Some(0.5),
                Some(-0.25)
            ),
        ]
    );
    let at_two = report_slices(
        &data,
        Split::Development,
        &schema,
        &outcomes,
        &path,
        2,
        None,
        10,
    )
    .unwrap();
    assert!(at_two.slices.iter().all(|slice| !slice.too_small));
    let mut reversed = data.clone();
    reversed.cases.reverse();
    let reordered = report_slices(
        &reversed,
        Split::Development,
        &schema,
        &outcomes,
        &path,
        3,
        None,
        10,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_vec(&report).unwrap(),
        serde_json::to_vec(&reordered).unwrap()
    );
    let windows = WindowSpec {
        reference: "reference".into(),
        current: "current".into(),
        assignments: ["a1", "a2"]
            .into_iter()
            .map(|id| WindowAssignment {
                case: ItemId::new(id).unwrap(),
                window: "reference".into(),
            })
            .chain(["b1", "b2"].into_iter().map(|id| WindowAssignment {
                case: ItemId::new(id).unwrap(),
                window: "current".into(),
            }))
            .collect(),
    };
    let with_windows = report_slices(
        &data,
        Split::Development,
        &schema,
        &outcomes,
        &path,
        3,
        Some(&windows),
        10,
    )
    .unwrap();
    let window = with_windows.windows.unwrap();
    assert_eq!(window.deltas["support"].agreement.value, Some(-0.5));
    assert_eq!(
        (
            window.slices[0].reference_count,
            window.slices[0].current_count
        ),
        (2, 0)
    );
    assert_eq!(
        window.slices[0].deltas["support"]
            .agreement
            .absent_reason
            .as_deref(),
        Some("missing_window_slice")
    );
    let mut duplicate = windows.clone();
    duplicate.assignments.push(duplicate.assignments[0].clone());
    assert!(matches!(
        report_slices(
            &data,
            Split::Development,
            &schema,
            &outcomes,
            &path,
            3,
            Some(&duplicate),
            10
        ),
        Err(EvidenceError::InvalidWindowMembership)
    ));
    duplicate.assignments.pop();
    duplicate.assignments.pop();
    assert!(matches!(
        report_slices(
            &data,
            Split::Development,
            &schema,
            &outcomes,
            &path,
            3,
            Some(&duplicate),
            10
        ),
        Err(EvidenceError::InvalidWindowMembership)
    ));
    duplicate.assignments.push(WindowAssignment {
        case: ItemId::new("unknown").unwrap(),
        window: "current".into(),
    });
    assert!(matches!(
        report_slices(
            &data,
            Split::Development,
            &schema,
            &outcomes,
            &path,
            3,
            Some(&duplicate),
            10
        ),
        Err(EvidenceError::InvalidWindowMembership)
    ));
}

/// Trace: FR-067-AC-3, FR-067-AC-7, IT-011-SC-02, IT-011-SC-08
#[test]
fn missing_slice_is_distinct_and_invalid_present_values_refuse() {
    let (mut data, outcomes) = fixture();
    data.cases[0].inputs.clear();
    data.cases[1].inputs.insert(
        "repo".into(),
        Datum::new("literal", Value::Text("missing".into())).unwrap(),
    );
    let encoded = serde_json::to_vec(&data).unwrap();
    assert_eq!(serde_json::from_slice::<Dataset>(&encoded).unwrap(), data);
    data.validate(10).unwrap();
    let schema = BTreeMap::from([("support".into(), ValueType::Boolean)]);
    let report = report_slices(
        &data,
        Split::Development,
        &schema,
        &outcomes,
        &["repo".into()],
        2,
        None,
        10,
    )
    .unwrap();
    assert_eq!(report.slices[0].key, SliceKey::Missing);
    assert!(
        report
            .slices
            .iter()
            .any(|slice| slice.key == SliceKey::Value("missing".into()))
    );
    assert_eq!(
        report
            .slices
            .iter()
            .map(|slice| slice.selected_cases)
            .sum::<u32>(),
        4
    );
    data.cases[0].inputs.insert(
        "repo".into(),
        Datum::new("bad", Value::Degree(Degree::new(0.8).unwrap())).unwrap(),
    );
    assert!(
        matches!(report_slices(&data, Split::Development, &schema, &outcomes, &["repo".into()], 2, None, 10), Err(EvidenceError::InvalidSliceValue { case, path }) if case == ItemId::new("b2").unwrap() && path == "repo")
    );
    data.cases[0].inputs.insert(
        "repo".into(),
        Datum::new("blank", Value::Text("  ".into())).unwrap(),
    );
    assert!(matches!(
        report_slices(
            &data,
            Split::Development,
            &schema,
            &outcomes,
            &["repo".into()],
            2,
            None,
            10
        ),
        Err(EvidenceError::InvalidSliceValue { .. })
    ));
}

/// Trace: FR-067-AC-3, IT-011-SC-02
#[test]
fn nested_record_paths_and_optional_absence_keep_case_identity() {
    let (mut data, outcomes) = fixture();
    for case in &mut data.cases {
        let Value::Text(value) = &case.inputs["repo"].value else {
            panic!("Text repo expected");
        };
        case.inputs.insert(
            "repo".into(),
            Datum::new(
                "nested",
                Value::Record(BTreeMap::from([(
                    "team".into(),
                    Value::Optional(Some(Box::new(Value::Text(value.clone())))),
                )])),
            )
            .unwrap(),
        );
    }
    data.cases[0].inputs.insert(
        "repo".into(),
        Datum::new("absent", Value::Optional(None)).unwrap(),
    );
    let schema = BTreeMap::from([("support".into(), ValueType::Boolean)]);
    let path = ["repo".into(), "team".into()];
    let report = report_slices(
        &data,
        Split::Development,
        &schema,
        &outcomes,
        &path,
        2,
        None,
        10,
    )
    .unwrap();
    assert_eq!(report.slices[0].key, SliceKey::Missing);
    assert_eq!(
        report
            .slices
            .iter()
            .map(|slice| slice.selected_cases)
            .sum::<u32>(),
        4
    );
    data.cases[0].inputs.insert(
        "repo".into(),
        Datum::new("wrong", Value::Boolean(true)).unwrap(),
    );
    assert!(
        matches!(report_slices(&data, Split::Development, &schema, &outcomes, &path, 2, None, 10), Err(EvidenceError::InvalidSliceValue { case, path }) if case == ItemId::new("b2").unwrap() && path == "repo.team")
    );
}

/// Trace: FR-067-AC-6, IT-011-SC-03
#[test]
fn probability_slice_reuses_shared_ece_and_risk_rows() {
    let (data, _) = fixture();
    let values = [("a1", 0.9), ("a2", 0.2), ("b1", 0.7), ("b2", 0.8)];
    let outcomes = values
        .into_iter()
        .map(|(id, p)| {
            (
                ItemId::new(id).unwrap(),
                CaseOutcome::Completed {
                    outputs: Inputs::from([(
                        "support".into(),
                        Datum::new(id, Value::Probability(Probability::new(p).unwrap())).unwrap(),
                    )]),
                    models: Vec::new(),
                },
            )
        })
        .collect();
    let schema = BTreeMap::from([("support".into(), ValueType::Probability)]);
    let report = report_slices(
        &data,
        Split::Development,
        &schema,
        &outcomes,
        &["repo".into()],
        2,
        None,
        10,
    )
    .unwrap();
    for slice in &report.slices {
        let subset = Dataset {
            id: data.id.clone(),
            cases: data
                .cases
                .iter()
                .filter(|case| {
                    let Value::Text(value) = &case.inputs["repo"].value else {
                        return false;
                    };
                    slice.key == SliceKey::Value(value.clone())
                })
                .cloned()
                .collect(),
        };
        let ordinary = measure(&subset, Split::Development, &schema, &outcomes, 10).unwrap();
        assert_eq!(
            slice.measurement.outputs["support"].metrics,
            ordinary.outputs["support"].metrics
        );
        let Metrics::Probability {
            ece, risk_coverage, ..
        } = &slice.measurement.outputs["support"].metrics
        else {
            panic!("Probability expected");
        };
        assert!(ece.is_some());
        assert_eq!(risk_coverage.len(), 6);
        assert_eq!(slice.deltas["support"].risk_coverage.len(), 6);
    }
}
