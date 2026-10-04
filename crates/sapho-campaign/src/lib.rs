// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Durable host infrastructure. Domain acceptance belongs to compiled consumers.
pub mod adapter;
pub mod control;
pub mod execution;
pub mod export;
pub mod lifecycle;
pub mod migration;
pub mod runner;
pub mod storage;
use thiserror::Error;
/// Stable host failure category, independent of graph execution errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// Invalid identity or configuration.
    Invalid,
    /// Bounded operation denied by policy.
    Refused,
    /// Another writer or incompatible state.
    Conflict,
    /// Storage operation or integrity failure.
    Storage,
}
/// Structured campaign failure. Messages are diagnostic, never routing keys.
#[derive(Debug, Error, serde::Serialize)]
#[error("{code:?}: {message}")]
pub struct Error {
    /// Stable category.
    pub code: ErrorCode,
    /// Human-readable context.
    pub message: String,
}
impl Error {
    /// Construct an explicit categorized error.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::new(ErrorCode::Storage, e.to_string())
    }
}
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Self::new(ErrorCode::Storage, e.to_string())
    }
}
/// Campaign operation result.
pub type Result<T> = std::result::Result<T, Error>;
/// Trusted compiled extensions use the same transactional storage types.
pub use rusqlite;
