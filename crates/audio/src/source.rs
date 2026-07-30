//! The `AudioSource` seam: what a capture backend hands `session` instead of
//! `session` ever seeing a platform type — see the spec's prior decision on
//! `SourceFactory`/`AudioSource` in `docs/specs/spec-capture-foundation.md`.

use std::time::Duration;

use thiserror::Error;
use transcriber_core::StreamIdentity;

use crate::format::StreamFormat;
use crate::frame::Frame;

/// One outcome of [`AudioSource::pull`].
///
/// Keeps three states a capture backend must tell apart from one another —
/// "here is data", "nothing right now", and "this will never produce data
/// again" — as one ordinary return value instead of forcing "nothing right
/// now" to masquerade as an error. That distinction is not cosmetic: the
/// Windows backend's own event-wait timeout *is* "nothing right now" (the
/// spec's prior decision — a conversation pause, not a fault), and its
/// loopback client detects stream end only via a read error, which still
/// must not propagate as an [`AudioSourceError`] once translated here, or
/// every silent pause would look identical to a fatal failure to the caller.
#[derive(Debug)]
pub enum AudioSourceEvent {
    /// A frame of samples or an explicit, counted gap — see [`Frame`].
    Frame(Frame),
    /// Nothing was available before `pull`'s timeout elapsed. Not an error:
    /// the source is still live and a later `pull` may return data again.
    Idle,
    /// The stream has ended and will never produce another [`Frame`].
    /// Terminal — callers should stop calling `pull` on this source.
    Ended,
}

/// A capability a source could not provide, disclosed instead of treated as
/// a hard failure.
///
/// Deliberately platform-agnostic: `audio` has no `cfg(windows)`, so this
/// cannot name a Windows API. Today's only cause is the Windows backend's
/// `is_aec_supported()` probe coming back `false` (spec's prior decision:
/// warn and continue rather than refuse to start), but the type itself
/// carries no Windows vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceDegradation {
    /// The platform could not enable echo cancellation for this source.
    /// Capture continues; on speakers, the other side may bleed into this
    /// stream.
    EchoCancellationUnavailable,
}

/// A real failure pulling from a source — anything [`AudioSourceEvent::Idle`]
/// and [`AudioSourceEvent::Ended`] do not already cover.
///
/// The Windows backend's prior decisions (spec) translate almost every
/// condition a naive implementation would treat as fatal into one of those
/// two ordinary outcomes instead: an event-wait timeout is `Idle`, and the
/// loopback client's read error *is* how stream end is detected, so it
/// becomes `Ended`, not this. This variant is for what is left over — an
/// unexpected, unclassified platform failure.
#[derive(Debug, Error)]
pub enum AudioSourceError {
    /// The platform backend reported a failure `pull` cannot classify as
    /// `Idle` or `Ended`.
    #[error("audio source failed: {reason}")]
    Failed {
        /// What the platform backend reported.
        reason: String,
    },
}

/// One stream of PCM audio: the user's microphone, or the selected
/// application's render stream.
///
/// `Send` because a dedicated capture thread owns one of these end to end
/// (`docs/architecture.md`, Boundaries: "WASAPI-Capture läuft ... auf einem
/// eigenen Thread je Quelle") — `session` (a later issue) moves a
/// `Box<dyn AudioSource>` onto that thread, which requires `Send` on the
/// trait object. Object-safe by construction: every method takes `&self` or
/// `&mut self` and returns a concrete or already-boxed type, so
/// `Box<dyn AudioSource>` — what [`crate::SourceFactory`] hands back — is a
/// valid type.
pub trait AudioSource: Send {
    /// Which side of the conversation this stream belongs to.
    fn identity(&self) -> StreamIdentity;

    /// The format every frame this source produces is shaped as.
    fn format(&self) -> StreamFormat;

    /// Blocks up to `timeout` for the next frame.
    ///
    /// A dedicated capture thread is the natural caller: it loops on this,
    /// pushing [`AudioSourceEvent::Frame`] payloads into a
    /// [`crate::RingBuffer`] and continuing past [`AudioSourceEvent::Idle`]
    /// without treating it as a problem. [`AudioSourceEvent::Ended`] means no
    /// further call will ever return data again.
    ///
    /// # Errors
    ///
    /// Returns [`AudioSourceError`] only for a failure that is neither
    /// ordinary silence nor a detected stream end — see that type's
    /// documentation.
    fn pull(&mut self, timeout: Duration) -> Result<AudioSourceEvent, AudioSourceError>;

    /// Capability degradations this source is currently operating under, if
    /// any. Sources that never degrade — including the test tone — keep the
    /// default `None`.
    fn degradation(&self) -> Option<SourceDegradation> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{AudioSourceError, SourceDegradation};

    #[test]
    fn error_display_carries_the_reported_reason() {
        let error = AudioSourceError::Failed {
            reason: "device invalidated mid-read".to_string(),
        };
        assert!(error.to_string().contains("device invalidated mid-read"));
    }

    #[test]
    fn degradation_variants_are_distinguishable() {
        assert_eq!(
            SourceDegradation::EchoCancellationUnavailable,
            SourceDegradation::EchoCancellationUnavailable
        );
    }
}
