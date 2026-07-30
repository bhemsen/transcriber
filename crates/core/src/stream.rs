//! Structural identity of a captured audio stream.

/// Which side of the conversation a stream of audio belongs to.
///
/// A session never mixes the two: the microphone and the captured
/// application each keep their own [`StreamIdentity`] end to end, from the
/// ring buffer through the transcript.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamIdentity {
    /// The user's own microphone.
    Local,
    /// The selected application's render stream — the other side.
    Remote,
}
