// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Explicit host policy for approximate complete distributions (FR-005/020).
use crate::{ErrorCode, Probability, Result, SaphoError};
use serde::{Deserialize, Serialize};

pub(crate) const MASS_ROUNDOFF: f64 = 1e-6;
const MAX_MASS_ERROR: f64 = 0.05;

/// Host acceptance policy, retained with requests and answers, never provider auth.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DistributionPolicy {
    /// Require unit mass within numerical roundoff.
    /// A struct variant makes Serde reject unknown fields on this tagged payload.
    Strict {},
    /// Accept bounded nonunit complete mass; projections derive normalization.
    Approximate {
        /// Positive absolute mass-error allowance, at most 0.05.
        max_mass_error: Probability,
    },
}
impl DistributionPolicy {
    /// Construct a positive bounded approximation policy.
    pub fn approximate(max_mass_error: f64) -> Result<Self> {
        let policy = Self::Approximate {
            max_mass_error: Probability::new(max_mass_error)?,
        };
        policy.validate()?;
        Ok(policy)
    }
    /// Validate native or deserialized policy before use.
    pub fn validate(self) -> Result<()> {
        match self {
            Self::Strict {} => Ok(()),
            Self::Approximate { max_mass_error }
                if max_mass_error.get() > 0.0 && max_mass_error.get() <= MAX_MASS_ERROR =>
            {
                Ok(())
            }
            Self::Approximate { .. } => Err(SaphoError::new(
                ErrorCode::InvalidValue,
                "Invalid approximate mass allowance",
            )
            .with_context("max_mass_error", MAX_MASS_ERROR.to_string())),
        }
    }
    pub(crate) fn allowed_error(self) -> f64 {
        match self {
            Self::Strict {} => MASS_ROUNDOFF,
            Self::Approximate { max_mass_error } => max_mass_error.get() + MASS_ROUNDOFF,
        }
    }
}

/// Derived interpretation of an approximate full distribution; raw values stay intact.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct DistributionAdjustment {
    /// Sum of unchanged provider outcome probabilities.
    pub raw_mass: f64,
    /// Multiplier applied only when projecting complete outcome mass.
    pub scale: f64,
}
