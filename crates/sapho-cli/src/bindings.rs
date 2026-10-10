// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Credential-free binding metadata, explicit capture and offline exact replay.
use crate::CliError;
use sapho_core::{
    BackendBinding, BackendId, BackendRegistry, DistributionPolicy, ErrorCode, SaphoError,
};
use sapho_recording::{Exchange, Recording, RecordingBackend, ReplayBackend};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};

/// Supported stock live adapter; custom Rust hosts inject the core backend seam.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    /// Hosted Jev using SDK environment configuration.
    Jev,
    /// Host-configured CLM System One service.
    Clm,
    /// Hosted Claude Messages API, with host-only bearer and workspace headers.
    Claude,
    /// A running Ollama server; the URL comes from `OLLAMA_BASE_URL`, never from a document.
    Ollama,
}
/// Generation members of an `ollama` binding; the defaults are the CLI's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct OllamaOptions {
    /// Whether the model thinks before answering; typed questions need `false`.
    pub think: bool,
    /// Context size in tokens.
    pub num_ctx: u64,
    /// Tokens reserved for the answer.
    pub num_predict: u64,
    /// Bound on each single request to the server, in seconds.
    pub timeout_seconds: u64,
}
impl Default for OllamaOptions {
    fn default() -> Self {
        Self {
            think: false,
            num_ctx: 32768,
            num_predict: 512,
            timeout_seconds: 600,
        }
    }
}
/// One declared binding; no credential or endpoint fields are accepted.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BindingConfig {
    /// Live adapter selected by the command host.
    pub provider: Provider,
    /// Requested model identity.
    pub model: String,
    /// Optional strict returned-model expectation.
    pub expected_model: Option<String>,
    /// Explicit complete-distribution interpretation policy.
    pub distribution_policy: DistributionPolicy,
    /// Generation members; present exactly for provider `ollama`.
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub ollama: Option<OllamaOptions>,
}
/// Named binding metadata document in YAML or JSON.
pub type Bindings = BTreeMap<BackendId, BindingConfig>;
impl BindingConfig {
    fn attach(&self, backend: Arc<dyn sapho_core::ModelBackend>) -> BackendBinding {
        BackendBinding {
            backend,
            model: self.model.clone(),
            expected_model: self.expected_model.clone(),
            distribution_policy: self.distribution_policy,
        }
    }
}
/// Decorate only required host bindings and retain explicit successful-exchange handles.
pub fn recording_bindings(
    required: &[BackendId],
    bindings: &BackendRegistry,
    max_bytes: usize,
) -> Result<(BackendRegistry, Vec<Arc<RecordingBackend>>), CliError> {
    let mut registry = BackendRegistry::default();
    let mut recorders = Vec::new();
    for id in required {
        let mut binding = bindings.get(id)?;
        let recorder = Arc::new(RecordingBackend::new(binding.backend, max_bytes)?);
        binding.backend = recorder.clone();
        registry.register(id.clone(), binding)?;
        recorders.push(recorder);
    }
    Ok((registry, recorders))
}
/// The model, strict expectation and policy each backend name was recorded with.
#[derive(Default)]
struct Identities(BTreeMap<BackendId, (String, Option<String>, DistributionPolicy)>);
impl Identities {
    /// Record one exchange's identity; a backend recorded with two identities is refused.
    fn observe(&mut self, exchange: &Exchange) -> Result<(), SaphoError> {
        let request = &exchange.request;
        let identity = (
            request.model.clone(),
            request.expected_model.clone(),
            request.distribution_policy,
        );
        if self
            .0
            .get(&request.backend)
            .is_some_and(|previous| previous != &identity)
        {
            return Err(SaphoError::new(
                ErrorCode::RecordingMismatch,
                "Conflicting recorded binding identities",
            )
            .with_context("backend", request.backend.to_string()));
        }
        self.0.insert(request.backend.clone(), identity);
        Ok(())
    }
    /// Check explicit metadata against what was recorded and bind every identity to `backend`.
    fn registry(
        mut self,
        backend: Arc<ReplayBackend>,
        explicit: Option<&Bindings>,
    ) -> Result<BackendRegistry, CliError> {
        if let Some(explicit) = explicit {
            for (id, config) in explicit {
                let identity = (
                    config.model.clone(),
                    config.expected_model.clone(),
                    config.distribution_policy,
                );
                if self.0.get(id).is_some_and(|previous| previous != &identity) {
                    return Err(SaphoError::new(
                        ErrorCode::RecordingMismatch,
                        "Explicit metadata differs from recording",
                    )
                    .with_context("backend", id.to_string())
                    .into());
                }
                self.0.insert(id.clone(), identity);
            }
        }
        let mut registry = BackendRegistry::default();
        for (id, (model, expected_model, distribution_policy)) in self.0 {
            registry.register(
                id,
                BackendBinding {
                    backend: backend.clone(),
                    model,
                    expected_model,
                    distribution_policy,
                },
            )?;
        }
        Ok(registry)
    }
}
/// Infer unique model/policy identity; provider selection has no meaning for ReplayBackend.
pub fn replay_bindings(
    recording: &Recording,
    explicit: Option<&Bindings>,
    max_bytes: usize,
) -> Result<BackendRegistry, CliError> {
    let backend = Arc::new(ReplayBackend::new(recording, max_bytes)?);
    let mut identities = Identities::default();
    for exchange in &recording.exchanges {
        identities.observe(exchange)?;
    }
    identities.registry(backend, explicit)
}
/// As [`replay_bindings`], reading the recording's JSON directly into the replay index so
/// that a recording of many exchanges is never also held as a typed [`Recording`].
pub fn replay_bindings_json(
    recording: &[u8],
    explicit: Option<&Bindings>,
    max_bytes: usize,
) -> Result<BackendRegistry, CliError> {
    let mut identities = Identities::default();
    let backend = Arc::new(ReplayBackend::from_json(
        recording,
        max_bytes,
        |exchange| identities.observe(exchange),
    )?);
    identities.registry(backend, explicit)
}
impl<'de> Deserialize<'de> for BindingConfig {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            provider: Provider,
            model: Option<String>,
            expected_model: Option<String>,
            distribution_policy: DistributionPolicy,
            think: Option<bool>,
            num_ctx: Option<u64>,
            num_predict: Option<u64>,
            timeout_seconds: Option<u64>,
        }
        let wire = Wire::deserialize(decoder)?;
        let model = match (wire.provider, wire.model) {
            (_, Some(model)) if !model.is_empty() => model,
            (Provider::Clm, None) => sapho_systemone::CLM_DEFAULT_MODEL.to_owned(),
            _ => return Err(serde::de::Error::custom("A nonempty model is required")),
        };
        let given = [
            ("think", wire.think.is_some()),
            ("num_ctx", wire.num_ctx.is_some()),
            ("num_predict", wire.num_predict.is_some()),
            ("timeout_seconds", wire.timeout_seconds.is_some()),
        ];
        let ollama = if wire.provider == Provider::Ollama {
            let default = OllamaOptions::default();
            Some(OllamaOptions {
                think: wire.think.unwrap_or(default.think),
                num_ctx: wire.num_ctx.unwrap_or(default.num_ctx),
                num_predict: wire.num_predict.unwrap_or(default.num_predict),
                timeout_seconds: wire.timeout_seconds.unwrap_or(default.timeout_seconds),
            })
        } else if let Some((member, _)) = given.iter().find(|(_, present)| *present) {
            return Err(serde::de::Error::custom(format!(
                "Member {member} belongs to provider ollama"
            )));
        } else {
            None
        };
        Ok(Self {
            provider: wire.provider,
            model,
            expected_model: wire.expected_model,
            distribution_policy: wire.distribution_policy,
            ollama,
        })
    }
}
/// Prepare only required live providers synchronously, before async evaluation.
pub fn live_bindings(
    required: &[BackendId],
    bindings: &Bindings,
) -> Result<BackendRegistry, CliError> {
    let mut registry = BackendRegistry::default();
    for id in required {
        let binding = bindings.get(id).ok_or_else(|| {
            SaphoError::new(ErrorCode::UnknownBackend, "Binding metadata absent")
                .with_context("backend", id.to_string())
        })?;
        if binding.model.is_empty()
            || binding
                .expected_model
                .as_ref()
                .is_some_and(|model| model.is_empty())
        {
            return Err(SaphoError::new(ErrorCode::Config, "Empty model identity")
                .with_context("backend", id.to_string())
                .into());
        }
        let backend: Arc<dyn sapho_core::ModelBackend> = match binding.provider {
            Provider::Jev => prepare_jev()?,
            Provider::Clm => prepare_clm()?,
            Provider::Claude => prepare_claude()?,
            Provider::Ollama => prepare_ollama(binding)?,
        };
        registry.register(id.clone(), binding.attach(backend))?;
    }
    Ok(registry)
}
/// The Ollama server URL: the `OLLAMA_BASE_URL` value when set and nonempty, else the
/// loopback default. An empty value counts as unset.
#[cfg(feature = "ollama")]
pub fn ollama_base_url(environment: Option<String>) -> String {
    crate::resolve_endpoint(
        None,
        environment.filter(|value| !value.is_empty()),
        sapho_ollama::DEFAULT_BASE_URL,
    )
}
/// Build the Ollama backend for one required binding; no credential or secret store is used.
#[cfg(feature = "ollama")]
fn prepare_ollama(binding: &BindingConfig) -> Result<Arc<dyn sapho_core::ModelBackend>, CliError> {
    use sapho_ollama::{Limits, OllamaBackend, Server, Settings};
    let options = binding.ollama.unwrap_or_default();
    let environment = std::env::var("OLLAMA_BASE_URL")
        .map(Some)
        .or_else(|error| match error {
            std::env::VarError::NotPresent => Ok(None),
            std::env::VarError::NotUnicode(_) => Err(CliError::ProviderConfiguration),
        })?;
    let endpoint = ollama_base_url(environment);
    let limits = Limits {
        timeout: std::time::Duration::from_secs(options.timeout_seconds),
        ..Limits::default()
    };
    let server = Server::new(&endpoint, limits).map_err(SaphoError::from)?;
    let settings = Settings {
        model: binding.model.clone(),
        think: options.think,
        num_ctx: options.num_ctx,
        num_predict: options.num_predict,
    };
    OllamaBackend::new(server, settings)
        .map(|backend| Arc::new(backend) as Arc<dyn sapho_core::ModelBackend>)
        .map_err(|error| SaphoError::from(error).into())
}
#[cfg(not(feature = "ollama"))]
fn prepare_ollama(_: &BindingConfig) -> Result<Arc<dyn sapho_core::ModelBackend>, CliError> {
    Err(CliError::Feature(Provider::Ollama))
}
#[cfg(any(feature = "jev", feature = "clm", feature = "claude"))]
fn credential(provider: Provider) -> Result<Option<ix_cli_kit::secrets::SecretValue>, CliError> {
    crate::resolve_credential(
        provider,
        &ix_cli_kit::secrets::SecretStore::system(),
        None,
        provider.credential_environment().and_then(std::env::var_os),
    )
}
#[cfg(feature = "claude")]
fn prepare_claude() -> Result<Arc<dyn sapho_core::ModelBackend>, CliError> {
    let secret =
        credential(Provider::Claude)?.ok_or(CliError::CredentialMissing(Provider::Claude))?;
    let workspace_id = std::env::var("ANTHROPIC_WORKSPACE_ID")
        .map(Some)
        .or_else(|error| match error {
            std::env::VarError::NotPresent => Ok(None),
            std::env::VarError::NotUnicode(_) => Err(CliError::ProviderConfiguration),
        })?;
    sapho_claude::ClaudeBackend::new(
        &secret,
        workspace_id.as_deref(),
        sapho_claude::Limits::default(),
    )
    .map(|backend| Arc::new(backend) as Arc<dyn sapho_core::ModelBackend>)
    .map_err(|_| CliError::ProviderConfiguration)
}
#[cfg(not(feature = "claude"))]
fn prepare_claude() -> Result<Arc<dyn sapho_core::ModelBackend>, CliError> {
    Err(CliError::Feature(Provider::Claude))
}
#[cfg(feature = "clm")]
fn prepare_clm() -> Result<Arc<dyn sapho_core::ModelBackend>, CliError> {
    let secret = credential(Provider::Clm)?;
    let environment = std::env::var("CLM_BASE_URL")
        .map(Some)
        .or_else(|error| match error {
            std::env::VarError::NotPresent => Ok(None),
            std::env::VarError::NotUnicode(_) => Err(CliError::ProviderConfiguration),
        })?;
    let endpoint = crate::resolve_endpoint(None, environment, sapho_clm::DEFAULT_BASE_URL);
    sapho_clm::ClmBackend::new(&endpoint, secret.as_ref(), sapho_clm::Limits::default())
        .map(|backend| Arc::new(backend) as Arc<dyn sapho_core::ModelBackend>)
        .map_err(|_| CliError::ProviderConfiguration)
}
#[cfg(not(feature = "clm"))]
fn prepare_clm() -> Result<Arc<dyn sapho_core::ModelBackend>, CliError> {
    Err(CliError::Feature(Provider::Clm))
}
#[cfg(feature = "jev")]
fn prepare_jev() -> Result<Arc<dyn sapho_core::ModelBackend>, CliError> {
    use typesafe_sdk_client::Client;
    use typesafe_sdk_config::Builder;
    use typesafe_sdk_env::Process;
    use typesafe_sdk_http::Reqwest;
    let secret = credential(Provider::Jev)?.ok_or(CliError::CredentialMissing(Provider::Jev))?;
    let config = Builder::new()
        .api_key(secret.expose_secret())
        .log_level(typesafe_sdk_log::Level::Off)
        .build(&Process)
        .map_err(|_| CliError::ProviderConfiguration)?;
    let transport = Reqwest::new().map_err(|_| CliError::ProviderConfiguration)?;
    Ok(Arc::new(sapho_jev::JevBackend::new(
        Client::with_transport(config, Arc::new(transport)),
    )))
}
#[cfg(not(feature = "jev"))]
fn prepare_jev() -> Result<Arc<dyn sapho_core::ModelBackend>, CliError> {
    Err(CliError::Feature(Provider::Jev))
}
