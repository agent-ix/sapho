// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Cooperative bounded orchestration, including dependent map subgraphs.
use crate::observation::{self, ModelCallObservation, ObservationClock, ObservationConfig};
use crate::{ModelEvidence, NodeStatus, NodeTrace, SampleEvidence, ShadowTrace, Trace, operators};
use futures::{
    future::BoxFuture,
    stream::{FuturesUnordered, StreamExt},
};
use sapho_core::{
    Answer, Answers, BackendRegistry, Datum, Degree, ErrorCode, Inputs, ModelRequest, NodeId,
    PrimitiveContext, Question, Result, SaphoError, Value, check_ports, measured_json_bytes,
    validate_response,
};
use sapho_graph::{
    Binding, CompiledGraph, CompiledNode, Operation, ShadowObservation, ShadowProjection,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::Semaphore;

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
    calibration_identity_check: bool,
}
impl Engine {
    /// Bind model implementations, rejecting unknown names before work starts.
    pub fn new(graph: CompiledGraph, backends: BackendRegistry) -> Result<Self> {
        validate_backends(&graph, &backends)?;
        Ok(Self {
            graph: Arc::new(graph),
            backends,
            calibration_identity_check: false,
        })
    }
    /// Enforce reported binding/model identity at every executed calibration node.
    pub fn with_calibration_identity_check(mut self) -> Self {
        self.calibration_identity_check = true;
        self
    }
    /// Execute with bounded work; no trace persistence or model retry is implicit.
    pub async fn run(
        &self,
        inputs: &Inputs,
        limits: RunLimits,
    ) -> std::result::Result<RunResult, RunFailure> {
        self.run_internal(inputs, limits, None).await.0
    }
    /// Execute with a separate model-call observation sidecar.
    pub async fn run_observed(
        &self,
        inputs: &Inputs,
        limits: RunLimits,
        config: &ObservationConfig,
    ) -> Result<(
        std::result::Result<RunResult, RunFailure>,
        Vec<ModelCallObservation>,
    )> {
        config.validate()?;
        Ok(self.run_internal(inputs, limits, Some(config)).await)
    }
    async fn run_internal(
        &self,
        inputs: &Inputs,
        limits: RunLimits,
        config: Option<&ObservationConfig>,
    ) -> (
        std::result::Result<RunResult, RunFailure>,
        Vec<ModelCallObservation>,
    ) {
        let fail = |error| RunFailure {
            error,
            trace: Trace::default(),
        };
        if let Err(error) = limits
            .validate()
            .and_then(|()| check_ports(&self.graph.signature().inputs, inputs))
        {
            return (Err(fail(error)), Vec::new());
        }
        let Some(deadline) = Instant::now().checked_add(limits.duration) else {
            return (
                Err(fail(SaphoError::new(
                    ErrorCode::InvalidValue,
                    "Duration exceeds monotonic clock range",
                ))),
                Vec::new(),
            );
        };
        let model_permits = Arc::new(Semaphore::new(limits.concurrency));
        let mut state = RunState {
            limits,
            deadline,
            nodes: 0,
            items: 0,
            requests: 0,
            reserved_requests: 0,
            bytes: 0,
            trace: Trace::default(),
            cancelled: Arc::new(AtomicBool::new(false)),
            clock: config.map(|c| c.clock.clone()),
            durations: BTreeMap::new(),
            sample_durations: BTreeMap::new(),
            started: Arc::new(Mutex::new(BTreeMap::new())),
            model_permits,
        };
        if let Err(error) = state.data(inputs) {
            return (
                Err(RunFailure {
                    error,
                    trace: state.trace.clone(),
                }),
                Vec::new(),
            );
        }
        let result = match self
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
        };
        let observations = config.map_or_else(Vec::new, |c| {
            let started = state
                .started
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            let attempted = started.keys().copied().collect::<BTreeSet<_>>();
            if c.mode == observation::ObservationMode::Live {
                for (index, at) in started.iter() {
                    state.durations.entry(*index).or_insert_with(|| {
                        u64::try_from(c.clock.now().saturating_duration_since(*at).as_micros())
                            .unwrap_or(u64::MAX)
                    });
                }
            }
            observation::collect(
                &state.trace,
                &state.durations,
                &state.sample_durations,
                &attempted,
                c,
            )
        });
        (result, observations)
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
                        samples: None,
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
                    if let Operation::Ask { backend, samples } = &node.spec().operation {
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
                                sample_index: None,
                                distribution_policy: binding.distribution_policy,
                                backend: backend.clone(),
                                model: binding.model,
                                expected_model: binding.expected_model,
                                state: state_value,
                                questions: questions.clone(),
                            };
                            request.validate()?;
                            let count = usize::try_from(*samples).map_err(|_| {
                                SaphoError::new(
                                    ErrorCode::LimitExceeded,
                                    "Ask sample count exceeds platform range",
                                )
                            })?;
                            state.reserve_requests(count)?;
                            if count == 1 {
                                state.trace_entry(index)?.model = Some(ModelEvidence {
                                    request: request.clone(),
                                    response: None,
                                    raw: None,
                                });
                            } else {
                                state.trace_entry(index)?.samples = Some(Vec::new());
                            }
                            Ok((binding.backend, request, count))
                        })();
                        match prepared {
                            Ok((backend, request, count)) => {
                                asks.push((node, index, np, ins, backend, request, count))
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
                let mut cursor = 0usize;
                while cursor < asks.len() {
                    if asks[cursor].6 == 1 {
                        let end = asks
                            .iter()
                            .enumerate()
                            .skip(cursor)
                            .take(state.limits.concurrency)
                            .take_while(|(_, ask)| ask.6 == 1)
                            .last()
                            .map_or(cursor + 1, |(i, _)| i + 1);
                        let mut futures = FuturesUnordered::new();
                        let deadline = tokio::time::Instant::from_std(state.deadline);
                        let clock = state.clock.clone();
                        let started_calls = state.started.clone();
                        for (order, (_, index, _, _, backend, request, _)) in
                            asks.iter().enumerate().take(end).skip(cursor)
                        {
                            state.data(request)?;
                            let permit =
                                state
                                    .model_permits
                                    .clone()
                                    .try_acquire_owned()
                                    .map_err(|_| {
                                        SaphoError::new(
                                            ErrorCode::CodeFailed,
                                            "Model permit unavailable",
                                        )
                                    })?;
                            state.dispatch_request()?;
                            let backend = backend.clone();
                            let request = request.clone();
                            let index = *index;
                            let clock = clock.clone();
                            let started_calls = started_calls.clone();
                            futures.push(async move {
                                let started = clock.as_ref().map(|clock| clock.now());
                                if let Some(at) = started {
                                    started_calls
                                        .lock()
                                        .unwrap_or_else(|poison| poison.into_inner())
                                        .insert(index, at);
                                }
                                let response = tokio::time::timeout_at(deadline, async {
                                    let _permit = permit;
                                    backend.infer(&request).await
                                })
                                .await
                                .map_err(|_| {
                                    SaphoError::new(
                                        ErrorCode::DeadlineExceeded,
                                        "Model deadline exceeded",
                                    )
                                })
                                .and_then(|result| result);
                                let elapsed = started.and_then(|start| {
                                    clock
                                        .as_ref()
                                        .map(|clock| clock.now().saturating_duration_since(start))
                                });
                                (order, response, elapsed)
                            });
                        }
                        while let Some((order, result, elapsed)) = futures.next().await {
                            let (node, index, np, ins, _, request, _) = &asks[order];
                            if let Some(elapsed) = elapsed {
                                state.durations.insert(
                                    *index,
                                    u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX),
                                );
                            }
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
                                Err(error) => {
                                    state.failed(*index, &error);
                                    return Err(error);
                                }
                            }
                        }
                        cursor = end;
                    } else {
                        let (node, index, np, ins, backend, request, count) = &asks[cursor];
                        let result = self
                            .repeated_ask(
                                np,
                                ins,
                                backend.clone(),
                                request.clone(),
                                *count,
                                *index,
                                state,
                            )
                            .await;
                        match result {
                            Ok(outputs) => {
                                self.finish(node, *index, outputs, false, state, &mut values)?
                            }
                            Err(error) => {
                                state.failed(*index, &error);
                                return Err(error);
                            }
                        }
                        cursor += 1;
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
                    samples: None,
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
                                sample_index: None,
                                distribution_policy: binding.distribution_policy,
                                backend: backend.clone(),
                                model: binding.model,
                                expected_model: binding.expected_model,
                                state: state_value,
                                questions: questions.clone(),
                            };
                            request.validate()?;
                            state.data(&request)?;
                            state.reserve_requests(1)?;
                            let permit =
                                state
                                    .model_permits
                                    .clone()
                                    .try_acquire_owned()
                                    .map_err(|_| {
                                        SaphoError::new(
                                            ErrorCode::CodeFailed,
                                            "Model permit unavailable",
                                        )
                                    })?;
                            state.dispatch_request()?;
                            state.trace_entry(index)?.model = Some(ModelEvidence {
                                request: request.clone(),
                                response: None,
                                raw: None,
                            });
                            let started = state.clock.as_ref().map(|clock| clock.now());
                            if let Some(at) = started {
                                state
                                    .started
                                    .lock()
                                    .unwrap_or_else(|poison| poison.into_inner())
                                    .insert(index, at);
                            }
                            let response = tokio::time::timeout_at(
                                tokio::time::Instant::from_std(state.deadline),
                                async {
                                    let _permit = permit;
                                    binding.backend.infer(&request).await
                                },
                            )
                            .await
                            .map_err(|_| {
                                SaphoError::new(
                                    ErrorCode::DeadlineExceeded,
                                    "Shadow model deadline exceeded",
                                )
                            })
                            .and_then(|response| response);
                            if let (Some(started), Some(clock)) = (started, state.clock.as_ref()) {
                                let elapsed = clock.now().saturating_duration_since(started);
                                state.durations.insert(
                                    index,
                                    u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX),
                                );
                            }
                            let response = response?;
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
    async fn repeated_ask(
        &self,
        path: &[String],
        ins: &Inputs,
        backend: Arc<dyn sapho_core::ModelBackend>,
        request: ModelRequest,
        count: usize,
        trace_index: usize,
        state: &mut RunState,
    ) -> Result<Inputs> {
        let mut dispatched = 0usize;
        let result = self
            .repeated_ask_inner(
                path,
                ins,
                backend,
                request,
                count,
                trace_index,
                state,
                &mut dispatched,
            )
            .await;
        state.release_requests(count.saturating_sub(dispatched));
        result
    }
    #[allow(clippy::too_many_arguments)]
    async fn repeated_ask_inner(
        &self,
        path: &[String],
        ins: &Inputs,
        backend: Arc<dyn sapho_core::ModelBackend>,
        base: ModelRequest,
        count: usize,
        trace_index: usize,
        state: &mut RunState,
        dispatched: &mut usize,
    ) -> Result<Inputs> {
        let deadline = tokio::time::Instant::from_std(state.deadline);
        let mut answer_items = Vec::with_capacity(count.min(state.limits.model_requests));
        let mut model_items = Vec::with_capacity(count.min(state.limits.model_requests));
        for start in (0..count).step_by(state.limits.concurrency) {
            let end = start.saturating_add(state.limits.concurrency).min(count);
            let mut prepared = Vec::with_capacity(end - start);
            let mut request_bytes = 0usize;
            for sample in start..end {
                let sample_index = u32::try_from(sample).map_err(|_| {
                    SaphoError::new(ErrorCode::LimitExceeded, "Sample index exceeds u32")
                })?;
                let mut request = base.clone();
                request.sample_index = Some(sample_index);
                let permit =
                    tokio::time::timeout_at(deadline, state.model_permits.clone().acquire_owned())
                        .await
                        .map_err(|_| {
                            SaphoError::new(
                                ErrorCode::DeadlineExceeded,
                                "Model permit deadline exceeded",
                            )
                        })?
                        .map_err(|_| {
                            SaphoError::new(ErrorCode::CodeFailed, "Model permit pool closed")
                        })?;
                let bytes = measured_json_bytes(
                    &request,
                    state
                        .limits
                        .data_bytes
                        .saturating_sub(state.bytes)
                        .saturating_sub(request_bytes),
                )?;
                request_bytes = request_bytes.checked_add(bytes).ok_or_else(|| {
                    SaphoError::new(ErrorCode::LimitExceeded, "Request bytes overflow")
                })?;
                prepared.push((sample_index, request, permit));
            }
            state.bytes = increment(
                state.bytes,
                request_bytes,
                state.limits.data_bytes,
                "data bytes",
            )?;
            let mut futures = FuturesUnordered::new();
            let clock = state.clock.clone();
            for (sample_index, request, permit) in prepared {
                state.dispatch_request()?;
                *dispatched += 1;
                let backend = backend.clone();
                let clock = clock.clone();
                futures.push(async move {
                    let started = clock.as_ref().map(|clock| clock.now());
                    let outcome = tokio::time::timeout_at(deadline, async {
                        let _permit = permit;
                        backend.infer(&request).await
                    })
                    .await
                    .map_err(|_| {
                        SaphoError::new(ErrorCode::DeadlineExceeded, "Model deadline exceeded")
                    })
                    .and_then(|result| result);
                    let elapsed = started.and_then(|start| {
                        clock
                            .as_ref()
                            .map(|clock| clock.now().saturating_duration_since(start))
                    });
                    (sample_index, request, outcome, elapsed)
                });
            }
            let mut completed = BTreeMap::new();
            while let Some((sample_index, request, outcome, elapsed)) = futures.next().await {
                if let Some(elapsed) = elapsed {
                    state.sample_durations.insert(
                        (trace_index, sample_index),
                        u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX),
                    );
                }
                completed.insert(sample_index, (request, outcome));
            }
            let mut first_error = None;
            for (sample_index, (request, outcome)) in completed {
                let mut evidence = ModelEvidence {
                    request: request.clone(),
                    response: None,
                    raw: None,
                };
                let processed: Result<(Datum, Datum)> = (|| {
                    let response = outcome?;
                    evidence.response = Some(response.clone());
                    state.data(&response)?;
                    let answers = validate_response(&request, &response)?;
                    let item_id = sample_index.to_string();
                    let mut answer = Datum::new(item_id.clone(), Value::Answers(answers))?;
                    let mut model = Datum::new(item_id, Value::Text(response.model))?;
                    let sources = ins
                        .values()
                        .flat_map(|datum| datum.sources.clone())
                        .collect::<Vec<_>>();
                    answer.inherit_sources(sources.clone());
                    model.inherit_sources(sources);
                    Ok((answer, model))
                })();
                match processed {
                    Ok((answer, model)) => {
                        answer_items.push(answer);
                        model_items.push(model);
                        state
                            .trace_entry(trace_index)?
                            .samples
                            .as_mut()
                            .ok_or_else(|| {
                                SaphoError::new(
                                    ErrorCode::CodeFailed,
                                    "Sample evidence slot absent",
                                )
                            })?
                            .push(SampleEvidence {
                                index: sample_index,
                                status: NodeStatus::Completed,
                                model: evidence,
                                error: None,
                            });
                    }
                    Err(error) => {
                        evidence.raw = error.raw.as_deref().cloned();
                        if first_error.is_none() {
                            first_error = Some(error.clone());
                        }
                        state
                            .trace_entry(trace_index)?
                            .samples
                            .as_mut()
                            .ok_or_else(|| {
                                SaphoError::new(
                                    ErrorCode::CodeFailed,
                                    "Sample evidence slot absent",
                                )
                            })?
                            .push(SampleEvidence {
                                index: sample_index,
                                status: NodeStatus::Failed,
                                model: evidence,
                                error: Some(error),
                            });
                    }
                }
            }
            if let Some(error) = first_error {
                return Err(error);
            }
        }
        state.add_items(count.checked_mul(2).ok_or_else(|| {
            SaphoError::new(ErrorCode::LimitExceeded, "Sample item count overflow")
        })?)?;
        let disagreement = disagreement(&answer_items)?;
        Ok(Inputs::from([
            (
                "answers".into(),
                make_datum(path, "answers", Value::List(answer_items), ins)?,
            ),
            (
                "models".into(),
                make_datum(path, "models", Value::List(model_items), ins)?,
            ),
            (
                "disagreement".into(),
                make_datum(path, "disagreement", Value::Degree(disagreement), ins)?,
            ),
        ]))
    }
    async fn non_model(
        &self,
        node: &CompiledNode,
        path: &[String],
        ins: &Inputs,
        state: &mut RunState,
    ) -> Result<Inputs> {
        if self.calibration_identity_check && matches!(node.spec().operation, Operation::Calibrate)
        {
            check_calibration_identity(path, node, ins, &self.graph, &state.trace)?;
        }
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
            | Operation::Calibrate
            | Operation::CalibratedAsProbability
            | Operation::Degree
            | Operation::Reduce { .. }
            | Operation::Complement
            | Operation::Coalesce => {
                one_output(path, operators::logic(&node.spec().operation, ins)?, ins)
            }
            Operation::MergePresent {} => {
                let node_path = serde_json::to_string(path)
                    .map_err(|error| SaphoError::new(ErrorCode::CodeFailed, error.to_string()))?;
                one_output(
                    path,
                    operators::merge_present(ins)
                        .map_err(|error| error.with_context("node_path", node_path))?,
                    ins,
                )
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
        if let Operation::Ask { backend, .. } | Operation::ShadowAsk { backend, .. } =
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
    reserved_requests: usize,
    bytes: usize,
    trace: Trace,
    cancelled: Arc<AtomicBool>,
    clock: Option<Arc<dyn ObservationClock>>,
    durations: BTreeMap<usize, u64>,
    sample_durations: BTreeMap<(usize, u32), u64>,
    started: Arc<Mutex<BTreeMap<usize, Instant>>>,
    model_permits: Arc<Semaphore>,
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
    fn reserve_requests(&mut self, n: usize) -> Result<()> {
        let occupied = self
            .requests
            .checked_add(self.reserved_requests)
            .ok_or_else(|| {
                SaphoError::new(ErrorCode::LimitExceeded, "Model request ledger overflow")
            })?;
        increment(occupied, n, self.limits.model_requests, "model requests")?;
        self.reserved_requests += n;
        Ok(())
    }
    fn dispatch_request(&mut self) -> Result<()> {
        self.reserved_requests = self.reserved_requests.checked_sub(1).ok_or_else(|| {
            SaphoError::new(ErrorCode::CodeFailed, "Model request reservation absent")
        })?;
        self.requests += 1;
        Ok(())
    }
    fn release_requests(&mut self, n: usize) {
        self.reserved_requests = self.reserved_requests.saturating_sub(n);
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

/// Modal categorical disagreement across complete ordered answer blocks.
fn disagreement(items: &[Datum]) -> Result<Degree> {
    let mut counts: BTreeMap<Vec<(String, u8, String)>, usize> = BTreeMap::new();
    for datum in items {
        let Value::Answers(Answers {
            questions, values, ..
        }) = &datum.value
        else {
            return Err(SaphoError::new(
                ErrorCode::TypeMismatch,
                "Sample needs Answers",
            ));
        };
        let mut signature = Vec::with_capacity(questions.len());
        for named in questions {
            let answer = values
                .get(&named.id)
                .ok_or_else(|| SaphoError::new(ErrorCode::MissingAnswer, "Sample answer absent"))?;
            let (kind, category) = match (&named.question, answer) {
                (Question::Boolean { .. }, Answer::Boolean { probability }) => {
                    (0, (probability.get() >= 0.5).to_string())
                }
                (Question::Choice { .. }, Answer::Choice { selected, .. }) => (1, selected.clone()),
                (Question::Score { .. }, Answer::Score { expected, .. }) => {
                    let lower = expected.floor();
                    let nearest = if expected - lower > 0.5 {
                        lower + 1.0
                    } else {
                        lower
                    };
                    (2, format!("{nearest:.0}"))
                }
                _ => {
                    return Err(SaphoError::new(
                        ErrorCode::TypeMismatch,
                        "Sample answer kind differs",
                    ));
                }
            };
            signature.push((named.id.clone(), kind, category));
        }
        *counts.entry(signature).or_default() += 1;
    }
    let modal =
        counts.values().copied().max().ok_or_else(|| {
            SaphoError::new(ErrorCode::InvalidValue, "No samples for disagreement")
        })?;
    Degree::new((items.len() - modal) as f64 / items.len() as f64)
}

#[cfg(test)]
mod sampling_tests {
    use super::*;
    use sapho_core::{ChoiceOption, DistributionPolicy, NamedQuestion, Probability};

    fn block(boolean: f64, choice: &str, score: f64, confidence: f64) -> Datum {
        let questions = vec![
            NamedQuestion {
                id: "boolean".into(),
                question: Question::Boolean {
                    instructions: "True?".into(),
                    yes: "Yes".into(),
                    no: "No".into(),
                },
            },
            NamedQuestion {
                id: "choice".into(),
                question: Question::Choice {
                    instructions: "Which?".into(),
                    options: vec![
                        ChoiceOption {
                            label: "a".into(),
                            description: "A".into(),
                        },
                        ChoiceOption {
                            label: "b".into(),
                            description: "B".into(),
                        },
                    ],
                },
            },
            NamedQuestion {
                id: "score".into(),
                question: Question::Score {
                    instructions: "Level?".into(),
                    levels: vec!["Low".into(), "Mid".into(), "High".into()],
                },
            },
        ];
        let values = BTreeMap::from([
            (
                "boolean".into(),
                Answer::Boolean {
                    probability: Probability::new(boolean).unwrap(),
                },
            ),
            (
                "choice".into(),
                Answer::Choice {
                    selected: choice.into(),
                    confidence: Probability::new(confidence).unwrap(),
                    probabilities: None,
                },
            ),
            (
                "score".into(),
                Answer::Score {
                    expected: score,
                    confidence: Probability::new(confidence).unwrap(),
                    probabilities: None,
                },
            ),
        ]);
        Datum::new(
            "sample",
            Value::Answers(Answers {
                distribution_policy: DistributionPolicy::Strict {},
                questions,
                values,
            }),
        )
        .unwrap()
    }

    /// Trace: FR-076-AC-1, FR-076-AC-2, IT-014-SC-04
    #[test]
    fn complete_answer_signatures_use_threshold_selected_label_and_lower_score_tie() {
        let a = block(0.5, "a", 1.5, 0.2);
        let same = block(0.9, "a", 1.1, 0.9);
        let b = block(0.49, "a", 1.1, 0.2);
        let c = block(0.5, "b", 1.1, 0.2);
        let d = block(0.5, "a", 1.51, 0.2);
        assert_eq!(
            disagreement(&[a.clone(), same.clone(), same.clone()])
                .unwrap()
                .get(),
            0.0
        );
        assert_eq!(
            disagreement(&[a.clone(), b.clone(), same.clone()])
                .unwrap()
                .get(),
            1.0 / 3.0
        );
        assert_eq!(disagreement(&[a.clone(), b, c]).unwrap().get(), 2.0 / 3.0);
        assert_eq!(disagreement(&[a, same, d]).unwrap().get(), 1.0 / 3.0);
    }
}

fn compiled_at_path(graph: &Arc<CompiledGraph>, path: &[String]) -> Option<CompiledNode> {
    if path.first().is_none_or(|part| part != "root") || path.len() < 2 {
        return None;
    }
    let mut scope = graph.clone();
    let mut index = 1;
    while index + 2 < path.len() {
        let map = scope
            .stages()
            .iter()
            .flatten()
            .find(|node| node.spec().id.as_str() == path[index])?;
        scope = map.mapped_graph()?;
        index += 2;
    }
    scope
        .stages()
        .iter()
        .flatten()
        .find(|node| node.spec().id.as_str() == path[index])
        .cloned()
}
fn calibration_sources(
    graph: &Arc<CompiledGraph>,
    trace: &BTreeMap<Vec<String>, &NodeTrace>,
    owner: &[String],
    binding: &Binding,
    visited: &mut BTreeSet<Vec<String>>,
    contributing: &mut Vec<(sapho_core::BackendId, Option<String>)>,
) {
    match binding {
        Binding::Literal { .. } => {}
        Binding::Input { name, .. } => {
            let scope = &owner[..owner.len() - 1];
            if scope.len() < 3 {
                return;
            }
            let map_path = &scope[..scope.len() - 1];
            let Some(map) = compiled_at_path(graph, map_path) else {
                return;
            };
            if name == "item" {
                let Some(item_id) = scope.last() else {
                    return;
                };
                let Some(Value::List(items)) = trace
                    .get(map_path)
                    .and_then(|node| node.inputs.get("items"))
                    .map(|datum| &datum.value)
                else {
                    return;
                };
                let Some(position) = items.iter().position(|datum| datum.id.as_str() == item_id)
                else {
                    return;
                };
                let Some(Binding::Node {
                    node: list_id,
                    path: projection,
                    ..
                }) = map.spec().inputs.get("items")
                else {
                    return;
                };
                if !projection.is_empty() {
                    return;
                }
                let mut list_path = map_path[..map_path.len() - 1].to_vec();
                list_path.push(list_id.to_string());
                let Some(list) = compiled_at_path(graph, &list_path) else {
                    return;
                };
                let Operation::List { order, .. } = &list.spec().operation else {
                    return;
                };
                let Some(selected) = order
                    .get(position)
                    .and_then(|port| list.spec().inputs.get(port))
                else {
                    return;
                };
                calibration_sources(graph, trace, &list_path, selected, visited, contributing);
                return;
            }
            if let Some(parent_binding) = map.spec().inputs.get(name) {
                calibration_sources(
                    graph,
                    trace,
                    map_path,
                    parent_binding,
                    visited,
                    contributing,
                );
            }
        }
        Binding::Node { node, .. } => {
            let mut source = owner[..owner.len() - 1].to_vec();
            source.push(node.to_string());
            if !visited.insert(source.clone()) {
                return;
            }
            let (Some(compiled), Some(evidence)) =
                (compiled_at_path(graph, &source), trace.get(&source))
            else {
                return;
            };
            if evidence.status != NodeStatus::Completed {
                return;
            }
            if let Operation::Ask { backend } = &compiled.spec().operation {
                contributing.push((
                    backend.clone(),
                    evidence
                        .model
                        .as_ref()
                        .and_then(|model| model.response.as_ref())
                        .map(|response| response.model.clone()),
                ));
            } else {
                for input in compiled.spec().inputs.values() {
                    calibration_sources(graph, trace, &source, input, visited, contributing);
                }
            }
        }
    }
}
fn check_calibration_identity(
    path: &[String],
    node: &CompiledNode,
    ins: &Inputs,
    graph: &Arc<CompiledGraph>,
    trace: &Trace,
) -> Result<()> {
    let Value::CalibrationMap(map) = &ins
        .get("map")
        .ok_or_else(|| SaphoError::new(ErrorCode::MissingInput, "Calibration map absent"))?
        .value
    else {
        return Err(SaphoError::new(
            ErrorCode::TypeMismatch,
            "Calibration map input differs",
        ));
    };
    map.validate()?;
    let mismatch = || {
        SaphoError::new(
            ErrorCode::ModelMismatch,
            "Calibration source differs from fitted model",
        )
        .with_context("map_id", &map.map_id)
        .with_context("node", path.join("/"))
        .with_context("expected_binding", map.fit_binding.as_str())
        .with_context("expected_model", &map.fit_actual_model)
    };
    let by_path = trace
        .nodes
        .iter()
        .map(|node| (node.path.clone(), node))
        .collect::<BTreeMap<_, _>>();
    let mut visited = BTreeSet::new();
    let mut contributing = Vec::new();
    let value_binding = node.spec().inputs.get("value").ok_or_else(mismatch)?;
    calibration_sources(
        graph,
        &by_path,
        path,
        value_binding,
        &mut visited,
        &mut contributing,
    );
    if contributing.len() != 1 {
        return Err(mismatch().with_context("observed_count", contributing.len().to_string()));
    }
    let (binding, model) = &contributing[0];
    if binding != &map.fit_binding || model.as_deref() != Some(map.fit_actual_model.as_str()) {
        return Err(mismatch()
            .with_context("observed_binding", binding.as_str())
            .with_context("observed_model", model.as_deref().unwrap_or("<absent>")));
    }
    Ok(())
}
