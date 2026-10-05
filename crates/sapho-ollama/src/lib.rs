// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Calls an already running Ollama server for structured extraction, typed questions
//! with probabilities read from answer-token log-probabilities, and embeddings
//! (FR-049 through FR-054).
//!
//! Each call is one stateless request: fixed instructions, one input and one schema,
//! with nothing carried over from earlier calls. Prompt size is counted by the server
//! in tokens, so an input that does not fit is reported as
//! [`ErrorCode::TooLarge`](sapho_core::ErrorCode::TooLarge) instead of being cut. At most
//! one request is in flight in the process, across extraction, questions and embeddings.
//!
//! ```no_run
//! use sapho_ollama::{DEFAULT_BASE_URL, Limits, OllamaBackend, Server, Settings};
//! let server = Server::new(DEFAULT_BASE_URL, Limits::default())?;
//! let _backend = OllamaBackend::new(
//!     server,
//!     Settings { model: "qwen3:30b".into(), think: false, num_ctx: 32768, num_predict: 2048 },
//! )?;
//! # Ok::<(), sapho_core::ExtractError>(())
//! ```
//!
//! [`OllamaBackend`] implements [`sapho_core::Extractor`] (call it through
//! [`sapho_core::extract`]) and [`sapho_core::ModelBackend`].
mod backend;
mod embed;
mod http;
mod identity;
mod questions;
pub use backend::{OllamaBackend, Settings};
pub use embed::{DEFAULT_MAX_INPUTS, Embeddings, OllamaEmbedder};
pub use http::{DEFAULT_BASE_URL, Limits, Server};
