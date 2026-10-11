// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Hand-counted label-free confidence movement fixtures.
use crate::*;

fn selector() -> DriftSelector {
    DriftSelector {
        binding: sapho_core::BackendId::new("judge").unwrap(),
        question_id: "q".into(),
        actual_model: "actual".into(),
    }
}
fn window(name: &str, confidences: Vec<f64>) -> ConfidenceWindow {
    ConfidenceWindow {
        name: name.into(),
        retained_exchanges: u32::try_from(confidences.len()).unwrap(),
        skipped_nonmatching: 0,
        skipped_missing_question: 0,
        confidences,
    }
}

/// Trace: FR-068-AC-1, FR-068-AC-2, FR-068-AC-5, IT-011-SC-04, IT-011-SC-07
#[test]
fn confidence_means_shares_and_order_are_exact() {
    let reference = window("reference", vec![0.6, 0.8]);
    let current = window("current", vec![0.7, 0.9]);
    let report =
        compare_confidence_windows(selector(), 0.8, reference.clone(), current.clone()).unwrap();
    assert_eq!(
        (
            report.reference.matching_questions,
            report.current.matching_questions
        ),
        (2, 2)
    );
    assert!((report.reference.mean_confidence.unwrap() - 0.7).abs() < 1e-12);
    assert!((report.current.mean_confidence.unwrap() - 0.8).abs() < 1e-12);
    assert_eq!(
        (
            report.reference.high_confidence_share,
            report.current.high_confidence_share
        ),
        (Some(0.5), Some(0.5))
    );
    assert!((report.mean_delta.unwrap() - 0.1).abs() < 1e-12);
    assert!((report.mean_distance.unwrap() - 0.1).abs() < 1e-12);
    assert_eq!(
        (report.share_delta, report.share_distance),
        (Some(0.0), Some(0.0))
    );
    let reversed = compare_confidence_windows(
        selector(),
        0.8,
        window("reference", vec![0.8, 0.6]),
        window("current", vec![0.9, 0.7]),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_vec(&report).unwrap(),
        serde_json::to_vec(&reversed).unwrap()
    );
    assert!(!serde_json::to_string(&report).unwrap().contains("label"));
}

/// Trace: FR-068-AC-3, FR-068-AC-4, FR-068-AC-6, IT-011-SC-05, IT-011-SC-07
#[test]
fn repeats_count_empty_is_absent_and_invalid_values_refuse() {
    let repeated = compare_confidence_windows(
        selector(),
        0.8,
        window("reference", vec![0.6, 0.8, 0.6]),
        window("current", vec![0.7, 0.9]),
    )
    .unwrap();
    assert_eq!(repeated.reference.matching_questions, 3);
    assert!((repeated.reference.mean_confidence.unwrap() - 2.0 / 3.0).abs() < 1e-12);
    assert!((repeated.reference.high_confidence_share.unwrap() - 1.0 / 3.0).abs() < 1e-12);
    let empty = compare_confidence_windows(
        selector(),
        0.8,
        window("reference", vec![]),
        window("current", vec![0.7]),
    )
    .unwrap();
    assert_eq!(empty.reference.mean_confidence, None);
    assert_eq!(
        (
            empty.mean_delta,
            empty.mean_distance,
            empty.share_delta,
            empty.share_distance
        ),
        (None, None, None, None)
    );
    assert_eq!(
        empty.absent_reason.as_deref(),
        Some("insufficient_observations")
    );
    for invalid in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
        assert!(matches!(
            compare_confidence_windows(
                selector(),
                invalid,
                window("reference", vec![0.6]),
                window("current", vec![0.7])
            ),
            Err(EvidenceError::InvalidDriftInput)
        ));
        assert!(matches!(
            compare_confidence_windows(
                selector(),
                0.8,
                window("reference", vec![invalid]),
                window("current", vec![0.7])
            ),
            Err(EvidenceError::InvalidDriftInput)
        ));
    }
    let mut inconsistent = window("reference", vec![0.6]);
    inconsistent.retained_exchanges = 2;
    assert!(matches!(
        compare_confidence_windows(selector(), 0.8, inconsistent, window("current", vec![0.7])),
        Err(EvidenceError::InvalidDriftInput)
    ));
    let mut missing = selector();
    missing.question_id.clear();
    assert!(matches!(
        compare_confidence_windows(
            missing,
            0.8,
            window("reference", vec![0.6]),
            window("current", vec![0.7])
        ),
        Err(EvidenceError::InvalidDriftInput)
    ));
}

/// Trace: FR-068-AC-4, IT-011-SC-07
#[test]
fn drift_report_identities_have_a_byte_ceiling() {
    let max = "x".repeat(MAX_DRIFT_IDENTITY_BYTES);
    assert!(
        compare_confidence_windows(
            DriftSelector {
                binding: sapho_core::BackendId::new(max.clone()).unwrap(),
                question_id: max.clone(),
                actual_model: max.clone()
            },
            0.8,
            window(&max, vec![]),
            window("current", vec![]),
        )
        .is_ok()
    );
    let over = "x".repeat(MAX_DRIFT_IDENTITY_BYTES + 1);
    for bad in [
        DriftSelector {
            binding: sapho_core::BackendId::new(over.clone()).unwrap(),
            ..selector()
        },
        DriftSelector {
            question_id: over.clone(),
            ..selector()
        },
        DriftSelector {
            actual_model: over.clone(),
            ..selector()
        },
    ] {
        assert!(matches!(
            compare_confidence_windows(
                bad,
                0.8,
                window("reference", vec![]),
                window("current", vec![])
            ),
            Err(EvidenceError::InvalidDriftInput)
        ));
    }
    assert!(matches!(
        compare_confidence_windows(
            selector(),
            0.8,
            window(&over, vec![]),
            window("current", vec![])
        ),
        Err(EvidenceError::InvalidDriftInput)
    ));
}
