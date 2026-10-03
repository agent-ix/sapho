// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Shared typed values, evidence and extension ports (FR-001 through FR-006).
//! This crate has no graph, runtime, transport or filesystem dependency.
//!
//! # Construct and validate attributed input
//!
//! Use [`ValueType`] as the boundary schema and [`Datum`] to retain identity.
//! [`Probability`] and [`Degree`] share a numeric range, but have distinct meanings.
//!
//! ```
//! use sapho_core::{decode_plain, SourceId, SourceRef, Value, ValueType};
//! let source = SourceRef { source: SourceId::new("requirements")?, start: Some(0), end: Some(4) };
//! let input = decode_plain("req-7", &ValueType::Probability, &serde_json::json!(0.7), &[source])?;
//! input.validate()?;
//! ValueType::Probability.check(&input.value)?;
//! assert!(matches!(input.value, Value::Probability(p) if p.get() == 0.7));
//! assert_eq!(input.sources.len(), 1);
//! # Ok::<(), sapho_core::SaphoError>(())
//! ```
//!
//! Register host code through [`PrimitiveRegistry`] and inference through
//! [`BackendRegistry`]. [`validate_response`] returns [`Answers`] suitable for
//! probability projection; reported confidence and heuristic degrees are separate.
mod data;
mod distribution;
mod error;
mod model;
mod ports;
mod value;
pub use data::{decode_json, decode_plain};
pub use distribution::{DistributionAdjustment, DistributionPolicy};
pub use error::{ErrorCode, Result, SaphoError};
pub use model::*;
pub use ports::*;
pub use value::*;

#[cfg(test)]
mod tests;
