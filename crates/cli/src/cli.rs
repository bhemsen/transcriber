//! The command-line surface: `list-sources` and `capture`, both taking the
//! shared `--lang` flag — see [`crate::language::Language`].

use clap::{Parser, Subcommand};

use crate::language::Language;

/// The phase 1-4 capture harness: list applications with an active render
/// stream, then capture one after an explicit consent confirmation.
#[derive(Debug, Parser)]
#[command(name = "transcriber-cli", version)]
pub(crate) struct Cli {
    /// Which subcommand to run.
    #[command(subcommand)]
    pub(crate) command: Command,
}

/// The two entry points this harness offers.
#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// List applications with an active render stream and the resolved
    /// process-tree root PID that would be captured for each.
    ListSources {
        /// Output language.
        #[arg(long, value_enum, default_value_t = Language::De)]
        lang: Language,
    },
    /// Capture one application's audio (and, if present, the microphone)
    /// after an explicit consent confirmation.
    Capture {
        /// The resolved root PID shown by `list-sources` for the
        /// application to capture.
        #[arg(long)]
        pid: u32,
        /// Output language.
        #[arg(long, value_enum, default_value_t = Language::De)]
        lang: Language,
    },
}
