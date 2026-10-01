// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Structured command-host boundary refusals.
use crate::Provider;
use sapho_core::SaphoError;
use serde::Serialize;
use std::path::PathBuf;

/// Public typed CLI-host error; diagnostic strings are never used for routing.
#[derive(Debug, thiserror::Error, Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum CliError {
    /// Shared engine/configuration refusal.
    #[error(transparent)]
    Engine(#[from] SaphoError),
    /// Evidence contract or ranking refusal.
    #[error(transparent)]
    Evidence(#[from] sapho_evidence::EvidenceError),
    /// Acquisition refusal.
    #[error(transparent)]
    Selection(#[from] sapho_select::SelectionError),
    /// An explicit host file operation failed.
    #[error("I/O at {path}: {source}")]
    Io {
        /// Affected destination or source.
        path: PathBuf,
        /// Native error; machine output retains the typed category and path.
        #[serde(skip)]
        source: std::io::Error,
    },
    /// Extension is not a supported representation.
    #[error("Unknown format for {0}; use .yaml/.yml/.json or --format")]
    Format(PathBuf),
    /// Live provider support was not compiled.
    #[error("Provider {0:?} requires the jev feature")]
    Feature(Provider),
    /// SDK environment/transport preparation was refused; credentials are never retained.
    #[error("Unable to configure Jev client")]
    ProviderConfiguration,
    /// Named input is not a regular file; pipes are accepted only through stdin.
    #[error("Input is not a regular file: {0}")]
    InputFileType(PathBuf),
    /// Bounded stdin polling is unavailable on this platform.
    #[error("Bounded stdin acquisition requires Unix")]
    UnsupportedStdin,
    /// OS runtime initialization failed.
    #[error("Unable to initialize command runtime: {0}")]
    Runtime(#[serde(skip)] std::io::Error),
    /// Command syntax was invalid.
    #[error("{0}")]
    Arguments(String),
}
