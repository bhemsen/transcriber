//! Typed errors for the domain types in this crate.

use thiserror::Error;

use crate::state::SessionState;

/// An invalid attempt to move a [`SessionState`] along an edge it does not
/// have.
///
/// The state machine has exactly two edges — `Capturing -> Stopping` and
/// `Stopping -> Ended` — and `Ended` is terminal; every other request is one
/// of these variants rather than a panic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SessionStateError {
    /// Only a `Capturing` session can move to `Stopping`.
    #[error("cannot stop a session in state {0:?}; only Capturing can move to Stopping")]
    NotCapturing(SessionState),
    /// Only a `Stopping` session can move to `Ended`.
    #[error("cannot end a session in state {0:?}; only Stopping can move to Ended")]
    NotStopping(SessionState),
}
