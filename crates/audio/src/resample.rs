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
//! low-pass cutoff at the target's Nyquist frequency (8 kHz for a 16 kHz
//! target), computed from the exact input/output FFT sizes rather than left
//! for the caller to tune. Record of this choice: spec Decision log,
//! 2026-07-30.

use rubato::{FftFixedInOut, Resampler as _, ResamplerConstructionError};
use thiserror::Error;

use crate::format::StreamFormat;
use crate::ring_buffer::{ReaderId, RingBuffer};

/// Fixed output rate every [`StreamResampler`] converts to.
pub const TARGET_SAMPLE_RATE_HZ: u32 = 16_000;

/// Native-format mono frames pulled from the ring buffer per resample step —
/// 10 ms at 48 kHz. Divisible by the 48 kHz -> 16 kHz ratio (3), so the
/// configured [`FftFixedInOut`] needs no internal rounding for that pair.
const CHUNK_FRAMES_IN: usize = 480;

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
pub struct StreamResampler {
    reader: ReaderId,
    channels: usize,
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
            CHUNK_FRAMES_IN,
            1,
        )
        .map_err(|source| ResampleError::Construction {
            source_hz: source_format.sample_rate_hz(),
            target_hz: TARGET_SAMPLE_RATE_HZ,
            source,
        })?;
        let chunk_frames_out = resampler.output_frames_next();
        Ok(Self {
            reader,
            channels,
            raw_scratch: vec![0.0; CHUNK_FRAMES_IN * channels],
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
        while self.mono_pending.len() < CHUNK_FRAMES_IN {
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

    /// Feeds exactly [`CHUNK_FRAMES_IN`] pending mono samples through
    /// `rubato`, appending the result to `output_ready`.
    fn resample_one_chunk(&mut self) {
        if self.output_cursor > 0 {
            self.output_ready.drain(..self.output_cursor);
            self.output_cursor = 0;
        }
        let chunk: Vec<f32> = self.mono_pending.drain(..CHUNK_FRAMES_IN).collect();
        let input = [chunk];
        let mut output = [vec![0.0_f32; self.chunk_frames_out]];
        let result = self
            .resampler
            .process_into_buffer(&input, &mut output, None);
        debug_assert!(
            result.is_ok(),
            "input/output chunk sizes are fixed by construction and must match"
        );
        if let Ok((_, produced)) = result {
            self.output_ready.extend_from_slice(&output[0][..produced]);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::PI;

    use super::{CHUNK_FRAMES_IN, StreamResampler, TARGET_SAMPLE_RATE_HZ};
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
    /// `frequency_hz`, identical on both channels so downmixing to mono
    /// leaves the tone's amplitude unchanged.
    fn stereo_sine(frequency_hz: f32, frame_count: usize) -> Vec<f32> {
        let mut samples = Vec::with_capacity(frame_count * 2);
        for n in 0..frame_count {
            let phase = 2.0 * PI * frequency_hz * (n as f32) / SOURCE_SAMPLE_RATE_HZ as f32;
            let value = phase.sin();
            samples.push(value);
            samples.push(value);
        }
        samples
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
        let frame_count = CHUNK_FRAMES_IN * 100; // 48_000 frames = 1 s exactly
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
        let frame_count = CHUNK_FRAMES_IN * 100;
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
        let frame_count = CHUNK_FRAMES_IN * 6;
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
}
