//! Pure logic for one WASAPI loopback capture packet: mapping its buffer
//! flags to the [`Frame`] `audio-win` hands to `session`, decoding its raw
//! bytes into samples, and converting `pull`'s timeout to the millisecond
//! form `wasapi::Handle::wait_for_event` wants.
//!
//! Kept free of the `wasapi` crate — mirrors `wasapi::BufferFlags`'s three
//! fields under this crate's own name, the same reason
//! [`crate::sessions::SessionActivity`] mirrors `wasapi::SessionState` —
//! so this module runs on a CI runner with no audio device
//! (`docs/specs/spec-capture-foundation.md`, Windows-backend prior
//! decisions). [`crate::sources`] is this crate's one edge module; it calls
//! into `wasapi` and translates flags into [`PacketFlags`] before any of
//! this module's logic sees them.

use transcriber_audio::{DeviceTimestamp, Frame, GapCause};

/// Mirrors `wasapi::BufferFlags`'s three fields under this crate's own name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PacketFlags {
    /// `AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY`: samples were dropped
    /// between this packet and the previous one.
    pub(crate) data_discontinuity: bool,
    /// `AUDCLNT_BUFFERFLAGS_SILENT`: the device reports silence for this
    /// packet's span.
    pub(crate) silent: bool,
    /// `AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR`: the device declares its own
    /// chosen time base unreliable for this packet.
    pub(crate) timestamp_error: bool,
}

/// `data_discontinuity` and `timestamp_error` gaps recorded on one loopback
/// stream so far — the spec's "each cause gets its own counter" decision
/// (Windows-backend prior decisions).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct GapCounters {
    /// Gaps recorded because of `data_discontinuity`.
    pub(crate) discontinuity: u64,
    /// Gaps recorded because of `timestamp_error`.
    pub(crate) timestamp_error: u64,
}

/// Decodes raw bytes read from the device into interleaved `f32` samples,
/// four bytes at a time, native-endian — the byte layout of the 32-bit
/// float format this module's caller requests explicitly (spec's prior
/// decision: `GetMixFormat` is `E_NOTIMPL` on the loopback client, so the
/// format is never read back, only supplied).
///
/// `chunks_exact(4)` silently drops a trailing partial chunk rather than
/// panicking; the caller only ever passes a byte slice whose length WASAPI
/// itself reported as an exact multiple of 4, so no such remainder is
/// expected in practice.
pub(crate) fn bytes_to_f32_samples(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// Maps one decoded packet to the [`Frame`] the ring buffer receives,
/// choosing between a dense run of samples and an explicit, counted gap —
/// the spec's "all three `BufferFlags` handled" decision:
/// `data_discontinuity` and `timestamp_error` each become a gap on their own
/// counter, checked in that order; `silent` is materialised as zeros
/// (ignoring whatever `samples` actually held) so the sample axis stays
/// dense; anything else is passed through as ordinary samples.
///
/// `interleaved_len` is `frame_count * channels`, the unit both
/// [`Frame::silent`] and [`Frame::gap`] expect — always `samples.len()` for
/// an unflagged packet, so callers should derive it from the decoded sample
/// vector rather than recomputing it independently.
pub(crate) fn frame_for_packet(
    timestamp: DeviceTimestamp,
    interleaved_len: usize,
    samples: Vec<f32>,
    flags: PacketFlags,
    counters: &mut GapCounters,
) -> Frame {
    if flags.data_discontinuity {
        counters.discontinuity += 1;
        return Frame::gap(timestamp, interleaved_len, GapCause::Discontinuity);
    }
    if flags.timestamp_error {
        counters.timestamp_error += 1;
        return Frame::gap(timestamp, interleaved_len, GapCause::TimestampError);
    }
    if flags.silent {
        return Frame::silent(timestamp, interleaved_len);
    }
    Frame::samples(timestamp, samples)
}

/// Converts `AudioSource::pull`'s timeout to the millisecond `u32`
/// `wasapi::Handle::wait_for_event` wants, saturating instead of
/// overflowing for a timeout past `u32::MAX` ms (over 49 days) — never a
/// realistic caller value, but this keeps the conversion total rather than
/// panicking on one.
pub(crate) fn timeout_millis(timeout: std::time::Duration) -> u32 {
    u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests;
