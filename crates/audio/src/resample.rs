//! Downmix and resample on the read side: a reader pulls native-format
//! samples out of a [`RingBuffer`] and gets 16 kHz mono back, the format
//! phase 2/3's ASR and VAD consumers expect. The ring buffer itself never
//! changes format — see the spec's prior decisions ("Ringpuffer, Zeitachse
//! und Resampling", `docs/specs/spec-capture-foundation.md`).
//!
//! Downmix happens *before* resampling: averaging channels to mono first
//! cannot introduce frequency content the source did not already carry, and
//! it roughly halves the sample count the resampler has to filter. The
//! anti-aliasing itself is `rubato`'s job — [`FftFixedInOut`] places its
//! low-pass cutoff just below the target's Nyquist frequency (8 kHz for a
//! 16 kHz target; `rubato`'s `BlackmanHarris2` window puts the actual -3 dB
//! point a little under that, never above it), computed from the exact
//! input/output FFT sizes rather than left for the caller to tune. Record of
//! this choice: spec Decision log, 2026-07-30.

use rubato::{FftFixedInOut, Resampler as _, ResamplerConstructionError};
use thiserror::Error;
use zeroize::Zeroizing;

use crate::format::StreamFormat;
use crate::ring_buffer::{ReaderId, RingBuffer};

/// Fixed output rate every [`StreamResampler`] converts to.
pub const TARGET_SAMPLE_RATE_HZ: u32 = 16_000;

/// Desired native-format mono frames per resample step — 10 ms at 48 kHz,
/// handed to [`FftFixedInOut::new`] as a hint. `rubato` rounds this up to a
/// multiple of `source_hz / gcd(source_hz, TARGET_SAMPLE_RATE_HZ)`; for the
/// 48 kHz -> 16 kHz pair (ratio exactly 3:1) that rounding is a no-op, but
/// [`StreamResampler`] never assumes so — it always reads the resampler's
/// *actual* chunk size back via `input_frames_next()` rather than reusing
/// this constant, so every source rate stays correct, not just this one.
const CHUNK_FRAMES_IN_HINT: usize = 480;

/// Failure constructing a [`StreamResampler`] for a source format.
#[derive(Debug, Error)]
pub enum ResampleError {
    /// `rubato` rejected the source/target sample-rate pair.
    #[error("cannot resample {source_hz} Hz to {target_hz} Hz: {source}")]
    Construction {
        /// The rejected source sample rate.
        source_hz: u32,
        /// The fixed target sample rate, [`TARGET_SAMPLE_RATE_HZ`].
        target_hz: u32,
        /// The underlying `rubato` construction error.
        #[source]
        source: ResamplerConstructionError,
    },
}

/// Per-reader downmix + resample state: a [`RingBuffer`]'s native
/// multi-channel format becomes [`TARGET_SAMPLE_RATE_HZ`] mono, on the read
/// side, without the ring buffer's own storage changing format.
///
/// Bound to one [`ReaderId`] at construction. Two readers of the same
/// buffer each own their own `StreamResampler` — the FFT overlap tail and
/// buffered partial frames of a lagging reader never touch another reader's
/// state, which is what makes the phase 2/3 fan-out purely additive.
///
/// Has no `Debug` impl: `raw_scratch`, `raw_leftover`, `mono_pending` and
/// `output_ready` hold PCM directly, the same reason [`RingBuffer`] has
/// none, and `FftFixedInOut` (which holds PCM too, in its FFT overlap
/// state) implements none either.
pub struct StreamResampler {
    reader: ReaderId,
    channels: usize,
    chunk_frames_in: usize,
    chunk_frames_out: usize,
    /// Scratch space for one `RingBuffer::read` call, sized
    /// `chunk_frames_in * channels` and never resized after construction.
    raw_scratch: Zeroizing<Vec<f32>>,
    /// A trailing partial frame (fewer than `channels` samples) carried
    /// from one `downmix` call to the next; always shorter than `channels`.
    raw_leftover: Zeroizing<Vec<f32>>,
    /// Downmixed mono samples not yet fed through `rubato`, in FIFO order.
    mono_pending: Zeroizing<Vec<f32>>,
    /// Resampled mono samples not yet handed to a caller of `read`, in FIFO
    /// order starting at `output_cursor`.
    output_ready: Zeroizing<Vec<f32>>,
    output_cursor: usize,
    resampler: FftFixedInOut<f32>,
}

impl StreamResampler {
    /// Builds a resampler for `reader`, reading `source_format`-shaped
    /// samples from the ring buffer that issued `reader` and producing
    /// [`TARGET_SAMPLE_RATE_HZ`] mono.
    pub fn new(reader: ReaderId, source_format: StreamFormat) -> Result<Self, ResampleError> {
        let channels = source_format.channels() as usize;
        let resampler = FftFixedInOut::<f32>::new(
            source_format.sample_rate_hz() as usize,
            TARGET_SAMPLE_RATE_HZ as usize,
            CHUNK_FRAMES_IN_HINT,
            1,
        )
        .map_err(|source| ResampleError::Construction {
            source_hz: source_format.sample_rate_hz(),
            target_hz: TARGET_SAMPLE_RATE_HZ,
            source,
        })?;
        // Read the resampler's *actual* chunk sizes back rather than reusing
        // the hint: rubato rounds a hint up to a multiple that keeps the
        // ratio exact, and for most source rates that is not the hint
        // itself (only e.g. 48 kHz's 3:1 ratio leaves it unchanged).
        let chunk_frames_in = resampler.input_frames_next();
        let chunk_frames_out = resampler.output_frames_next();
        Ok(Self {
            reader,
            channels,
            chunk_frames_in,
            chunk_frames_out,
            raw_scratch: Zeroizing::new(vec![0.0; chunk_frames_in * channels]),
            raw_leftover: Zeroizing::new(Vec::with_capacity(channels)),
            mono_pending: Zeroizing::new(Vec::with_capacity(chunk_frames_in)),
            output_ready: Zeroizing::new(Vec::with_capacity(chunk_frames_out)),
            output_cursor: 0,
            resampler,
        })
    }

    /// Writes up to `out.len()` resampled mono samples, pulling as many
    /// native-format frames from `ring` as needed. Returns how many were
    /// written; fewer than `out.len()` (possibly 0) usually means `ring`
    /// had nothing further buffered for this reader yet, not an error — but
    /// note there is no flush: up to `chunk_frames_in - 1` mono samples
    /// (buffered because they do not yet fill a full `rubato` chunk) plus
    /// `rubato`'s own fixed processing delay never appear in any `read`
    /// call's output. Phase 2/3's session-end path will need a way to
    /// force this remainder out; this type does not have one yet.
    pub fn read(&mut self, ring: &mut RingBuffer, out: &mut [f32]) -> usize {
        let mut written = 0;
        while written < out.len() {
            written += self.drain_ready(&mut out[written..]);
            if written == out.len() || !self.fill_pending_and_resample(ring) {
                break;
            }
        }
        written
    }

    /// Copies already-resampled samples into `out`, advancing the cursor.
    /// Returns how many were copied.
    fn drain_ready(&mut self, out: &mut [f32]) -> usize {
        let available = self.output_ready.len() - self.output_cursor;
        let take = available.min(out.len());
        let start = self.output_cursor;
        out[..take].copy_from_slice(&self.output_ready[start..start + take]);
        self.output_cursor += take;
        take
    }

    /// Pulls native-format samples from `ring` until a full input chunk has
    /// been downmixed, then resamples it into `output_ready`. Returns
    /// whether a chunk was produced; `false` means `ring` had nothing left.
    fn fill_pending_and_resample(&mut self, ring: &mut RingBuffer) -> bool {
        while self.mono_pending.len() < self.chunk_frames_in {
            let read = ring.read(self.reader, &mut self.raw_scratch);
            if read == 0 {
                return false;
            }
            self.downmix(read);
        }
        self.resample_one_chunk();
        true
    }

    /// Averages the first `len` samples of `raw_scratch` (interleaved,
    /// `channels` per frame) down to mono, appending to `mono_pending`. A
    /// trailing partial frame is carried in `raw_leftover` for next time —
    /// `RingBuffer::read` has no reason to always hand back whole frames.
    ///
    /// Mutates `raw_leftover` in place (append, then drop the consumed
    /// prefix) rather than building a fresh combined buffer each call, so
    /// its small allocation is made once and reused, not replaced.
    fn downmix(&mut self, len: usize) {
        self.raw_leftover
            .extend_from_slice(&self.raw_scratch[..len]);
        let complete_frames = self.raw_leftover.len() / self.channels;
        let complete_len = complete_frames * self.channels;
        for frame in self.raw_leftover[..complete_len].chunks_exact(self.channels) {
            let sum: f32 = frame.iter().sum();
            self.mono_pending.push(sum / self.channels as f32);
        }
        self.raw_leftover.drain(..complete_len);
    }

    /// Feeds exactly `chunk_frames_in` pending mono samples through
    /// `rubato`, appending the result to `output_ready`.
    ///
    /// Reads straight out of `mono_pending` and writes straight into a
    /// freshly extended tail of `output_ready` — no temporary `Vec` is
    /// allocated for either side of `rubato`'s call, so no PCM sample ever
    /// exists outside the four `Zeroizing`-wrapped fields on this type.
    fn resample_one_chunk(&mut self) {
        if self.output_cursor > 0 {
            self.output_ready.drain(..self.output_cursor);
            self.output_cursor = 0;
        }
        let output_start = self.output_ready.len();
        self.output_ready
            .resize(output_start + self.chunk_frames_out, 0.0);

        let input = [&self.mono_pending[..self.chunk_frames_in]];
        let mut output = [&mut self.output_ready[output_start..]];
        let result = self
            .resampler
            .process_into_buffer(&input, &mut output, None);
        // `input[0].len() == chunk_frames_in == input_frames_next()`,
        // `output[0].len() == chunk_frames_out == output_frames_next()`, and
        // there is exactly one channel on both sides — the only inputs
        // `process_into_buffer` validates — so this cannot fail for any
        // `source_format` `new()` accepted. If it ever does, dropping the
        // chunk (rather than panicking on the audio read path) is the
        // deliberately chosen failure mode; `debug_assert!` still surfaces
        // it during development.
        debug_assert!(
            result.is_ok(),
            "chunk_frames_in/out are read back from the resampler itself and must match"
        );
        let produced = result.map_or(0, |(_, produced)| produced);
        self.output_ready.truncate(output_start + produced);
        self.mono_pending.drain(..self.chunk_frames_in);
    }
}

#[cfg(test)]
mod tests;
