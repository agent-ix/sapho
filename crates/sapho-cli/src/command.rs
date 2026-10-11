// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Process orchestration: all acquisition/persistence surrounds, never enters, async work.
use crate::args::*;
use sapho_cli::{
    ArtifactWriter, Bindings, CliError, ExitStatus, GraphArtifact, MAX_SERVICE_CONFIG_BYTES,
    Provider, RunReport, Runner, ServiceConfig, inspect, live_bindings_with_services, load_graph,
    plain_inputs, read_bytes, read_bytes_with_timeout, recording_bindings, replay_bindings_json,
    select_format, sweep,
};
use sapho_core::{
    BackendId, BackendRegistry, CalibrationMap, Datum, ErrorCode, Inputs, ItemId, ModelIdentity,
    NodeId, PrimitiveRegistry, ProviderDescriptor, Question, SaphoError, SourceId, Value,
    ValueType, bounded_json, check_ports, decode_json, measured_json_bytes,
};
use sapho_evidence::{
    Candidate, CaseOutcome, Dataset, EvidenceError, FitAttribution, GateRegistration, GateReport,
    GateVerdict, Measurement, Metric as ScoreMetric, RankedCandidate, Roster, RosterCall,
    RosterCase, RosterContributor, RosterMapping, RosterMode, RosterQuestionKind, RosterSelection,
    ShadowComparison, ShadowComparisonRequest, ShadowMetric, Split, calibration_cases_digest,
    compare_shadow, evaluate_gate, export_training, fit_calibration, measure, measure_for_gate,
    project_roster_literal, rank, register_gate, roster, verify_gate,
};
use sapho_graph::{
    Binding, GraphFormat, GraphSpec, Operation, graph_semantic_identity, parse_config,
};
use sapho_recording::{Recording, RecordingBackend, ReplayBackend};
use sapho_runtime::{
    ModelCallObservation, NodeStatus, ObservationConfig, ObservationMode, SystemObservationClock,
    Trace,
};
use serde::Serialize;
use serde_json::value::RawValue;
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::Path,
    sync::Arc,
};

/// A finished JSON document that can be streamed to a sink any number of times.
trait Document {
    fn write(&self, sink: &mut dyn std::io::Write) -> std::io::Result<()>;
}
impl<T: Serialize> Document for T {
    fn write(&self, sink: &mut dyn std::io::Write) -> std::io::Result<()> {
        serde_json::to_writer(sink, self).map_err(std::io::Error::from)
    }
}
/// The command result. The document is streamed to stdout rather than held as bytes, so a
/// report of a large Dataset exists once in memory.
pub(crate) struct Response {
    document: Box<dyn Document>,
    pub(crate) exit: ExitStatus,
}
impl Response {
    /// Write the document and its final newline to `sink`.
    pub(crate) fn write_to(&self, sink: &mut dyn std::io::Write) -> std::io::Result<()> {
        let mut buffered = std::io::BufWriter::new(sink);
        self.document.write(&mut buffered)?;
        buffered.write_all(b"\n")?;
        buffered.flush()
    }
}
fn respond<T: Serialize + 'static>(
    report: T,
    exit: ExitStatus,
    max_bytes: usize,
    destination: Option<ArtifactWriter>,
) -> Result<Response, CliError> {
    if let Some(writer) = destination {
        writer.finish_json(&report, max_bytes)?;
    } else {
        measured_json_bytes(&report, max_bytes)?;
    }
    Ok(Response {
        document: Box::new(report),
        exit,
    })
}
fn claim(path: Option<&Path>) -> Result<Option<ArtifactWriter>, CliError> {
    path.map(ArtifactWriter::create).transpose()
}
fn runtime() -> Result<tokio::runtime::Runtime, CliError> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(CliError::Runtime)
}
fn bindings(path: Option<&Path>) -> Result<Bindings, CliError> {
    let Some(path) = path else {
        return Ok(Bindings::new());
    };
    let format = select_format(path, None)?;
    let bytes = read_bytes(path, 1_048_576)?;
    parse_config(
        std::str::from_utf8(&bytes)
            .map_err(|_| SaphoError::new(ErrorCode::Config, "Bindings must be UTF-8"))?,
        format,
    )
    .map_err(CliError::from)
}
fn live_registry(
    required: &[sapho_core::BackendId],
    metadata: &Bindings,
    explicit: Option<&Path>,
) -> Result<BackendRegistry, CliError> {
    let needs_services = required.iter().any(|id| {
        metadata
            .get(id)
            .is_some_and(|binding| binding.provider == Provider::Systemone)
    });
    let services = if needs_services {
        let environment = std::env::var_os("SAPHO_SERVICE_CONFIG");
        let path = explicit
            .map(std::path::PathBuf::from)
            .or_else(|| environment.map(std::path::PathBuf::from))
            .ok_or_else(|| {
                SaphoError::new(ErrorCode::Config, "Service configuration path absent")
            })?;
        ServiceConfig::from_json(&read_bytes(&path, MAX_SERVICE_CONFIG_BYTES)?)?
    } else {
        ServiceConfig::default()
    };
    live_bindings_with_services(
        required,
        metadata,
        &services,
        &ix_cli_kit::secrets::SecretStore::system(),
    )
}
fn dataset(path: &Path, max_bytes: usize, max_cases: usize) -> Result<Dataset, CliError> {
    let data: Dataset = decode_json(&read_bytes(path, max_bytes)?, max_bytes)?;
    data.validate(max_cases)?;
    Ok(data)
}
/// Bind every recorded backend to the exact replay of the recording at `path`. The recording
/// is read straight into the replay index and is not kept as a typed document.
fn replay_registry(
    path: &Path,
    metadata: &Bindings,
    max_bytes: usize,
) -> Result<BackendRegistry, CliError> {
    replay_bindings_json(&read_bytes(path, max_bytes)?, Some(metadata), max_bytes)
}
fn prepare<F: FnOnce() -> Result<BackendRegistry, CliError>>(
    graph: &GraphSpec,
    metadata: &Bindings,
    replay: Option<F>,
    service_config: Option<&Path>,
) -> Result<(Runner, Vec<sapho_core::BackendId>), CliError> {
    let primitives = PrimitiveRegistry::default();
    let inspection = inspect(graph, &primitives)?;
    let registry = if let Some(replay) = replay {
        replay()?
    } else {
        live_registry(&inspection.backends, metadata, service_config)?
    };
    Ok((
        Runner::new_stock(graph, &primitives, registry)?,
        inspection.backends,
    ))
}
fn inputs(run: &RunArgs, runner: &Runner) -> Result<Inputs, CliError> {
    let bytes = read_bytes(&run.input, run.limits.max_input_bytes)?;
    if run.typed_input {
        let inputs = decode_json(&bytes, run.limits.max_input_bytes)?;
        check_ports(&runner.inspection().signature.inputs, &inputs)?;
        Ok(inputs)
    } else {
        plain_inputs(
            &runner.inspection().signature,
            &decode_json::<serde_json::Value>(&bytes, run.limits.max_input_bytes)?,
        )
    }
}
fn combined_recording(
    recorders: &[Arc<RecordingBackend>],
    max_bytes: usize,
) -> Result<Recording, CliError> {
    let mut combined = Recording::default();
    let mut used = 15usize;
    for recorder in recorders {
        for exchange in recorder.snapshot()?.exchanges {
            let bytes = bounded_json(&exchange, max_bytes)?;
            used = used
                .checked_add(bytes.len())
                .and_then(|n| n.checked_add(1))
                .filter(|n| *n <= max_bytes)
                .ok_or_else(|| {
                    SaphoError::new(
                        ErrorCode::LimitExceeded,
                        "Combined recording exceeds byte ceiling",
                    )
                })?;
            combined.exchanges.push(exchange);
        }
    }
    Ok(combined)
}
enum Invocation {
    Run,
    Record(std::path::PathBuf),
    Replay(std::path::PathBuf),
}
fn invoke(run: RunArgs, invocation: Invocation) -> Result<Response, CliError> {
    let artifact = load_graph(&run.graph.graph, run.graph.format.map(GraphFormat::from))?;
    let metadata = bindings(run.bindings.as_deref())?;
    let replay = if let Invocation::Replay(path) = &invocation {
        Some(replay_registry(
            path,
            &metadata,
            run.limits.max_artifact_bytes,
        )?)
    } else {
        None
    };
    let runtime = runtime()?;
    let (runner, recorders) = {
        let _entered = runtime.enter();
        let primitives = PrimitiveRegistry::default();
        let inspection = inspect(&artifact.definition, &primitives)?;
        let registry = if let Some(replay) = replay {
            replay
        } else {
            live_registry(
                &inspection.backends,
                &metadata,
                run.service_config.as_deref(),
            )?
        };
        let (registry, recorders) = if matches!(invocation, Invocation::Record(_)) {
            recording_bindings(
                &inspection.backends,
                &registry,
                run.limits.max_artifact_bytes,
            )?
        } else {
            (registry, Vec::new())
        };
        (
            Runner::new_stock(&artifact.definition, &primitives, registry)?,
            recorders,
        )
    };
    let input = inputs(&run, &runner)?;
    let (output, trace, record_destination) = (
        claim(run.output.as_deref())?,
        claim(run.trace.as_deref())?,
        if let Invocation::Record(path) = &invocation {
            Some(ArtifactWriter::create(path)?)
        } else {
            None
        },
    );
    let report = runtime.block_on(runner.run(&input, run.limits.run(), run.fail_on.as_deref()));
    if let Some(writer) = record_destination {
        writer.finish(
            &combined_recording(&recorders, run.limits.max_artifact_bytes)?
                .to_json(run.limits.max_artifact_bytes)?,
            run.limits.max_artifact_bytes,
        )?;
    }
    if let Some(writer) = trace {
        writer.finish(
            &bounded_json(&report.trace, run.limits.max_artifact_bytes)?,
            run.limits.max_artifact_bytes,
        )?;
    }
    let exit = report.exit;
    respond(report, exit, run.limits.max_artifact_bytes, output)
}
/// One evaluated case: the retained outcome for scoring and the case's finished report as JSON.
///
/// The report is serialized as soon as the case ends, so a large Dataset holds one compact
/// document per case rather than every trace as live structures.
struct CaseRun {
    id: ItemId,
    outcome: CaseOutcome,
    shadow: Option<CaseOutcome>,
    document: Vec<u8>,
    calibration: Vec<CalibrationUse>,
    ask_models: BTreeMap<String, Vec<ModelIdentity>>,
}
struct CalibrationUse {
    map_id: String,
}
/// The per-case documents of a report, each held compressed and expanded only while it is
/// written. The documents repeat their graph structure, so they shrink to a small fraction,
/// which keeps the memory of a report in proportion to the Dataset rather than to its traces.
struct Documents(Vec<Vec<u8>>);
impl Serialize for Documents {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::{Error, SerializeSeq};
        let mut items = serializer.serialize_seq(Some(self.0.len()))?;
        for compressed in &self.0 {
            let mut json = Vec::new();
            flate2::read::DeflateDecoder::new(compressed.as_slice())
                .read_to_end(&mut json)
                .map_err(S::Error::custom)?;
            let raw: &RawValue = serde_json::from_slice(&json).map_err(S::Error::custom)?;
            items.serialize_element(raw)?;
        }
        items.end()
    }
}
fn compress(json: &[u8]) -> Result<Vec<u8>, SaphoError> {
    let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder
        .write_all(json)
        .and_then(|()| encoder.finish())
        .map_err(|e| SaphoError::new(ErrorCode::Config, e.to_string()))
}
#[derive(Serialize)]
struct CaseRecord<'a> {
    id: &'a ItemId,
    inputs: &'a Inputs,
    report: &'a RunReport,
}
async fn run_cases(
    runner: &Runner,
    data: &Dataset,
    split: Split,
    limits: &Limits,
    shadow: Option<(&str, &str)>,
) -> Result<Vec<CaseRun>, CliError> {
    let mut runs = Vec::new();
    for case in data.selected(split) {
        let report = runner.run(&case.inputs, limits.run(), None).await;
        let json = serde_json::to_vec(&CaseRecord {
            id: &case.id,
            inputs: &case.inputs,
            report: &report,
        })
        .map_err(|e| SaphoError::new(ErrorCode::Config, e.to_string()))?;
        runs.push(CaseRun {
            id: case.id.clone(),
            outcome: outcome(&report),
            shadow: shadow.map(|(id, output)| shadow_outcome(&report, id, output)),
            document: compress(&json)?,
            calibration: report
                .trace
                .nodes
                .iter()
                .filter_map(|node| {
                    if !matches!(node.operation, Operation::Calibrate)
                        || node.status != NodeStatus::Completed
                    {
                        return None;
                    }
                    let Value::CalibrationMap(map) = &node.inputs.get("map")?.value else {
                        return None;
                    };
                    Some(CalibrationUse {
                        map_id: map.map_id.clone(),
                    })
                })
                .collect(),
            ask_models: report
                .trace
                .nodes
                .iter()
                .fold(BTreeMap::new(), |mut by_node, node| {
                    if let ([root, id], Operation::Ask { .. }, Some(response)) = (
                        node.path.as_slice(),
                        &node.operation,
                        node.model
                            .as_ref()
                            .and_then(|model| model.response.as_ref()),
                    ) && root == "root"
                    {
                        by_node
                            .entry(id.clone())
                            .or_insert_with(Vec::new)
                            .push(ModelIdentity {
                                name: response.model.clone(),
                            });
                    }
                    by_node
                }),
        });
    }
    Ok(runs)
}
fn shadow_outcome(report: &RunReport, id: &str, output: &str) -> CaseOutcome {
    let found = report
        .trace
        .shadows
        .iter()
        .filter(|observation| observation.node.as_str() == id && observation.output == output)
        .collect::<Vec<_>>();
    let models = found
        .iter()
        .filter_map(|observation| observation.model.as_ref()?.response.as_ref())
        .map(|response| ModelIdentity {
            name: response.model.clone(),
        })
        .collect();
    match found.as_slice() {
        [observation] if observation.status == sapho_runtime::NodeStatus::Completed => {
            if let Some(value) = &observation.value {
                CaseOutcome::Completed {
                    outputs: Inputs::from([(output.into(), value.clone())]),
                    models,
                }
            } else {
                CaseOutcome::Failed {
                    error: SaphoError::new(
                        ErrorCode::MissingAnswer,
                        "Shadow observation value absent",
                    ),
                    models,
                }
            }
        }
        [observation] => CaseOutcome::Failed {
            error: observation.error.clone().unwrap_or_else(|| {
                SaphoError::new(ErrorCode::MissingAnswer, "Shadow observation unavailable")
            }),
            models,
        },
        [] => CaseOutcome::Failed {
            error: SaphoError::new(
                ErrorCode::MissingAnswer,
                "Selected shadow observation absent",
            ),
            models,
        },
        _ => CaseOutcome::Failed {
            error: SaphoError::new(
                ErrorCode::DuplicateId,
                "Several shadow observations map to selected output",
            ),
            models,
        },
    }
}
/// Every model that answered during the run, as its backend reported it.
fn models(report: &RunReport) -> Vec<ModelIdentity> {
    report
        .trace
        .nodes
        .iter()
        .filter_map(|node| {
            matches!(node.operation, sapho_graph::Operation::Ask { .. })
                .then_some(node.model.as_ref()?.response.as_ref()?)
        })
        .map(|response| ModelIdentity {
            name: response.model.clone(),
        })
        .collect()
}
fn outcome(report: &RunReport) -> CaseOutcome {
    match (&report.outputs, &report.error) {
        (_, Some(error)) => CaseOutcome::Failed {
            error: error.clone(),
            models: models(report),
        },
        (Some(outputs), None) => CaseOutcome::Completed {
            outputs: outputs.clone(),
            models: models(report),
        },
        (None, None) => CaseOutcome::Failed {
            error: SaphoError::new(ErrorCode::MissingInput, "Run has no outputs"),
            models: models(report),
        },
    }
}
/// Separate what scoring needs from the finished per-case documents, in case order.
fn split(
    runs: Vec<CaseRun>,
) -> (
    BTreeMap<ItemId, CaseOutcome>,
    BTreeMap<ItemId, CaseOutcome>,
    Documents,
) {
    let mut outcomes = BTreeMap::new();
    let mut shadows = BTreeMap::new();
    let mut documents = Vec::with_capacity(runs.len());
    for run in runs {
        if let Some(shadow) = run.shadow {
            shadows.insert(run.id.clone(), shadow);
        }
        outcomes.insert(run.id, run.outcome);
        documents.push(run.document);
    }
    (outcomes, shadows, Documents(documents))
}
#[derive(Serialize)]
struct MeasurementReport {
    graph: GraphArtifact,
    measurement: Measurement,
    #[serde(skip_serializing_if = "Option::is_none")]
    comparison: Option<ShadowComparison>,
    runs: Documents,
}
#[derive(Serialize)]
struct CalibrationMismatchReport {
    graph: GraphArtifact,
    error: SaphoError,
    runs: Documents,
}
fn raw_output_ask_source(graph: &GraphSpec, output: &str) -> Result<(NodeId, BackendId), CliError> {
    let mut pending = Vec::new();
    let Some(Binding::Node { node, .. }) = graph.outputs.get(output) else {
        return Err(
            SaphoError::new(ErrorCode::Config, "Raw output must come from a graph node").into(),
        );
    };
    pending.push(node.clone());
    let by_id = graph
        .nodes
        .iter()
        .map(|node| (node.id.clone(), node))
        .collect::<BTreeMap<_, _>>();
    let mut visited = std::collections::BTreeSet::new();
    let mut asks = Vec::new();
    while let Some(id) = pending.pop() {
        if !visited.insert(id.clone()) {
            continue;
        }
        let node = by_id
            .get(&id)
            .ok_or_else(|| SaphoError::new(ErrorCode::Config, "Output dependency absent"))?;
        if let Operation::Ask { backend, .. } = &node.operation {
            asks.push((id.clone(), backend.clone()));
        }
        if matches!(node.operation, Operation::Map { .. }) {
            return Err(SaphoError::new(
                ErrorCode::Config,
                "Calibration fit requires one explicit Ask in the raw output closure",
            )
            .into());
        }
        for binding in node.inputs.values().chain(node.guard.iter()) {
            if let Binding::Node { node, .. } = binding {
                pending.push(node.clone());
            }
        }
    }
    match asks.as_slice() {
        [source] => Ok(source.clone()),
        _ => Err(SaphoError::new(
            ErrorCode::Config,
            "Calibration fit requires exactly one contributing Ask",
        )
        .into()),
    }
}
fn fit_calibration_command(args: FitCalibrationArgs) -> Result<Response, CliError> {
    if !matches!(args.split, Partition::Development) {
        return Err(EvidenceError::InvalidCalibration("held_out_fit".into()).into());
    }
    let data = dataset(
        &args.dataset,
        args.limits.max_artifact_bytes,
        args.max_cases,
    )?;
    if data.selected(Split::Development).next().is_none() {
        return Err(EvidenceError::EmptySplit(Split::Development).into());
    }
    let graph = load_graph(&args.graph.graph, args.graph.format.map(GraphFormat::from))?;
    let declared = BackendId::new(&args.binding)?;
    let found = raw_output_ask_source(&graph.definition, &args.output_name)?.1;
    if found != declared {
        return Err(SaphoError::new(
            ErrorCode::ModelMismatch,
            "Fitting binding differs from raw output Ask",
        )
        .into());
    }
    let metadata = bindings(args.bindings.as_deref())?;
    let replay = args
        .replay
        .as_deref()
        .map(|path| replay_registry(path, &metadata, args.limits.max_artifact_bytes))
        .transpose()?;
    let runtime = runtime()?;
    let (runner, _) = {
        let _entered = runtime.enter();
        prepare(
            &graph.definition,
            &metadata,
            replay.map(|registry| move || Ok(registry)),
            args.service_config.as_deref(),
        )?
    };
    if runner.inspection().signature.outputs.get(&args.output_name) != Some(&ValueType::Probability)
    {
        return Err(SaphoError::new(
            ErrorCode::TypeMismatch,
            "Fitting output must be raw Probability",
        )
        .into());
    }
    let destination = ArtifactWriter::create(&args.output)?;
    let result = (|| -> Result<Binding, CliError> {
        let mut outcomes = BTreeMap::new();
        let mut attribution = FitAttribution::new();
        for case in data.selected(Split::Development) {
            let report = runtime.block_on(runner.run(&case.inputs, args.limits.run(), None));
            let lineage = contributing_responses(&graph.definition, &report.trace);
            let contributors = lineage.get(&args.output_name).ok_or_else(|| {
                SaphoError::new(ErrorCode::ModelMismatch, "Fitting output lineage absent")
            })?;
            let [contributor] = contributors.as_slice() else {
                return Err(SaphoError::new(
                    ErrorCode::ModelMismatch,
                    "Fitting output has ambiguous model attribution",
                )
                .into());
            };
            if contributor.binding != declared {
                return Err(SaphoError::new(
                    ErrorCode::ModelMismatch,
                    "Fitting binding differs from response",
                )
                .into());
            }
            let actual = contributor.actual_model.as_ref().ok_or_else(|| {
                SaphoError::new(ErrorCode::ModelMismatch, "Fitting response absent")
            })?;
            attribution.insert(
                case.id.clone(),
                (
                    declared.clone(),
                    ModelIdentity {
                        name: actual.clone(),
                    },
                ),
            );
            outcomes.insert(case.id.clone(), outcome(&report));
        }
        let identity = graph_semantic_identity(&graph.definition)?;
        let map = fit_calibration(
            &data,
            Split::Development,
            &args.output_name,
            &outcomes,
            &attribution,
            &identity,
            args.max_cases,
        )?;
        Ok(Binding::Literal {
            value: Datum::new("calibration_map", Value::CalibrationMap(Box::new(map)))?,
            value_type: ValueType::CalibrationMap,
        })
    })();
    let literal = match result {
        Ok(literal) => literal,
        Err(error) => {
            destination.discard()?;
            return Err(error);
        }
    };
    if let Err(error) = measured_json_bytes(&literal, args.limits.max_artifact_bytes) {
        destination.discard()?;
        return Err(error.into());
    }
    respond(
        literal,
        ExitStatus::Completed,
        args.limits.max_artifact_bytes,
        Some(destination),
    )
}
fn check_development_calibration_observations(
    dataset: &Dataset,
    graph: &GraphSpec,
    runs: &[CaseRun],
    max_cases: usize,
) -> Result<(), CliError> {
    let mut maps = BTreeMap::<String, CalibrationMap>::new();
    for node in graph
        .nodes
        .iter()
        .chain(graph.subgraphs.values().flat_map(|body| body.nodes.iter()))
    {
        for binding in node.inputs.values() {
            if let Binding::Literal { value, .. } = binding
                && let Value::CalibrationMap(map) = &value.value
            {
                maps.insert(map.map_id.clone(), (**map).clone());
            }
        }
    }
    for map in maps.values() {
        if !runs
            .iter()
            .any(|run| run.calibration.iter().any(|used| used.map_id == map.map_id))
        {
            continue;
        }
        if calibration_cases_digest(dataset, &map.raw_output, max_cases)? != map.fit_cases_digest {
            continue;
        }
        if !graph.outputs.contains_key(&map.raw_output) {
            return Err(SaphoError::new(
                ErrorCode::ModelMismatch,
                "Calibration raw fit output unavailable for development verification",
            )
            .with_context("map_id", &map.map_id)
            .with_context("raw_output", &map.raw_output)
            .into());
        }
        let (source_node, source_binding) =
            raw_output_ask_source(graph, &map.raw_output).map_err(|_| {
                SaphoError::new(
                    ErrorCode::ModelMismatch,
                    "Calibration raw output source differs",
                )
            })?;
        if source_binding != map.fit_binding {
            return Err(SaphoError::new(
                ErrorCode::ModelMismatch,
                "Calibration raw output binding differs",
            )
            .with_context("map_id", &map.map_id)
            .into());
        }
        // The fitted cohort needs one raw output and one contributing response
        // per case. Executed calibration nodes can be guarded or repeated.
        if runs.iter().any(|run| {
            !matches!(&run.outcome, CaseOutcome::Completed { outputs, .. }
                if outputs.get(&map.raw_output).is_some_and(|datum| matches!(&datum.value, Value::Probability(_))))
                || run.ask_models.get(source_node.as_str()).is_none_or(|models| models.len() != 1)
        }) {
            continue;
        }
        let mut outcomes = BTreeMap::new();
        let mut attribution = FitAttribution::new();
        for run in runs {
            outcomes.insert(run.id.clone(), run.outcome.clone());
            let model = run
                .ask_models
                .get(source_node.as_str())
                .and_then(|models| models.first())
                .ok_or_else(|| {
                    SaphoError::new(
                        ErrorCode::CodeFailed,
                        "Comparable calibration response absent",
                    )
                })?;
            attribution.insert(run.id.clone(), (source_binding.clone(), model.clone()));
        }
        let fitted = fit_calibration(
            dataset,
            Split::Development,
            &map.raw_output,
            &outcomes,
            &attribution,
            &map.fit_graph_semantic_identity,
            max_cases,
        )
        .map_err(|_| {
            SaphoError::new(
                ErrorCode::ModelMismatch,
                "Calibration fit observations differ",
            )
        })?;
        if fitted.fit_observation_digest != map.fit_observation_digest {
            return Err(SaphoError::new(
                ErrorCode::ModelMismatch,
                "Calibration fit observations differ",
            )
            .with_context("map_id", &map.map_id)
            .with_context("expected", &map.fit_observation_digest)
            .with_context("observed", fitted.fit_observation_digest)
            .into());
        }
    }
    Ok(())
}
fn measurement(args: MeasureArgs) -> Result<Response, CliError> {
    let options = &args.options;
    let data = dataset(
        &options.dataset,
        options.limits.max_artifact_bytes,
        options.max_cases,
    )?;
    let selected = Split::from(args.split);
    let graph = load_graph(&args.graph.graph, args.graph.format.map(GraphFormat::from))?;
    let metadata = bindings(options.bindings.as_deref())?;
    let replay = options
        .replay
        .as_deref()
        .map(|path| replay_registry(path, &metadata, options.limits.max_artifact_bytes))
        .transpose()?;
    let runtime = runtime()?;
    let (runner, _) = {
        let _entered = runtime.enter();
        prepare(
            &graph.definition,
            &metadata,
            replay.map(|registry| move || Ok(registry)),
            options.service_config.as_deref(),
        )?
    };
    let selection = args.shadow.as_deref().zip(args.shadow_output.as_deref());
    if args.shadow.is_some() != args.shadow_output.is_some() {
        return Err(SaphoError::new(
            ErrorCode::Config,
            "Shadow comparison requires --shadow and --shadow-output",
        )
        .into());
    }
    if let Some((shadow, output)) = selection {
        let count = runner
            .inspection()
            .shadows
            .iter()
            .filter(|declared| declared.node.as_str() == shadow)
            .flat_map(|declared| &declared.observations)
            .filter(|observation| observation.output == output)
            .count();
        if count != 1 {
            return Err(SaphoError::new(
                ErrorCode::Config,
                "Comparison must select exactly one declared shadow observation",
            )
            .with_context("shadow", shadow)
            .with_context("output", output)
            .into());
        }
    }
    let destination = claim(options.output.as_deref())?;
    let case_runs = match runtime.block_on(run_cases(
        &runner,
        &data,
        selected,
        &options.limits,
        selection,
    )) {
        Ok(value) => value,
        Err(error) => {
            if let Some(destination) = destination {
                destination.discard()?;
            }
            return Err(error);
        }
    };
    if selected == Split::Development {
        match check_development_calibration_observations(
            &data,
            &graph.definition,
            &case_runs,
            options.max_cases,
        ) {
            Ok(()) => {}
            Err(CliError::Engine(error)) if error.code == ErrorCode::ModelMismatch => {
                let (_, _, runs) = split(case_runs);
                return respond(
                    CalibrationMismatchReport { graph, error, runs },
                    ExitStatus::Refused,
                    options.limits.max_artifact_bytes,
                    destination,
                );
            }
            Err(error) => {
                if let Some(destination) = destination {
                    destination.discard()?;
                }
                return Err(error);
            }
        }
    }
    let (outcomes, shadows, runs) = split(case_runs);
    let measurement = match measure(
        &data,
        selected,
        &runner.inspection().signature.outputs,
        &outcomes,
        options.max_cases,
    ) {
        Ok(value) => value,
        Err(error) => {
            if let Some(destination) = destination {
                destination.discard()?;
            }
            return Err(error.into());
        }
    };
    let comparison = selection
        .map(|(shadow, output)| {
            let request = ShadowComparisonRequest {
                output: output.into(),
                shadow: shadow.into(),
                metric: args
                    .shadow_metric
                    .map(ShadowMetric::from)
                    .unwrap_or(ShadowMetric::Agreement),
                min_scored: args.shadow_min_scored,
                margin: args.shadow_margin,
            };
            compare_shadow(
                &data,
                selected,
                &runner.inspection().signature.outputs,
                &outcomes,
                &shadows,
                &request,
                options.max_cases,
            )
            .map_err(CliError::from)
        })
        .transpose()?;
    let exit = if measurement.complete() {
        ExitStatus::Completed
    } else {
        ExitStatus::Refused
    };
    respond(
        MeasurementReport {
            graph,
            measurement,
            comparison,
            runs,
        },
        exit,
        options.limits.max_artifact_bytes,
        destination,
    )
}
fn gate_register(args: GateRegisterArgs) -> Result<Response, CliError> {
    let data = dataset(&args.dataset, args.max_artifact_bytes, args.max_cases)?;
    let graph = load_graph(&args.graph.graph, args.graph.format.map(GraphFormat::from))?;
    let inspection = inspect(&graph.definition, &PrimitiveRegistry::default())?;
    let identity = graph_semantic_identity(&graph.definition)?;
    let bytes = read_bytes(&args.gate, args.max_artifact_bytes)?;
    let receipt = register_gate(
        &bytes,
        &data,
        &identity,
        &inspection.signature.outputs,
        args.max_cases,
    )?;
    ArtifactWriter::create(&args.receipt)?.finish_json(&receipt, args.max_artifact_bytes)?;
    respond(
        receipt,
        ExitStatus::Completed,
        args.max_artifact_bytes,
        None,
    )
}
#[derive(Serialize)]
struct GateEvaluationReport {
    graph: GraphArtifact,
    gate: GateReport,
    primary: Measurement,
    primary_runs: Documents,
    repetitions: Vec<Measurement>,
    repetition_runs: Vec<Documents>,
    /// Live means fresh requests; replay means distinct supplied recordings.
    repetition_source: &'static str,
}
fn gate_evaluate(args: GateEvaluateArgs) -> Result<Response, CliError> {
    let options = &args.options;
    if !(2..=8).contains(&args.repetitions) {
        return Err(EvidenceError::InvalidGate("Repetition count must be 2..=8".into()).into());
    }
    if options.replay.is_some() != !args.repeat_replay.is_empty()
        || (options.replay.is_some() && args.repeat_replay.len() != args.repetitions)
    {
        return Err(EvidenceError::InvalidGate(
            "Replay requires one distinct primary and one file per additional run".into(),
        )
        .into());
    }
    let data = dataset(
        &options.dataset,
        options.limits.max_artifact_bytes,
        options.max_cases,
    )?;
    let graph = load_graph(&args.graph.graph, args.graph.format.map(GraphFormat::from))?;
    let identity = graph_semantic_identity(&graph.definition)?;
    let inspection = inspect(&graph.definition, &PrimitiveRegistry::default())?;
    let gate_bytes = read_bytes(&args.gate, options.limits.max_artifact_bytes)?;
    let receipt_bytes = read_bytes(&args.receipt, options.limits.max_artifact_bytes)?;
    let registration: GateRegistration =
        decode_json(&receipt_bytes, options.limits.max_artifact_bytes)?;
    verify_gate(
        &registration,
        &gate_bytes,
        &data,
        &identity,
        &inspection.signature.outputs,
        options.max_cases,
    )?;
    if data.selected(Split::HeldOut).next().is_none() {
        return Err(EvidenceError::EmptySplit(Split::HeldOut).into());
    }
    let metadata = bindings(options.bindings.as_deref())?;
    let replay_paths = options
        .replay
        .iter()
        .chain(args.repeat_replay.iter())
        .collect::<Vec<_>>();
    let mut replay_data = Vec::new();
    let mut seen_paths = std::collections::BTreeSet::new();
    #[cfg(unix)]
    let mut seen_files = std::collections::BTreeSet::new();
    for path in replay_paths {
        let canonical = std::fs::canonicalize(path).map_err(|source| CliError::Io {
            path: (*path).clone(),
            source,
        })?;
        if !seen_paths.insert(canonical) {
            return Err(EvidenceError::InvalidGate(
                "Repetition reuses the same recording file".into(),
            )
            .into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = std::fs::metadata(path).map_err(|source| CliError::Io {
                path: (*path).clone(),
                source,
            })?;
            if !seen_files.insert((metadata.dev(), metadata.ino())) {
                return Err(EvidenceError::InvalidGate(
                    "Repetition aliases the same recording file".into(),
                )
                .into());
            }
        }
        let bytes = read_bytes(path, options.limits.max_artifact_bytes)?;
        replay_data.push(bytes);
    }
    let runtime = runtime()?;
    let mut runners = Vec::new();
    {
        let _entered = runtime.enter();
        if replay_data.is_empty() {
            for _ in 0..=args.repetitions {
                runners.push(
                    prepare(
                        &graph.definition,
                        &metadata,
                        None::<fn() -> Result<BackendRegistry, CliError>>,
                        options.service_config.as_deref(),
                    )?
                    .0,
                );
            }
        } else {
            for bytes in replay_data {
                let registry = replay_bindings_json(
                    &bytes,
                    Some(&metadata),
                    options.limits.max_artifact_bytes,
                )?;
                runners.push(
                    prepare(
                        &graph.definition,
                        &metadata,
                        Some(move || Ok(registry)),
                        None,
                    )?
                    .0,
                );
            }
        }
    }
    let destination = claim(options.output.as_deref())?;
    let mut measurements = Vec::new();
    let mut documents = Vec::new();
    for runner in &runners {
        let (outcomes, _, runs) = split(runtime.block_on(run_cases(
            runner,
            &data,
            Split::HeldOut,
            &options.limits,
            None,
        ))?);
        measurements.push(measure_for_gate(
            &data,
            Split::HeldOut,
            &identity,
            &runner.inspection().signature.outputs,
            &outcomes,
            options.max_cases,
        )?);
        documents.push(runs);
    }
    let primary = measurements.remove(0);
    let primary_runs = documents.remove(0);
    let gate = evaluate_gate(
        &registration,
        &data,
        &identity,
        &inspection.signature.outputs,
        &primary,
        &measurements,
        options.max_cases,
    )?;
    let exit = if !gate.complete {
        ExitStatus::Refused
    } else if gate.verdict == GateVerdict::Go {
        ExitStatus::Completed
    } else {
        ExitStatus::Finding
    };
    respond(
        GateEvaluationReport {
            graph,
            gate,
            primary: primary.into_measurement(),
            primary_runs,
            repetitions: measurements
                .into_iter()
                .map(|run| run.into_measurement())
                .collect(),
            repetition_runs: documents,
            repetition_source: if options.replay.is_some() {
                "caller_supplied_recording_files"
            } else {
                "fresh_live_requests"
            },
        },
        exit,
        options.limits.max_artifact_bytes,
        destination,
    )
}
fn contributing_responses(
    graph: &GraphSpec,
    trace: &Trace,
) -> BTreeMap<String, Vec<RosterContributor>> {
    let by_path = trace
        .nodes
        .iter()
        .map(|node| (node.path.clone(), node))
        .collect::<BTreeMap<_, _>>();
    graph
        .outputs
        .iter()
        .map(|(output, binding)| {
            let mut pending = Vec::new();
            if let Binding::Node { node, .. } = binding {
                pending.push(vec!["root".into(), node.to_string()]);
            }
            let mut visited = std::collections::BTreeSet::new();
            while let Some(path) = pending.pop() {
                if !visited.insert(path.clone()) {
                    continue;
                }
                if let Some(node) = by_path.get(&path) {
                    pending.extend(node.dependencies.iter().cloned());
                    if matches!(node.operation, Operation::Map { .. }) {
                        pending.extend(
                            by_path
                                .keys()
                                .filter(|child| {
                                    child.len() > path.len() && child.starts_with(&path)
                                })
                                .cloned(),
                        );
                    }
                }
            }
            let questions = visited
                .iter()
                .filter_map(|path| match &by_path.get(path)?.operation {
                    Operation::Probability { question, .. } => Some(question.as_str()),
                    _ => None,
                })
                .collect::<std::collections::BTreeSet<_>>();
            let contributors = visited
                .iter()
                .filter_map(|path| {
                    let node = by_path.get(path)?;
                    let Operation::Ask { backend, .. } = &node.operation else {
                        return None;
                    };
                    if node.status != NodeStatus::Completed {
                        return None;
                    }
                    let model = node.model.as_ref()?;
                    let response = model.response.as_ref()?;
                    let mut kinds = model
                        .request
                        .questions
                        .iter()
                        .filter(|named| {
                            questions.is_empty() || questions.contains(named.id.as_str())
                        })
                        .map(|named| match named.question {
                            Question::Boolean { .. } => RosterQuestionKind::Boolean,
                            Question::Choice { .. } => RosterQuestionKind::Choice,
                            Question::Score { .. } => RosterQuestionKind::Score,
                        });
                    let first = kinds.next();
                    let question_kind = first.filter(|first| kinds.all(|kind| kind == *first));
                    Some(RosterContributor {
                        binding: backend.clone(),
                        actual_model: Some(response.model.clone()),
                        question_kind,
                    })
                })
                .collect();
            (output.clone(), contributors)
        })
        .collect()
}
fn roster_call(call: ModelCallObservation) -> RosterCall {
    RosterCall {
        path: call.path,
        binding: call.binding,
        actual_model: call.actual_model,
        completed: call.status == NodeStatus::Completed,
        usage: call.usage,
        elapsed_micros: call.elapsed_micros,
        descriptor: call.descriptor,
        mode: match call.mode {
            ObservationMode::Live => RosterMode::Live,
            ObservationMode::Replay => RosterMode::Replay,
        },
    }
}
fn stock_descriptors(metadata: &Bindings) -> BTreeMap<sapho_core::BackendId, ProviderDescriptor> {
    metadata
        .iter()
        .map(|(id, config)| {
            let name = match config.provider {
                sapho_cli::Provider::Jev => "jev",
                sapho_cli::Provider::Clm => "clm",
                sapho_cli::Provider::Ollama => "ollama",
                sapho_cli::Provider::Systemone => "systemone",
                sapho_cli::Provider::Decisions => "decisions",
                sapho_cli::Provider::Claude => "claude",
            };
            (
                id.clone(),
                ProviderDescriptor {
                    provider: Some(name.into()),
                    adapter: Some(name.into()),
                },
            )
        })
        .collect()
}
fn roster_command(args: RosterArgs) -> Result<Response, CliError> {
    let options = &args.options;
    let output_path = options
        .output
        .as_deref()
        .ok_or_else(|| SaphoError::new(ErrorCode::Config, "Roster requires --output"))?;
    let selected = Split::from(args.split);
    let data = dataset(
        &options.dataset,
        options.limits.max_artifact_bytes,
        options.max_cases,
    )?;
    if data.selected(selected).next().is_none() {
        return Err(EvidenceError::EmptySplit(selected).into());
    }
    let graph = load_graph(&args.graph.graph, args.graph.format.map(GraphFormat::from))?;
    let mapping_text = read_bytes(&args.mapping, 1_048_576)?;
    let mappings: BTreeMap<String, RosterMapping> = parse_config(
        std::str::from_utf8(&mapping_text)
            .map_err(|_| SaphoError::new(ErrorCode::Config, "Roster mapping must be UTF-8"))?,
        select_format(&args.mapping, None)?,
    )?;
    let metadata = bindings(options.bindings.as_deref())?;
    let replay = options
        .replay
        .as_deref()
        .map(|path| replay_registry(path, &metadata, options.limits.max_artifact_bytes))
        .transpose()?;
    let mode = if replay.is_some() {
        ObservationMode::Replay
    } else {
        ObservationMode::Live
    };
    let runtime = runtime()?;
    let (runner, required) = {
        let _entered = runtime.enter();
        prepare(
            &graph.definition,
            &metadata,
            replay.map(|registry| move || Ok(registry)),
            options.service_config.as_deref(),
        )?
    };
    for (output, mapping) in &mappings {
        sapho_core::validate_name(output)?;
        if !runner.inspection().signature.outputs.contains_key(output)
            || !required.contains(&mapping.binding)
        {
            return Err(
                SaphoError::new(ErrorCode::Config, "Invalid roster output mapping")
                    .with_context("output", output)
                    .into(),
            );
        }
    }
    let config = ObservationConfig {
        mode,
        descriptors: stock_descriptors(&metadata),
        clock: Arc::new(SystemObservationClock),
    };
    config.validate()?;
    let destination = claim(Some(output_path))?;
    let report = (|| -> Result<Roster, CliError> {
        let mut cases = BTreeMap::new();
        for case in data.selected(selected) {
            let (report, calls) = runtime.block_on(runner.run_observed(
                &case.inputs,
                options.limits.run(),
                None,
                &config,
            ))?;
            let lineage = contributing_responses(&graph.definition, &report.trace);
            cases.insert(
                case.id.clone(),
                RosterCase {
                    outcome: outcome(&report),
                    calls: calls.into_iter().map(roster_call).collect(),
                    lineage,
                },
            );
        }
        let identity = graph_semantic_identity(&graph.definition)?;
        Ok(roster(
            &data,
            selected,
            &identity,
            env!("CARGO_PKG_VERSION"),
            &runner.inspection().signature.outputs,
            &mappings,
            &cases,
            options.max_cases,
        )?)
    })();
    let report = match report {
        Ok(report) => report,
        Err(error) => {
            if let Some(destination) = destination {
                destination.discard()?;
            }
            return Err(error);
        }
    };
    respond(
        report,
        ExitStatus::Completed,
        options.limits.max_artifact_bytes,
        destination,
    )
}
fn roster_literal_command(args: RosterLiteralArgs) -> Result<Response, CliError> {
    let report: Roster = decode_json(
        &read_bytes(&args.roster, args.max_artifact_bytes)?,
        args.max_artifact_bytes,
    )?;
    let selections: Vec<RosterSelection> =
        decode_json(&read_bytes(&args.selection, 1_048_576)?, 1_048_576)?;
    let projected = project_roster_literal(&report, &selections, args.max_artifact_bytes)?;
    let literal = Binding::Literal {
        value: projected.value,
        value_type: projected.value_type,
    };
    let destination = claim(Some(&args.output))?;
    respond(
        literal,
        ExitStatus::Completed,
        args.max_artifact_bytes,
        destination,
    )
}
#[derive(Serialize)]
struct CandidateReport {
    id: SourceId,
    path: std::path::PathBuf,
    graph: Option<GraphArtifact>,
    measurement: Option<Measurement>,
    runs: Documents,
    error: Option<CliError>,
}
#[derive(Serialize)]
struct TuneReport {
    dataset: SourceId,
    output: String,
    metric: ScoreMetric,
    candidates: Vec<CandidateReport>,
    ranking: Vec<RankedCandidate>,
    error: Option<EvidenceError>,
}
fn tuning(args: TuneArgs) -> Result<Response, CliError> {
    if args.candidate.is_empty() {
        return Err(EvidenceError::NoCandidates.into());
    }
    if args.candidate.len() > args.max_candidates {
        return Err(EvidenceError::CandidateLimit {
            limit: args.max_candidates,
        }
        .into());
    }
    let options = &args.options;
    let data = dataset(
        &options.dataset,
        options.limits.max_artifact_bytes,
        options.max_cases,
    )?;
    if data.selected(Split::Development).next().is_none() {
        return Err(EvidenceError::EmptySplit(Split::Development).into());
    }
    let metadata = bindings(options.bindings.as_deref())?;
    // A recording that cannot be read or holds an invalid exchange refuses the command; a
    // conflict inside it, or metadata that differs from it, is each candidate's own error.
    let recording = options
        .replay
        .as_deref()
        .map(|path| {
            let bytes = read_bytes(path, options.limits.max_artifact_bytes)?;
            ReplayBackend::check_json(&bytes, options.limits.max_artifact_bytes)?;
            Ok::<_, CliError>(bytes)
        })
        .transpose()?;
    let runtime = runtime()?;
    let mut prepared = Vec::new();
    let mut reports = Vec::new();
    for (index, path) in args.candidate.into_iter().enumerate() {
        let id = SourceId::new(
            serde_json::to_string(&(index, &path))
                .map_err(|e| SaphoError::new(ErrorCode::Config, e.to_string()))?,
        )?;
        let artifact = load_graph(&path, args.format.map(GraphFormat::from));
        let (graph, runner, error) = match artifact {
            Err(error) => (None, None, Some(error)),
            Ok(graph) => {
                let prepared = {
                    let _entered = runtime.enter();
                    prepare(
                        &graph.definition,
                        &metadata,
                        recording.as_deref().map(|bytes| {
                            || {
                                replay_bindings_json(
                                    bytes,
                                    Some(&metadata),
                                    options.limits.max_artifact_bytes,
                                )
                            }
                        }),
                        options.service_config.as_deref(),
                    )
                };
                match prepared {
                    Ok((runner, _)) => (Some(graph), Some(runner), None),
                    Err(error) => (Some(graph), None, Some(error)),
                }
            }
        };
        prepared.push(runner);
        reports.push(CandidateReport {
            id,
            path,
            graph,
            measurement: None,
            runs: Documents(Vec::new()),
            error,
        });
    }
    let destination = claim(options.output.as_deref())?;
    let runs = runtime.block_on(async {
        let mut result = Vec::new();
        for runner in &prepared {
            result.push(if let Some(runner) = runner {
                run_cases(runner, &data, Split::Development, &options.limits, None).await?
            } else {
                Vec::new()
            });
        }
        Ok::<_, CliError>(result)
    })?;
    let mut scored = Vec::new();
    for ((report, runner), runs) in reports.iter_mut().zip(prepared).zip(runs) {
        if let Some(runner) = runner {
            let (outcomes, _, runs) = split(runs);
            let measurement = measure(
                &data,
                Split::Development,
                &runner.inspection().signature.outputs,
                &outcomes,
                options.max_cases,
            )?;
            scored.push(Candidate {
                id: report.id.clone(),
                measurement: measurement.clone(),
            });
            report.measurement = Some(measurement);
            report.runs = runs;
        }
    }
    let metric = ScoreMetric::from(args.metric);
    let (ranking, error) = match rank(&scored, &args.output_name, metric, args.max_candidates) {
        Ok(mut ranking) => {
            for entry in &mut ranking {
                entry.input_index = reports
                    .iter()
                    .position(|report| report.id == entry.id)
                    .ok_or_else(|| {
                        SaphoError::new(
                            ErrorCode::InvalidValue,
                            "Ranked candidate is absent from report",
                        )
                    })?;
            }
            (ranking, None)
        }
        Err(error) => (Vec::new(), Some(error)),
    };
    let exit = if error.is_some() {
        ExitStatus::Refused
    } else {
        ExitStatus::Completed
    };
    respond(
        TuneReport {
            dataset: data.id,
            output: args.output_name,
            metric,
            candidates: reports,
            ranking,
            error,
        },
        exit,
        options.limits.max_artifact_bytes,
        destination,
    )
}
fn selection(selector: Selector) -> Result<Response, CliError> {
    use sapho_select::{GitMode, GitOptions, select_files, select_git, select_json};
    let (inputs, limits) = match selector {
        Selector::Files { paths, limits } => {
            let inputs = select_files(
                &paths.root,
                &paths.patterns(),
                &limits.limits(),
                &limits.port,
            )?;
            (inputs, limits)
        }
        Selector::Git {
            paths,
            limits,
            mode,
            base,
            head,
        } => {
            let mode = match (mode, base, head) {
                (GitComparison::WorkingTree, None, None) => GitMode::WorkingTree,
                (GitComparison::Staged, None, None) => GitMode::Staged,
                (GitComparison::Revisions, Some(base), Some(head)) => {
                    GitMode::Revisions { base, head }
                }
                _ => {
                    return Err(CliError::Arguments(
                        "--base and --head are required together only for --mode revisions".into(),
                    ));
                }
            };
            let inputs = select_git(
                &paths.root,
                &GitOptions {
                    mode,
                    ..GitOptions::default()
                },
                &paths.patterns(),
                &limits.limits(),
                &limits.port,
            )?;
            (inputs, limits)
        }
        Selector::Json {
            input,
            pointer,
            schema,
            format,
            id_prefix,
            limits,
        } => {
            let acquisition = limits.limits();
            acquisition.validate()?;
            let started = std::time::Instant::now();
            let bytes = read_bytes_with_timeout(&schema, 1_048_576, acquisition.duration)?;
            let ty: ValueType = parse_config(
                std::str::from_utf8(&bytes)
                    .map_err(|_| SaphoError::new(ErrorCode::Config, "Schema must be UTF-8"))?,
                select_format(&schema, format.map(GraphFormat::from))?,
            )?;
            let remaining = acquisition
                .duration
                .checked_sub(started.elapsed())
                .filter(|remaining| !remaining.is_zero())
                .ok_or(sapho_select::SelectionError::Deadline)?;
            let ceiling = acquisition.file_bytes.min(acquisition.total_bytes);
            let bytes = read_bytes_with_timeout(&input, ceiling, remaining)?;
            let selected = select_json(
                &bytes,
                &pointer,
                &ty,
                &id_prefix,
                &limits.port,
                &input.to_string_lossy(),
                limits.max_file_bytes,
            )?;
            if started.elapsed() >= acquisition.duration {
                return Err(sapho_select::SelectionError::Deadline.into());
            }
            (selected, limits)
        }
    };
    let writer = claim(limits.output.as_deref())?;
    respond(
        inputs,
        ExitStatus::Completed,
        limits.max_artifact_bytes,
        writer,
    )
}
pub(crate) fn execute(cli: Cli) -> Result<Response, CliError> {
    match cli.command {
        Command::Validate(args) | Command::Inspect(args) => {
            let graph = load_graph(&args.graph, args.format.map(GraphFormat::from))?;
            respond(
                inspect(&graph.definition, &PrimitiveRegistry::default())?,
                ExitStatus::Completed,
                8 * 1_048_576,
                None,
            )
        }
        Command::Run(run) => invoke(run, Invocation::Run),
        Command::Record { run, recording } => invoke(run, Invocation::Record(recording)),
        Command::Replay { run, recording } => invoke(run, Invocation::Replay(recording)),
        Command::Measure(args) => measurement(args),
        Command::FitCalibration(args) => fit_calibration_command(args),
        Command::GateRegister(args) => gate_register(args),
        Command::GateEvaluate(args) => gate_evaluate(args),
        Command::Roster(args) => roster_command(args),
        Command::RosterLiteral(args) => roster_literal_command(args),
        Command::Tune(args) => tuning(args),
        Command::Sweep(args) => {
            let report = sweep(
                &args.graph,
                &args.grid,
                &args.output_dir,
                args.max_candidates,
            )?;
            respond(report, ExitStatus::Completed, 8 * 1_048_576, None)
        }
        Command::ExportTraining {
            dataset: path,
            output,
            max_cases,
            max_artifact_bytes,
        } => {
            let data = dataset(&path, max_artifact_bytes, max_cases)?;
            let bytes = export_training(&data, max_cases, max_artifact_bytes)?;
            ArtifactWriter::create(&output)?.finish(&bytes, max_artifact_bytes)?;
            #[derive(Serialize)]
            struct ExportReport {
                dataset: SourceId,
                development_cases: usize,
                output: std::path::PathBuf,
            }
            respond(
                ExportReport {
                    dataset: data.id.clone(),
                    development_cases: data.selected(Split::Development).count(),
                    output,
                },
                ExitStatus::Completed,
                max_artifact_bytes,
                None,
            )
        }
        Command::Select { source } => selection(source),
    }
}
