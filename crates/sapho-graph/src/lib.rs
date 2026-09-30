// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Declarative graph definitions and pure typed compilation (FR-007/008/009).
mod compile;
mod spec;
pub use compile::{CompiledGraph, CompiledNode, compile};
pub use spec::*;

#[cfg(test)]
mod tests;
