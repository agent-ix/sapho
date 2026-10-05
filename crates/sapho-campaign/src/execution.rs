// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Shared bounded exact-recording bindings. Provider construction stays explicit.
use crate::{Error, ErrorCode, Result, runner::Capture};
use sapho_core::{BackendId, BackendRegistry};
use sapho_recording::RecordingBackend;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};
/// Credential-free immutable invocation provenance retained before dispatch.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    /// Named model/provider identity and known installed digest/capabilities.
    pub backends: BTreeMap<String, serde_json::Value>,
}
/// Recording session shared by all domain consumers. Does not install providers.
pub struct Session {
    /// Explicit wrapped backend registry passed to the native engine/evaluator.
    pub bindings: BackendRegistry,
    recordings: Vec<(BackendId, Arc<RecordingBackend>)>,
    meters: Vec<Arc<crate::meter::Meter>>,
    max_bytes: usize,
}
impl Session {
    /// Wrap exactly the required host bindings under independent exact-recording bounds.
    pub fn record(
        required: &[BackendId],
        registry: &BackendRegistry,
        max_bytes: usize,
    ) -> Result<Self> {
        if max_bytes == 0 {
            return Err(Error::new(
                ErrorCode::Invalid,
                "positive recording limit required",
            ));
        }
        let mut bindings = BackendRegistry::default();
        let mut recordings = Vec::new();
        let mut meters = Vec::new();
        for id in required {
            let mut binding = registry
                .get(id)
                .map_err(|e| Error::new(ErrorCode::Invalid, e.to_string()))?;
            let meter = Arc::new(crate::meter::Meter::new(
                binding.backend,
                max_bytes,
                100_000,
            )?);
            meters.push(meter.clone());
            let recording = Arc::new(
                RecordingBackend::new(meter, max_bytes)
                    .map_err(|e| Error::new(ErrorCode::Invalid, e.to_string()))?,
            );
            binding.backend = recording.clone();
            bindings
                .register(id.clone(), binding)
                .map_err(|e| Error::new(ErrorCode::Invalid, e.to_string()))?;
            recordings.push((id.clone(), recording));
        }
        Ok(Self {
            bindings,
            recordings,
            meters,
            max_bytes,
        })
    }
    /// Snapshot successful exchanges after execution, retaining export failure evidence.
    /// Call synchronously after await, including failed engine runs.
    pub fn capture(&self) -> Result<Vec<Capture>> {
        let mut captures = Vec::new();
        for meter in &self.meters {
            captures.extend(meter.capture()?);
        }
        for (id, recording) in &self.recordings {
            let snapshot = recording.snapshot().and_then(|r| r.to_json(self.max_bytes));
            match snapshot {
                Ok(bytes) => captures.push(Capture {
                    kind: format!("exact_recording:{}", id.as_str()),
                    bytes,
                }),
                Err(error) => captures.push(Capture {
                    kind: format!("recording_export_failed:{}", id.as_str()),
                    bytes: sapho_core::bounded_json(
                        &serde_json::json!({"backend":id,"error":error}),
                        4096,
                    )
                    .map_err(|e| Error::new(ErrorCode::Storage, e.to_string()))?,
                }),
            }
        }
        Ok(captures)
    }
}
