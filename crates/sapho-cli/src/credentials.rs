// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Synchronous shared credential resolution; never reached by offline commands.
use crate::{CliError, Provider};
use ix_cli_kit::secrets::{AppScope, CredentialBackend, SecretKey, SecretStore, SecretValue};
use std::ffi::OsString;

impl Provider {
    /// Host environment variable for this live provider's credential; `None` for a
    /// provider that needs no credential.
    pub const fn credential_environment(self) -> Option<&'static str> {
        match self {
            Self::Jev => Some("TYPESAFE_API_KEY"),
            Self::Clm => Some("CLM_API_KEY"),
            Self::Decisions => Some("OPENAI_API_KEY"),
            Self::Ollama => None,
        }
    }
    const fn credential_key(self) -> Option<&'static str> {
        match self {
            Self::Jev => Some("jev-api-key"),
            Self::Clm => Some("clm-api-key"),
            Self::Decisions => Some("openai-api-key"),
            Self::Ollama => None,
        }
    }
}
/// Resolve a synchronous native credential seam with an explicit environment snapshot.
/// Call outside Tokio workers; only live providers may call this function.
pub fn resolve_credential<B: CredentialBackend>(
    provider: Provider,
    store: &SecretStore<B>,
    explicit: Option<SecretValue>,
    environment: Option<OsString>,
) -> Result<Option<SecretValue>, CliError> {
    let (Some(environment_name), Some(key_name)) =
        (provider.credential_environment(), provider.credential_key())
    else {
        return Ok(None);
    };
    let scope = AppScope::try_from("agent-ix/sapho").map_err(CliError::from)?;
    let key = SecretKey::try_from(key_name).map_err(CliError::from)?;
    let resolved = store.resolve_from(
        explicit,
        environment.map(|value| (environment_name, value)),
        &scope,
        &key,
    )?;
    let secret = resolved.map(|resolved| resolved.value);
    if secret
        .as_ref()
        .is_some_and(|value| value.expose_secret().trim().is_empty())
    {
        return Err(CliError::CredentialInvalid);
    }
    if matches!(provider, Provider::Jev | Provider::Decisions) && secret.is_none() {
        return Err(CliError::CredentialMissing(provider));
    }
    Ok(secret)
}
/// Resolve a host endpoint using shared precedence without echoing untrusted values.
pub fn resolve_endpoint(
    explicit: Option<String>,
    environment: Option<String>,
    default: &str,
) -> String {
    ix_cli_kit::config::resolve(
        explicit,
        environment.map(|value| ("provider endpoint", value)),
        None,
        default.to_owned(),
    )
    .value
}
