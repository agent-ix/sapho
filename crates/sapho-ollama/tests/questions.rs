// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Typed questions answered from log-probabilities (FR-054, IT-007-SC-06).
mod common;
use common::*;
use sapho_core::{
    Answer, BackendId, ChoiceOption, DistributionPolicy, ErrorCode, ModelBackend, ModelRequest,
    ModelResponse, NamedQuestion, Question, SaphoError, Value, validate_response,
};
use sapho_ollama::{OllamaBackend, Server};
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;

const MODEL: &str = "qwen3:30b";

fn boolean(id: &str) -> NamedQuestion {
    NamedQuestion {
        id: id.into(),
        question: Question::Boolean {
            instructions: "Is it a fruit?".into(),
            yes: "a fruit".into(),
            no: "not a fruit".into(),
        },
    }
}
fn choice(id: &str, labels: &[&str]) -> NamedQuestion {
    NamedQuestion {
        id: id.into(),
        question: Question::Choice {
            instructions: "Which kind?".into(),
            options: labels
                .iter()
                .map(|l| ChoiceOption {
                    label: (*l).into(),
                    description: format!("the {l} kind"),
                })
                .collect(),
        },
    }
}
fn score(id: &str, levels: usize) -> NamedQuestion {
    NamedQuestion {
        id: id.into(),
        question: Question::Score {
            instructions: "How ripe?".into(),
            levels: (0..levels).map(|i| format!("level {i}")).collect(),
        },
    }
}
fn ask(questions: Vec<NamedQuestion>, policy: DistributionPolicy) -> ModelRequest {
    ModelRequest {
        distribution_policy: policy,
        backend: BackendId::new("ollama").unwrap(),
        model: MODEL.into(),
        expected_model: None,
        state: Value::Record(BTreeMap::from([
            ("zeta".into(), Value::Text("last".into())),
            ("alpha".into(), Value::Number(1.5)),
        ])),
        questions,
    }
}
/// A generated token with its own log-probability and the alternatives the server lists.
fn tok(text: &str, logprob: f64, alternatives: &[(&str, f64)]) -> Json {
    json!({
        "token": text,
        "logprob": logprob,
        "bytes": text.as_bytes(),
        "top_logprobs": alternatives
            .iter()
            .map(|(t, l)| json!({"token": t, "logprob": l, "bytes": t.as_bytes()}))
            .collect::<Vec<_>>()
    })
}
/// A plain structural token with no interesting alternatives.
fn fill(text: &str) -> Json {
    tok(text, -0.01, &[])
}
fn reply(response: &str, tokens: Vec<Json>) -> Json {
    let mut out = generated(MODEL, response);
    out["logprobs"] = Json::Array(tokens);
    out
}
async fn server(answer: Json) -> Fake {
    serving(answer).await
}
fn binding(fake: &Fake, think: bool) -> OllamaBackend {
    OllamaBackend::new(
        Server::new(&fake.url, limits()).unwrap(),
        settings(MODEL, think, 4096, 512),
    )
    .unwrap()
}
async fn asked(fake: &Fake, questions: Vec<NamedQuestion>) -> Result<ModelResponse, SaphoError> {
    binding(fake, false)
        .infer(&ask(questions, DistributionPolicy::Strict {}))
        .await
}
fn boolean_p(response: &ModelResponse, id: &str) -> f64 {
    match &response.answers[id] {
        Answer::Boolean { probability } => probability.get(),
        other => panic!("{other:?}"),
    }
}
fn bool_reply(tokens: Vec<Json>) -> Json {
    reply(r#"{"b": "yes"}"#, tokens)
}
fn yes_tokens(alternatives: &[(&str, f64)]) -> Vec<Json> {
    vec![
        fill("{\""),
        fill("b"),
        fill("\": \""),
        tok("yes", -2.228, alternatives),
        fill("\"}"),
    ]
}

/// Trace: FR-054-AC-1, IT-007-SC-06
#[tokio::test]
async fn one_request_carries_the_fixed_system_text_canonical_state_and_ordered_schema() {
    let _serial = serial().await;
    let response = r#"{"q2": "beta", "q1": "yes", "q3": "1"}"#;
    let tokens = vec![
        fill(r#"{"q2": ""#),
        tok("beta", -0.5, &[("beta", -0.5), ("alpha", -1.2)]),
        fill(r#"", "q1": ""#),
        tok("yes", -0.3, &[("yes", -0.3), ("no", -1.4)]),
        fill(r#"", "q3": ""#),
        tok("1", -0.7, &[("1", -0.7), ("0", -1.0), ("2", -3.0)]),
        fill("\"}"),
    ];
    let fake = server(reply(response, tokens)).await;
    let questions = vec![
        choice("q2", &["alpha", "beta"]),
        boolean("q1"),
        score("q3", 3),
    ];
    let result = asked(&fake, questions).await.unwrap();
    assert_eq!(fake.generates().len(), 1);
    let sent = fake.generates()[0].json();
    assert_eq!(sent["think"], false);
    assert_eq!(sent["logprobs"], true);
    assert_eq!(sent["top_logprobs"], 20);
    assert_eq!(
        sent["system"],
        "Answer every question about the input. Reply with JSON only.\n\n\
         q2: Which kind?\nalpha: the alpha kind\nbeta: the beta kind\n\n\
         q1: Is it a fruit?\nyes: a fruit\nno: not a fruit\n\n\
         q3: How ripe?\n0: level 0\n1: level 1\n2: level 2"
    );
    assert_eq!(sent["prompt"], r#"{"alpha":1.5,"zeta":"last"}"#);
    assert_eq!(
        sent["format"],
        json!({
            "type": "object",
            "properties": {
                "q2": {"type": "string", "enum": ["alpha", "beta"]},
                "q1": {"type": "string", "enum": ["yes", "no"]},
                "q3": {"type": "string", "enum": ["0", "1", "2"]}
            },
            "required": ["q2", "q1", "q3"],
            "additionalProperties": false
        })
    );
    let body = String::from_utf8(fake.generates()[0].body.clone()).unwrap();
    let order: Vec<_> = ["\"q2\":{", "\"q1\":{", "\"q3\":{"]
        .iter()
        .map(|m| body.find(m).unwrap())
        .collect();
    assert!(order.is_sorted(), "properties follow question order");
    assert_eq!(result.answers.len(), 3);
    assert_eq!(result.model, MODEL);
}

/// Trace: FR-054-AC-2
#[tokio::test]
async fn boolean_probability_counts_each_token_once_and_ignores_forbidden_case() {
    let _serial = serial().await;
    let listed = [("yes", -2.228), ("no", -5.759), ("Yes", -0.162)];
    let fake = server(bool_reply(yes_tokens(&listed))).await;
    let response = asked(&fake, vec![boolean("b")]).await.unwrap();
    assert!((boolean_p(&response, "b") - 0.9715).abs() < 1e-4);

    let bounded = [("yes", -2.228), ("maybe", -9.68)];
    let fake = server(bool_reply(yes_tokens(&bounded))).await;
    let response = asked(&fake, vec![boolean("b")]).await.unwrap();
    let (yes, floor) = ((-2.228f64).exp(), (-9.68f64).exp());
    assert!((boolean_p(&response, "b") - yes / (yes + floor)).abs() < 1e-9);
}

/// Trace: FR-054-AC-3
#[tokio::test]
async fn choice_distribution_is_complete_when_listed_and_partial_under_the_bound() {
    let _serial = serial().await;
    let labels = ["alpha", "beta", "gamma"];
    let response = r#"{"c": "alpha"}"#;
    let tokens = |alternatives: &[(&str, f64)]| {
        vec![
            fill("{\""),
            fill("c"),
            fill("\": \""),
            tok("alpha", -0.6, alternatives),
            fill("\"}"),
        ]
    };
    let all = [("alpha", -0.6), ("beta", -1.6), ("gamma", -2.6)];
    let fake = server(reply(response, tokens(&all))).await;
    let result = asked(&fake, vec![choice("c", &labels)]).await.unwrap();
    let Answer::Choice {
        selected,
        confidence,
        probabilities,
    } = &result.answers["c"]
    else {
        panic!()
    };
    let probabilities = probabilities.as_ref().unwrap();
    assert_eq!(selected, "alpha");
    assert_eq!(probabilities.len(), 3);
    let sum: f64 = probabilities.values().map(|p| p.get()).sum();
    assert!((sum - 1.0).abs() < 1e-9);
    assert!((confidence.get() - probabilities["alpha"].get()).abs() < 1e-12);

    let some = [("alpha", -0.6), ("beta", -1.6), ("other", -4.0)];
    let fake = server(reply(response, tokens(&some))).await;
    let result = asked(&fake, vec![choice("c", &labels)]).await.unwrap();
    let Answer::Choice {
        confidence,
        probabilities,
        ..
    } = &result.answers["c"]
    else {
        panic!()
    };
    let probabilities = probabilities.as_ref().unwrap();
    assert_eq!(probabilities.len(), 2, "gamma has no attributed token");
    let (a, b, bound) = ((-0.6f64).exp(), (-1.6f64).exp(), (-4.0f64).exp());
    let z = a + b + bound;
    assert!((confidence.get() - a / z).abs() < 1e-9);
    assert!((probabilities["beta"].get() - b / z).abs() < 1e-9);
    assert!(!probabilities.contains_key("gamma"));
}

/// Trace: FR-054-AC-3
#[tokio::test]
async fn score_expected_value_weights_only_the_levels_with_mass() {
    let _serial = serial().await;
    let tokens = vec![
        fill("{\""),
        fill("s"),
        fill("\": \""),
        tok("2", -0.5, &[("2", -0.5), ("1", -1.5), ("9", -3.0)]),
        fill("\"}"),
    ];
    let fake = server(reply(r#"{"s": "2"}"#, tokens)).await;
    let result = asked(&fake, vec![score("s", 4)]).await.unwrap();
    let Answer::Score {
        expected,
        confidence,
        probabilities,
    } = &result.answers["s"]
    else {
        panic!()
    };
    let (p2, p1) = ((-0.5f64).exp(), (-1.5f64).exp());
    let level3 = (-1.5f64).exp().min(p2).min((-3.0f64).exp());
    let z = p2 + p1 + 2.0 * level3;
    let known = (p2 + p1) / z;
    assert!((expected - (2.0 * p2 / z + 1.0 * p1 / z) / known).abs() < 1e-9);
    assert!((confidence.get() - p2 / z).abs() < 1e-9);
    assert_eq!(probabilities.as_ref().unwrap().len(), 2);
}

/// Trace: FR-054-AC-4
#[tokio::test]
async fn missing_or_mismatched_log_probabilities_produce_no_answer() {
    let _serial = serial().await;
    let fake = server(generated(MODEL, r#"{"b": "yes"}"#)).await;
    let error = asked(&fake, vec![boolean("b")]).await.unwrap_err();
    assert_eq!(error.code, ErrorCode::Config);
    assert_eq!(error.context["reason"], "logprobs_unavailable");

    let torn = vec![
        fill("{\""),
        fill("b"),
        fill("\": \""),
        tok("no", -0.1, &[]),
        fill("\"}"),
    ];
    let fake = server(bool_reply(torn)).await;
    let error = asked(&fake, vec![boolean("b")]).await.unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidAnswer);
    assert_eq!(error.context["reason"], "logprobs_mismatch");
}

/// Trace: FR-054-AC-5
#[tokio::test]
async fn a_thinking_binding_is_refused_before_any_request() {
    let _serial = serial().await;
    let fake = server(bool_reply(yes_tokens(&[]))).await;
    let error = binding(&fake, true)
        .infer(&ask(vec![boolean("b")], DistributionPolicy::Strict {}))
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Config);
    assert_eq!(error.context["reason"], "think_unsupported_for_questions");
    assert_eq!(fake.total(), 0);
}

/// Trace: FR-054-AC-6
#[tokio::test]
async fn context_refusal_carries_the_three_numbers_as_context_fields() {
    let _serial = serial().await;
    let inner = json!({"error": {"code": 400, "type": "exceed_context_size_error", "n_prompt_tokens": 9000, "n_ctx": 4096}});
    let fake = Fake::start(move |r| match r.path.as_str() {
        "/api/show" => description(BLOB),
        _ => Reply::status(400, json!({"error": inner.to_string()})),
    })
    .await;
    let error = asked(&fake, vec![boolean("b")]).await.unwrap_err();
    assert_eq!(error.code, ErrorCode::TooLarge);
    assert_eq!(error.context["reported_input_tokens"], "9000");
    assert_eq!(error.context["context_tokens"], "4096");
    assert_eq!(error.context["reserved_output_tokens"], "512");
}

/// Trace: FR-054-AC-7, FR-052-AC-1
#[tokio::test]
async fn the_response_validates_under_both_policies_and_carries_the_digest() {
    let _serial = serial().await;
    let tokens = vec![
        fill("{\""),
        fill("c"),
        fill("\": \""),
        tok(
            "alpha",
            -0.6,
            &[("alpha", -0.6), ("beta", -1.6), ("gamma", -2.6)],
        ),
        fill("\"}"),
    ];
    let fake = server(reply(r#"{"c": "alpha"}"#, tokens)).await;
    let backend = binding(&fake, false);
    for policy in [
        DistributionPolicy::Strict {},
        DistributionPolicy::approximate(0.05).unwrap(),
    ] {
        let request = ask(vec![choice("c", &["alpha", "beta", "gamma"])], policy);
        let response = backend.infer(&request).await.unwrap();
        validate_response(&request, &response).unwrap();
        assert_eq!(
            response.digest.as_deref(),
            Some(format!("sha256:{BLOB}").as_str())
        );
        let usage = response.usage.unwrap();
        assert_eq!((usage.input_tokens, usage.output_tokens), (48, 8));
    }
}

/// Trace: FR-054-AC-8
#[tokio::test]
async fn answer_values_must_start_with_different_bytes_and_scores_have_ten_levels() {
    let _serial = serial().await;
    let fake = server(bool_reply(yes_tokens(&[]))).await;
    let error = asked(&fake, vec![choice("c", &["explanation", "example"])])
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Config);
    assert_eq!(error.context["reason"], "answer_values_not_distinct");
    let error = asked(&fake, vec![score("s", 11)]).await.unwrap_err();
    assert_eq!(error.code, ErrorCode::Config);
    assert_eq!(error.context["reason"], "too_many_levels");
    assert_eq!(fake.total(), 0);
    let ten = asked(&fake, vec![score("s", 10)]).await.unwrap_err();
    assert_ne!(
        ten.context.get("reason").map(String::as_str),
        Some("too_many_levels")
    );
}

/// Trace: FR-054-AC-8
#[tokio::test]
async fn a_one_byte_first_token_is_attributed_to_the_label_starting_with_that_byte() {
    let _serial = serial().await;
    let labels = ["explanation", "sample", "test"];
    let tokens = vec![
        fill("{\""),
        fill("c"),
        fill("\": \""),
        tok(
            "ex",
            -0.4,
            &[("ex", -0.4), ("e", -2.5), ("sa", -1.4), ("Ex", -0.2)],
        ),
        fill("planation"),
        fill("\"}"),
    ];
    let fake = server(reply(r#"{"c": "explanation"}"#, tokens)).await;
    let result = asked(&fake, vec![choice("c", &labels)]).await.unwrap();
    let Answer::Choice {
        probabilities,
        confidence,
        ..
    } = &result.answers["c"]
    else {
        panic!()
    };
    let probabilities = probabilities.as_ref().unwrap();
    let (ex, e, sa) = ((-0.4f64).exp(), (-2.5f64).exp(), (-1.4f64).exp());
    let bound = sa.min((-0.2f64).exp()).min(ex).min(e).min((-0.4f64).exp());
    let z = ex + e + sa + bound;
    assert!((probabilities["explanation"].get() - (ex + e) / z).abs() < 1e-9);
    assert!((probabilities["sample"].get() - sa / z).abs() < 1e-9);
    assert!(!probabilities.contains_key("test"));
    assert!((confidence.get() - (ex + e) / z).abs() < 1e-9);
}

/// Trace: FR-054-AC-2
#[tokio::test]
async fn a_token_that_also_carries_the_opening_quote_still_locates_the_answer() {
    let _serial = serial().await;
    let tokens = vec![
        fill("{\"b\":"),
        tok(
            " \"yes",
            -2.228,
            &[(" \"yes", -2.228), (" \"no", -5.759), (" \"Yes", -0.162)],
        ),
        fill("\"}"),
    ];
    let fake = server(bool_reply(tokens)).await;
    let response = asked(&fake, vec![boolean("b")]).await.unwrap();
    assert!((boolean_p(&response, "b") - 0.9715).abs() < 1e-4);
}

/// Trace: FR-054-AC-4
#[tokio::test]
async fn answers_that_are_not_one_allowed_string_per_question_are_refused() {
    let _serial = serial().await;
    for text in [
        r#"{"b": "maybe"}"#,
        r#"{"other": "yes"}"#,
        r#"{"b": "yes", "b": "no"}"#,
        r#"["yes"]"#,
    ] {
        let tokens = vec![tok(text, -0.1, &[])];
        let fake = server(reply(text, tokens)).await;
        let error = asked(&fake, vec![boolean("b")]).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidAnswer, "{text}");
    }
}

/// Trace: FR-054-AC-2
#[tokio::test]
async fn the_bound_falls_back_to_the_selected_mass_and_never_exceeds_it() {
    let _serial = serial().await;
    // No listed alternatives at all: the missing value is bounded by the selected one.
    let fake = server(bool_reply(yes_tokens(&[]))).await;
    let response = asked(&fake, vec![boolean("b")]).await.unwrap();
    assert!((boolean_p(&response, "b") - 0.5).abs() < 1e-12);
    // Every listed alternative is more likely than the selected token, so the selected
    // mass is the smaller bound.
    let fake = server(bool_reply(yes_tokens(&[("maybe", -0.1)]))).await;
    let response = asked(&fake, vec![boolean("b")]).await.unwrap();
    assert!((boolean_p(&response, "b") - 0.5).abs() < 1e-12);
}

/// Trace: FR-054-AC-2
#[tokio::test]
async fn a_listed_alternative_with_other_structural_bytes_is_left_to_the_bound() {
    let _serial = serial().await;
    let tokens = vec![
        fill("{\"b\":"),
        tok(" \"yes", -2.228, &[(" \"yes", -2.228), (":\"no", -1.0)]),
        fill("\"}"),
    ];
    let fake = server(bool_reply(tokens)).await;
    let response = asked(&fake, vec![boolean("b")]).await.unwrap();
    // `no` has no attributed token, so it takes the bound and the answer is even.
    assert!((boolean_p(&response, "b") - 0.5).abs() < 1e-12);
}

/// Trace: FR-054-AC-3
#[tokio::test]
async fn a_label_ending_in_a_quote_is_matched_by_its_escaped_form() {
    let _serial = serial().await;
    let tokens = vec![
        fill("{\"c\": \""),
        tok("a\\", -1.0, &[("a\\", -1.0), ("a\\\"", -0.7), ("b", -2.0)]),
        fill("\"\"}"),
    ];
    let fake = server(reply(r#"{"c": "a\""}"#, tokens)).await;
    let result = asked(&fake, vec![choice("c", &["a\"", "b"])])
        .await
        .unwrap();
    let Answer::Choice {
        selected,
        probabilities,
        ..
    } = &result.answers["c"]
    else {
        panic!()
    };
    assert_eq!(selected, "a\"");
    let probabilities = probabilities.as_ref().unwrap();
    let (quoted, plain) = ((-1.0f64).exp() + (-0.7f64).exp(), (-2.0f64).exp());
    assert!((probabilities["a\""].get() - quoted / (quoted + plain)).abs() < 1e-9);
}

/// Trace: FR-054-AC-9
#[tokio::test]
async fn the_response_retains_the_exact_exchange_and_error_paths_keep_it_too() {
    let _serial = serial().await;
    let listed = [("yes", -2.228), ("no", -5.759), ("Yes", -0.162)];
    let answer = bool_reply(yes_tokens(&listed));
    let sent = serde_json::to_string(&answer).unwrap();
    let fake = server(answer).await;
    let response = asked(&fake, vec![boolean("b")]).await.unwrap();
    let raw = response.raw.as_ref().unwrap();
    assert_eq!(raw.request, fake.generates()[0].text());
    assert_eq!(raw.response, sent);
    // The probability can be recomputed from the retained bytes alone.
    let kept: Json = serde_json::from_str(&raw.response).unwrap();
    let alternatives = &kept["logprobs"][3]["top_logprobs"];
    let mass = |token: &str| {
        alternatives
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["token"] == token)
            .map(|a| a["logprob"].as_f64().unwrap().exp())
            .unwrap()
    };
    let recomputed = mass("yes") / (mass("yes") + mass("no"));
    assert!((recomputed - boolean_p(&response, "b")).abs() < 1e-12);

    let torn = vec![
        fill("{\""),
        fill("b"),
        fill("\": \""),
        tok("no", -0.1, &[]),
        fill("\"}"),
    ];
    let bad = bool_reply(torn);
    let sent = serde_json::to_string(&bad).unwrap();
    let fake = server(bad).await;
    let error = asked(&fake, vec![boolean("b")]).await.unwrap_err();
    assert_eq!(error.raw.as_deref().unwrap().response, sent);
    assert_eq!(error.usage.as_deref().unwrap().input_tokens, Some(48));
}

/// Trace: FR-054-AC-10
#[tokio::test]
async fn a_candidate_not_sharing_the_bytes_before_the_value_is_not_attributed() {
    let _serial = serial().await;
    let tokens = vec![
        fill("{\"b\": "),
        tok("\"y", -0.2, &[(" n", -2.0), ("\"x", -6.0)]),
        fill("es\"}"),
    ];
    let fake = server(bool_reply(tokens)).await;
    let response = asked(&fake, vec![boolean("b")]).await.unwrap();
    // ` n` is not attributed to `no`, which takes the bound exp(-6): 0.9970, not 0.8581.
    assert!((boolean_p(&response, "b") - 0.9970).abs() < 1e-4);
}

#[tokio::test]
async fn a_request_for_another_model_than_the_binding_is_refused_before_sending() {
    let _serial = serial().await;
    let fake = server(bool_reply(yes_tokens(&[]))).await;
    let mut request = ask(vec![boolean("b")], DistributionPolicy::Strict {});
    request.model = "other".into();
    let error = binding(&fake, false).infer(&request).await.unwrap_err();
    assert_eq!(error.context["reason"], "request_model_differs");
    assert_eq!(fake.total(), 0);
}

/// Trace: FR-006-AC-6
#[tokio::test]
async fn a_header_credential_never_enters_the_raw_exchange() {
    let _serial = serial().await;
    let sentinel = "SENTINEL-BEARER-5521";
    let fake = server(bool_reply(yes_tokens(&[]))).await;
    let backend = OllamaBackend::new(
        Server::new(&fake.url, limits())
            .unwrap()
            .with_header("Authorization", &format!("Bearer {sentinel}"))
            .unwrap(),
        settings(MODEL, false, 4096, 512),
    )
    .unwrap();
    let response = backend
        .infer(&ask(vec![boolean("b")], DistributionPolicy::Strict {}))
        .await
        .unwrap();
    assert!(
        fake.generates()[0].head.contains(sentinel),
        "the double sends the header"
    );
    let serialized = serde_json::to_string(&response.raw).unwrap().to_lowercase();
    assert!(!serialized.contains(&sentinel.to_lowercase()));
    assert!(!serialized.contains("authorization"));
}
