// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Public process arguments; defaults come from the owning limit contracts.
use clap::{Args, Parser, Subcommand, ValueEnum};
use sapho_graph::GraphFormat;
use sapho_runtime::RunLimits;
use sapho_select::{Patterns, SelectionLimits};
use std::{path::PathBuf, time::Duration};
#[derive(Parser)]
#[command(
    name = "sapho",
    version,
    about = "Typed configurable logic graphs: evaluate, record, measure and tune"
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}
#[derive(Subcommand)]
pub(crate) enum Command {
    /// Load and compile a graph without executing it.
    Validate(GraphArgs),
    /// Show signatures, dependency stages and required host implementations.
    Inspect(GraphArgs),
    /// Evaluate supplied data using explicit bindings and limits.
    Run(RunArgs),
    /// Evaluate and explicitly retain successful model exchanges.
    Record {
        #[command(flatten)]
        run: RunArgs,
        #[arg(long)]
        recording: PathBuf,
    },
    /// Evaluate offline using exact recorded model requests.
    Replay {
        #[command(flatten)]
        run: RunArgs,
        #[arg(long)]
        recording: PathBuf,
    },
    /// Measure agreement against supplied labelled cases on an explicit split.
    Measure(MeasureArgs),
    /// Compare explicit candidates using development labels only.
    Tune(TuneArgs),
    /// Export curated development supervision; performs no inference or training.
    ExportTraining {
        #[arg(long)]
        dataset: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value_t = 1024)]
        max_cases: usize,
        #[arg(long,default_value_t=8*1_048_576)]
        max_artifact_bytes: usize,
    },
    /// Acquire data as typed Inputs for run --typed-input.
    Select {
        #[command(subcommand)]
        source: Selector,
    },
}
#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum Format {
    Yaml,
    Json,
}
impl From<Format> for GraphFormat {
    fn from(value: Format) -> Self {
        match value {
            Format::Yaml => Self::Yaml,
            Format::Json => Self::Json,
        }
    }
}
#[derive(Args)]
pub(crate) struct GraphArgs {
    pub(crate) graph: PathBuf,
    #[arg(long, value_enum)]
    pub(crate) format: Option<Format>,
}
#[derive(Args)]
pub(crate) struct Limits {
    #[arg(long,default_value_t=RunLimits::default().node_instances)]
    pub(crate) max_nodes: usize,
    #[arg(long,default_value_t=RunLimits::default().collection_items)]
    pub(crate) max_items: usize,
    #[arg(long,default_value_t=RunLimits::default().model_requests)]
    pub(crate) max_model_calls: usize,
    #[arg(long,default_value_t=RunLimits::default().concurrency)]
    pub(crate) concurrency: usize,
    #[arg(long,default_value_t=RunLimits::default().data_bytes)]
    pub(crate) max_data_bytes: usize,
    #[arg(long,default_value_t=RunLimits::default().duration.as_secs())]
    pub(crate) timeout_secs: u64,
    #[arg(long, default_value_t = 1_048_576)]
    pub(crate) max_input_bytes: usize,
    #[arg(long,default_value_t=8*1_048_576)]
    pub(crate) max_artifact_bytes: usize,
}
impl Limits {
    pub(crate) fn run(&self) -> RunLimits {
        RunLimits {
            node_instances: self.max_nodes,
            collection_items: self.max_items,
            model_requests: self.max_model_calls,
            concurrency: self.concurrency,
            data_bytes: self.max_data_bytes,
            duration: Duration::from_secs(self.timeout_secs),
        }
    }
}
#[derive(Args)]
pub(crate) struct RunArgs {
    #[command(flatten)]
    pub(crate) graph: GraphArgs,
    #[arg(long, default_value = "-")]
    pub(crate) input: PathBuf,
    #[arg(long)]
    pub(crate) typed_input: bool,
    #[arg(long)]
    pub(crate) bindings: Option<PathBuf>,
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
    #[arg(long)]
    pub(crate) trace: Option<PathBuf>,
    #[arg(long)]
    pub(crate) fail_on: Option<String>,
    #[command(flatten)]
    pub(crate) limits: Limits,
}
#[derive(Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
pub(crate) enum Partition {
    Development,
    HeldOut,
}
impl From<Partition> for sapho_evidence::Split {
    fn from(value: Partition) -> Self {
        match value {
            Partition::Development => Self::Development,
            Partition::HeldOut => Self::HeldOut,
        }
    }
}
#[derive(Args)]
pub(crate) struct MeasurementOptions {
    #[arg(long)]
    pub(crate) dataset: PathBuf,
    #[arg(long)]
    pub(crate) bindings: Option<PathBuf>,
    #[arg(long)]
    pub(crate) replay: Option<PathBuf>,
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
    #[arg(long, default_value_t = 1024)]
    pub(crate) max_cases: usize,
    #[command(flatten)]
    pub(crate) limits: Limits,
}
#[derive(Args)]
pub(crate) struct MeasureArgs {
    #[command(flatten)]
    pub(crate) graph: GraphArgs,
    #[command(flatten)]
    pub(crate) options: MeasurementOptions,
    #[arg(long, value_enum)]
    pub(crate) split: Partition,
}
#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum Metric {
    Agreement,
    Brier,
}
impl From<Metric> for sapho_evidence::Metric {
    fn from(value: Metric) -> Self {
        match value {
            Metric::Agreement => Self::Agreement,
            Metric::Brier => Self::Brier,
        }
    }
}
#[derive(Args)]
pub(crate) struct TuneArgs {
    #[arg(long, required = true)]
    pub(crate) candidate: Vec<PathBuf>,
    #[arg(long, value_enum)]
    pub(crate) format: Option<Format>,
    #[arg(long)]
    pub(crate) output_name: String,
    #[arg(long, value_enum)]
    pub(crate) metric: Metric,
    #[arg(long, default_value_t = 16)]
    pub(crate) max_candidates: usize,
    #[command(flatten)]
    pub(crate) options: MeasurementOptions,
}
#[derive(Args)]
pub(crate) struct Acquisition {
    #[arg(long, default_value = "items")]
    pub(crate) port: String,
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
    #[arg(long,default_value_t=SelectionLimits::default().files)]
    pub(crate) max_files: usize,
    #[arg(long,default_value_t=SelectionLimits::default().entries)]
    pub(crate) max_entries: usize,
    #[arg(long,default_value_t=SelectionLimits::default().file_bytes)]
    pub(crate) max_file_bytes: usize,
    #[arg(long,default_value_t=SelectionLimits::default().total_bytes)]
    pub(crate) max_total_bytes: usize,
    #[arg(long,default_value_t=SelectionLimits::default().stderr_bytes)]
    pub(crate) max_stderr_bytes: usize,
    #[arg(long,default_value_t=SelectionLimits::default().duration.as_secs())]
    pub(crate) timeout_secs: u64,
    #[arg(long,default_value_t=8*1_048_576)]
    pub(crate) max_artifact_bytes: usize,
}
impl Acquisition {
    pub(crate) fn limits(&self) -> SelectionLimits {
        SelectionLimits {
            files: self.max_files,
            entries: self.max_entries,
            file_bytes: self.max_file_bytes,
            total_bytes: self.max_total_bytes,
            stderr_bytes: self.max_stderr_bytes,
            duration: Duration::from_secs(self.timeout_secs),
        }
    }
}
#[derive(Args)]
pub(crate) struct Paths {
    #[arg(long)]
    pub(crate) root: PathBuf,
    #[arg(long, default_value = "**/*")]
    pub(crate) include: Vec<String>,
    #[arg(long)]
    pub(crate) exclude: Vec<String>,
}
impl Paths {
    pub(crate) fn patterns(&self) -> Patterns {
        Patterns {
            include: self.include.clone(),
            exclude: self.exclude.clone(),
        }
    }
}
#[derive(Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
pub(crate) enum GitComparison {
    WorkingTree,
    Staged,
    Revisions,
}
#[derive(Subcommand)]
pub(crate) enum Selector {
    /// Acquire sorted regular text files under an explicit root.
    Files {
        #[command(flatten)]
        paths: Paths,
        #[command(flatten)]
        limits: Acquisition,
    },
    /// Acquire complete tracked Git patches; requires Git 2.34+ on Unix.
    Git {
        #[command(flatten)]
        paths: Paths,
        #[command(flatten)]
        limits: Acquisition,
        #[arg(long, value_enum)]
        mode: GitComparison,
        #[arg(long)]
        base: Option<String>,
        #[arg(long)]
        head: Option<String>,
    },
    /// Project an RFC 6901 pointer under a declared ValueType schema.
    Json {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value = "")]
        pointer: String,
        #[arg(long)]
        schema: PathBuf,
        #[arg(long, value_enum)]
        format: Option<Format>,
        #[arg(long, default_value = "selected")]
        id_prefix: String,
        #[command(flatten)]
        limits: Acquisition,
    },
}
