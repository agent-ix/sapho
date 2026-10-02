// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Sapho command process: synchronous acquisition/persistence around bounded async evaluation.
mod args;
mod command;
use clap::Parser;
use ix_cli_kit::{
    Outcome,
    streams::{write_primary_stdout, write_result},
};
use sapho_cli::CliError;
use sapho_core::bounded_json;
use std::process::ExitCode;
fn main() -> ExitCode {
    let cli = match args::Cli::try_parse() {
        Ok(cli) => cli,
        Err(error)
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            let _ = error.print();
            return ExitCode::SUCCESS;
        }
        Err(error) => return failure(CliError::Arguments(error.to_string())),
    };
    match command::execute(cli) {
        Ok(response) => {
            if write_primary_stdout(&response.bytes)
                .and_then(|()| write_primary_stdout(b"\n"))
                .is_err()
            {
                return Outcome::Refused.into();
            }
            ExitCode::from(response.exit.code())
        }
        Err(error) => failure(error),
    }
}
fn failure(error: CliError) -> ExitCode {
    let _ = write_result(&mut std::io::stderr().lock(), &error.to_string());
    #[derive(serde::Serialize)]
    struct Failure<'a> {
        error: &'a CliError,
    }
    if let Ok(bytes) = bounded_json(&Failure { error: &error }, 8 * 1_048_576) {
        let _ = write_primary_stdout(&bytes);
        let _ = write_primary_stdout(b"\n");
    }
    Outcome::Refused.into()
}
