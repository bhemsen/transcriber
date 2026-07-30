//! The seam through which `session`'s tests and, later, `cli` (the
//! composition root, see the spec's prior decision) open streams without
//! either one knowing which backend it is talking to.

use thiserror::Error;
use transcriber_core::{CaptureSubject, StreamIdentity};

use crate::source::AudioSource;

/// Builds the two streams one session captures: the chosen application
/// (`Remote`) and the user's own microphone (`Local`).
///
/// Object-safe, so `cli` (Phase 1) and `app` (Phase 5, spec's prior
/// decision: "dieselbe Fabrik") can hold a `Box<dyn SourceFactory>` and pick
/// [`crate::TestToneSources`] or a platform backend's implementation
/// (`audio-win`'s `WindowsSources`) at runtime — `session` depends only on
/// this trait and [`AudioSource`], never on a concrete backend, which is
/// what keeps it free of `cfg(windows)`.
pub trait SourceFactory {
    /// Lists the applications currently eligible as a capture subject —
    /// what `list-sources` shows the user before the consent prompt.
    ///
    /// # Errors
    ///
    /// Returns [`SourceFactoryError::Enumeration`] if the platform backend
    /// could not enumerate candidates at all.
    fn list_subjects(&self) -> Result<Vec<CaptureSubject>, SourceFactoryError>;

    /// Opens the `Remote` stream for `subject`.
    ///
    /// The spec requires re-checking process identity by name *and start
    /// time* between `list-sources` and `capture`, so a recycled PID cannot
    /// be silently captured — but [`CaptureSubject`] does not yet carry a
    /// start time, only `process_name` and `root_pid`. A backend that needs
    /// evidence for that re-check has to source it itself (e.g. caching the
    /// start time it observed during `list_subjects` behind its own
    /// interior state) until `CaptureSubject` grows a field for it; that
    /// would be a `core` change, not a change to this trait's signature.
    ///
    /// # Errors
    ///
    /// Returns [`SourceFactoryError::Open`] if `subject` can no longer be
    /// captured — including a backend's re-check finding that the process
    /// identity changed since `subject` was listed (a recycled PID), which
    /// must fail loudly rather than silently open the wrong application.
    fn open_remote(
        &self,
        subject: &CaptureSubject,
    ) -> Result<Box<dyn AudioSource>, SourceFactoryError>;

    /// Opens the `Local` (microphone) stream.
    ///
    /// `Ok(None)` is the ordinary "no microphone present" outcome — the
    /// spec's prior decision that a missing microphone is a warning, not an
    /// abort, so the caller continues with the `Remote` stream alone.
    ///
    /// # Errors
    ///
    /// Returns [`SourceFactoryError::Open`] only for a real failure opening
    /// a microphone that IS present — never for its absence.
    fn open_local(&self) -> Result<Option<Box<dyn AudioSource>>, SourceFactoryError>;
}

/// Failure building or opening a stream through a [`SourceFactory`].
#[derive(Debug, Error)]
pub enum SourceFactoryError {
    /// Enumerating capture subjects failed.
    #[error("failed to list capture subjects: {reason}")]
    Enumeration {
        /// What the platform backend reported.
        reason: String,
    },
    /// Opening a source failed for a reason other than a simply-absent
    /// microphone — [`SourceFactory::open_local`]'s ordinary outcome for
    /// that case is `Ok(None)`, never this variant.
    #[error("failed to open the {identity:?} source: {reason}")]
    Open {
        /// Which stream failed to open.
        identity: StreamIdentity,
        /// What the platform backend reported.
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::SourceFactoryError;
    use transcriber_core::StreamIdentity;

    #[test]
    fn enumeration_error_display_carries_the_reported_reason() {
        let error = SourceFactoryError::Enumeration {
            reason: "no render devices".to_string(),
        };
        assert!(error.to_string().contains("no render devices"));
    }

    #[test]
    fn open_error_display_names_the_failed_identity() {
        let error = SourceFactoryError::Open {
            identity: StreamIdentity::Local,
            reason: "access denied".to_string(),
        };
        let rendered = error.to_string();
        assert!(rendered.contains("Local"));
        assert!(rendered.contains("access denied"));
    }
}
