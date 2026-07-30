//! Typed failures for [`crate::Session`].

use thiserror::Error;
use transcriber_core::{SessionStateError, StreamIdentity};

/// Everything that can go wrong operating a [`crate::Session`].
#[derive(Debug, Error)]
pub enum SessionError {
    /// An invalid state transition — see [`SessionStateError`]. The state
    /// machine has exactly two edges and `Ended` is terminal, so this is
    /// what a second `request_stop` or `end` (or `end` before
    /// `request_stop`) returns instead of panicking.
    #[error(transparent)]
    State(#[from] SessionStateError),
    /// The operating system refused to start a stream's capture thread.
    #[error("failed to spawn the {identity:?} capture thread: {reason}")]
    ThreadSpawn {
        /// Which stream's thread failed to start.
        identity: StreamIdentity,
        /// What the OS reported.
        reason: String,
    },
    /// A stream's capture thread panicked instead of returning normally.
    #[error("the {identity:?} capture thread panicked")]
    CaptureThreadPanicked {
        /// Which stream's thread panicked.
        identity: StreamIdentity,
    },
}

#[cfg(test)]
mod tests {
    use super::SessionError;
    use transcriber_core::StreamIdentity;

    #[test]
    fn thread_spawn_error_names_the_identity_and_reason() {
        let error = SessionError::ThreadSpawn {
            identity: StreamIdentity::Remote,
            reason: "out of system resources".to_string(),
        };
        let rendered = error.to_string();
        assert!(rendered.contains("Remote"));
        assert!(rendered.contains("out of system resources"));
    }

    #[test]
    fn panicked_error_names_the_identity() {
        let error = SessionError::CaptureThreadPanicked {
            identity: StreamIdentity::Local,
        };
        assert!(error.to_string().contains("Local"));
    }
}
