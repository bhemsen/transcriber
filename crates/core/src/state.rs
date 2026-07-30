//! The session state machine: `Capturing -> Stopping -> Ended`.

use crate::error::SessionStateError;

/// Where a running session is in its lifecycle.
///
/// Deliberately has no `Idle` variant: a session that has not started simply
/// does not exist as a value yet — [`crate::ConsentAttestation`] gates the
/// only constructor of `Session` in the `session` crate. "Idle" is the
/// absence of a `Session`, not a state one can observe here. There are
/// exactly two edges, and `Ended` is terminal — a session is never restarted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionState {
    /// Actively capturing audio.
    Capturing,
    /// Capture has been asked to stop; buffers are being drained and zeroed.
    Stopping,
    /// The session has finished. Terminal — there is no edge out of it.
    Ended,
}

impl SessionState {
    /// Moves from `Capturing` to `Stopping`.
    ///
    /// # Errors
    ///
    /// Returns [`SessionStateError::NotCapturing`] if called on any other
    /// state.
    pub fn request_stop(self) -> Result<Self, SessionStateError> {
        match self {
            Self::Capturing => Ok(Self::Stopping),
            other => Err(SessionStateError::NotCapturing(other)),
        }
    }

    /// Moves from `Stopping` to `Ended`.
    ///
    /// # Errors
    ///
    /// Returns [`SessionStateError::NotStopping`] if called on any other
    /// state.
    pub fn end(self) -> Result<Self, SessionStateError> {
        match self {
            Self::Stopping => Ok(Self::Ended),
            other => Err(SessionStateError::NotStopping(other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SessionState;
    use crate::error::SessionStateError;

    #[test]
    fn capturing_stops_into_stopping() {
        assert_eq!(
            SessionState::Capturing.request_stop(),
            Ok(SessionState::Stopping)
        );
    }

    #[test]
    fn stopping_ends_into_ended() {
        assert_eq!(SessionState::Stopping.end(), Ok(SessionState::Ended));
    }

    #[test]
    fn ended_is_terminal() {
        assert_eq!(
            SessionState::Ended.request_stop(),
            Err(SessionStateError::NotCapturing(SessionState::Ended))
        );
        assert_eq!(
            SessionState::Ended.end(),
            Err(SessionStateError::NotStopping(SessionState::Ended))
        );
    }

    #[test]
    fn stopping_cannot_stop_again() {
        assert_eq!(
            SessionState::Stopping.request_stop(),
            Err(SessionStateError::NotCapturing(SessionState::Stopping))
        );
    }

    #[test]
    fn capturing_cannot_end_directly() {
        assert_eq!(
            SessionState::Capturing.end(),
            Err(SessionStateError::NotStopping(SessionState::Capturing))
        );
    }
}
