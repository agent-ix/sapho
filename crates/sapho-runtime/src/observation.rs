// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Opt-in model-call evidence kept outside deterministic traces (FR-061).
use crate::{NodeStatus, Trace};
use sapho_core::{BackendId, ProviderDescriptor, SaphoError, Usage};
use sapho_graph::Operation;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Instant,
};

/// Whether the observed calls used a live backend or an exact recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationMode {
    /// Live calls can have measured elapsed time.
    Live,
    /// Replay calls carry no latency measurement.
    Replay,
}

/// Monotonic clock seam for deterministic elapsed-time tests.
pub trait ObservationClock: Send + Sync {
    /// Return a monotonic instant.
    fn now(&self) -> Instant;
}
/// Production monotonic clock.
pub struct SystemObservationClock;
impl ObservationClock for SystemObservationClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// Opt-in settings validated before the run.
pub struct ObservationConfig {
    /// Live or replay call semantics.
    pub mode: ObservationMode,
    /// Optional bounded descriptors indexed by binding.
    pub descriptors: BTreeMap<BackendId, ProviderDescriptor>,
    /// Clock used only for live elapsed times.
    pub clock: Arc<dyn ObservationClock>,
}
impl ObservationConfig {
    /// Validate descriptors before a model request can be sent.
    pub fn validate(&self) -> Result<(), SaphoError> {
        for descriptor in self.descriptors.values() {
            descriptor.validate()?;
        }
        Ok(())
    }
}

/// One attempted model call, keyed by the trace node path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelCallObservation {
    /// Structured trace path, including mapped item scope.
    pub path: Vec<String>,
    /// Declared backend binding.
    pub binding: BackendId,
    /// Model requested by that binding.
    pub requested_model: String,
    /// Actual identity from a response, when present.
    pub actual_model: Option<String>,
    /// Terminal node status.
    pub status: NodeStatus,
    /// Provider usage from a response, when present.
    pub usage: Option<Usage>,
    /// Live elapsed microseconds; absent for replay.
    pub elapsed_micros: Option<u64>,
    /// Validated nonsecret metadata.
    pub descriptor: Option<ProviderDescriptor>,
    /// Execution mode for refusing mixed roster inputs.
    pub mode: ObservationMode,
}

pub(crate) fn collect(
    trace: &Trace,
    durations: &BTreeMap<usize, u64>,
    sample_durations: &BTreeMap<(usize, u32), u64>,
    dispatched: &BTreeSet<usize>,
    config: &ObservationConfig,
) -> Vec<ModelCallObservation> {
    trace
        .nodes
        .iter()
        .enumerate()
        .flat_map(|(index, node)| {
            let binding = match &node.operation {
                Operation::Ask { backend, .. } | Operation::ShadowAsk { backend, .. } => {
                    backend.clone()
                }
                _ => return Vec::new(),
            };
            let build =
                |model: &crate::ModelEvidence, status, elapsed_micros| ModelCallObservation {
                    path: node.path.clone(),
                    binding: binding.clone(),
                    requested_model: model.request.model.clone(),
                    actual_model: model.response.as_ref().map(|r| r.model.clone()),
                    status,
                    usage: model.response.as_ref().and_then(|r| r.usage.clone()),
                    elapsed_micros: (config.mode == ObservationMode::Live)
                        .then_some(elapsed_micros)
                        .flatten(),
                    descriptor: config.descriptors.get(&binding).cloned(),
                    mode: config.mode,
                };
            if let Some(samples) = &node.samples {
                samples
                    .iter()
                    .map(|sample| {
                        build(
                            &sample.model,
                            sample.status,
                            sample_durations.get(&(index, sample.index)).copied(),
                        )
                    })
                    .collect()
            } else if dispatched.contains(&index) {
                node.model
                    .as_ref()
                    .map(|model| build(model, node.status, durations.get(&index).copied()))
                    .into_iter()
                    .collect()
            } else {
                Vec::new()
            }
        })
        .collect()
}
