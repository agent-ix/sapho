// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Public invocation seam; no paths, credentials or persistence inside evaluation.
use crate::CliError;
use sapho_core::{
    BackendId, BackendRegistry, ErrorCode, Inputs, NodeId, PrimitiveId, PrimitiveRegistry,
    SaphoError, Signature, Value, check_ports, decode_plain,
};
use sapho_graph::{CompiledGraph, GraphSpec, Operation, compile};
use sapho_runtime::{Engine, RunLimits, Trace};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Command exit policy, separate from application review semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExitStatus {
    /// Completed without an explicit finding.
    Completed,
    /// The selected Boolean finding output was true.
    Finding,
    /// Configuration, execution or coverage was refused.
    Refused,
}
impl ExitStatus {
    /// Process code: success 0, selected finding 1, refusal 2.
    pub const fn code(self) -> u8 {
        match self {
            Self::Completed => ix_cli_kit::Outcome::Ok.code(),
            Self::Finding => ix_cli_kit::Outcome::Partial.code(),
            Self::Refused => ix_cli_kit::Outcome::Refused.code(),
        }
    }
}
/// Stable stage ordering inside a root or mapped node scope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageGroup {
    /// Structured node scope, empty for root.
    pub scope: Vec<String>,
    /// Dependency stages preserving declaration-order ties.
    pub stages: Vec<Vec<NodeId>>,
}
/// Pure compile/inspection report without native work, credentials or inference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inspection {
    /// Exact root contract.
    pub signature: Signature,
    /// Root and mapped stage groups.
    pub groups: Vec<StageGroup>,
    /// Model names required by reachable inference nodes.
    pub backends: Vec<BackendId>,
    /// Native implementations required to compile all declared definitions.
    pub primitives: Vec<PrimitiveId>,
}
fn describe(graph: &CompiledGraph, spec: &GraphSpec) -> Inspection {
    fn visit(
        graph: &CompiledGraph,
        path: Vec<String>,
        groups: &mut Vec<StageGroup>,
        backends: &mut BTreeSet<BackendId>,
    ) {
        groups.push(StageGroup {
            scope: path.clone(),
            stages: graph
                .stages()
                .iter()
                .map(|stage| stage.iter().map(|n| n.spec().id.clone()).collect())
                .collect(),
        });
        for node in graph.stages().iter().flatten() {
            if let Operation::Ask { backend, .. } = &node.spec().operation {
                backends.insert(backend.clone());
            }
            if let Some(child) = node.mapped_graph() {
                let mut scope = path.clone();
                scope.push(node.spec().id.to_string());
                visit(&child, scope, groups, backends);
            }
        }
    }
    let mut groups = Vec::new();
    let mut backends = BTreeSet::new();
    visit(graph, Vec::new(), &mut groups, &mut backends);
    let primitives = spec
        .nodes
        .iter()
        .chain(spec.subgraphs.values().flat_map(|body| &body.nodes))
        .filter_map(|n| {
            if let Operation::Code { primitive, .. } = &n.operation {
                Some(primitive.clone())
            } else {
                None
            }
        })
        .collect::<BTreeSet<_>>();
    Inspection {
        signature: graph.signature().clone(),
        groups,
        backends: backends.into_iter().collect(),
        primitives: primitives.into_iter().collect(),
    }
}
/// Validate and describe a graph using the host's explicit primitive registry; no execution.
pub fn inspect(spec: &GraphSpec, primitives: &PrimitiveRegistry) -> Result<Inspection, CliError> {
    let graph = compile(spec, primitives)?;
    Ok(describe(&graph, spec))
}
/// Schema-directed plain input conversion; typed-input callers use existing Inputs unchanged.
pub fn plain_inputs(signature: &Signature, json: &serde_json::Value) -> Result<Inputs, CliError> {
    let fields = json
        .as_object()
        .ok_or_else(|| SaphoError::new(ErrorCode::TypeMismatch, "Input must be a JSON map"))?;
    if fields.len() != signature.inputs.len()
        || signature
            .inputs
            .keys()
            .any(|name| !fields.contains_key(name))
    {
        return Err(
            SaphoError::new(ErrorCode::MissingInput, "Input ports differ from signature").into(),
        );
    }
    let inputs = signature
        .inputs
        .iter()
        .map(|(name, ty)| {
            let value = fields
                .get(name)
                .ok_or_else(|| SaphoError::new(ErrorCode::MissingInput, "Input absent"))?;
            let id = serde_json::to_string(&("input", name))
                .map_err(|e| SaphoError::new(ErrorCode::InvalidValue, e.to_string()))?;
            Ok((name.clone(), decode_plain(id, ty, value, &[])?))
        })
        .collect::<Result<Inputs, CliError>>()?;
    check_ports(&signature.inputs, &inputs)?;
    Ok(inputs)
}
/// Typed output envelope, including partial trace when execution or finding selection fails.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunReport {
    /// Checked graph outputs on successful execution.
    pub outputs: Option<Inputs>,
    /// Complete or partial execution evidence.
    pub trace: Trace,
    /// Stable engine/selection refusal, if present.
    pub error: Option<SaphoError>,
    /// Explicit process outcome.
    pub exit: ExitStatus,
}
/// Prepared executor for data-only or custom-host graphs; evaluation performs no file I/O.
pub struct Runner {
    engine: Engine,
    inspection: Inspection,
}
impl Runner {
    /// Compile and bind host implementations before evaluation.
    pub fn new(
        spec: &GraphSpec,
        primitives: &PrimitiveRegistry,
        backends: BackendRegistry,
    ) -> Result<Self, CliError> {
        let graph = compile(spec, primitives)?;
        let inspection = describe(&graph, spec);
        Ok(Self {
            engine: Engine::new(graph, backends)?,
            inspection,
        })
    }
    /// Borrow the prepared invocation contract without performing work.
    pub fn inspection(&self) -> &Inspection {
        &self.inspection
    }
    /// Execute under explicit limits; an optional finding names one Boolean output.
    pub async fn run(
        &self,
        inputs: &Inputs,
        limits: RunLimits,
        fail_on: Option<&str>,
    ) -> RunReport {
        match self.engine.run(inputs, limits).await {
            Err(failure) => RunReport {
                outputs: None,
                trace: failure.trace,
                error: Some(failure.error),
                exit: ExitStatus::Refused,
            },
            Ok(result) => {
                let decision = if let Some(name) = fail_on {
                    match result.outputs.get(name).map(|d| &d.value) {
                        Some(Value::Boolean(true)) => Ok(ExitStatus::Finding),
                        Some(Value::Boolean(false)) => Ok(ExitStatus::Completed),
                        _ => Err(SaphoError::new(
                            ErrorCode::TypeMismatch,
                            "--fail-on requires a Boolean output",
                        )
                        .with_context("output", name)),
                    }
                } else {
                    Ok(ExitStatus::Completed)
                };
                let (exit, error) = match decision {
                    Ok(exit) => (exit, None),
                    Err(error) => (ExitStatus::Refused, Some(error)),
                };
                RunReport {
                    outputs: Some(result.outputs),
                    trace: result.trace,
                    error,
                    exit,
                }
            }
        }
    }
}
