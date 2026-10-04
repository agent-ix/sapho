// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Process orchestration: all acquisition/persistence surrounds, never enters, async work.
use crate::args::*;
use sapho_cli::{
    ArtifactWriter, Bindings, CliError, ExitStatus, GraphArtifact, RunReport, Runner, inspect,
    live_bindings, load_graph, plain_inputs, read_bytes, read_bytes_with_timeout,
    recording_bindings, replay_bindings, select_format,
};
use sapho_core::{
    ErrorCode, Inputs, ItemId, PrimitiveRegistry, SaphoError, SourceId, ValueType, bounded_json,
    check_ports, decode_json,
};
use sapho_evidence::{
    Candidate, CaseOutcome, Dataset, EvidenceError, Measurement, Metric as ScoreMetric,
    RankedCandidate, Split, export_training, measure, rank,
};
use sapho_graph::{GraphFormat, GraphSpec, parse_config};
use sapho_recording::{Recording, RecordingBackend};
use serde::Serialize;
use std::{collections::BTreeMap, path::Path, sync::Arc};

pub(crate) struct Response {
    pub(crate) bytes: Vec<u8>,
    pub(crate) exit: ExitStatus,
}
fn respond<T: Serialize>(
    report: &T,
    exit: ExitStatus,
    max_bytes: usize,
    destination: Option<ArtifactWriter>,
) -> Result<Response, CliError> {
    let bytes = bounded_json(report, max_bytes)?;
    if let Some(writer) = destination {
        writer.finish(&bytes, max_bytes)?;
    }
    Ok(Response { bytes, exit })
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
fn dataset(path: &Path, max_bytes: usize, max_cases: usize) -> Result<Dataset, CliError> {
    let data: Dataset = decode_json(&read_bytes(path, max_bytes)?, max_bytes)?;
    data.validate(max_cases)?;
    Ok(data)
}
fn prepare(
    graph: &GraphSpec,
    metadata: &Bindings,
    recording: Option<&Recording>,
    max_bytes: usize,
) -> Result<(Runner, Vec<sapho_core::BackendId>), CliError> {
    let primitives = PrimitiveRegistry::default();
    let inspection = inspect(graph, &primitives)?;
    let registry = if let Some(recording) = recording {
        replay_bindings(recording, Some(metadata), max_bytes)?
    } else {
        live_bindings(&inspection.backends, metadata)?
    };
    Ok((
        Runner::new(graph, &primitives, registry)?,
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
    let recording = if let Invocation::Replay(path) = &invocation {
        Some(Recording::from_json(
            &read_bytes(path, run.limits.max_artifact_bytes)?,
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
        let registry = if let Some(recording) = &recording {
            replay_bindings(recording, Some(&metadata), run.limits.max_artifact_bytes)?
        } else {
            live_bindings(&inspection.backends, &metadata)?
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
            Runner::new(&artifact.definition, &primitives, registry)?,
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
    respond(&report, report.exit, run.limits.max_artifact_bytes, output)
}
#[derive(Serialize)]
struct CaseRun {
    id: ItemId,
    inputs: Inputs,
    report: RunReport,
}
async fn run_cases(runner: &Runner, data: &Dataset, split: Split, limits: &Limits) -> Vec<CaseRun> {
    let mut reports = Vec::new();
    for case in data.selected(split) {
        reports.push(CaseRun {
            id: case.id.clone(),
            inputs: case.inputs.clone(),
            report: runner.run(&case.inputs, limits.run(), None).await,
        });
    }
    reports
}
fn outcomes(runs: &[CaseRun]) -> BTreeMap<ItemId, CaseOutcome> {
    runs.iter()
        .map(|run| {
            let outcome = match (&run.report.outputs, &run.report.error) {
                (_, Some(error)) => CaseOutcome::Failed {
                    error: error.clone(),
                },
                (Some(outputs), None) => CaseOutcome::Completed {
                    outputs: outputs.clone(),
                },
                (None, None) => CaseOutcome::Failed {
                    error: SaphoError::new(ErrorCode::MissingInput, "Run has no outputs"),
                },
            };
            (run.id.clone(), outcome)
        })
        .collect()
}
#[derive(Serialize)]
struct MeasurementReport {
    graph: GraphArtifact,
    measurement: Measurement,
    runs: Vec<CaseRun>,
}
fn measurement(args: MeasureArgs) -> Result<Response, CliError> {
    let options = &args.options;
    let data = dataset(
        &options.dataset,
        options.limits.max_artifact_bytes,
        options.max_cases,
    )?;
    let split = Split::from(args.split);
    let graph = load_graph(&args.graph.graph, args.graph.format.map(GraphFormat::from))?;
    let metadata = bindings(options.bindings.as_deref())?;
    let recording = options
        .replay
        .as_deref()
        .map(|path| {
            Recording::from_json(
                &read_bytes(path, options.limits.max_artifact_bytes)?,
                options.limits.max_artifact_bytes,
            )
            .map_err(CliError::from)
        })
        .transpose()?;
    let runtime = runtime()?;
    let (runner, _) = {
        let _entered = runtime.enter();
        prepare(
            &graph.definition,
            &metadata,
            recording.as_ref(),
            options.limits.max_artifact_bytes,
        )?
    };
    let destination = claim(options.output.as_deref())?;
    let runs = runtime.block_on(run_cases(&runner, &data, split, &options.limits));
    let measurement = measure(
        &data,
        split,
        &runner.inspection().signature.outputs,
        &outcomes(&runs),
        options.max_cases,
    )?;
    let exit = if measurement.complete() {
        ExitStatus::Completed
    } else {
        ExitStatus::Refused
    };
    respond(
        &MeasurementReport {
            graph,
            measurement,
            runs,
        },
        exit,
        options.limits.max_artifact_bytes,
        destination,
    )
}
#[derive(Serialize)]
struct CandidateReport {
    id: SourceId,
    path: std::path::PathBuf,
    graph: Option<GraphArtifact>,
    measurement: Option<Measurement>,
    runs: Vec<CaseRun>,
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
    let recording = options
        .replay
        .as_deref()
        .map(|path| {
            Recording::from_json(
                &read_bytes(path, options.limits.max_artifact_bytes)?,
                options.limits.max_artifact_bytes,
            )
            .map_err(CliError::from)
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
                        recording.as_ref(),
                        options.limits.max_artifact_bytes,
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
            runs: Vec::new(),
            error,
        });
    }
    let destination = claim(options.output.as_deref())?;
    let runs = runtime.block_on(async {
        let mut result = Vec::new();
        for runner in &prepared {
            result.push(if let Some(runner) = runner {
                run_cases(runner, &data, Split::Development, &options.limits).await
            } else {
                Vec::new()
            });
        }
        result
    });
    let mut scored = Vec::new();
    for ((report, runner), runs) in reports.iter_mut().zip(prepared).zip(runs) {
        if let Some(runner) = runner {
            let measurement = measure(
                &data,
                Split::Development,
                &runner.inspection().signature.outputs,
                &outcomes(&runs),
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
        &TuneReport {
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
        &inputs,
        ExitStatus::Completed,
        limits.max_artifact_bytes,
        writer,
    )
}
pub(crate) fn execute(cli: Cli) -> Result<Response, CliError> {
    match cli.command {
        #[cfg(feature = "campaign")]
        Command::Campaign(args) => crate::campaign_command::execute(args),
        Command::Validate(args) | Command::Inspect(args) => {
            let graph = load_graph(&args.graph, args.format.map(GraphFormat::from))?;
            respond(
                &inspect(&graph.definition, &PrimitiveRegistry::default())?,
                ExitStatus::Completed,
                8 * 1_048_576,
                None,
            )
        }
        Command::Run(run) => invoke(run, Invocation::Run),
        Command::Record { run, recording } => invoke(run, Invocation::Record(recording)),
        Command::Replay { run, recording } => invoke(run, Invocation::Replay(recording)),
        Command::Measure(args) => measurement(args),
        Command::Tune(args) => tuning(args),
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
                &ExportReport {
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
