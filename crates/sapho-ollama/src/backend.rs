// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! The stateless generate exchange and the Extractor implementation (FR-049, FR-050, FR-052).
use crate::{
    http::{Reply, Server, failure},
    identity,
};
use async_trait::async_trait;
use sapho_core::{
    Completion, ErrorCode, ExtractError, ExtractRequest, ExtractUsage, Extractor, ModelIdentity,
    RawExchange, TooLarge,
};
use serde::{Deserialize, Serialize};

/// What a binding fixes for every call: the model and its explicit generation limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// Model tag the server runs.
    pub model: String,
    /// Whether the model thinks before answering; always sent explicitly.
    pub think: bool,
    /// Context size in tokens; a prompt that does not fit is refused, never shortened.
    pub num_ctx: u64,
    /// Tokens reserved for the answer; generation stops at this limit.
    pub num_predict: u64,
}

/// An Ollama model binding that extracts records and answers typed questions.
///
/// Each call is one stateless generate request; nothing is carried between calls.
#[derive(Debug)]
pub struct OllamaBackend {
    pub(crate) server: Server,
    pub(crate) settings: Settings,
}

#[derive(Serialize)]
pub(crate) struct Options {
    temperature: u8,
    num_ctx: u64,
    num_predict: u64,
}
/// The generate request body; these are the only members ever sent.
#[derive(Serialize)]
pub(crate) struct GenerateBody<'a, F: Serialize + ?Sized> {
    pub model: &'a str,
    pub system: &'a str,
    pub prompt: &'a str,
    pub format: &'a F,
    pub think: bool,
    pub stream: bool,
    pub truncate: bool,
    pub shift: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<u8>,
    pub options: Options,
}
/// One generated token with its own log-probability and the listed alternatives.
#[derive(Deserialize)]
pub(crate) struct TokenLogprob {
    pub logprob: f64,
    pub bytes: Vec<u8>,
    #[serde(default)]
    pub top_logprobs: Vec<Alternative>,
}
/// A candidate the server listed for a token position.
#[derive(Deserialize)]
pub(crate) struct Alternative {
    pub logprob: f64,
    pub bytes: Vec<u8>,
}
// The server adds members of its own (`context`, `created_at`, ...), so unknown members
// are tolerated here; only the listed ones are read.
#[derive(Deserialize)]
struct GenerateReply {
    model: String,
    #[serde(default)]
    response: String,
    done: bool,
    done_reason: Option<String>,
    prompt_eval_count: Option<u64>,
    eval_count: Option<u64>,
    total_duration: Option<u64>,
    load_duration: Option<u64>,
    prompt_eval_duration: Option<u64>,
    eval_duration: Option<u64>,
    logprobs: Option<Vec<TokenLogprob>>,
}

/// A complete, accepted generate response.
pub(crate) struct Generated {
    pub response: String,
    pub logprobs: Option<Vec<TokenLogprob>>,
    pub usage: ExtractUsage,
    pub raw: RawExchange,
    pub model: ModelIdentity,
}

pub(crate) fn nanos_to_ms(nanos: Option<u64>) -> Option<u64> {
    nanos.map(|n| n / 1_000_000)
}

/// The server's context refusal as Ollama 0.32 reports it: HTTP 400 whose `error` member
/// is itself a JSON document with a typed error. Returns the server's prompt token count.
pub(crate) fn context_refusal(body: &str) -> Option<Option<u64>> {
    #[derive(Deserialize)]
    struct Outer {
        error: String,
    }
    #[derive(Deserialize)]
    struct Inner {
        error: Detail,
    }
    #[derive(Deserialize)]
    struct Detail {
        r#type: String,
        n_prompt_tokens: Option<u64>,
    }
    let outer: Outer = serde_json::from_str(body).ok()?;
    let inner: Inner = serde_json::from_str(&outer.error).ok()?;
    (inner.error.r#type == "exceed_context_size_error").then_some(inner.error.n_prompt_tokens)
}

impl OllamaBackend {
    /// Bind a model with its generation limits.
    ///
    /// Refused with `Config` when the model is empty, either limit is zero, or
    /// `num_predict` is not smaller than `num_ctx`.
    pub fn new(server: Server, settings: Settings) -> Result<Self, ExtractError> {
        let refused = |reason| failure(ErrorCode::Config, reason, "Invalid Ollama binding");
        if settings.model.is_empty() {
            return Err(refused("empty_model"));
        }
        if settings.num_ctx == 0 || settings.num_predict == 0 {
            return Err(refused("zero_limit"));
        }
        if settings.num_predict >= settings.num_ctx {
            return Err(refused("num_predict_not_below_num_ctx"));
        }
        Ok(Self { server, settings })
    }

    pub(crate) fn too_large(&self, reported: Option<u64>) -> ExtractError {
        ExtractError::new(ErrorCode::TooLarge, "The prompt does not fit the context")
            .with_too_large(TooLarge {
                reported_input_tokens: reported,
                context_tokens: self.settings.num_ctx,
                reserved_output_tokens: self.settings.num_predict,
            })
    }

    pub(crate) fn body<'a, F: Serialize + ?Sized>(
        &'a self,
        system: &'a str,
        prompt: &'a str,
        format: &'a F,
        logprobs: bool,
    ) -> GenerateBody<'a, F> {
        GenerateBody {
            model: &self.settings.model,
            system,
            prompt,
            format,
            think: self.settings.think,
            stream: false,
            truncate: false,
            shift: false,
            logprobs: logprobs.then_some(true),
            top_logprobs: logprobs.then_some(20),
            options: Options {
                temperature: 0,
                num_ctx: self.settings.num_ctx,
                num_predict: self.settings.num_predict,
            },
        }
    }

    /// Send one generate request and return the accepted response.
    ///
    /// Every error raised after a response arrived retains the raw exchange.
    pub(crate) async fn generate<F: Serialize + ?Sized>(
        &self,
        body: &GenerateBody<'_, F>,
    ) -> Result<Generated, ExtractError> {
        let request = serde_json::to_string(body).map_err(|_| {
            failure(
                ErrorCode::Config,
                "request_unserializable",
                "Request is not serializable",
            )
        })?;
        self.server.check_request(&request)?;
        let reply = self
            .server
            .exclusive(async {
                identity::ensure_installed(&self.server, &self.settings.model).await?;
                self.server.post("api/generate", &request).await
            })
            .await?;
        self.accept(reply, &request)
    }

    fn accept(&self, reply: Reply, request: &str) -> Result<Generated, ExtractError> {
        if !reply.success() {
            // The body is read only to recognise the server's context refusal.
            if reply.status == 400
                && let Some(reported) = reply.text().and_then(context_refusal)
            {
                return Err(reply.refusal(self.too_large(reported)));
            }
            return Err(reply.status_error());
        }
        let text = reply.success_text()?;
        let raw = RawExchange {
            request: request.to_string(),
            response: text.to_string(),
        };
        let invalid = |reason, message| {
            ExtractError::new(ErrorCode::InvalidAnswer, message)
                .with_reason(reason)
                .with_raw(raw.clone())
        };
        let parsed: GenerateReply = serde_json::from_str(text)
            .map_err(|_| invalid("malformed_response", "Response is not a generate response"))?;
        let usage = ExtractUsage {
            input_tokens: parsed.prompt_eval_count,
            output_tokens: parsed.eval_count,
            elapsed_ms: nanos_to_ms(parsed.total_duration),
            load_ms: nanos_to_ms(parsed.load_duration),
            prompt_ms: nanos_to_ms(parsed.prompt_eval_duration),
            generation_ms: nanos_to_ms(parsed.eval_duration),
        };
        let with_usage = |error: ExtractError| error.with_usage(usage.clone());
        if parsed.model != self.settings.model {
            return Err(with_usage(
                failure(
                    ErrorCode::ModelMismatch,
                    "name_mismatch",
                    "The response names a different model",
                )
                .with_raw(raw),
            ));
        }
        if parsed
            .prompt_eval_count
            .is_some_and(|n| n.saturating_add(self.settings.num_predict) > self.settings.num_ctx)
        {
            return Err(with_usage(
                self.too_large(parsed.prompt_eval_count).with_raw(raw),
            ));
        }
        if !parsed.done {
            return Err(with_usage(invalid(
                "malformed_response",
                "Response is not complete",
            )));
        }
        if parsed.done_reason.as_deref() == Some("length") {
            return Err(with_usage(invalid(
                "output_truncated",
                "The answer stopped at the output limit",
            )));
        }
        if parsed.response.is_empty() {
            return Err(with_usage(invalid(
                "empty_response",
                "The response text is empty",
            )));
        }
        Ok(Generated {
            response: parsed.response,
            logprobs: parsed.logprobs,
            usage,
            raw,
            model: ModelIdentity { name: parsed.model },
        })
    }
}

#[async_trait]
impl Extractor for OllamaBackend {
    async fn exchange(&self, request: &ExtractRequest) -> Result<Completion, ExtractError> {
        if request.model != self.settings.model {
            return Err(failure(
                ErrorCode::Config,
                "request_model_differs",
                "The request names a model other than the binding's",
            ));
        }
        let body = self.body(
            &request.instructions,
            &request.input,
            &request.schema,
            false,
        );
        let generated = self.generate(&body).await?;
        Ok(Completion {
            answer: generated.response.into_bytes(),
            usage: generated.usage,
            raw: generated.raw,
            model: generated.model,
        })
    }
}
