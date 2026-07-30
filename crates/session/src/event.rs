//! What a running [`crate::Session`] publishes on its event bus.

use transcriber_audio::SourceDegradation;
use transcriber_core::{SessionState, StreamIdentity};

/// One notification a [`crate::Session`] publishes on its
/// `tokio::sync::broadcast` bus (`docs/specs/spec-capture-foundation.md`,
/// prior decisions: feature `sync` only, no runtime entered on the capture
/// path — [`tokio::sync::broadcast::Sender::send`] works without one).
///
/// `Clone` because `broadcast` hands every subscriber its own copy.
#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// The session moved to a new [`SessionState`].
    StateChanged(SessionState),
    /// [`crate::Session::start`] found no microphone — a warning, not an
    /// abort; the session continues with the `Remote` stream alone.
    MicrophoneUnavailable,
    /// A stream started under a disclosed capability degradation, e.g. the
    /// platform could not enable echo cancellation.
    StreamDegraded {
        /// Which stream is degraded.
        identity: StreamIdentity,
        /// What capability is missing.
        degradation: SourceDegradation,
    },
    /// A stream's source reported [`transcriber_audio::AudioSourceEvent::Ended`]
    /// — its capture thread has stopped and will produce no further frames.
    StreamEnded {
        /// Which stream ended.
        identity: StreamIdentity,
    },
    /// A stream's source failed with an unclassified error; its capture
    /// thread has stopped.
    StreamFailed {
        /// Which stream failed.
        identity: StreamIdentity,
        /// What the source reported.
        reason: String,
    },
}
