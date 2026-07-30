//! PCM frame and format types, and the fixed-size, multi-reader ring buffer
//! that holds one stream's audio in memory.
//!
//! This crate has no I/O and no HTTP client: nothing here writes to disk or
//! reaches the network (`docs/constitution.md`, Architecture principles).
//! What arrives here is exactly what a capture backend measured, copied into
//! a buffer whose capacity never changes for the lifetime of the stream.
//!
//! Deliberately **not** here yet: the `AudioSource` / `SourceFactory` traits,
//! the synthetic test-tone source, and downmix/resampling. Those land in
//! later issues of the same phase and depend on the types this crate
//! exports — see `docs/specs/spec-capture-foundation.md`.

mod format;
mod frame;
mod ring_buffer;

pub use format::{FormatError, StreamFormat};
pub use frame::{DeviceTimestamp, Frame, GapCause};
pub use ring_buffer::{RING_BUFFER_CAPACITY_SECONDS, ReaderId, RingBuffer};
