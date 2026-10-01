// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Sapho command process: synchronous acquisition/persistence around bounded async evaluation.
mod args;
mod command;
use clap::Parser;
use sapho_cli::CliError;
use sapho_core::bounded_json;
use std::{io::Write, process::ExitCode};
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
            if std::io::stdout()
                .lock()
                .write_all(&response.bytes)
                .and_then(|()| std::io::stdout().lock().write_all(b"\n"))
                .is_err()
            {
                return ExitCode::from(2);
            }
            ExitCode::from(response.exit.code())
        }
        Err(error) => failure(error),
    }
}
fn failure(error: CliError) -> ExitCode {
    let _ = writeln!(std::io::stderr().lock(), "{error}");
    #[derive(serde::Serialize)]
    struct Failure<'a> {
        error: &'a CliError,
    }
    if let Ok(bytes) = bounded_json(&Failure { error: &error }, 8 * 1_048_576) {
        let _ = std::io::stdout().lock().write_all(&bytes);
        let _ = std::io::stdout().lock().write_all(b"\n");
    }
    ExitCode::from(2)
}
