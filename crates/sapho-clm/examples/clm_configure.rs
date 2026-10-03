// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Prepare a backend without contacting the host-managed service.
use sapho_clm::{ClmBackend, DEFAULT_BASE_URL, DEFAULT_MODEL, Limits};
use sapho_core::{BackendBinding, BackendId, BackendRegistry, DistributionPolicy};
use std::{error::Error, sync::Arc};
fn main() -> Result<(), Box<dyn Error>> {
    let backend = ClmBackend::new(DEFAULT_BASE_URL, None, Limits::default())?;
    let mut registry = BackendRegistry::default();
    registry.register(
        BackendId::new("judge")?,
        BackendBinding {
            backend: Arc::new(backend),
            model: DEFAULT_MODEL.into(),
            expected_model: None,
            distribution_policy: DistributionPolicy::Strict {},
        },
    )?;
    println!("CLM binding configured; no inference performed");
    Ok(())
}
