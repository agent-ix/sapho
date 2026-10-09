// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Reasons emitted by Sapho extraction and Ollama adapters.

/// The decoded answer violates its JSON Schema.
pub const SCHEMA_VIOLATION: &str = "schema_violation";
/// The answer body is not JSON.
pub const NOT_JSON: &str = "not_json";
/// The server returned no answer text.
pub const EMPTY_RESPONSE: &str = "empty_response";
/// The server stopped generation at its output ceiling.
pub const OUTPUT_TRUNCATED: &str = "output_truncated";
/// The server response has an unusable shape or encoding.
pub const MALFORMED_RESPONSE: &str = "malformed_response";
/// Log probabilities cannot be attributed to the answer.
pub const LOGPROBS_MISMATCH: &str = "logprobs_mismatch";
/// A typed question has no usable answer.
pub const ANSWER_MALFORMED: &str = "answer_malformed";
/// An answer is outside the question's allowed values.
pub const ANSWER_NOT_ALLOWED: &str = "answer_not_allowed";
/// The embedding response has the wrong number of vectors.
pub const VECTOR_COUNT: &str = "vector_count";
/// An embedding vector has an unusable length.
pub const VECTOR_LENGTH: &str = "vector_length";
/// The server reports a different model name.
pub const NAME_MISMATCH: &str = "name_mismatch";
/// The server returned an unsuccessful HTTP status.
pub const HTTP_STATUS: &str = "http_status";
/// The client could not connect to the server.
pub const CONNECTION_FAILED: &str = "connection_failed";
/// The request permit could not be acquired.
pub const PERMIT_CLOSED: &str = "permit_closed";
/// The request exceeded its deadline.
pub const TIMEOUT: &str = "timeout";
/// The request body exceeds its byte ceiling.
pub const REQUEST_TOO_LARGE: &str = "request_too_large";
/// The response body exceeds its byte ceiling.
pub const RESPONSE_TOO_LARGE: &str = "response_too_large";
/// The requested model is absent from the server; a binding-level refusal.
pub const MODEL_NOT_FOUND: &str = "model_not_found";
/// The server cannot provide required log probabilities; a binding-level refusal.
pub const LOGPROBS_UNAVAILABLE: &str = "logprobs_unavailable";

/// Item-level reasons emitted by the stock extraction and Ollama adapters.
pub const ITEM_LEVEL: [&str; 17] = [
    SCHEMA_VIOLATION,
    NOT_JSON,
    EMPTY_RESPONSE,
    OUTPUT_TRUNCATED,
    MALFORMED_RESPONSE,
    LOGPROBS_MISMATCH,
    ANSWER_MALFORMED,
    ANSWER_NOT_ALLOWED,
    VECTOR_COUNT,
    VECTOR_LENGTH,
    NAME_MISMATCH,
    HTTP_STATUS,
    CONNECTION_FAILED,
    PERMIT_CLOSED,
    TIMEOUT,
    REQUEST_TOO_LARGE,
    RESPONSE_TOO_LARGE,
];
