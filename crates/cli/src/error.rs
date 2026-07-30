//! Typed failures for the `cli` binary.

use thiserror::Error;
use transcriber_audio::SourceFactoryError;
use transcriber_session::SessionError;

/// Everything that can make `list-sources` or `capture` fail.
#[derive(Debug, Error)]
pub(crate) enum CliError {
    /// Listing or opening a source failed — see [`SourceFactoryError`] for
    /// which of the two, and why.
    #[error(transparent)]
    Source(#[from] SourceFactoryError),
    /// `capture --pid <PID>` named a PID with no currently active render
    /// stream — either it was never one, or its render session ended since
    /// the last `list-sources`.
    #[error("no application with an active render stream at PID {pid}; run list-sources again")]
    SubjectNotFound {
        /// The PID that was searched for.
        pid: u32,
    },
    /// Starting the session itself failed — see [`SessionError`].
    #[error(transparent)]
    Session(#[from] SessionError),
}
