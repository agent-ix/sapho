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
}
/// One declared binding; no credential or endpoint fields are accepted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
/// Infer unique policy-sensitive metadata and bind only ReplayBackend; no SDK or environment access.
pub fn replay_bindings(
    recording: &Recording,
    explicit: Option<&Bindings>,
    max_bytes: usize,
) -> Result<BackendRegistry, CliError> {
    let backend = Arc::new(ReplayBackend::new(recording, max_bytes)?);
    let mut metadata = Bindings::new();
    for exchange in &recording.exchanges {
        let request = &exchange.request;
        let config = BindingConfig {
            provider: Provider::Jev,
            model: request.model.clone(),
            expected_model: request.expected_model.clone(),
            distribution_policy: request.distribution_policy,
        };
        if let Some(previous) = metadata.get(&request.backend) {
            if previous != &config {
                return Err(SaphoError::new(
                    ErrorCode::RecordingMismatch,
                    "Conflicting recorded binding identities",
                )
                .with_context("backend", request.backend.to_string())
                .into());
            }
        } else {
            metadata.insert(request.backend.clone(), config);
        }
    }
    if let Some(explicit) = explicit {
        for (id, config) in explicit {
            if metadata.get(id).is_some_and(|previous| previous != config) {
                return Err(SaphoError::new(
                    ErrorCode::RecordingMismatch,
                    "Explicit metadata differs from recorded binding",
                )
                .with_context("backend", id.to_string())
                .into());
            }
            metadata.insert(id.clone(), config.clone());
        }
    }
    let mut registry = BackendRegistry::default();
    for (id, config) in metadata {
        registry.register(id, config.attach(backend.clone()))?;
    }
    Ok(registry)
}
#[cfg(feature = "jev")]
/// Prepare required live bindings synchronously from SDK environment configuration.
pub fn live_bindings(
    required: &[BackendId],
    bindings: &Bindings,
) -> Result<BackendRegistry, CliError> {
    use typesafe_sdk_client::Client;
    use typesafe_sdk_config::Builder;
    use typesafe_sdk_env::Process;
    use typesafe_sdk_http::Reqwest;
    let mut registry = BackendRegistry::default();
    for id in required {
        let binding = bindings.get(id).ok_or_else(|| {
            SaphoError::new(ErrorCode::UnknownBackend, "Binding metadata absent")
                .with_context("backend", id.to_string())
        })?;
        let config = Builder::new()
            .build(&Process)
            .map_err(|_| CliError::ProviderConfiguration)?;
        let transport = Reqwest::new().map_err(|_| CliError::ProviderConfiguration)?;
        registry.register(
            id.clone(),
            binding.attach(Arc::new(sapho_jev::JevBackend::new(
                Client::with_transport(config, Arc::new(transport)),
            ))),
        )?;
    }
    Ok(registry)
}
#[cfg(not(feature = "jev"))]
/// Prepare required live bindings synchronously from SDK environment configuration.
pub fn live_bindings(
    required: &[BackendId],
    bindings: &Bindings,
) -> Result<BackendRegistry, CliError> {
    if let Some(id) = required.first() {
        let binding = bindings.get(id).ok_or_else(|| {
            SaphoError::new(ErrorCode::UnknownBackend, "Binding metadata absent")
                .with_context("backend", id.to_string())
        })?;
        return Err(CliError::Feature(binding.provider));
    }
    Ok(BackendRegistry::default())
}
