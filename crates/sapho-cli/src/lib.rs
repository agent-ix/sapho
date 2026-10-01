// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Invocation host APIs: prepare synchronously, evaluate asynchronously, persist explicitly.
//! Custom consumers register their own native primitives and model backends.
mod bindings;
mod error;
mod host;
mod io;
pub use bindings::{
    BindingConfig, Bindings, Provider, live_bindings, recording_bindings, replay_bindings,
};
pub use error::CliError;
pub use host::{ExitStatus, Inspection, RunReport, Runner, inspect, plain_inputs};
pub use io::{
    ArtifactWriter, GraphArtifact, load_graph, read_bytes, read_bytes_with_timeout, select_format,
    write_new,
};
