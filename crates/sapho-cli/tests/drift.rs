// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Bounded public host projection of valid recordings into label-free drift.
use sapho_cli::{compare_recordings, project_recording_confidences};
use sapho_core::{
    Answer, BackendId, ChoiceOption, DistributionPolicy, ModelRequest, ModelResponse,
    NamedQuestion, Probability, Question, Value,
};
use sapho_evidence::DriftSelector;
use sapho_recording::{Exchange, Recording};
use std::collections::BTreeMap;

fn boolean_question(wording: &str) -> Question {
    Question::Boolean {
        instructions: wording.into(),
        yes: "yes".into(),
        no: "no".into(),
    }
}
fn exchange(
    binding: &str,
    actual_model: &str,
    question_id: &str,
    question: Question,
    answer: Answer,
) -> Exchange {
    Exchange {
        request: ModelRequest {
            distribution_policy: DistributionPolicy::Strict {},
            backend: BackendId::new(binding).unwrap(),
            model: "requested".into(),
            expected_model: None,
            state: Value::Record(BTreeMap::new()),
            questions: vec![NamedQuestion {
                id: question_id.into(),
                question,
            }],
        },
        response: ModelResponse {
            model: actual_model.into(),
            raw: None,
            answers: BTreeMap::from([(question_id.into(), answer)]),
            usage: None,
        },
    }
}
fn boolean(p: f64) -> Answer {
    Answer::Boolean {
        probability: Probability::new(p).unwrap(),
    }
}
fn selector() -> DriftSelector {
    DriftSelector {
        binding: BackendId::new("judge").unwrap(),
        question_id: "q".into(),
        actual_model: "actual".into(),
    }
}
fn bytes(exchanges: Vec<Exchange>) -> Vec<u8> {
    Recording { exchanges }.to_json(1_048_576).unwrap()
}

/// Trace: FR-068-AC-1, FR-068-AC-2, FR-068-AC-5, FR-068-AC-6, IT-011-SC-04, IT-011-SC-05, IT-011-SC-07
#[test]
fn validated_recording_projection_counts_repeats_skips_and_order() {
    let question = boolean_question("Is it true?");
    let low = exchange("judge", "actual", "q", question.clone(), boolean(0.6));
    let high = exchange("judge", "actual", "q", question.clone(), boolean(0.8));
    let missing = exchange(
        "judge",
        "actual",
        "other",
        boolean_question("Other?"),
        boolean(0.4),
    );
    let other = exchange("other", "actual", "q", question.clone(), boolean(0.3));
    let reference = bytes(vec![low.clone(), high.clone(), missing, other]);
    let current = bytes(vec![
        exchange("judge", "actual", "q", question.clone(), boolean(0.7)),
        exchange("judge", "actual", "q", question.clone(), boolean(0.9)),
    ]);
    let projected =
        project_recording_confidences(&reference, 1_048_576, "reference", &selector(), &question)
            .unwrap();
    assert_eq!(
        (
            projected.retained_exchanges,
            projected.skipped_nonmatching,
            projected.skipped_missing_question,
            projected.confidences.len()
        ),
        (4, 1, 1, 2)
    );
    let report = compare_recordings(
        &reference,
        &current,
        1_048_576,
        "reference",
        "current",
        selector(),
        &question,
        0.8,
    )
    .unwrap();
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
    assert_eq!(report.share_distance, Some(0.0));
    let mut retained = Recording::from_json(&reference, 1_048_576).unwrap();
    retained.exchanges.push(low);
    let repeated = retained.to_json(1_048_576).unwrap();
    let repeated_report = compare_recordings(
        &repeated,
        &current,
        1_048_576,
        "reference",
        "current",
        selector(),
        &question,
        0.8,
    )
    .unwrap();
    assert_eq!(repeated_report.reference.matching_questions, 3);
    assert!((repeated_report.reference.mean_confidence.unwrap() - 2.0 / 3.0).abs() < 1e-12);
    assert!((repeated_report.reference.high_confidence_share.unwrap() - 1.0 / 3.0).abs() < 1e-12);
    retained.exchanges.reverse();
    let reordered = compare_recordings(
        &retained.to_json(1_048_576).unwrap(),
        &current,
        1_048_576,
        "reference",
        "current",
        selector(),
        &question,
        0.8,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_vec(&repeated_report).unwrap(),
        serde_json::to_vec(&reordered).unwrap()
    );
    let serialized = serde_json::to_string(&report).unwrap();
    for forbidden in [
        "answers",
        "state",
        "raw",
        "credential",
        "https://",
        "label",
        "timestamp",
    ] {
        assert!(
            !serialized.contains(forbidden),
            "report exposed {forbidden}"
        );
    }
}

/// Trace: FR-068-AC-3, FR-068-AC-4, FR-068-AC-7, IT-011-SC-06, IT-011-SC-07
#[test]
fn incompatible_recordings_refuse_and_empty_drift_is_absent() {
    let question = boolean_question("Is it true?");
    let reference = bytes(vec![exchange(
        "judge",
        "actual",
        "q",
        question.clone(),
        boolean(0.6),
    )]);
    let empty = Recording::default().to_json(1_048_576).unwrap();
    let absent = compare_recordings(
        &reference,
        &empty,
        1_048_576,
        "reference",
        "current",
        selector(),
        &question,
        0.8,
    )
    .unwrap();
    assert_eq!(
        absent.absent_reason.as_deref(),
        Some("insufficient_observations")
    );
    assert_eq!((absent.mean_delta, absent.share_delta), (None, None));
    let changed = bytes(vec![exchange(
        "judge",
        "actual",
        "q",
        boolean_question("Different wording?"),
        boolean(0.7),
    )]);
    assert!(
        compare_recordings(
            &reference,
            &changed,
            1_048_576,
            "reference",
            "current",
            selector(),
            &question,
            0.8
        )
        .is_err()
    );
    let choice = Question::Choice {
        instructions: "Choose".into(),
        options: vec![
            ChoiceOption {
                label: "a".into(),
                description: "First".into(),
            },
            ChoiceOption {
                label: "b".into(),
                description: "Second".into(),
            },
        ],
    };
    let choice_recording = bytes(vec![exchange(
        "judge",
        "actual",
        "q",
        choice,
        Answer::Choice {
            selected: "a".into(),
            confidence: Probability::new(0.7).unwrap(),
            probabilities: Some(BTreeMap::from([
                ("a".into(), Probability::new(0.7).unwrap()),
                ("b".into(), Probability::new(0.3).unwrap()),
            ])),
        },
    )]);
    assert!(
        compare_recordings(
            &reference,
            &choice_recording,
            1_048_576,
            "reference",
            "current",
            selector(),
            &question,
            0.8
        )
        .is_err()
    );
    assert!(
        compare_recordings(
            b"bad",
            &empty,
            1_048_576,
            "reference",
            "current",
            selector(),
            &question,
            0.8
        )
        .is_err()
    );
    assert!(
        compare_recordings(
            &reference,
            &empty,
            1,
            "reference",
            "current",
            selector(),
            &question,
            0.8
        )
        .is_err()
    );
    assert!(
        compare_recordings(
            &reference,
            &empty,
            1_048_576,
            "reference",
            "current",
            selector(),
            &question,
            f64::NAN
        )
        .is_err()
    );
}

/// Trace: FR-068-AC-2, FR-068-AC-7, IT-011-SC-06
#[test]
fn choice_and_score_use_reported_confidence_when_selected_consistently() {
    let choice = Question::Choice {
        instructions: "Choose".into(),
        options: vec![
            ChoiceOption {
                label: "a".into(),
                description: "First".into(),
            },
            ChoiceOption {
                label: "b".into(),
                description: "Second".into(),
            },
        ],
    };
    let choice_bytes = bytes(vec![exchange(
        "judge",
        "actual",
        "q",
        choice.clone(),
        Answer::Choice {
            selected: "a".into(),
            confidence: Probability::new(0.73).unwrap(),
            probabilities: None,
        },
    )]);
    let projected =
        project_recording_confidences(&choice_bytes, 1_048_576, "reference", &selector(), &choice)
            .unwrap();
    assert_eq!(projected.confidences, vec![0.73]);
    let score = Question::Score {
        instructions: "Score".into(),
        levels: vec!["Low".into(), "High".into()],
    };
    let score_bytes = bytes(vec![exchange(
        "judge",
        "actual",
        "q",
        score.clone(),
        Answer::Score {
            expected: 0.4,
            confidence: Probability::new(0.61).unwrap(),
            probabilities: None,
        },
    )]);
    let projected =
        project_recording_confidences(&score_bytes, 1_048_576, "current", &selector(), &score)
            .unwrap();
    assert_eq!(projected.confidences, vec![0.61]);
}

/// Trace: FR-068-AC-4, IT-011-SC-07
#[test]
fn selector_is_exact_and_oversized_identity_refuses_before_projection() {
    let question = boolean_question("Is it true?");
    let actual_model = "https://model.example/opaque-id";
    let transport_endpoint = "https://transport.example/secret-endpoint";
    let credential = "credential-sentinel-never-report";
    let chosen = DriftSelector {
        actual_model: actual_model.into(),
        ..selector()
    };
    let recording = bytes(vec![exchange(
        "judge",
        actual_model,
        "q",
        question.clone(),
        boolean(0.7),
    )]);
    let report = compare_recordings(
        &recording,
        &recording,
        1_048_576,
        "reference",
        "current",
        chosen.clone(),
        &question,
        0.8,
    )
    .unwrap();
    assert_eq!(report.selector.actual_model, actual_model);
    let serialized = serde_json::to_string(&report).unwrap();
    assert!(!serialized.contains(transport_endpoint));
    assert!(!serialized.contains(credential));
    let over = "x".repeat(sapho_evidence::MAX_DRIFT_IDENTITY_BYTES + 1);
    for bad in [
        DriftSelector {
            binding: BackendId::new(over.clone()).unwrap(),
            ..chosen.clone()
        },
        DriftSelector {
            question_id: over.clone(),
            ..chosen.clone()
        },
        DriftSelector {
            actual_model: over.clone(),
            ..chosen.clone()
        },
    ] {
        assert!(
            project_recording_confidences(
                b"invalid recording",
                1_048_576,
                "reference",
                &bad,
                &question
            )
            .is_err()
        );
    }
    assert!(
        project_recording_confidences(b"invalid recording", 1_048_576, &over, &chosen, &question)
            .is_err()
    );
}
