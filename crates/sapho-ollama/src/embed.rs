// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Embeddings through the same serialized client (FR-053).
use crate::{
    http::{Server, failure},
    identity,
};
use sapho_core::{ErrorCode, ExtractError, ModelIdentity, RawExchange};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// Inputs per request unless the host chooses another maximum.
pub const DEFAULT_MAX_INPUTS: usize = 32;
/// The refusal Ollama 0.32 gives for an input longer than the context with `truncate: false`.
const INPUT_TOO_LONG: &str = "the input length exceeds the context length";

#[derive(Serialize)]
struct EmbedBody<'a> {
    model: &'a str,
    input: &'a [String],
    truncate: bool,
}
// Tolerates the server's own extra members; only the listed ones are read. A component
// that is not a finite number cannot parse as `f64`, so it is a malformed response.
#[derive(Deserialize)]
struct EmbedReply {
    model: String,
    embeddings: Vec<Vec<f64>>,
    prompt_eval_count: Option<u64>,
}
#[derive(Deserialize)]
struct ErrorBody {
    error: String,
}

/// One vector per input, in input order.
#[derive(Debug, Clone, PartialEq)]
pub struct Embeddings {
    /// Finite vectors of equal length.
    pub vectors: Vec<Vec<f64>>,
    /// The model that answered, with its weights digest.
    pub model: ModelIdentity,
    /// Prompt tokens, when the server reported them.
    pub input_tokens: Option<u64>,
    /// Exact request and response bytes.
    pub raw: RawExchange,
}

/// An embedding model on an Ollama server.
#[derive(Debug)]
pub struct OllamaEmbedder {
    server: Server,
    model: String,
    max_inputs: usize,
    weights: OnceLock<String>,
}
impl OllamaEmbedder {
    /// Bind an embedding model; `Config` for an empty model or a zero maximum.
    pub fn new(
        server: Server,
        model: impl Into<String>,
        max_inputs: usize,
    ) -> Result<Self, ExtractError> {
        let model = model.into();
        if model.is_empty() || max_inputs == 0 {
            return Err(failure(
                ErrorCode::Config,
                "invalid_embedding_binding",
                "Invalid embedding binding",
            ));
        }
        Ok(Self {
            server,
            model,
            max_inputs,
            weights: OnceLock::new(),
        })
    }

    /// Embed `inputs` in one request.
    ///
    /// An empty list or one longer than the maximum is `Config`. An input the server
    /// refuses as longer than the context is `TooLarge` with reason `input_exceeds_context`.
    pub async fn embed(&self, inputs: &[String]) -> Result<Embeddings, ExtractError> {
        if inputs.is_empty() || inputs.len() > self.max_inputs {
            return Err(failure(
                ErrorCode::Config,
                "input_count",
                "The number of inputs is outside the binding's range",
            ));
        }
        let request = serde_json::to_string(&EmbedBody {
            model: &self.model,
            input: inputs,
            truncate: false,
        })
        .map_err(|_| {
            failure(
                ErrorCode::Config,
                "request_unserializable",
                "Request is not serializable",
            )
        })?;
        self.server.check_request(&request)?;
        let (digest, reply) = self
            .server
            .exclusive(async {
                let digest = identity::resolve(&self.server, &self.model, &self.weights).await?;
                Ok((digest, self.server.post("api/embed", &request).await?))
            })
            .await?;
        let raw = reply.raw(&request);
        if reply.status == 400
            && serde_json::from_str::<ErrorBody>(&reply.body)
                .is_ok_and(|b| b.error == INPUT_TOO_LONG)
        {
            return Err(failure(
                ErrorCode::TooLarge,
                "input_exceeds_context",
                "An input does not fit the embedding context",
            )
            .with_raw(raw));
        }
        if !(200..300).contains(&reply.status) {
            return Err(failure(
                ErrorCode::BackendFailed,
                "http_status",
                "Ollama answered with an error status",
            )
            .with_raw(raw));
        }
        let invalid = |reason| {
            failure(
                ErrorCode::InvalidAnswer,
                reason,
                "Unusable embedding response",
            )
            .with_raw(raw.clone())
        };
        let parsed: EmbedReply =
            serde_json::from_str(&reply.body).map_err(|_| invalid("malformed_response"))?;
        if parsed.model != self.model {
            return Err(failure(
                ErrorCode::ModelMismatch,
                "name_mismatch",
                "The response names a different model",
            )
            .with_raw(raw));
        }
        let width = parsed.embeddings.first().map_or(0, Vec::len);
        if parsed.embeddings.len() != inputs.len() {
            return Err(invalid("vector_count"));
        }
        if width == 0 || parsed.embeddings.iter().any(|v| v.len() != width) {
            return Err(invalid("vector_length"));
        }
        Ok(Embeddings {
            vectors: parsed.embeddings,
            model: ModelIdentity {
                name: parsed.model,
                digest: Some(digest),
            },
            input_tokens: parsed.prompt_eval_count,
            raw,
        })
    }
}
