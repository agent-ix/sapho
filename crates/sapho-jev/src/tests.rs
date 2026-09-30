// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Contract tests at the real SDK transport seam; no sockets or credentials.
use super::*;
use sapho_core::{
    BackendId, ChoiceOption, Datum, DistributionState, NamedQuestion, SourceId, SourceRef, Value,
    validate_response,
};
use std::sync::Arc;
use typesafe_sdk_config::Builder;
use typesafe_sdk_env::Fixed;
use typesafe_sdk_http::{Exchange, Mock};
fn request() -> ModelRequest {
    let mut item = Datum::new("local-item", Value::Text("synthetic context".into())).unwrap();
    item.sources.push(SourceRef {
        source: SourceId::new("opaque-source").unwrap(),
        start: Some(0),
        end: Some(17),
    });
    ModelRequest {
        backend: BackendId::new("hosted").unwrap(),
        model: "requested-alias".into(),
        expected_model: None,
        state: Value::Record(BTreeMap::from([(
            "candidates".into(),
            Value::List(vec![item]),
        )])),
        questions: vec![
            NamedQuestion {
                id: "boolean".into(),
                question: Question::Boolean {
                    instructions: "Is the declared property present?".into(),
                    yes: "Property present".into(),
                    no: "Property absent".into(),
                },
            },
            NamedQuestion {
                id: "choice".into(),
                question: Question::Choice {
                    instructions: "Select the applicable label".into(),
                    options: vec![
                        ChoiceOption {
                            label: "z-last".into(),
                            description: "First offered alternative".into(),
                        },
                        ChoiceOption {
                            label: "a-first".into(),
                            description: "Second offered alternative".into(),
                        },
                        ChoiceOption {
                            label: "middle".into(),
                            description: "Third alternative".into(),
                        },
                    ],
                },
            },
            NamedQuestion {
                id: "score".into(),
                question: Question::Score {
                    instructions: "Rate the declared strength".into(),
                    levels: vec!["absent".into(), "partial".into(), "complete".into()],
                },
            },
        ],
    }
}
const RESPONSE: &str = r#"{"model":"resolved-identity","answers":{"boolean":{"type":"noul","noul":0.8},"choice":{"type":"choice","choice":"z-last","confidence":0.7,"probabilities":{"z-last":0.7,"a-first":0.2}},"score":{"type":"score","score":1.4,"confidence":0.6,"legend":{"0":"absent","1":"partial","2":"complete"},"probabilities":{"0":0.1,"1":0.4,"2":0.5}}},"usage":{"input_tokens":42,"output_tokens":7}}"#;
fn backend(script: Vec<Exchange>) -> (JevBackend, Arc<Mock>) {
    let mock = Arc::new(Mock::new(script));
    let config = Builder::new()
        .api_key("synthetic-key")
        .base_url("https://invalid.example")
        .retry(RetryPolicy {
            max_retries: 5,
            ..RetryPolicy::default()
        })
        .build(&Fixed::default())
        .unwrap();
    (
        JevBackend::new(Client::with_transport(config, mock.clone())),
        mock,
    )
}
/// Trace: FR-025-AC-1, FR-025-AC-2, FR-025-AC-3
#[tokio::test]
async fn sdk_translation_preserves_order_rubrics_state_and_raw_answer_evidence() {
    let (backend, mock) = backend(vec![Exchange::ok(RESPONSE)]);
    let response = backend.infer(&request()).await.unwrap();
    assert_eq!(mock.attempts(), 1);
    let sent = mock.requests();
    let body = sent[0].body.as_ref().unwrap();
    let wire: serde_json::Value = serde_json::from_str(body).unwrap();
    assert_eq!(wire["model"], "requested-alias");
    assert_eq!(
        wire["state"]["candidates"],
        serde_json::json!(["synthetic context"])
    );
    assert!(!body.contains("opaque-source"));
    assert!(body.find("z-last").unwrap() < body.find("a-first").unwrap());
    assert_eq!(
        wire["questions"]["score"]["criteria"],
        serde_json::json!(["absent", "partial", "complete"])
    );
    assert_eq!(
        wire["questions"]["boolean"]["criteria"]["true"],
        "Property present"
    );
    assert_eq!(response.model, "resolved-identity");
    assert_eq!(
        response.usage,
        Some(Usage {
            input_tokens: 42,
            output_tokens: 7
        })
    );
    let answers = validate_response(&request(), &response).unwrap();
    assert_eq!(
        answers.distribution_state("choice").unwrap(),
        DistributionState::Partial
    );
    assert_eq!(
        answers
            .probability("choice", &["z-last".into()])
            .unwrap()
            .get(),
        0.7
    );
    assert_eq!(
        answers
            .probability("choice", &["middle".into()])
            .unwrap_err()
            .code,
        ErrorCode::UnsupportedDistribution
    );
    assert_eq!(
        answers.values["score"],
        Answer::Score {
            expected: 1.4,
            confidence: Probability::new(0.6).unwrap(),
            probabilities: Some(BTreeMap::from([
                ("0".into(), Probability::new(0.1).unwrap()),
                ("1".into(), Probability::new(0.4).unwrap()),
                ("2".into(), Probability::new(0.5).unwrap())
            ]))
        }
    );
    let mut strict = request();
    strict.expected_model = Some("requested-alias".into());
    assert_eq!(
        validate_response(&strict, &response).unwrap_err().code,
        ErrorCode::ModelMismatch
    );
}
/// Trace: FR-026-AC-1, FR-026-AC-2, FR-026-AC-3
#[tokio::test]
async fn sdk_failures_are_classified_sanitized_and_never_retried() {
    for (status, code) in [
        (401, ErrorCode::Unauthorized),
        (403, ErrorCode::Unauthorized),
        (422, ErrorCode::ServiceValidation),
        (429, ErrorCode::RateLimited),
        (503, ErrorCode::BackendFailed),
    ] {
        let (backend, mock) = backend(vec![
            Exchange::status(status, "echoed synthetic-key"),
            Exchange::ok(RESPONSE),
        ]);
        let error = backend.infer(&request()).await.unwrap_err();
        assert_eq!(error.code, code);
        assert_eq!(mock.attempts(), 1);
        assert_eq!(error.context["http_status"], status.to_string());
        assert!(!format!("{error:?}").contains("synthetic-key"));
    }
    let (backend, mock) = backend(vec![
        Exchange::Fail(typesafe_sdk_error::Error::Connection {
            message: "synthetic-key".into(),
        }),
        Exchange::ok(RESPONSE),
    ]);
    assert_eq!(
        backend.infer(&request()).await.unwrap_err().code,
        ErrorCode::BackendFailed
    );
    assert_eq!(mock.attempts(), 1);
}
/// Trace: FR-005-AC-2, FR-026-AC-2
#[tokio::test]
async fn sdk_invalid_unit_probability_is_refused() {
    let invalid = RESPONSE.replace("\"noul\":0.8", "\"noul\":1.8");
    let (backend, mock) = backend(vec![Exchange::ok(&invalid)]);
    assert_eq!(
        backend.infer(&request()).await.unwrap_err().code,
        ErrorCode::InvalidAnswer
    );
    assert_eq!(mock.attempts(), 1);
    let mut invalid = request();
    invalid.questions[0].id.clear();
    assert_eq!(
        backend.infer(&invalid).await.unwrap_err().code,
        ErrorCode::InvalidValue
    );
    assert_eq!(mock.attempts(), 1);
}
