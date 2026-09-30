// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Bounded deterministic graph execution and generic operators (FR-010..024).
mod engine;
mod operators;
mod trace;
pub use engine::{Engine, RunFailure, RunLimits, RunResult};
pub use trace::{ModelEvidence, NodeStatus, NodeTrace, Trace};
