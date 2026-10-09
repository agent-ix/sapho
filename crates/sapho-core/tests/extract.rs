// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Extractor port behaviour through the public core boundary (FR-048).
use sapho_core::{
    Completion, ErrorCode, ExtractError, ExtractRequest, ExtractUsage, ModelIdentity, RawExchange,
    SaphoError, ScriptedExtractor, TooLarge, extract,
};
use serde_json::{Value, json};

fn request(schema: Value) -> ExtractRequest {
    ExtractRequest {
        model: "m".into(),
        instructions: "Extract.".into(),
        input: "item one".into(),
        schema,
    }
}
fn object_schema() -> Value {
    json!({"type": "object", "properties": {"a": {"type": "integer"}}, "required": ["a"]})
}
fn raw() -> RawExchange {
    RawExchange {
        request: "req".into(),
        response: "resp".into(),
    }
}
fn completion(answer: &str) -> Completion {
    Completion {
        answer: answer.as_bytes().to_vec(),
        usage: ExtractUsage {
            input_tokens: Some(12),
            output_tokens: None,
            elapsed_ms: Some(340),
            ..ExtractUsage::default()
        },
        raw: raw(),
        model: ModelIdentity { name: "m".into() },
    }
}
fn too_large() -> ExtractError {
    ExtractError::new(ErrorCode::TooLarge, "Input does not fit").with_too_large(TooLarge {
        reported_input_tokens: Some(4112),
        context_tokens: 4096,
        reserved_output_tokens: 512,
    })
}

/// Trace: FR-048-AC-1
#[tokio::test]
async fn schema_valid_value_reaches_the_caller_unchanged() {
    let double = ScriptedExtractor::new([Ok(completion(r#"{"a": 3}"#))]);
    let response = extract(&double, &request(object_schema())).await.unwrap();
    let expected = completion("");
    assert_eq!(response.value, json!({"a": 3}));
    assert_eq!(response.usage, expected.usage);
    assert_eq!(response.usage.output_tokens, None);
    assert_eq!(response.raw, expected.raw);
    assert_eq!(response.model, expected.model);
}

/// Trace: FR-048-AC-2
#[tokio::test]
async fn violating_and_non_json_answers_are_invalid_with_pointer_and_exchange() {
    let double = ScriptedExtractor::new([
        Ok(completion(r#"{"a": "three"}"#)),
        Ok(completion("{not json")),
    ]);
    let violation = extract(&double, &request(object_schema()))
        .await
        .unwrap_err();
    assert_eq!(violation.code, ErrorCode::InvalidAnswer);
    assert_eq!(violation.reason, Some("schema_violation"));
    assert_eq!(violation.pointer.as_deref(), Some("/a"));
    assert_eq!(violation.raw.as_deref(), Some(&raw()));
    assert_eq!(violation.usage.as_deref(), Some(&completion("").usage));

    let torn = extract(&double, &request(object_schema()))
        .await
        .unwrap_err();
    assert_eq!(torn.code, ErrorCode::InvalidAnswer);
    assert_eq!(torn.reason, Some("not_json"));
    assert_eq!(torn.pointer, None);
    assert_eq!(torn.raw.as_deref(), Some(&raw()));
}

/// Trace: FR-048-AC-3
#[tokio::test]
async fn too_large_is_distinct_and_its_numbers_are_fields() {
    let double = ScriptedExtractor::new([
        Err(too_large()),
        Err(ExtractError::new(ErrorCode::BackendFailed, "down")),
        Err(ExtractError::new(ErrorCode::DeadlineExceeded, "slow")),
        Err(ExtractError::new(ErrorCode::LimitExceeded, "big body")),
    ]);
    let mut codes = Vec::new();
    for _ in 0..4 {
        codes.push(
            extract(&double, &request(object_schema()))
                .await
                .unwrap_err(),
        );
    }
    assert_eq!(
        codes.iter().map(|e| e.code).collect::<Vec<_>>(),
        [
            ErrorCode::TooLarge,
            ErrorCode::BackendFailed,
            ErrorCode::DeadlineExceeded,
            ErrorCode::LimitExceeded
        ]
    );
    assert_eq!(
        codes[0].too_large,
        Some(TooLarge {
            reported_input_tokens: Some(4112),
            context_tokens: 4096,
            reserved_output_tokens: 512
        })
    );
    assert!(codes[1..].iter().all(|e| e.too_large.is_none()));
    assert_eq!(
        serde_json::to_value(ErrorCode::TooLarge).unwrap(),
        json!("too_large")
    );
    let as_saphoerr: SaphoError = codes[0].clone().into();
    assert_eq!(as_saphoerr.code, ErrorCode::TooLarge);
    assert_eq!(as_saphoerr.context["reported_input_tokens"], "4112");
    assert_eq!(as_saphoerr.context["context_tokens"], "4096");
    assert_eq!(as_saphoerr.context["reserved_output_tokens"], "512");
}

/// Trace: FR-048-AC-4
#[tokio::test]
async fn invalid_requests_are_refused_before_the_implementation_runs() {
    let double = ScriptedExtractor::new([]);
    let mut empty_model = request(object_schema());
    empty_model.model.clear();
    let mut empty_instructions = request(object_schema());
    empty_instructions.instructions.clear();
    let mut empty_input = request(object_schema());
    empty_input.input.clear();
    let refused = [
        (empty_model, "empty_model"),
        (empty_instructions, "empty_instructions"),
        (empty_input, "empty_input"),
        (request(json!([1])), "schema_not_object"),
        (request(json!({"type": "nonsense"})), "schema_invalid"),
        (
            request(json!({"$ref": "https://example.invalid/s.json"})),
            "schema_external_ref",
        ),
        (
            request(json!({"properties": {"a": {"$ref": "other.json#/x"}}})),
            "schema_external_ref",
        ),
        (
            request(json!({"items": [{"$ref": "file:///etc/passwd"}]})),
            "schema_external_ref",
        ),
    ];
    for (bad, reason) in refused {
        let error = extract(&double, &bad).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::Config, "{reason}");
        assert_eq!(error.reason, Some(reason));
    }
    assert!(double.requests().is_empty());
}

/// Trace: FR-048-AC-4
#[tokio::test]
async fn references_inside_the_schema_document_are_followed() {
    let schema = json!({
        "$defs": {"n": {"type": "integer"}},
        "type": "object",
        "properties": {"a": {"$ref": "#/$defs/n"}},
    });
    let double = ScriptedExtractor::new([
        Ok(completion(r#"{"a": 1}"#)),
        Ok(completion(r#"{"a": "x"}"#)),
    ]);
    assert!(extract(&double, &request(schema.clone())).await.is_ok());
    let error = extract(&double, &request(schema)).await.unwrap_err();
    assert_eq!(error.pointer.as_deref(), Some("/a"));
}

/// Trace: FR-048-AC-5
#[tokio::test]
async fn scripted_double_records_requests_in_order() {
    let double = ScriptedExtractor::new([Ok(completion("{\"a\":1}")), Ok(completion("{\"a\":2}"))]);
    let mut first = request(object_schema());
    first.input = "first".into();
    let mut second = request(object_schema());
    second.input = "second".into();
    extract(&double, &first).await.unwrap();
    extract(&double, &second).await.unwrap();
    assert_eq!(double.requests(), [first, second]);
    let exhausted = extract(&double, &request(object_schema()))
        .await
        .unwrap_err();
    assert_eq!(exhausted.code, ErrorCode::BackendFailed);
}

/// Trace: FR-048-AC-6
#[test]
fn core_manifest_has_no_transport_runtime_and_validator_resolves_nothing() {
    let manifest = include_str!("../Cargo.toml");
    let dependencies = manifest
        .split("[dependencies]")
        .nth(1)
        .and_then(|rest| rest.split("[dev-dependencies]").next())
        .unwrap();
    for banned in [
        "tokio",
        "reqwest",
        "hyper",
        "ureq",
        "surf",
        "isahc",
        "async-std",
        "smol",
        "cap-std",
    ] {
        assert!(!dependencies.contains(banned), "{banned}");
    }
    let workspace = include_str!("../../../Cargo.toml");
    let validator = workspace
        .lines()
        .find(|line| line.starts_with("jsonschema"))
        .unwrap();
    assert!(validator.contains("default-features = false"));
    assert!(!validator.contains("resolve-"));
}

/// Trace: FR-048-AC-4
#[tokio::test]
async fn only_schema_keywords_are_references_not_property_names_or_data() {
    let double = ScriptedExtractor::new([Ok(completion(r#"{"$ref": "x"}"#))]);
    let schema = json!({
        "type": "object",
        "properties": {"$ref": {"type": "string", "const": {"$ref": "other.json"}}},
        "default": {"$ref": "elsewhere.json"},
        "enum": [{"$ref": "data"}, {"$ref": "http://example.invalid/x"}, {}],
    });
    let response = extract(&double, &request(schema)).await.unwrap_err();
    // The enum above never matches the answer; the point is that it was not refused as Config.
    assert_eq!(response.code, ErrorCode::InvalidAnswer);

    let nested = json!({"properties": {"a": {"items": {"$ref": "other.json#/x"}}}});
    let error = extract(&double, &request(nested)).await.unwrap_err();
    assert_eq!(error.reason, Some("schema_external_ref"));
    let listed = json!({"anyOf": [{"type": "string"}, {"$ref": "https://example.invalid/s"}]});
    let error = extract(&double, &request(listed)).await.unwrap_err();
    assert_eq!(error.reason, Some("schema_external_ref"));
}

/// Trace: FR-006-AC-5
#[test]
fn errors_keep_the_exchange_and_usage_in_fields_but_never_print_or_serialize_them() {
    let sentinel = "SENTINEL-BODY-7731";
    let planted = RawExchange {
        request: format!("{{\"in\":\"{sentinel}\"}}"),
        response: format!("{{\"out\":\"{sentinel}\"}}"),
    };
    let usage = completion("").usage;
    let error: SaphoError = too_large()
        .with_raw(planted.clone())
        .with_usage(usage.clone())
        .into();
    assert_eq!(error.raw.as_deref(), Some(&planted));
    assert_eq!(error.usage.as_deref(), Some(&usage));
    for text in [
        error.to_string(),
        format!("{error:?}"),
        format!("{error:#?}"),
        serde_json::to_string(&error).unwrap(),
    ] {
        assert!(!text.contains(sentinel), "{text}");
    }
    let json = serde_json::to_value(&error).unwrap();
    let members: std::collections::BTreeSet<_> = json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(members, ["code", "context", "message"].into());
    let mut bare = error.clone();
    bare.raw = None;
    bare.usage = None;
    assert_eq!(error, bare, "evidence is not part of an error's identity");
}
