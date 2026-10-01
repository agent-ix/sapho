// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Shared typed values, evidence and extension ports (FR-001 through FR-006).
//! This crate has no graph, runtime, transport or filesystem dependency.
mod distribution;
mod error;
mod model;
mod ports;
mod value;
pub use distribution::{DistributionAdjustment, DistributionPolicy};
pub use error::{ErrorCode, Result, SaphoError};
pub use model::*;
pub use ports::*;
pub use value::*;

#[cfg(test)]
mod tests;
