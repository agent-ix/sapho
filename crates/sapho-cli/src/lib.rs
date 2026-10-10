// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Invocation host APIs: prepare synchronously, evaluate asynchronously, persist explicitly.
//! Custom consumers register their own native primitives and model backends.
//!
//! # Inspect a graph in a custom host
//!
//! ```
//! use sapho_core::{PrimitiveRegistry, ValueType};
//! use sapho_graph::GraphSpec;
//! let spec = GraphSpec::parse(include_str!("../../../examples/graphs/review.yaml"))?;
//! let inspection = sapho_cli::inspect(&spec, &PrimitiveRegistry::default())?;
//! assert_eq!(inspection.signature.outputs.get("needs_review"), Some(&ValueType::Boolean));
//! assert!(inspection.backends.is_empty());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! [`Runner`] accepts explicit native and backend registries; async evaluation
//! performs no file acquisition or persistence. Resolve live credentials and
//! acquire inputs before evaluation, then persist [`RunReport`] in the host.
//! [`ExitStatus`] separates ordinary completion, selected findings and refusals.
mod bindings;
mod error;
mod host;
mod io;
#[cfg(feature = "ollama")]
pub use bindings::ollama_base_url;
pub use bindings::{
    BindingConfig, Bindings, OllamaOptions, Provider, live_bindings, recording_bindings,
    replay_bindings, replay_bindings_json,
};
pub use error::CliError;
pub use host::{ExitStatus, Inspection, RunReport, Runner, inspect, plain_inputs};
pub use io::{
    ArtifactWriter, GraphArtifact, load_graph, read_bytes, read_bytes_with_timeout, select_format,
    write_new,
};

mod credentials;
pub use credentials::{resolve_credential, resolve_endpoint};
mod drift;
pub use drift::{compare_recordings, project_recording_confidences};
