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
/// Deliberately has no `Debug` impl, for the same reason as [`RingBuffer`]:
/// every field that would be worth printing either holds PCM directly
/// (`raw_scratch`, `raw_leftover`, `mono_pending`, `output_ready`) or is
/// `rubato`'s own PCM-holding internal state.
pub struct StreamResampler {
    reader: ReaderId,
    channels: usize,
    chunk_frames_in: usize,
    raw_scratch: Vec<f32>,
    raw_leftover: Vec<f32>,
    mono_pending: Vec<f32>,
    output_ready: Vec<f32>,
    output_cursor: usize,
    resampler: FftFixedInOut<f32>,
    chunk_frames_out: usize,
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
            raw_scratch: vec![0.0; chunk_frames_in * channels],
            raw_leftover: Vec::new(),
            mono_pending: Vec::new(),
            output_ready: Vec::new(),
            output_cursor: 0,
            resampler,
            chunk_frames_out,
        })
    }

    /// Writes up to `out.len()` resampled mono samples, pulling as many
    /// native-format frames from `ring` as needed. Returns how many were
    /// written; fewer than `out.len()` (possibly 0) means `ring` had
    /// nothing further buffered for this reader yet, not an error.
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
    fn downmix(&mut self, len: usize) {
        let mut combined = std::mem::take(&mut self.raw_leftover);
        combined.extend_from_slice(&self.raw_scratch[..len]);
        let complete_frames = combined.len() / self.channels;
        let complete_len = complete_frames * self.channels;
        for frame in combined[..complete_len].chunks_exact(self.channels) {
            let sum: f32 = frame.iter().sum();
            self.mono_pending.push(sum / self.channels as f32);
        }
        self.raw_leftover = combined[complete_len..].to_vec();
    }

    /// Feeds exactly `chunk_frames_in` pending mono samples through
    /// `rubato`, appending the result to `output_ready`.
    fn resample_one_chunk(&mut self) {
        if self.output_cursor > 0 {
            self.output_ready.drain(..self.output_cursor);
            self.output_cursor = 0;
        }
        let chunk: Vec<f32> = self.mono_pending.drain(..self.chunk_frames_in).collect();
        let input = [chunk];
        let mut output = [vec![0.0_f32; self.chunk_frames_out]];
        let result = self
            .resampler
            .process_into_buffer(&input, &mut output, None);
        // `chunk.len() == chunk_frames_in == input_frames_next()`,
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
        if let Ok((_, produced)) = result {
            self.output_ready.extend_from_slice(&output[0][..produced]);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::PI;

    use super::{CHUNK_FRAMES_IN_HINT, StreamResampler, TARGET_SAMPLE_RATE_HZ};
    use crate::format::StreamFormat;
    use crate::frame::{DeviceTimestamp, Frame};
    use crate::ring_buffer::RingBuffer;

    const SOURCE_SAMPLE_RATE_HZ: u32 = 48_000;

    fn source_format() -> StreamFormat {
        let Ok(format) = StreamFormat::new(SOURCE_SAMPLE_RATE_HZ, 2) else {
            panic!("48 kHz stereo must be a valid format");
        };
        format
    }

    /// Builds `frame_count` stereo frames of a full-scale sine at
    /// `frequency_hz` and `sample_rate_hz`, identical on both channels so
    /// downmixing to mono leaves the tone's amplitude unchanged.
    fn stereo_sine_at(frequency_hz: f32, sample_rate_hz: u32, frame_count: usize) -> Vec<f32> {
        let mut samples = Vec::with_capacity(frame_count * 2);
        for n in 0..frame_count {
            let phase = 2.0 * PI * frequency_hz * (n as f32) / sample_rate_hz as f32;
            let value = phase.sin();
            samples.push(value);
            samples.push(value);
        }
        samples
    }

    /// [`stereo_sine_at`] at the fixed [`SOURCE_SAMPLE_RATE_HZ`] every test
    /// but the non-48-kHz regression test below uses.
    fn stereo_sine(frequency_hz: f32, frame_count: usize) -> Vec<f32> {
        stereo_sine_at(frequency_hz, SOURCE_SAMPLE_RATE_HZ, frame_count)
    }

    /// Magnitude of a single-frequency Goertzel filter evaluated over
    /// `samples` at `sample_rate_hz` — a targeted single-bin DFT, chosen so
    /// the aliasing test below needs no second dependency.
    fn goertzel_magnitude(samples: &[f32], sample_rate_hz: f32, frequency_hz: f32) -> f32 {
        let omega = 2.0 * PI * frequency_hz / sample_rate_hz;
        let coeff = 2.0 * omega.cos();
        let (mut s_prev, mut s_prev2) = (0.0_f32, 0.0_f32);
        for &sample in samples {
            let s = sample + coeff * s_prev - s_prev2;
            s_prev2 = s_prev;
            s_prev = s;
        }
        let real = s_prev - s_prev2 * omega.cos();
        let imag = s_prev2 * omega.sin();
        (real * real + imag * imag).sqrt()
    }

    /// Pushes `input` (interleaved stereo, native format) into a fresh ring
    /// buffer and drains it fully through one `StreamResampler`.
    fn resample_all(input: Vec<f32>) -> Vec<f32> {
        let mut ring = RingBuffer::new(source_format());
        let reader = ring.add_reader();
        let Ok(mut resampler) = StreamResampler::new(reader, source_format()) else {
            panic!("48 kHz stereo -> 16 kHz mono must construct");
        };
        ring.push(Frame::samples(DeviceTimestamp::from_ticks(0), input));

        let mut output = Vec::new();
        let mut scratch = [0.0_f32; 160];
        loop {
            let read = resampler.read(&mut ring, &mut scratch);
            output.extend_from_slice(&scratch[..read]);
            if read == 0 {
                break;
            }
        }
        output
    }

    #[test]
    fn frequency_is_preserved_and_output_length_is_exact() {
        let frame_count = CHUNK_FRAMES_IN_HINT * 100; // 48_000 frames = 1 s exactly
        let input = stereo_sine(1_000.0, frame_count);
        let output = resample_all(input);

        let expected_len = frame_count / 3;
        assert_eq!(
            output.len(),
            expected_len,
            "48 kHz -> 16 kHz is an exact 3:1 ratio here, so the length must be exact"
        );

        let at_tone = goertzel_magnitude(&output, TARGET_SAMPLE_RATE_HZ as f32, 1_000.0);
        let at_other = goertzel_magnitude(&output, TARGET_SAMPLE_RATE_HZ as f32, 3_000.0);
        assert!(
            at_tone > at_other * 10.0,
            "the 1 kHz tone must dominate an unrelated 3 kHz bin: {at_tone} vs {at_other}"
        );
    }

    /// The aliasing assertion the spec's verification demands: a tone above
    /// the 16 kHz target's 8 kHz Nyquist is valid content at 48 kHz (well
    /// under *its* 24 kHz Nyquist), but a naive decimate-by-3 with no
    /// low-pass would fold it down to `|9_000 - 16_000| = 7_000` Hz. This
    /// must fail if `StreamResampler` ever regressed to that naive
    /// decimation instead of `rubato`'s filtered resampling.
    #[test]
    fn content_above_the_new_nyquist_is_filtered_not_aliased() {
        let frame_count = CHUNK_FRAMES_IN_HINT * 100;
        let tone_hz = 9_000.0;
        let alias_hz = 7_000.0;
        let input = stereo_sine(tone_hz, frame_count);

        let naive_decimated: Vec<f32> = input
            .chunks_exact(2)
            .map(|frame| frame[0]) // both channels are identical
            .step_by(3)
            .collect();
        let naive_alias_magnitude =
            goertzel_magnitude(&naive_decimated, TARGET_SAMPLE_RATE_HZ as f32, alias_hz);

        let output = resample_all(input);
        let filtered_alias_magnitude =
            goertzel_magnitude(&output, TARGET_SAMPLE_RATE_HZ as f32, alias_hz);

        assert!(
            naive_alias_magnitude > filtered_alias_magnitude * 10.0,
            "rubato's anti-alias filter must suppress the 7 kHz alias a naive \
             decimation would show: naive {naive_alias_magnitude} vs filtered {filtered_alias_magnitude}"
        );
    }

    /// The per-reader-state acceptance item: two readers of the same ring
    /// buffer, drained at very different paces (many tiny reads vs. one
    /// large read), must produce byte-identical output from the same
    /// underlying samples — proof that neither `StreamResampler`'s internal
    /// chunking nor its `rubato` state depends on, or leaks into, the
    /// other's.
    #[test]
    fn two_readers_at_different_paces_resample_identically() {
        let frame_count = CHUNK_FRAMES_IN_HINT * 6;
        let input = stereo_sine(1_000.0, frame_count);

        let mut ring = RingBuffer::new(source_format());
        let fast = ring.add_reader();
        let slow = ring.add_reader();
        let Ok(mut fast_resampler) = StreamResampler::new(fast, source_format()) else {
            panic!("48 kHz stereo -> 16 kHz mono must construct");
        };
        let Ok(mut slow_resampler) = StreamResampler::new(slow, source_format()) else {
            panic!("48 kHz stereo -> 16 kHz mono must construct");
        };
        ring.push(Frame::samples(DeviceTimestamp::from_ticks(0), input));

        let mut fast_output = Vec::new();
        let mut small = [0.0_f32; 37]; // deliberately not a multiple of any chunk size
        loop {
            let read = fast_resampler.read(&mut ring, &mut small);
            fast_output.extend_from_slice(&small[..read]);
            if read == 0 {
                break;
            }
        }

        let mut slow_output = vec![0.0_f32; frame_count / 3];
        let read = slow_resampler.read(&mut ring, &mut slow_output);
        slow_output.truncate(read);

        assert_eq!(fast_output.len(), slow_output.len());
        assert_eq!(fast_output, slow_output);
    }

    /// Exercises the `raw_leftover` path directly: pushing five raw samples
    /// at a time (never a multiple of the two channels) forces
    /// `RingBuffer::read` to keep handing back partial frames, so
    /// `downmix`'s leftover carry-over runs on almost every call instead of
    /// only once at the very end of a stream.
    #[test]
    fn partial_frames_split_across_pushes_are_carried_over_correctly() {
        let frame_count = CHUNK_FRAMES_IN_HINT * 2;
        let input = stereo_sine(1_000.0, frame_count);

        let mut ring = RingBuffer::new(source_format());
        let reader = ring.add_reader();
        let Ok(mut resampler) = StreamResampler::new(reader, source_format()) else {
            panic!("48 kHz stereo -> 16 kHz mono must construct");
        };

        let mut output = Vec::new();
        let mut scratch = [0.0_f32; 160];
        for piece in input.chunks(5) {
            ring.push(Frame::samples(
                DeviceTimestamp::from_ticks(0),
                piece.to_vec(),
            ));
            let read = resampler.read(&mut ring, &mut scratch);
            output.extend_from_slice(&scratch[..read]);
        }

        let expected_len = frame_count / 3;
        assert_eq!(
            output.len(),
            expected_len,
            "a push pattern that never aligns to a stereo frame must not change the resampled length"
        );
        let at_tone = goertzel_magnitude(&output, TARGET_SAMPLE_RATE_HZ as f32, 1_000.0);
        let at_other = goertzel_magnitude(&output, TARGET_SAMPLE_RATE_HZ as f32, 5_000.0);
        assert!(
            at_tone > at_other * 10.0,
            "the tone must survive a constantly misaligned push pattern: {at_tone} vs {at_other}"
        );
    }

    /// The regression test for the bug the review caught: `StreamResampler`
    /// used to always read exactly [`CHUNK_FRAMES_IN_HINT`] native frames
    /// per step, which only happens to match `rubato`'s actual
    /// `input_frames_next()` for 48 kHz -> 16 kHz (ratio exactly 3:1). For a
    /// rate like 44.1 kHz, `rubato` rounds up to 882-frame chunks; feeding it
    /// 480 at a time made every `process_into_buffer` call fail and, via the
    /// `debug_assert!`-guarded `if let Ok(..)` in `resample_one_chunk`,
    /// silently discarded 100% of the audio in release builds while
    /// panicking in debug/test builds. `StreamResampler::new` now reads
    /// `input_frames_next()` back instead of assuming the hint, so this must
    /// produce real, non-empty, correctly-pitched output.
    #[test]
    fn a_non_48_khz_source_rate_still_resamples_correctly() {
        let source_hz = 44_100;
        let Ok(format) = StreamFormat::new(source_hz, 2) else {
            panic!("44.1 kHz stereo must be a valid format");
        };
        let mut ring = RingBuffer::new(format);
        let reader = ring.add_reader();
        let Ok(mut resampler) = StreamResampler::new(reader, format) else {
            panic!("44.1 kHz stereo -> 16 kHz mono must construct");
        };

        let frame_count = source_hz as usize * 2; // 2 s
        let input = stereo_sine_at(1_000.0, source_hz, frame_count);
        ring.push(Frame::samples(DeviceTimestamp::from_ticks(0), input));

        let mut output = Vec::new();
        let mut scratch = [0.0_f32; 512];
        loop {
            let read = resampler.read(&mut ring, &mut scratch);
            output.extend_from_slice(&scratch[..read]);
            if read == 0 {
                break;
            }
        }

        let expected_approx =
            (frame_count as f32 * TARGET_SAMPLE_RATE_HZ as f32 / source_hz as f32) as usize;
        assert!(
            output.len() > expected_approx / 2,
            "44.1 kHz input must not be silently discarded: got {} samples, expected roughly {expected_approx}",
            output.len()
        );

        let at_tone = goertzel_magnitude(&output, TARGET_SAMPLE_RATE_HZ as f32, 1_000.0);
        let at_other = goertzel_magnitude(&output, TARGET_SAMPLE_RATE_HZ as f32, 5_000.0);
        assert!(
            at_tone > at_other * 10.0,
            "the 1 kHz tone must survive a non-48 kHz source rate: {at_tone} vs {at_other}"
        );
    }
}
