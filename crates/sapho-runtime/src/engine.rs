// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Cooperative bounded orchestration, including dependent map subgraphs.
use crate::{ModelEvidence, NodeStatus, NodeTrace, ShadowTrace, Trace, operators};
use futures::{
    future::BoxFuture,
    stream::{FuturesUnordered, StreamExt},
};
use sapho_core::{
    BackendRegistry, Datum, ErrorCode, Inputs, ModelRequest, NodeId, PrimitiveContext, Result,
    SaphoError, Value, check_ports, measured_json_bytes, validate_response,
};
use sapho_graph::{
    Binding, CompiledGraph, CompiledNode, Operation, ShadowObservation, ShadowProjection,
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

/// Explicit caller ceilings, shared by all nested subgraph work.
#[derive(Debug, Clone)]
pub struct RunLimits {
    /// Maximum total executed node instances.
    pub node_instances: usize,
    /// Maximum cumulatively expanded collection items.
    pub collection_items: usize,
    /// Maximum total explicit model calls.
    pub model_requests: usize,
    /// Maximum simultaneous model/native workers.
    pub concurrency: usize,
    /// Maximum cumulatively accounted serialized input/output bytes.
    pub data_bytes: usize,
    /// Total monotonic execution budget.
    pub duration: Duration,
}
impl Default for RunLimits {
    fn default() -> Self {
        Self {
            node_instances: 4096,
            collection_items: 16384,
            model_requests: 128,
            concurrency: 4,
            data_bytes: 8 * 1_048_576,
            duration: Duration::from_secs(60),
        }
    }
}
impl RunLimits {
    fn validate(&self) -> Result<()> {
        if [
            self.node_instances,
            self.collection_items,
            self.model_requests,
            self.concurrency,
            self.data_bytes,
        ]
        .contains(&0)
            || self.duration.is_zero()
        {
            return Err(SaphoError::new(
                ErrorCode::InvalidValue,
                "Run ceilings must be non-zero",
            ));
        }
        Ok(())
    }
}
/// Completed graph outputs and ordered execution evidence.
#[derive(Debug, Clone)]
pub struct RunResult {
    /// Named outputs with their source sidecars.
    pub outputs: Inputs,
    /// Complete deterministic evidence content.
    pub trace: Trace,
}
/// Aborted graph execution, retaining its preceding and failed evidence.
#[derive(Debug, Clone)]
pub struct RunFailure {
    /// Structured failure category and detail.
    pub error: SaphoError,
    /// Partial trace accumulated before refusal.
    pub trace: Trace,
}
impl std::fmt::Display for RunFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}
impl std::error::Error for RunFailure {}
/// Embeddable executor over a compiled immutable graph and explicit model bindings.
pub struct Engine {
    graph: Arc<CompiledGraph>,
    backends: BackendRegistry,
}
impl Engine {
    /// Bind model implementations, rejecting unknown names before work starts.
    pub fn new(graph: CompiledGraph, backends: BackendRegistry) -> Result<Self> {
        validate_backends(&graph, &backends)?;
        Ok(Self {
            graph: Arc::new(graph),
            backends,
        })
    }
    /// Execute with bounded work; no trace persistence or model retry is implicit.
    pub async fn run(
        &self,
        inputs: &Inputs,
        limits: RunLimits,
    ) -> std::result::Result<RunResult, RunFailure> {
        let fail = |error| RunFailure {
            error,
            trace: Trace::default(),
        };
        limits.validate().map_err(fail)?;
        check_ports(&self.graph.signature().inputs, inputs).map_err(fail)?;
        let deadline = Instant::now().checked_add(limits.duration).ok_or_else(|| {
            fail(SaphoError::new(
                ErrorCode::InvalidValue,
                "Duration exceeds monotonic clock range",
            ))
        })?;
        let mut state = RunState {
            limits,
            deadline,
            nodes: 0,
            items: 0,
            requests: 0,
            bytes: 0,
            trace: Trace::default(),
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        state.data(inputs).map_err(|error| RunFailure {
            error,
            trace: state.trace.clone(),
        })?;
        match self
            .graph_run(&self.graph, inputs.clone(), vec!["root".into()], &mut state)
            .await
        {
            Ok((outputs, mut values)) => {
                self.run_shadows(inputs, &mut values, &mut state).await;
                Ok(RunResult {
                    outputs,
                    trace: state.trace.clone(),
                })
            }
            Err(error) => {
                for entry in &mut state.trace.nodes {
                    if entry.status == NodeStatus::Failed && entry.error.is_none() {
                        entry.error = Some(SaphoError::new(
                            error.code,
                            "Node did not complete before run aborted",
                        ));
                    }
                }
                Err(RunFailure {
                    error,
                    trace: state.trace.clone(),
                })
            }
        }
    }
    fn graph_run<'a>(
        &'a self,
        graph: &'a CompiledGraph,
        inputs: Inputs,
        path: Vec<String>,
        state: &'a mut RunState,
    ) -> BoxFuture<'a, Result<(Inputs, BTreeMap<NodeId, Inputs>)>> {
        Box::pin(async move {
            check_ports(&graph.signature().inputs, &inputs)?;
            let mut values = BTreeMap::new();
            for stage in graph.stages() {
                let mut jobs = Vec::new();
                // Reserve evidence rows in declaration order before concurrent completion.
                for node in stage {
                    state.check_time()?;
                    state.add_node()?;
                    let mut np = path.clone();
                    np.push(node.spec().id.to_string());
                    let index = state.trace.nodes.len();
                    let dependencies = node
                        .spec()
                        .inputs
                        .values()
                        .chain(node.spec().guard.iter())
                        .filter_map(|b| match b {
                            Binding::Node { node, .. } => {
                                let mut dependency = path.clone();
                                dependency.push(node.to_string());
                                Some(dependency)
                            }
                            Binding::Input { .. } | Binding::Literal { .. } => None,
                        })
                        .collect();
                    state.trace.nodes.push(NodeTrace {
                        path: np.clone(),
                        dependencies,
                        guard: None,
                        operation: node.spec().operation.clone(),
                        inputs: Inputs::new(),
                        outputs: Inputs::new(),
                        status: NodeStatus::Failed,
                        error: None,
                        model: None,
                    });
                    let prepared = (|| {
                        if let Some(g) = &node.spec().guard {
                            let guard = resolve(g, &inputs, &values)?;
                            state.trace_entry(index)?.guard = Some(guard.clone());
                            if matches!(guard.value, Value::Boolean(false)) {
                                let out = node
                                    .output_types()
                                    .keys()
                                    .map(|k| {
                                        Ok((
                                            k.clone(),
                                            make_datum(
                                                &np,
                                                k,
                                                Value::Optional(None),
                                                &Inputs::from([("guard".into(), guard.clone())]),
                                            )?,
                                        ))
                                    })
                                    .collect::<Result<Inputs>>()?;
                                return Ok((Inputs::new(), Some(out)));
                            }
                        }
                        let ins = node
                            .spec()
                            .inputs
                            .iter()
                            .map(|(k, b)| Ok((k.clone(), resolve(b, &inputs, &values)?)))
                            .collect::<Result<Inputs>>()?;
                        check_ports(&node.signature().inputs, &ins)?;
                        Ok((ins, None))
                    })();
                    let (ins, skipped) = match prepared {
                        Ok(v) => v,
                        Err(e) => {
                            state.failed(index, &e);
                            return Err(e);
                        }
                    };
                    state.trace_entry(index)?.inputs = ins.clone();
                    jobs.push((node, index, np, ins, skipped));
                }
                let mut asks = Vec::new();
                for (node, index, np, ins, skipped) in jobs {
                    if let Some(outputs) = skipped {
                        self.finish(node, index, outputs, true, state, &mut values)?;
                        continue;
                    }
                    if let Operation::Ask { backend } = &node.spec().operation {
                        let prepared = (|| {
                            let binding = self.backends.get(backend)?;
                            let state_value = operators::operand(&ins, "state")?.clone();
                            let Value::Questions(questions) =
                                operators::operand(&ins, "questions")?
                            else {
                                return Err(SaphoError::new(
                                    ErrorCode::TypeMismatch,
                                    "Ask needs Questions",
                                ));
                            };
                            let request = ModelRequest {
                                distribution_policy: binding.distribution_policy,
                                backend: backend.clone(),
                                model: binding.model,
                                expected_model: binding.expected_model,
                                state: state_value,
                                questions: questions.clone(),
                            };
                            request.validate()?;
                            state.data(&request)?;
                            state.add_request()?;
                            state.trace_entry(index)?.model = Some(ModelEvidence {
                                request: request.clone(),
                                response: None,
                                raw: None,
                            });
                            Ok((binding.backend, request))
                        })();
                        match prepared {
                            Ok((backend, request)) => {
                                asks.push((node, index, np, ins, backend, request))
                            }
                            Err(e) => {
                                state.failed(index, &e);
                                return Err(e);
                            }
                        }
                    } else {
                        let result = self.non_model(node, &np, &ins, state).await;
                        match result {
                            Ok(outputs) => {
                                self.finish(node, index, outputs, false, state, &mut values)?
                            }
                            Err(e) => {
                                state.failed(index, &e);
                                return Err(e);
                            }
                        }
                    }
                }
                // Fixed-sized chunks bound ready model work while preserving result order.
                for chunk in asks.chunks(state.limits.concurrency) {
                    let mut futures = FuturesUnordered::new();
                    let deadline = tokio::time::Instant::from_std(state.deadline);
                    for (order, (_, _, _, _, backend, request)) in chunk.iter().enumerate() {
                        futures.push(async move {
                            let response =
                                tokio::time::timeout_at(deadline, backend.infer(request))
                                    .await
                                    .map_err(|_| {
                                        SaphoError::new(
                                            ErrorCode::DeadlineExceeded,
                                            "Model deadline exceeded",
                                        )
                                    })
                                    .and_then(|r| r);
                            (order, response)
                        });
                    }
                    while let Some((order, result)) = futures.next().await {
                        let (node, index, np, ins, _, request) =
                            chunk.get(order).ok_or_else(|| {
                                SaphoError::new(ErrorCode::BackendFailed, "Model task order absent")
                            })?;
                        let processed = (|| {
                            let response = result?;
                            if let Some(evidence) = &mut state.trace_entry(*index)?.model {
                                evidence.response = Some(response.clone());
                            }
                            state.data(&response)?;
                            let validated = validate_response(request, &response)?;
                            Ok(Inputs::from([
                                (
                                    "answers".into(),
                                    make_datum(np, "answers", Value::Answers(validated), ins)?,
                                ),
                                (
                                    "model".into(),
                                    make_datum(np, "model", Value::Text(response.model), ins)?,
                                ),
                            ]))
                        })();
                        match processed {
                            Ok(outputs) => {
                                self.finish(node, *index, outputs, false, state, &mut values)?
                            }
                            Err(e) => {
                                state.failed(*index, &e);
                                return Err(e);
                            }
                        }
                    }
                }
            }
            let outputs = graph
                .outputs()
                .iter()
                .map(|(k, b)| Ok((k.clone(), resolve(b, &inputs, &values)?)))
                .collect::<Result<Inputs>>()?;
            check_ports(&graph.signature().outputs, &outputs)?;
            state.check_time()?;
            Ok((outputs, values))
        })
    }
    async fn run_shadows(
        &self,
        inputs: &Inputs,
        values: &mut BTreeMap<NodeId, Inputs>,
        state: &mut RunState,
    ) {
        let path = vec!["root".to_owned()];
        let mut blocked = BTreeMap::<NodeId, SaphoError>::new();
        for stage in self.graph.shadow_stages() {
            for node in stage {
                let mut np = path.clone();
                np.push(node.spec().id.to_string());
                let index = state.trace.nodes.len();
                let dependencies = node
                    .spec()
                    .inputs
                    .values()
                    .chain(node.spec().guard.iter())
                    .filter_map(|binding| match binding {
                        Binding::Node { node, .. } => Some(vec!["root".into(), node.to_string()]),
                        Binding::Input { .. } | Binding::Literal { .. } => None,
                    })
                    .collect();
                state.trace.nodes.push(NodeTrace {
                    path: np.clone(),
                    dependencies,
                    guard: None,
                    operation: node.spec().operation.clone(),
                    inputs: Inputs::new(),
                    outputs: Inputs::new(),
                    status: NodeStatus::Failed,
                    error: None,
                    model: None,
                });
                let result: Result<()> = async {
                    state.check_time()?;
                    state.add_node()?;
                    for binding in node.spec().inputs.values().chain(node.spec().guard.iter()) {
                        if let Binding::Node { node: producer, .. } = binding
                            && let Some(error) = blocked.get(producer)
                        {
                            return Err(error.clone());
                        }
                    }
                    if let Some(guard_binding) = &node.spec().guard {
                        let guard = resolve(guard_binding, inputs, values)?;
                        state.trace_entry(index)?.guard = Some(guard.clone());
                        if matches!(guard.value, Value::Boolean(false)) {
                            let outputs = node
                                .output_types()
                                .keys()
                                .map(|port| {
                                    Ok((
                                        port.clone(),
                                        make_datum(
                                            &np,
                                            port,
                                            Value::Optional(None),
                                            &Inputs::from([("guard".into(), guard.clone())]),
                                        )?,
                                    ))
                                })
                                .collect::<Result<Inputs>>()?;
                            return self.finish(node, index, outputs, true, state, values);
                        }
                    }
                    let resolved = node
                        .spec()
                        .inputs
                        .iter()
                        .map(|(name, binding)| {
                            Ok((name.clone(), resolve(binding, inputs, values)?))
                        })
                        .collect::<Result<Inputs>>()?;
                    check_ports(&node.signature().inputs, &resolved)?;
                    state.trace_entry(index)?.inputs = resolved.clone();
                    match &node.spec().operation {
                        Operation::ShadowAsk { backend, .. } => {
                            let binding = self.backends.get(backend)?;
                            let state_value = operators::operand(&resolved, "state")?.clone();
                            let Value::Questions(questions) =
                                operators::operand(&resolved, "questions")?
                            else {
                                return Err(SaphoError::new(
                                    ErrorCode::TypeMismatch,
                                    "Shadow ask needs Questions",
                                ));
                            };
                            let request = ModelRequest {
                                distribution_policy: binding.distribution_policy,
                                backend: backend.clone(),
                                model: binding.model,
                                expected_model: binding.expected_model,
                                state: state_value,
                                questions: questions.clone(),
                            };
                            request.validate()?;
                            state.data(&request)?;
                            state.add_request()?;
                            state.trace_entry(index)?.model = Some(ModelEvidence {
                                request: request.clone(),
                                response: None,
                                raw: None,
                            });
                            let response = tokio::time::timeout_at(
                                tokio::time::Instant::from_std(state.deadline),
                                binding.backend.infer(&request),
                            )
                            .await
                            .map_err(|_| {
                                SaphoError::new(
                                    ErrorCode::DeadlineExceeded,
                                    "Shadow model deadline exceeded",
                                )
                            })??;
                            if let Some(evidence) = &mut state.trace_entry(index)?.model {
                                evidence.response = Some(response.clone());
                            }
                            state.data(&response)?;
                            let answers = validate_response(&request, &response)?;
                            let outputs = Inputs::from([
                                (
                                    "answers".into(),
                                    make_datum(&np, "answers", Value::Answers(answers), &resolved)?,
                                ),
                                (
                                    "model".into(),
                                    make_datum(
                                        &np,
                                        "model",
                                        Value::Text(response.model),
                                        &resolved,
                                    )?,
                                ),
                            ]);
                            self.finish(node, index, outputs, false, state, values)
                        }
                        _ => {
                            let outputs = self.non_model(node, &np, &resolved, state).await?;
                            self.finish(node, index, outputs, false, state, values)
                        }
                    }
                }
                .await;
                if let Err(error) = result {
                    let skipped = error.code == ErrorCode::LimitExceeded
                        || (error.code == ErrorCode::DeadlineExceeded
                            && error.message.as_ref() == "Run deadline exceeded")
                        || blocked.values().any(|upstream| upstream == &error);
                    state.failed(index, &error);
                    if skipped {
                        state.trace.nodes[index].status = NodeStatus::Skipped;
                    }
                    blocked.insert(node.spec().id.clone(), error);
                }
                if let Operation::ShadowAsk { observations, .. } = &node.spec().operation {
                    for observation in observations {
                        state.trace.shadows.push(shadow_observation(
                            node,
                            observation,
                            state.trace.nodes.get(index).expect("just appended"),
                            values,
                        ));
                    }
                }
            }
        }
    }
    async fn non_model(
        &self,
        node: &CompiledNode,
        path: &[String],
        ins: &Inputs,
        state: &mut RunState,
    ) -> Result<Inputs> {
        match &node.spec().operation {
            Operation::Record {} => one_output(
                path,
                Value::Record(
                    ins.iter()
                        .map(|(name, d)| (name.clone(), d.value.clone()))
                        .collect(),
                ),
                ins,
            ),
            Operation::List { order, .. } => {
                state.add_items(order.len())?;
                let items = order
                    .iter()
                    .map(|name| {
                        let input = ins.get(name).ok_or_else(|| {
                            SaphoError::new(ErrorCode::MissingInput, "List operand absent")
                        })?;
                        make_datum(
                            path,
                            name,
                            input.value.clone(),
                            &Inputs::from([(name.clone(), input.clone())]),
                        )
                    })
                    .collect::<Result<_>>()?;
                one_output(path, Value::List(items), ins)
            }
            Operation::Code { params, .. } => {
                let primitive = node.primitive().ok_or_else(|| {
                    SaphoError::new(
                        ErrorCode::UnknownPrimitive,
                        "Compiled code implementation absent",
                    )
                })?;
                let input = ins.clone();
                let params = params.clone();
                let ctx = PrimitiveContext::new(state.deadline, state.cancelled.clone());
                let worker =
                    tokio::task::spawn_blocking(move || primitive.execute(&ctx, &input, &params));
                tokio::time::timeout_at(tokio::time::Instant::from_std(state.deadline), worker)
                    .await
                    .map_err(|_| {
                        SaphoError::new(ErrorCode::DeadlineExceeded, "Native deadline exceeded")
                    })?
                    .map_err(|e| SaphoError::new(ErrorCode::CodeFailed, e.to_string()))?
            }
            Operation::Questions { questions } => {
                one_output(path, Value::Questions(questions.clone()), ins)
            }
            Operation::Map { .. } => {
                let graph = node.mapped_graph().ok_or_else(|| {
                    SaphoError::new(ErrorCode::UnknownReference, "Compiled mapped graph absent")
                })?;
                let items = operators::items(operators::operand(ins, "items")?)?;
                state.add_items(items.len())?;
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    state.check_time()?;
                    let mut captured = ins.clone();
                    captured.remove("items");
                    captured.insert("item".into(), item.clone());
                    let mut child = path.to_vec();
                    child.push(item.id.to_string());
                    let (mut outputs, _) = self.graph_run(&graph, captured, child, state).await?;
                    let mut d = outputs.remove("result").ok_or_else(|| {
                        SaphoError::new(ErrorCode::MissingInput, "Map result absent")
                    })?;
                    d.id = item.id.clone();
                    d.inherit_sources(item.sources.clone());
                    out.push(d);
                }
                one_output(path, Value::List(out), ins)
            }
            Operation::Filter => {
                let count = operators::items(operators::operand(ins, "items")?)?.len();
                state.add_items(count)?;
                one_output(path, Value::List(operators::filtered(ins)?), ins)
            }
            Operation::Pairs | Operation::Join { .. } => {
                let indices =
                    operators::pair_indices(&node.spec().operation, ins, state.remaining_items())?;
                state.add_items(indices.len())?;
                one_output(path, Value::List(operators::pairs(ins, &indices)?), ins)
            }
            Operation::Collect => {
                let count = operators::items(operators::operand(ins, "items")?)?
                    .iter()
                    .try_fold(0usize, |n, d| {
                        n.checked_add(operators::items(&d.value)?.len())
                            .ok_or_else(|| {
                                SaphoError::new(
                                    ErrorCode::LimitExceeded,
                                    "Collection count overflow",
                                )
                            })
                    })?;
                state.add_items(count)?;
                one_output(path, Value::List(operators::collected(ins)?), ins)
            }
            Operation::And
            | Operation::Or
            | Operation::Not
            | Operation::Compare { .. }
            | Operation::Probability { .. }
            | Operation::Degree
            | Operation::Reduce { .. }
            | Operation::Complement
            | Operation::Coalesce => {
                one_output(path, operators::logic(&node.spec().operation, ins)?, ins)
            }
            Operation::Ask { .. } | Operation::ShadowAsk { .. } => Err(SaphoError::new(
                ErrorCode::BackendFailed,
                "Ask must run through inference scheduler",
            )),
        }
    }
    fn finish(
        &self,
        node: &CompiledNode,
        index: usize,
        mut outputs: Inputs,
        skipped: bool,
        state: &mut RunState,
        values: &mut BTreeMap<sapho_core::NodeId, Inputs>,
    ) -> Result<()> {
        let inputs = state.trace_entry(index)?.inputs.clone();
        let processed = (|| {
            state.check_time()?;
            check_ports(
                if skipped {
                    node.output_types()
                } else {
                    &node.signature().outputs
                },
                &outputs,
            )?;
            for datum in outputs.values_mut() {
                datum.inherit_sources(inputs.values().flat_map(|d| d.sources.clone()));
                if let Some(guard) = &state.trace_entry(index)?.guard {
                    datum.inherit_sources(guard.sources.clone());
                }
                if !skipped && node.spec().guard.is_some() {
                    datum.value = Value::Optional(Some(Box::new(datum.value.clone())));
                }
            }
            check_ports(node.output_types(), &outputs)?;
            state.data(&outputs)?;
            Ok(())
        })();
        if let Err(e) = processed {
            state.failed(index, &e);
            return Err(e);
        }
        let entry = state.trace_entry(index)?;
        entry.status = if skipped {
            NodeStatus::Skipped
        } else {
            NodeStatus::Completed
        };
        entry.outputs = outputs.clone();
        values.insert(node.spec().id.clone(), outputs);
        Ok(())
    }
}
fn validate_backends(graph: &CompiledGraph, backends: &BackendRegistry) -> Result<()> {
    for node in graph.stages().iter().chain(graph.shadow_stages()).flatten() {
        if let Operation::Ask { backend } | Operation::ShadowAsk { backend, .. } =
            &node.spec().operation
        {
            backends.get(backend)?;
        }
        if let Some(g) = node.mapped_graph() {
            validate_backends(&g, backends)?;
        }
    }
    Ok(())
}
fn shadow_observation(
    node: &CompiledNode,
    observation: &ShadowObservation,
    trace: &NodeTrace,
    values: &BTreeMap<NodeId, Inputs>,
) -> ShadowTrace {
    let projected = if trace.status == NodeStatus::Completed {
        (|| {
            let answers = values
                .get(&node.spec().id)
                .and_then(|ports| ports.get("answers"))
                .ok_or_else(|| {
                    SaphoError::new(ErrorCode::MissingAnswer, "Shadow answers absent")
                })?;
            let answers = match &answers.value {
                Value::Answers(answers) => answers,
                Value::Optional(Some(inner)) => match inner.as_ref() {
                    Value::Answers(answers) => answers,
                    _ => {
                        return Err(SaphoError::new(
                            ErrorCode::TypeMismatch,
                            "Shadow answers type mismatch",
                        ));
                    }
                },
                _ => {
                    return Err(SaphoError::new(
                        ErrorCode::TypeMismatch,
                        "Shadow answers type mismatch",
                    ));
                }
            };
            let probability = answers.probability(&observation.question, &observation.labels)?;
            let value = match observation.projection {
                ShadowProjection::Boolean { threshold } => {
                    Value::Boolean(probability.get() >= threshold.get())
                }
                ShadowProjection::Probability => Value::Probability(probability),
            };
            let mut path = trace.path.clone();
            path.push(observation.name.clone());
            make_datum(&path, "value", value, &trace.inputs)
        })()
    } else {
        Err(trace
            .error
            .clone()
            .unwrap_or_else(|| SaphoError::new(ErrorCode::MissingInput, "Shadow ask skipped")))
    };
    let (status, value, error) = match projected {
        Ok(value) => (NodeStatus::Completed, Some(value), None),
        Err(error) => (
            if trace.status == NodeStatus::Skipped {
                NodeStatus::Skipped
            } else {
                NodeStatus::Failed
            },
            None,
            Some(error),
        ),
    };
    ShadowTrace {
        node: node.spec().id.clone(),
        name: observation.name.clone(),
        output: observation.output.clone(),
        status,
        value,
        error,
        model: trace.model.clone(),
    }
}
fn resolve(
    binding: &Binding,
    inputs: &Inputs,
    nodes: &BTreeMap<sapho_core::NodeId, Inputs>,
) -> Result<Datum> {
    let (mut d, path) = match binding {
        Binding::Input { name, path } => (
            inputs
                .get(name)
                .cloned()
                .ok_or_else(|| SaphoError::new(ErrorCode::MissingInput, "Graph input absent"))?,
            path,
        ),
        Binding::Node { node, port, path } => (
            nodes
                .get(node)
                .and_then(|ports| ports.get(port))
                .cloned()
                .ok_or_else(|| {
                    SaphoError::new(ErrorCode::UnknownReference, "Producer output absent")
                })?,
            path,
        ),
        Binding::Literal { value, .. } => return Ok(value.clone()),
    };
    for field in path {
        let Value::Record(mut values) = d.value else {
            return Err(SaphoError::new(
                ErrorCode::TypeMismatch,
                "Projection requires Record",
            ));
        };
        d.value = values.remove(field).ok_or_else(|| {
            SaphoError::new(ErrorCode::UnknownReference, "Projection field absent")
        })?;
    }
    Ok(d)
}
fn make_datum(path: &[String], port: &str, value: Value, inputs: &Inputs) -> Result<Datum> {
    let mut id = path.to_vec();
    id.push(port.into());
    let id = serde_json::to_string(&id)
        .map_err(|e| SaphoError::new(ErrorCode::InvalidValue, e.to_string()))?;
    let mut d = Datum::new(id, value)?;
    d.inherit_sources(inputs.values().flat_map(|d| d.sources.clone()));
    Ok(d)
}
fn one_output(path: &[String], value: Value, inputs: &Inputs) -> Result<Inputs> {
    Ok(Inputs::from([(
        "result".into(),
        make_datum(path, "result", value, inputs)?,
    )]))
}
struct RunState {
    limits: RunLimits,
    deadline: Instant,
    nodes: usize,
    items: usize,
    requests: usize,
    bytes: usize,
    trace: Trace,
    cancelled: Arc<AtomicBool>,
}
impl Drop for RunState {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}
impl RunState {
    fn check_time(&self) -> Result<()> {
        if Instant::now() >= self.deadline {
            Err(SaphoError::new(
                ErrorCode::DeadlineExceeded,
                "Run deadline exceeded",
            ))
        } else {
            Ok(())
        }
    }
    fn add_node(&mut self) -> Result<()> {
        self.nodes = increment(self.nodes, 1, self.limits.node_instances, "node instances")?;
        Ok(())
    }
    fn add_items(&mut self, n: usize) -> Result<()> {
        self.items = increment(
            self.items,
            n,
            self.limits.collection_items,
            "collection items",
        )?;
        Ok(())
    }
    fn add_request(&mut self) -> Result<()> {
        self.requests = increment(
            self.requests,
            1,
            self.limits.model_requests,
            "model requests",
        )?;
        Ok(())
    }
    fn remaining_items(&self) -> usize {
        self.limits.collection_items.saturating_sub(self.items)
    }
    fn data<T: serde::Serialize>(&mut self, data: &T) -> Result<()> {
        let n = measured_json_bytes(data, self.limits.data_bytes.saturating_sub(self.bytes))?;
        self.bytes = increment(self.bytes, n, self.limits.data_bytes, "data bytes")?;
        Ok(())
    }
    fn trace_entry(&mut self, index: usize) -> Result<&mut NodeTrace> {
        self.trace
            .nodes
            .get_mut(index)
            .ok_or_else(|| SaphoError::new(ErrorCode::CodeFailed, "Trace slot absent"))
    }
    fn failed(&mut self, index: usize, error: &SaphoError) {
        if let Some(e) = self.trace.nodes.get_mut(index) {
            e.status = NodeStatus::Failed;
            e.error = Some(error.clone());
            if let Some(evidence) = &mut e.model {
                evidence.raw = error.raw.as_deref().cloned();
            }
        }
    }
}
fn increment(value: usize, n: usize, max: usize, name: &str) -> Result<usize> {
    value.checked_add(n).filter(|v| *v <= max).ok_or_else(|| {
        SaphoError::new(ErrorCode::LimitExceeded, "Run work ceiling exceeded")
            .with_context("limit", name)
    })
}
