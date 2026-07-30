//! `transcriber-cli` — the phase 1-4 harness and composition root
//! (`docs/specs/spec-capture-foundation.md`): `list-sources` and `capture`
//! with the consent prompt and per-stream statistics. Binds
//! [`transcriber_audio_win::WindowsSources`] directly — the cfg-free
//! backend-selection layer arrives with the second platform backend, not
//! here (spec, Out of scope).
//!
//! Writes no audio and no file of any kind: every subcommand only ever
//! writes to stdout/stderr.

mod capture_command;
mod cli;
mod consent;
mod error;
mod language;
mod list_sources_command;
mod stats_display;
mod text;

use std::process::ExitCode;

use clap::Parser;
use transcriber_audio_win::WindowsSources;

use cli::{Cli, Command};
use error::CliError;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let factory = WindowsSources::new();

    let result = match cli.command {
        Command::ListSources { lang } => {
            list_sources_command::run(&factory, lang, &mut std::io::stdout())
        }
        Command::Capture { pid, lang } => capture_command::run(&factory, pid, lang),
    };

    report(result)
}

/// Prints an error, if any, and maps it to a process exit code.
fn report(result: Result<(), CliError>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
