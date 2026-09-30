// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Structured errors shared across engine crate boundaries.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A stable discriminant; callers never need to parse the error message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// Invalid configuration syntax or parameters.
    Config,
    /// An identity was registered or produced twice.
    DuplicateId,
    /// A native implementation was not registered.
    UnknownPrimitive,
    /// A model backend was not registered.
    UnknownBackend,
    /// A binding references an absent node, input, port or field.
    UnknownReference,
    /// A graph or subgraph contains a dependency cycle.
    Cycle,
    /// Connected or returned value types differ.
    TypeMismatch,
    /// Required input is absent.
    MissingInput,
    /// Value violates its invariant.
    InvalidValue,
    /// An answer violates its declared question.
    InvalidAnswer,
    /// An expected question answer is absent.
    MissingAnswer,
    /// A requested probability was not supplied.
    UnsupportedDistribution,
    /// A configured work or serialization ceiling was exceeded.
    LimitExceeded,
    /// A deadline or cooperative cancellation was observed.
    DeadlineExceeded,
    /// Native worker panicked or returned a failure.
    CodeFailed,
    /// Backend call failed outside a more specific service category.
    BackendFailed,
    /// Service credentials were refused.
    Unauthorized,
    /// Service rate limit was reached.
    RateLimited,
    /// Service rejected the request contract.
    ServiceValidation,
    /// Actual model differs from the caller's explicit expectation.
    ModelMismatch,
    /// No exact recorded request matches.
    ReplayMiss,
    /// Recording I/O failed.
    RecordingIo,
    /// Recording contains conflicting or invalid exchanges.
    RecordingMismatch,
}

/// A structured, serializable error with a stable code and diagnostic context.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[error("{code:?}: {message}")]
#[serde(deny_unknown_fields)]
pub struct SaphoError {
    /// Machine-readable refusal category.
    pub code: ErrorCode,
    /// Human-readable explanation; not a discriminant.
    pub message: Box<str>,
    /// Named diagnostic facts, never credentials.
    pub context: BTreeMap<String, String>,
}
impl SaphoError {
    /// Construct a refusal under an explicit category.
    pub fn new(code: ErrorCode, message: impl Into<Box<str>>) -> Self {
        Self {
            code,
            message: message.into(),
            context: BTreeMap::new(),
        }
    }
    /// Add an owned diagnostic fact.
    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.insert(key.into(), value.into());
        self
    }
}
/// Engine result type used at each crate boundary.
pub type Result<T> = std::result::Result<T, SaphoError>;
