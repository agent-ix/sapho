// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Declarative graph definitions and pure typed compilation (FR-007/008/009).
//!
//! # Parse, compile and inspect a graph
//!
//! Compilation checks exact ports, types, references and cycles without running
//! native code or models. Register any native implementations before compilation.
//! Model bindings are supplied later when constructing the runtime engine.
//!
//! ```
//! use sapho_core::{PrimitiveRegistry, ValueType};
//! use sapho_graph::{compile, GraphSpec};
//! let spec = GraphSpec::parse(include_str!("../../../examples/reference/facts.yaml"))?;
//! let graph = compile(&spec, &PrimitiveRegistry::default())?;
//! assert_eq!(graph.signature().outputs.get("both"), Some(&ValueType::Boolean));
//! assert!(!graph.stages().is_empty());
//! # Ok::<(), sapho_core::SaphoError>(())
//! ```
//!
//! [`GraphSpec::parse_with_format`] supports generated JSON. [`Binding`] selects
//! inputs, node outputs or checked literals; [`Operation`] declares closed generic
//! operations. Definitions live in [`GraphSpec::subgraphs`] and maps capture their
//! additional inputs explicitly. Guarded outputs have Optional schemas.
mod compile;
mod identity;
mod spec;
pub use compile::{CompiledGraph, CompiledNode, compile};
pub use identity::graph_semantic_identity;
pub use spec::*;

#[cfg(test)]
mod tests;
