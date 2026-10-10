// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! In-memory execution evidence; this module never writes files (FR-017).
use sapho_core::{Datum, Inputs, ModelRequest, ModelResponse, NodeId, RawExchange, SaphoError};
use sapho_graph::Operation;
use serde::{Deserialize, Serialize};
/// Terminal execution state of a node instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    /// Operation returned validated outputs.
    Completed,
    /// Boolean guard was false; no operation work ran.
    Skipped,
    /// Operation or preparation failed.
    Failed,
}
/// Exact model exchange, including a raw response when validation fails.
///
/// Equality is the deterministic trace comparison: it ignores every raw exchange, whose
/// provider bodies carry timestamps and duration counters that differ between identical runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelEvidence {
    /// Request reconstructed by this graph execution.
    pub request: ModelRequest,
    /// Raw response if inference returned one.
    pub response: Option<ModelResponse>,
    /// The exchange a failed ask's error carries, when the backend retained one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<RawExchange>,
}
impl PartialEq for ModelEvidence {
    fn eq(&self, other: &Self) -> bool {
        let bare = |r: &Option<ModelResponse>| {
            r.clone().map(|mut r| {
                r.raw = None;
                r
            })
        };
        self.request == other.request && bare(&self.response) == bare(&other.response)
    }
}
/// Evidence for one concrete node, including map item scope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeTrace {
    /// Structured execution path, avoiding delimiter collisions.
    pub path: Vec<String>,
    /// Fully scoped producer paths explicitly named by bindings.
    pub dependencies: Vec<Vec<String>>,
    /// Resolved Boolean guard, when configured; operation inputs can remain unresolved on skip.
    pub guard: Option<Datum>,
    /// Declared operation and parameters.
    pub operation: Operation,
    /// Resolved values and their source references.
    pub inputs: Inputs,
    /// Checked output values and references.
    pub outputs: Inputs,
    /// Completed, skipped or failed.
    pub status: NodeStatus,
    /// Refusal detail on a failed node.
    pub error: Option<SaphoError>,
    /// Request and raw response for an ask node.
    pub model: Option<ModelEvidence>,
}
/// One projected shadow value or typed best-effort refusal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShadowTrace {
    /// Observational ask node.
    pub node: NodeId,
    /// Observation name within that ask.
    pub name: String,
    /// Public decision output being compared.
    pub output: String,
    /// Completed, failed or skipped.
    pub status: NodeStatus,
    /// Projected value, when available.
    pub value: Option<Datum>,
    /// Typed refusal, when available.
    pub error: Option<SaphoError>,
    /// Exchange evidence of the shadow ask, when dispatched.
    pub model: Option<ModelEvidence>,
}
/// Ordered deterministic content of a run, including a partial failed run.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trace {
    /// Stable topological/declaration order; child paths follow their parent stage.
    pub nodes: Vec<NodeTrace>,
    /// Projected shadow observations, absent from legacy no-shadow JSON.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shadows: Vec<ShadowTrace>,
}
