// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Bounded deterministic graph execution and generic operators (FR-010..024).
//!
//! # Execute an offline graph
//!
//! [`Engine::new`] validates backend names; [`Engine::run`] checks supplied data
//! and shares finite budgets across all nested work. Successful results retain a
//! [`Trace`]; [`RunFailure`] retains the preceding and failed evidence.
//!
//! ```
//! # tokio::runtime::Runtime::new().unwrap().block_on(async {
//! use sapho_core::{BackendRegistry, Inputs, PrimitiveRegistry, Value};
//! use sapho_graph::{compile, GraphSpec};
//! use sapho_runtime::{Engine, RunLimits};
//! let spec = GraphSpec::parse(include_str!("../../../examples/reference/facts.yaml"))?;
//! let engine = Engine::new(compile(&spec, &PrimitiveRegistry::default())?, BackendRegistry::default())?;
//! let run = engine.run(&Inputs::new(), RunLimits::default()).await?;
//! assert_eq!(run.outputs.get("both").map(|d| &d.value), Some(&Value::Boolean(false)));
//! assert!(!run.trace.nodes.is_empty());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! # }).unwrap();
//! ```
//!
//! Native primitives execute through a blocking bridge and must cooperate with
//! cancellation. Ready model calls may run concurrently. Evidence order remains
//! deterministic. No retries, file persistence or input acquisition are implicit.
mod engine;
mod operators;
mod trace;
pub use engine::{Engine, RunFailure, RunLimits, RunResult};
pub use trace::{ModelEvidence, NodeStatus, NodeTrace, ShadowTrace, Trace};
