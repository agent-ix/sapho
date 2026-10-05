// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Offline cross-crate acceptance suite.
mod common;
#[path = "scenarios/ears.rs"]
mod ears;
#[path = "scenarios/raw.rs"]
mod raw;
#[path = "scenarios/recording.rs"]
mod recording;
#[path = "scenarios/runtime.rs"]
mod runtime;

#[path = "scenarios/assembly.rs"]
mod assembly;
