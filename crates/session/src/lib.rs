//! The session state machine and orchestrator.
//!
//! [`Session::start`] is the *only* way to obtain a [`Session`] and it takes
//! a [`transcriber_core::ConsentAttestation`] as its first argument — there
//! is no other constructor, no `Default`, no public field and nothing
//! `#[cfg(test)]`-gated that skips it. That is the whole point: the vision
//! (`docs/vision.md`) demands the recording *demonstrably* never starts
//! without a confirmed attestation, and `tests/compile_fail` proves it as a
//! permanent gate rather than a review judgement — see
//! `docs/specs/spec-capture-foundation.md`.
//!
//! `Session` owns the capture threads and the fixed-size ring buffers: for
//! each stream it spawns a thread that loops [`transcriber_audio::AudioSource::pull`]
//! and pushes frames into that stream's buffer, treating
//! [`transcriber_audio::AudioSourceEvent::Idle`] as "nothing right now" and
//! [`transcriber_audio::AudioSourceEvent::Ended`] as stream end. Sources
//! themselves are opened elsewhere (`audio-win`'s `WindowsSources`, or
//! [`transcriber_audio::TestToneSources`] for every test here — this crate
//! never depends on `audio-win` and contains no `cfg(windows)`).
//!
//! States: `Capturing -> Stopping -> Ended`, two edges, `Ended` terminal —
//! see [`transcriber_core::SessionState`], re-exported here since it is
//! [`Session::state`]'s return type. There is deliberately no `Idle`
//! variant: "idle" is the absence of a `Session` value. A missing
//! microphone is a warning, not an abort, and is carried as the *attribute*
//! [`Session::has_local_stream`], not a state.
//!
//! The event bus is `tokio::sync::broadcast` with feature `sync` only — no
//! tokio runtime is entered anywhere on the capture path, so the audio
//! threads stay fully synchronous.

mod capture;
mod clock;
mod error;
mod event;
mod plan;
mod session;

pub use error::SessionError;
pub use event::SessionEvent;
pub use plan::CapturePlan;
pub use session::Session;
pub use transcriber_core::SessionState;
