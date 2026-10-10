// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Host-only named System One service configuration and bounded secret resolution.
use crate::CliError;
use ix_cli_kit::secrets::{AppScope, CredentialBackend, SecretKey, SecretStore, SecretValue};
use sapho_core::{BackendId, ErrorCode, SaphoError, decode_json};
use serde::{
    Deserialize, Serialize,
    de::{MapAccess, Visitor},
};
use std::{collections::BTreeMap, fmt};

/// Maximum serialized host service configuration bytes.
pub const MAX_SERVICE_CONFIG_BYTES: usize = 1_048_576;

/// One service's optional bounded adapter ceilings.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceLimits {
    /// Full request deadline, including queue time.
    pub timeout_ms: u64,
    /// Maximum request bytes.
    pub request_bytes: usize,
    /// Maximum response bytes.
    pub response_bytes: usize,
    /// Maximum simultaneous calls.
    pub in_flight: usize,
}
impl ServiceLimits {
    fn validate(self) -> bool {
        (1..=30_000).contains(&self.timeout_ms)
            && (1..=1_048_576).contains(&self.request_bytes)
            && (1..=8_388_608).contains(&self.response_bytes)
            && (1..=4).contains(&self.in_flight)
    }
    #[cfg(feature = "clm")]
    pub(crate) fn adapter(self) -> sapho_clm::Limits {
        sapho_clm::Limits {
            timeout: std::time::Duration::from_millis(self.timeout_ms),
            request_bytes: self.request_bytes,
            response_bytes: self.response_bytes,
            in_flight: self.in_flight,
        }
    }
}

/// Host-only endpoint and optional OS-store key reference for one binding.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceEntry {
    /// Base URL; never copied into a graph, binding or report.
    pub base_url: String,
    /// Optional named key within the `agent-ix/sapho` OS secret scope.
    pub credential_key: Option<String>,
    /// Optional complete override, capped by stock adapter defaults.
    pub limits: Option<ServiceLimits>,
}

/// Exact service entries keyed by binding ID. Duplicate JSON keys are refused.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ServiceConfig {
    /// Named services; unused entries do not create transport or read secrets.
    pub services: BTreeMap<BackendId, ServiceEntry>,
}
impl<'de> Deserialize<'de> for ServiceConfig {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        struct Entries(BTreeMap<BackendId, ServiceEntry>);
        impl<'de> Deserialize<'de> for Entries {
            fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
                struct EntriesVisitor;
                impl<'de> Visitor<'de> for EntriesVisitor {
                    type Value = Entries;
                    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                        formatter.write_str("a map of unique backend IDs to service entries")
                    }
                    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Entries, M::Error> {
                        let mut entries = BTreeMap::new();
                        while let Some((id, entry)) = map.next_entry::<BackendId, ServiceEntry>()? {
                            if entries.insert(id, entry).is_some() {
                                return Err(serde::de::Error::custom(
                                    "Duplicate service binding ID",
                                ));
                            }
                        }
                        Ok(Entries(entries))
                    }
                }
                decoder.deserialize_map(EntriesVisitor)
            }
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            services: Entries,
        }
        Ok(Self {
            services: Wire::deserialize(decoder)?.services.0,
        })
    }
}
impl ServiceConfig {
    /// Parse and validate a bounded JSON host file without constructing transports.
    pub fn from_json(bytes: &[u8]) -> Result<Self, CliError> {
        let config: Self = decode_json(bytes, MAX_SERVICE_CONFIG_BYTES)?;
        config.validate()?;
        Ok(config)
    }
    /// Validate constructed or deserialized entries before live preparation.
    pub fn validate(&self) -> Result<(), CliError> {
        for (id, entry) in &self.services {
            id.validate()
                .map_err(|_| config_error(id, "Invalid service binding ID"))?;
            if entry.base_url.trim().is_empty() {
                return Err(config_error(id, "Empty service endpoint"));
            }
            if let Some(key) = &entry.credential_key {
                SecretKey::try_from(key.as_str())
                    .map_err(|_| config_error(id, "Invalid service credential key"))?;
            }
            if entry.limits.is_some_and(|limits| !limits.validate()) {
                return Err(config_error(id, "Invalid service limits"));
            }
        }
        Ok(())
    }
    /// Select exactly one entry for a required named binding.
    pub fn required(&self, id: &BackendId) -> Result<&ServiceEntry, CliError> {
        self.services
            .get(id)
            .ok_or_else(|| config_error(id, "Service configuration absent"))
    }
}
pub(crate) fn config_error(id: &BackendId, detail: &'static str) -> CliError {
    SaphoError::new(ErrorCode::Config, detail)
        .with_context("backend", id.to_string())
        .into()
}
/// Resolve only a required service's optional key from the native store.
pub fn service_credential<B: CredentialBackend>(
    entry: &ServiceEntry,
    store: &SecretStore<B>,
) -> Result<Option<SecretValue>, CliError> {
    let Some(name) = entry.credential_key.as_deref() else {
        return Ok(None);
    };
    let scope = AppScope::try_from("agent-ix/sapho")?;
    let key = SecretKey::try_from(name)?;
    let secret = store.get(&scope, &key)?;
    if secret
        .as_ref()
        .is_some_and(|value| value.expose_secret().trim().is_empty())
    {
        return Err(CliError::CredentialInvalid);
    }
    if secret.is_none() {
        return Err(CliError::CredentialMissing(crate::Provider::Systemone));
    }
    Ok(secret)
}
