// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! One-dependency embedding facade; implementation belongs to the leaf crates.
//! See `spec/spec.md` for module and crate ownership.
//!
//! # Embedding a graph without a backend
//!
//! ```
//! use std::{collections::BTreeMap, time::Duration};
//! use sapho::{core::*, graph::{GraphSpec, compile}, runtime::{Engine, RunLimits}};
//!
//! # async fn evaluate() -> Result<()> {
//! let spec = GraphSpec::parse(r#"
//! inputs:
//!   fact: {kind: boolean}
//! nodes:
//!   - id: negate
//!     operation: {kind: not}
//!     inputs:
//!       value: {kind: input, name: fact}
//! outputs:
//!   result: {kind: node, node: negate, port: result}
//! "#)?;
//! let compiled = compile(&spec, &PrimitiveRegistry::default())?;
//! let engine = Engine::new(compiled, BackendRegistry::default())?;
//! let inputs = BTreeMap::from([("fact".into(), Datum::new("fact-1", Value::Boolean(true))?)]);
//! let limits = RunLimits {
//!     node_instances: 10, collection_items: 10, model_requests: 10,
//!     concurrency: 1, data_bytes: 10_000, duration: Duration::from_secs(1),
//! };
//! let run = engine.run(&inputs, limits).await.map_err(|failure| failure.error)?;
//! assert_eq!(run.outputs.get("result").map(|d| &d.value), Some(&Value::Boolean(false)));
//! # Ok(())
//! # }
//! # tokio::runtime::Runtime::new().unwrap().block_on(evaluate()).unwrap();
//! ```
pub use sapho_core as core;
pub use sapho_graph as graph;
#[cfg(feature = "jev")]
pub use sapho_jev as jev;
pub use sapho_recording as recording;
pub use sapho_runtime as runtime;
