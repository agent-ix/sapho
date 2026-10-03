// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Prepare Jev in a synchronous host; no inference is made by this example.
use sapho_core::{BackendBinding, BackendId, BackendRegistry, DistributionPolicy};
use sapho_jev::JevBackend;
use std::{error::Error, sync::Arc};
use typesafe_sdk_client::Client;
use typesafe_sdk_config::Builder;
use typesafe_sdk_env::Process;
use typesafe_sdk_http::Reqwest;
fn main() -> Result<(), Box<dyn Error>> {
    let config = Builder::new().build(&Process)?;
    let client = Client::with_transport(config, Arc::new(Reqwest::new()?));
    let mut registry = BackendRegistry::default();
    registry.register(
        BackendId::new("judge")?,
        BackendBinding {
            backend: Arc::new(JevBackend::new(client)),
            model: "jev-latest".into(),
            expected_model: None,
            distribution_policy: DistributionPolicy::Strict {},
        },
    )?;
    println!("Jev binding configured; no inference performed");
    Ok(())
}
