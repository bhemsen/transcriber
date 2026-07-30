//! `list-sources`: every application with an active render stream, its
//! process name and the resolved process-tree root PID that `capture
//! --pid <PID>` would target.

use std::io::Write;

use transcriber_audio::SourceFactory;

use crate::error::CliError;
use crate::language::Language;
use crate::text::{list_sources_empty, list_sources_header};

/// Lists every currently eligible capture subject to `output`.
pub(crate) fn run(
    factory: &dyn SourceFactory,
    lang: Language,
    output: &mut impl Write,
) -> Result<(), CliError> {
    let subjects = factory.list_subjects()?;
    let _ = writeln!(output, "{}", list_sources_header(lang));
    if subjects.is_empty() {
        let _ = writeln!(output, "{}", list_sources_empty(lang));
        return Ok(());
    }
    for subject in &subjects {
        let _ = writeln!(
            output,
            "  PID {:<8} {}",
            subject.root_pid(),
            subject.process_name()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests;
