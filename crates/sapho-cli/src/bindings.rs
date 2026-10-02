// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Credential-free binding metadata, explicit capture and offline exact replay.
use crate::CliError;
use sapho_core::{
    BackendBinding, BackendId, BackendRegistry, DistributionPolicy, ErrorCode, SaphoError,
};
use sapho_recording::{Recording, RecordingBackend, ReplayBackend};
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
/// Infer unique model/policy identity; provider selection has no meaning for ReplayBackend.
pub fn replay_bindings(
    recording: &Recording,
    explicit: Option<&Bindings>,
    max_bytes: usize,
) -> Result<BackendRegistry, CliError> {
    let backend = Arc::new(ReplayBackend::new(recording, max_bytes)?);
    let mut identities: BTreeMap<BackendId, (String, Option<String>, DistributionPolicy)> =
        BTreeMap::new();
    for exchange in &recording.exchanges {
        let request = &exchange.request;
        let identity = (
            request.model.clone(),
            request.expected_model.clone(),
            request.distribution_policy,
        );
        if identities
            .get(&request.backend)
            .is_some_and(|previous| previous != &identity)
        {
            return Err(SaphoError::new(
                ErrorCode::RecordingMismatch,
                "Conflicting recorded binding identities",
            )
            .with_context("backend", request.backend.to_string())
            .into());
        }
        identities.insert(request.backend.clone(), identity);
    }
    if let Some(explicit) = explicit {
        for (id, config) in explicit {
            let identity = (
                config.model.clone(),
                config.expected_model.clone(),
                config.distribution_policy,
            );
            if identities
                .get(id)
                .is_some_and(|previous| previous != &identity)
            {
                return Err(SaphoError::new(
                    ErrorCode::RecordingMismatch,
                    "Explicit metadata differs from recording",
                )
                .with_context("backend", id.to_string())
                .into());
            }
            identities.insert(id.clone(), identity);
        }
    }
    let mut registry = BackendRegistry::default();
    for (id, (model, expected_model, distribution_policy)) in identities {
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
impl<'de> Deserialize<'de> for BindingConfig {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            provider: Provider,
            model: Option<String>,
            expected_model: Option<String>,
            distribution_policy: DistributionPolicy,
        }
        let wire = Wire::deserialize(decoder)?;
        let model = match (wire.provider, wire.model) {
            (_, Some(model)) if !model.is_empty() => model,
            (Provider::Clm, None) => sapho_systemone::CLM_DEFAULT_MODEL.to_owned(),
            _ => return Err(serde::de::Error::custom("A nonempty model is required")),
        };
        Ok(Self {
            provider: wire.provider,
            model,
            expected_model: wire.expected_model,
            distribution_policy: wire.distribution_policy,
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
        };
        registry.register(id.clone(), binding.attach(backend))?;
    }
    Ok(registry)
}
#[cfg(any(feature = "jev", feature = "clm"))]
fn credential(provider: Provider) -> Result<Option<ix_cli_kit::secrets::SecretValue>, CliError> {
    crate::resolve_credential(
        provider,
        &ix_cli_kit::secrets::SecretStore::system(),
        None,
        std::env::var_os(provider.credential_environment()),
    )
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
