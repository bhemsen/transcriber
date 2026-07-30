//! Fixed-size, multi-reader ring buffer for one stream's PCM samples.
//!
//! Allocated once and never grown — the property `docs/constitution.md`
//! requires and the risk this phase's spec calls out by name: the WASAPI
//! reference example works around an unusable `get_buffer_size` by growing a
//! `VecDeque` without bound, which is exactly the anti-pattern this type
//! exists to prevent.

use zeroize::Zeroizing;

use crate::format::StreamFormat;
use crate::frame::{Frame, GapCause};

/// Per-stream capacity, in seconds of audio.
///
/// Fixed at the constitution's ceiling (`docs/constitution.md`, Architecture
/// principles: "Ringpuffer ... ≤ 30 s je Strom") to maximise the lag
/// tolerance later ASR/VAD windows get — see the spec's prior decisions. At
/// 48 kHz stereo `f32` this is roughly 11.5 MB per stream.
pub const RING_BUFFER_CAPACITY_SECONDS: u32 = 30;

/// The constitution's per-stream ceiling, named separately from
/// [`RING_BUFFER_CAPACITY_SECONDS`] so that constant is checked against it
/// rather than silently becoming the ceiling itself if it is ever edited.
const CONSTITUTION_RING_BUFFER_CEILING_SECONDS: u32 = 30;

/// Compile-time guard tying [`RingBuffer::new`]'s capacity to the
/// constitution's ceiling: a future edit that raises
/// [`RING_BUFFER_CAPACITY_SECONDS`] past
/// [`CONSTITUTION_RING_BUFFER_CEILING_SECONDS`] fails the build rather than
/// silently shipping an oversized buffer.
#[expect(
    dead_code,
    reason = "the check is the point; nothing needs to read this item"
)]
const CAPACITY_WITHIN_CONSTITUTION_CEILING: () = assert!(
    RING_BUFFER_CAPACITY_SECONDS <= CONSTITUTION_RING_BUFFER_CEILING_SECONDS,
    "ring buffer capacity exceeds the constitution's per-stream ceiling"
);

/// Handle to one registered reader's cursor into a [`RingBuffer`].
///
/// Opaque, and only valid for the buffer that issued it via
/// [`RingBuffer::add_reader`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReaderId(usize);

/// One reader's position and accumulated loss.
struct ReaderState {
    read_pos: u64,
    loss_count: u64,
}

/// Fixed-size ring buffer of interleaved `f32` PCM samples for one stream.
///
/// The backing storage is allocated once, in [`RingBuffer::new`], to
/// [`RING_BUFFER_CAPACITY_SECONDS`] worth of samples for the given
/// [`StreamFormat`]; [`RingBuffer::push`] never reallocates it — overflow
/// overwrites the oldest samples instead. Each reader gets its own cursor via
/// [`RingBuffer::add_reader`]; a reader that falls behind loses the
/// overwritten samples rather than growing the buffer, and is told the loss
/// count via [`RingBuffer::loss_count`].
///
/// Deliberately has no `Debug` impl: the only field worth printing is the
/// PCM storage itself, and printing it would be exactly the sample leak
/// `docs/constitution.md` forbids.
pub struct RingBuffer {
    storage: Zeroizing<Vec<f32>>,
    write_pos: u64,
    readers: Vec<ReaderState>,
    discontinuity_count: u64,
    timestamp_error_count: u64,
}

impl RingBuffer {
    /// Allocates a buffer sized for `format` at
    /// [`RING_BUFFER_CAPACITY_SECONDS`] of audio.
    #[must_use]
    pub fn new(format: StreamFormat) -> Self {
        let capacity =
            u64::from(RING_BUFFER_CAPACITY_SECONDS) * format.interleaved_samples_per_second();
        Self {
            storage: Zeroizing::new(vec![0.0; capacity as usize]),
            write_pos: 0,
            readers: Vec::new(),
            discontinuity_count: 0,
            timestamp_error_count: 0,
        }
    }

    /// Samples this buffer can hold before the oldest unread samples are
    /// overwritten. Fixed at construction; never changes and never triggers
    /// a reallocation of the backing storage.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.storage.len()
    }

    /// Registers a new reader, starting from the next sample this buffer
    /// receives — it does not retroactively see samples already pushed.
    #[must_use]
    pub fn add_reader(&mut self) -> ReaderId {
        let id = ReaderId(self.readers.len());
        self.readers.push(ReaderState {
            read_pos: self.write_pos,
            loss_count: 0,
        });
        id
    }

    /// Cumulative samples `reader` has lost to overwritten, unread data,
    /// including any loss just accounted for by the last
    /// [`RingBuffer::read`] call.
    #[must_use]
    pub fn loss_count(&self, reader: ReaderId) -> u64 {
        self.readers[reader.0].loss_count
    }

    /// `data_discontinuity` gaps recorded so far.
    #[must_use]
    pub fn discontinuity_count(&self) -> u64 {
        self.discontinuity_count
    }

    /// `timestamp_error` gaps recorded so far.
    #[must_use]
    pub fn timestamp_error_count(&self) -> u64 {
        self.timestamp_error_count
    }

    /// Writes one packet.
    ///
    /// Samples — ordinary or silent, already materialised as zeros by
    /// [`Frame::silent`] — are copied into the ring, overwriting the oldest
    /// unread data if the buffer is full. A gap frame writes no samples at
    /// all: there is no data to represent the span it covers, so it only
    /// advances the counter matching its [`GapCause`].
    pub fn push(&mut self, frame: Frame) {
        match frame {
            Frame::Samples { samples, .. } => self.write_samples(&samples),
            Frame::Gap {
                cause: GapCause::Discontinuity,
                ..
            } => self.discontinuity_count += 1,
            Frame::Gap {
                cause: GapCause::TimestampError,
                ..
            } => self.timestamp_error_count += 1,
        }
    }

    /// Copies `samples` into the ring at the current write position,
    /// wrapping — and overwriting the oldest unread data — as needed.
    fn write_samples(&mut self, samples: &[f32]) {
        let capacity = self.capacity();
        for (offset, &sample) in samples.iter().enumerate() {
            let index = (self.write_pos as usize + offset) % capacity;
            self.storage[index] = sample;
        }
        self.write_pos += samples.len() as u64;
    }

    /// Explicitly zeroes every sample currently held in this buffer's
    /// backing storage, in place — the capacity invariant holds through
    /// this call, nothing is resized or reallocated.
    ///
    /// `docs/constitution.md`: PCM buffers are explicitly zeroed at session
    /// end, not merely dropped. `Zeroizing` already zeroes on `Drop`, but
    /// `transcriber-session`'s `Session::stop` calls this before the buffer
    /// (and the `Session` around it) goes away, so the guarantee holds
    /// while the buffer is still reachable, not only after.
    pub fn zeroize(&mut self) {
        self.storage.fill(0.0);
    }

    /// True if every sample this buffer's backing storage currently holds
    /// is exactly `0.0` — a query that proves [`Self::zeroize`]'s guarantee
    /// without ever exposing the samples themselves, the same reason this
    /// type has no `Debug` impl.
    #[must_use]
    pub fn all_zero(&self) -> bool {
        self.storage.iter().all(|&sample| sample == 0.0)
    }

    /// Copies up to `out.len()` unread samples for `reader` into `out`,
    /// oldest first, and advances its cursor. Returns how many were copied.
    ///
    /// If samples were overwritten before `reader` consumed them, this call
    /// first accounts for the loss — visible afterwards via
    /// [`RingBuffer::loss_count`] — and resumes from the oldest sample still
    /// held, however many capacities behind the reader had fallen.
    pub fn read(&mut self, reader: ReaderId, out: &mut [f32]) -> usize {
        let capacity = self.capacity();
        let write_pos = self.write_pos;
        let state = &mut self.readers[reader.0];
        let oldest_held = write_pos.saturating_sub(capacity as u64);
        if state.read_pos < oldest_held {
            state.loss_count += oldest_held - state.read_pos;
            state.read_pos = oldest_held;
        }
        let available = (write_pos - state.read_pos) as usize;
        let len = available.min(out.len());
        for (offset, slot) in out.iter_mut().take(len).enumerate() {
            let index = (state.read_pos as usize + offset) % capacity;
            *slot = self.storage[index];
        }
        state.read_pos += len as u64;
        len
    }
}

#[cfg(test)]
mod tests {
    use super::RingBuffer;
    use crate::format::StreamFormat;
    use crate::frame::{DeviceTimestamp, Frame, GapCause};

    /// A tiny format keeps the buffer small enough to push many multiples
    /// of its capacity quickly in tests.
    fn tiny_format() -> StreamFormat {
        let Ok(format) = StreamFormat::new(10, 1) else {
            panic!("10 Hz mono must be a valid format");
        };
        format
    }

    fn push_samples(buffer: &mut RingBuffer, values: &[f32]) {
        buffer.push(Frame::samples(
            DeviceTimestamp::from_ticks(0),
            values.to_vec(),
        ));
    }

    #[test]
    fn capacity_matches_thirty_seconds_of_the_given_format() {
        let buffer = RingBuffer::new(tiny_format());
        assert_eq!(buffer.capacity(), 300);
    }

    /// The property the spec's risk table calls out by name: capacity and
    /// the storage's base pointer must be identical before and after a run
    /// that pushes far more samples than the buffer holds — a reallocation
    /// (even one that lands on the same final size) would move the pointer,
    /// which a `capacity()` equality alone could not catch.
    #[test]
    fn capacity_and_base_pointer_never_change_across_a_full_run() {
        let mut buffer = RingBuffer::new(tiny_format());
        let capacity = buffer.capacity();
        let base_ptr = buffer.storage.as_ptr();

        for _ in 0..(capacity * 10) {
            push_samples(&mut buffer, &[1.0]);
            assert_eq!(buffer.capacity(), capacity);
            assert_eq!(buffer.storage.as_ptr(), base_ptr);
        }
    }

    #[test]
    fn fast_reader_loses_nothing() {
        let mut buffer = RingBuffer::new(tiny_format());
        let capacity = buffer.capacity();
        let fast = buffer.add_reader();
        let mut sink = vec![0.0; capacity];

        for _ in 0..20 {
            push_samples(&mut buffer, &[1.0]);
            buffer.read(fast, &mut sink[..1]);
        }

        assert_eq!(buffer.loss_count(fast), 0);
    }

    /// Falls behind by more than one full capacity before ever reading —
    /// lapped more than once — and must report the exact loss, not just a
    /// nonzero one.
    #[test]
    fn slow_reader_lapped_more_than_once_reports_exact_loss() {
        let mut buffer = RingBuffer::new(tiny_format());
        let capacity = buffer.capacity();
        let slow = buffer.add_reader();

        let pushed = capacity * 3 + 5;
        for i in 0..pushed {
            push_samples(&mut buffer, &[i as f32]);
        }

        let mut sink = vec![0.0; capacity];
        let read = buffer.read(slow, &mut sink);

        assert_eq!(buffer.loss_count(slow), (pushed - capacity) as u64);
        assert_eq!(read, capacity);
    }

    #[test]
    fn two_cursor_fast_loses_nothing_slow_reports_loss() {
        let mut buffer = RingBuffer::new(tiny_format());
        let capacity = buffer.capacity();
        let fast = buffer.add_reader();
        let slow = buffer.add_reader();
        let mut fast_sink = vec![0.0; capacity];

        for i in 0..(capacity * 4) {
            push_samples(&mut buffer, &[i as f32]);
            buffer.read(fast, &mut fast_sink[..1]);
        }

        let mut slow_sink = vec![0.0; capacity];
        buffer.read(slow, &mut slow_sink);

        assert_eq!(buffer.loss_count(fast), 0);
        assert!(buffer.loss_count(slow) > 0);
    }

    #[test]
    fn discontinuity_and_timestamp_error_get_separate_counters() {
        let mut buffer = RingBuffer::new(tiny_format());

        buffer.push(Frame::gap(
            DeviceTimestamp::from_ticks(0),
            3,
            GapCause::Discontinuity,
        ));
        assert_eq!(buffer.discontinuity_count(), 1);
        assert_eq!(buffer.timestamp_error_count(), 0);

        buffer.push(Frame::gap(
            DeviceTimestamp::from_ticks(1),
            2,
            GapCause::TimestampError,
        ));
        assert_eq!(buffer.discontinuity_count(), 1);
        assert_eq!(buffer.timestamp_error_count(), 1);
    }

    /// Silence must be materialised as real zero samples in the readable
    /// stream, keeping it dense, while a gap of either cause writes no
    /// samples at all and is only visible through its counter.
    #[test]
    fn silent_frames_stay_dense_while_gaps_write_no_samples() {
        let mut buffer = RingBuffer::new(tiny_format());
        let reader = buffer.add_reader();

        push_samples(&mut buffer, &[1.0, 2.0]);
        buffer.push(Frame::silent(DeviceTimestamp::from_ticks(2), 3));
        buffer.push(Frame::gap(
            DeviceTimestamp::from_ticks(5),
            4,
            GapCause::Discontinuity,
        ));
        push_samples(&mut buffer, &[9.0, 9.0]);

        let mut out = vec![-1.0; 7];
        let read = buffer.read(reader, &mut out);

        assert_eq!(read, 7);
        assert_eq!(out, vec![1.0, 2.0, 0.0, 0.0, 0.0, 9.0, 9.0]);
        assert_eq!(buffer.discontinuity_count(), 1);
    }

    #[test]
    fn a_fresh_buffer_is_all_zero() {
        let buffer = RingBuffer::new(tiny_format());
        assert!(buffer.all_zero());
    }

    #[test]
    fn zeroize_clears_written_samples_without_changing_capacity_or_the_base_pointer() {
        let mut buffer = RingBuffer::new(tiny_format());
        push_samples(&mut buffer, &[1.0, 2.0, 3.0]);
        assert!(!buffer.all_zero(), "sanity: real samples were written");

        let capacity = buffer.capacity();
        let base_ptr = buffer.storage.as_ptr();
        buffer.zeroize();

        assert!(buffer.all_zero());
        assert_eq!(buffer.capacity(), capacity);
        assert_eq!(buffer.storage.as_ptr(), base_ptr);
    }
}
