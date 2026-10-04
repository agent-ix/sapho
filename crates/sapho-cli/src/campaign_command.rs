// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Thin campaign commands around the reusable host and native graph adapter.
use clap::Subcommand;
use sapho_campaign::{
    adapter::{CampaignAdapter, Execution, ExecutionOutcome, GraphAdapter, GraphJob},
    control::{self, Control},
    execution::{Provenance, Session},
    lifecycle::{AttemptId, AttemptState, Campaign, Job},
    runner::{self, Capture},
    storage::{DEFAULT_ARTIFACT_BYTES, read_file},
};
use sapho_cli::{CliError, ExitStatus};
use std::{collections::BTreeMap, path::PathBuf};
#[derive(clap::Args)]
pub(crate) struct Args {
    #[arg(long)]
    state: PathBuf,
    #[command(subcommand)]
    command: Action,
}
#[derive(Subcommand)]
enum Action {
    /// Initialize a new local campaign ledger.
    Init,
    /// Admit one strict versioned graph job envelope.
    Import {
        #[arg(long)]
        job: PathBuf,
    },
    /// Run pending graph stages without automatic retries.
    Run {
        #[arg(long,conflicts_with_all=["bindings","recording"])]
        ollama_url: Option<String>,
        #[arg(long, requires = "ollama_url")]
        model: Option<String>,
        #[arg(long)]
        bindings: Option<PathBuf>,
        #[arg(long)]
        recording: Option<PathBuf>,
        #[arg(long, default_value_t = 32768)]
        context_tokens: usize,
        #[arg(long, default_value_t = 4096)]
        output_tokens: usize,
        #[arg(long)]
        no_thinking: bool,
    },
    /// Report counts independently of domain correctness.
    Status,
    /// Attach a dashboard; detach leaves a headless worker alone.
    Tui,
    /// Queue an operator pause at a safe boundary.
    Pause,
    /// Queue resume at a safe boundary.
    Resume,
    /// Explicitly authorize one retained failed stage attempt.
    Retry {
        #[arg(long)]
        attempt: i64,
        #[arg(long)]
        reason: String,
    },
    /// Freeze known evidence references into a new bundle directory.
    Export {
        #[arg(long)]
        output: PathBuf,
    },
    /// Copy a stock graph campaign into a new verified state directory.
    Migrate {
        #[arg(long)]
        output: PathBuf,
    },
    /// Verify exact evidence references; performs no inference.
    Doctor,
}
fn output(
    value: &impl serde::Serialize,
    exit: ExitStatus,
) -> Result<crate::command::Response, CliError> {
    Ok(crate::command::Response {
        bytes: sapho_core::bounded_json(value, DEFAULT_ARTIFACT_BYTES)?,
        exit,
    })
}
fn parse_job(path: &std::path::Path) -> Result<Job, CliError> {
    Ok(sapho_core::decode_json(
        &read_file(path, DEFAULT_ARTIFACT_BYTES)?,
        DEFAULT_ARTIFACT_BYTES,
    )?)
}
fn report_error(e: impl std::fmt::Display) -> sapho_campaign::Error {
    sapho_campaign::Error::new(sapho_campaign::ErrorCode::Refused, e.to_string())
}
pub(crate) fn execute(args: Args) -> Result<crate::command::Response, CliError> {
    let adapter = GraphAdapter;
    match args.command {
        Action::Init => {
            std::fs::create_dir_all(&args.state).map_err(|source| CliError::Io {
                path: args.state.clone(),
                source,
            })?;
            let c = Campaign::open(&args.state, true)?;
            output(&c.snapshot()?, ExitStatus::Completed)
        }
        Action::Import { job } => {
            let job = parse_job(&job)?;
            adapter.admit(&job)?;
            let mut c = Campaign::open(&args.state, true)?;
            let admitted = c.admit(&job)?;
            output(
                &serde_json::json!({"admitted":admitted,"job":job.id}),
                ExitStatus::Completed,
            )
        }
        Action::Status => output(
            &Campaign::open(&args.state, false)?.snapshot()?,
            ExitStatus::Completed,
        ),
        Action::Migrate {
            output: destination,
        } => {
            let source = Campaign::open(&args.state, false)?;
            for id in source.jobs()? {
                adapter.admit(&source.job(&id)?)?;
            }
            let mut query=source.ledger().connection().prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%'").map_err(sapho_campaign::Error::from)?;
            let tables = query
                .query_map([], |r| r.get::<_, String>(0))
                .map_err(sapho_campaign::Error::from)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(sapho_campaign::Error::from)?;
            if tables.iter().any(|name| {
                ![
                    "events",
                    "campaign_jobs",
                    "campaign_attempts",
                    "campaign_control",
                ]
                .contains(&name.as_str())
            }) {
                return Err(report_error(
                    "stock migration refuses domain extensions; use the owning adapter migration",
                )
                .into());
            }
            let target = sapho_campaign::migration::migrate(&source, &destination, |_, _| Ok(()))?;
            output(
                &serde_json::json!({"migrated":true,"verified":true,"snapshot":target.snapshot()?}),
                ExitStatus::Completed,
            )
        }
        Action::Doctor => {
            let c = Campaign::open(&args.state, false)?;
            c.verify()?;
            output(
                &serde_json::json!({"evidence_verified":true,"snapshot":c.snapshot()?}),
                ExitStatus::Completed,
            )
        }
        Action::Export {
            output: destination,
        } => {
            let c = Campaign::open(&args.state, false)?;
            let hash = c.export(&destination)?;
            output(
                &serde_json::json!({"manifest_sha256":hash,"publication":"staged_only"}),
                ExitStatus::Completed,
            )
        }
        Action::Pause | Action::Resume => {
            let paused = matches!(args.command, Action::Pause);
            let c = Campaign::open(&args.state, false)?;
            let id = control::submit(
                &args.state,
                &Control::Pause {
                    paused,
                    revision: c.snapshot()?.control_revision,
                },
            )?;
            output(
                &serde_json::json!({"queued":id,"applied":false}),
                ExitStatus::Completed,
            )
        }
        Action::Retry { attempt, reason } => {
            let c = Campaign::open(&args.state, false)?;
            let attempt = AttemptId::new(attempt)?;
            let expected = c.attempt(attempt)?.state;
            let id = control::submit(
                &args.state,
                &Control::Retry {
                    attempt,
                    expected,
                    reason,
                },
            )?;
            output(
                &serde_json::json!({"queued":id,"applied":false}),
                ExitStatus::Completed,
            )
        }
        Action::Tui => {
            sapho_campaign_tui::attach(
                || {
                    let c = Campaign::open(&args.state, false)?;
                    Ok(sapho_campaign_tui::View {
                        campaign: c.snapshot()?,
                        domain: adapter.snapshot(c.ledger())?,
                        attempts: c.attempts()?,
                    })
                },
                |control| control::submit(&args.state, control),
            )?;
            output(&serde_json::json!({"detached":true}), ExitStatus::Completed)
        }
        Action::Run {
            ollama_url,
            model,
            bindings,
            recording,
            context_tokens,
            output_tokens,
            no_thinking,
        } => {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .map_err(CliError::Runtime)?;
            let mut c = Campaign::open(&args.state, true)?;
            c.verify()?;
            c.recover()?;
            control::drain(&mut c)?;
            for id in c.jobs()? {
                let job = c.job(&id)?;
                adapter.admit(&job)?;
                for stage in adapter.stages(&job, c.ledger())? {
                    control::drain(&mut c)?;
                    if c.snapshot()?.paused {
                        break;
                    }
                    if let Some(previous) = c
                        .attempts()?
                        .into_iter()
                        .rev()
                        .find(|a| a.job == id && a.stage == stage)
                    {
                        if previous.state == AttemptState::Executed {
                            runner::run_stage(
                                &mut c,
                                &adapter,
                                &id,
                                &stage,
                                &Default::default(),
                                |_| Err(report_error("recovered sealing must not dispatch")),
                            )?;
                            continue;
                        }
                        if c.retry_authorization(previous.id)?.is_none() {
                            continue;
                        }
                    }
                    let payload: GraphJob = serde_json::from_value(job.payload.clone())
                        .map_err(|e| CliError::Arguments(e.to_string()))?;
                    let required =
                        sapho_cli::inspect(&payload.graph, &Default::default())?.backends;
                    let metadata = if let Some(path) = &bindings {
                        let format = sapho_cli::select_format(path, None)?;
                        let bytes = read_file(path, 1_048_576)?;
                        sapho_graph::parse_config::<sapho_cli::Bindings>(
                            std::str::from_utf8(&bytes)
                                .map_err(|e| CliError::Arguments(e.to_string()))?,
                            format,
                        )?
                    } else {
                        BTreeMap::new()
                    };
                    let mut provenance = Provenance::default();
                    #[cfg(feature = "ollama")]
                    let mut provider = None;
                    let registry = if let Some(path) = &recording {
                        let bytes = read_file(path, 16 * 1_048_576)?;
                        let record = sapho_recording::Recording::from_json(&bytes, 16 * 1_048_576)?;
                        provenance.backends.insert("replay".into(),serde_json::json!({"recording_sha256":sapho_campaign::storage::digest(&bytes)}));
                        sapho_cli::replay_bindings(
                            &record,
                            if metadata.is_empty() {
                                None
                            } else {
                                Some(&metadata)
                            },
                            16 * 1_048_576,
                        )?
                    } else if let Some(url) = &ollama_url {
                        #[cfg(feature = "ollama")]
                        {
                            let model = model.as_deref().ok_or_else(|| {
                                CliError::Arguments("--model is required with --ollama-url".into())
                            })?;
                            let local = std::sync::Arc::new(sapho_ollama::OllamaBackend::new(
                                url,
                                sapho_ollama::Limits {
                                    context_tokens,
                                    output_tokens,
                                    think: if no_thinking { Some(false) } else { None },
                                    ..Default::default()
                                },
                            )?);
                            let info = runtime.block_on(local.inspect(model))?;
                            if !info.completion {
                                return Err(CliError::Arguments(
                                    "model lacks completion capability".into(),
                                ));
                            }
                            let mut registry = sapho_core::BackendRegistry::default();
                            for name in &required {
                                provenance.backends.insert(name.as_str().into(),serde_json::json!({"provider":"ollama","model":info,"context_tokens":context_tokens,"output_tokens":output_tokens,"thinking":if no_thinking{Some(false)}else{None}}));
                                registry.register(
                                    name.clone(),
                                    sapho_core::BackendBinding {
                                        backend: local.clone(),
                                        model: model.into(),
                                        expected_model: None,
                                        distribution_policy:
                                            sapho_core::DistributionPolicy::Strict {},
                                    },
                                )?;
                            }
                            provider = Some(local);
                            registry
                        }
                        #[cfg(not(feature = "ollama"))]
                        {
                            let _ = (url, model, context_tokens, output_tokens, no_thinking);
                            return Err(CliError::Arguments("Ollama feature not compiled".into()));
                        }
                    } else {
                        for (name, binding) in &metadata {
                            provenance.backends.insert(
                                name.as_str().into(),
                                serde_json::to_value(binding)
                                    .map_err(|e| CliError::Arguments(e.to_string()))?,
                            );
                        }
                        sapho_cli::live_bindings(&required, &metadata)?
                    };
                    let session = Session::record(&required, &registry, 16 * 1_048_576)?;
                    runner::run_stage(&mut c, &adapter, &id, &stage, &provenance, |request| {
                        let result =
                            runtime.block_on(adapter.execute(request, session.bindings.clone()));
                        let mut captures = session.capture()?;
                        #[cfg(feature = "ollama")]
                        if let Some(provider) = &provider {
                            for (ordinal, receipt) in provider
                                .receipts()
                                .map_err(report_error)?
                                .into_iter()
                                .enumerate()
                            {
                                captures.push(Capture {
                                    kind: format!("raw_provider_request:{ordinal}"),
                                    bytes: receipt.request.clone(),
                                });
                                captures.push(Capture {
                                    kind: format!("raw_provider_response:{ordinal}"),
                                    bytes: receipt.response.clone(),
                                });
                                captures.push(Capture{kind:format!("provider_call_receipt:{ordinal}"),bytes:sapho_core::bounded_json(&serde_json::json!({"ordinal":ordinal,"made_call":receipt.made_call,"status":receipt.status,"request_sha256":sapho_campaign::storage::digest(&receipt.request),"response_sha256":sapho_campaign::storage::digest(&receipt.response),"confidence_evidence":receipt.confidence_evidence}),4096).map_err(report_error)?});
                            }
                        }
                        let execution = match result {
                            Ok(run) => run,
                            Err(error) => Execution {
                                outcome: ExecutionOutcome::Failed,
                                evidence: sapho_core::bounded_json(
                                    &serde_json::json!({"schema":1,"error":error}),
                                    4096,
                                )
                                .map_err(report_error)?,
                            },
                        };
                        Ok((execution, captures))
                    })?;
                }
            }
            control::drain(&mut c)?;
            let s = c.snapshot()?;
            let partial = s.paused
                || s.attempts
                    .keys()
                    .any(|state| *state != AttemptState::Completed);
            output(
                &s,
                if partial {
                    ExitStatus::Partial
                } else {
                    ExitStatus::Completed
                },
            )
        }
    }
}
