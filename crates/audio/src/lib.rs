//! PCM frame and format types, and the fixed-size, multi-reader ring buffer
//! that holds one stream's audio in memory.
//!
//! This crate has no I/O and no HTTP client: nothing here writes to disk or
//! reaches the network (`docs/constitution.md`, Architecture principles).
//! What arrives here is exactly what a capture backend measured, copied into
//! a buffer whose capacity never changes for the lifetime of the stream.
//!
//! Also here: downmix and resampling to 16 kHz mono on the read side (see
//! [`StreamResampler`]); the `AudioSource` / `SourceFactory` traits every
//! platform backend opens its streams through, so `session` never sees a
//! concrete backend type; and [`TestToneSource`] / [`TestToneSources`], the
//! synthetic debug path `docs/constitution.md` requires and the only way
//! anything in this phase runs on a CI runner with no audio device — see
//! `docs/specs/spec-capture-foundation.md`.

mod factory;
mod format;
mod frame;
mod resample;
mod ring_buffer;
mod source;
mod test_tone;

pub use factory::{SourceFactory, SourceFactoryError};
pub use format::{FormatError, StreamFormat};
pub use frame::{DeviceTimestamp, Frame, GapCause};
pub use resample::{ResampleError, StreamResampler, TARGET_SAMPLE_RATE_HZ};
pub use ring_buffer::{RING_BUFFER_CAPACITY_SECONDS, ReaderId, RingBuffer};
pub use source::{AudioSource, AudioSourceError, AudioSourceEvent, SourceDegradation};
pub use test_tone::{TestToneSource, TestToneSources};
