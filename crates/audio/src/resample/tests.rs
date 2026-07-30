//! Tests for [`crate::resample`], split into their own file so the shipped
//! code in `resample.rs` stays well under the constitution's 400-line
//! guideline (`docs/constitution.md`, Architecture principles).

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

/// [`stereo_sine_at`] at the fixed [`SOURCE_SAMPLE_RATE_HZ`] every test but
/// the non-48-kHz regression test below uses.
fn stereo_sine(frequency_hz: f32, frame_count: usize) -> Vec<f32> {
    stereo_sine_at(frequency_hz, SOURCE_SAMPLE_RATE_HZ, frame_count)
}

/// Like [`stereo_sine`], but the right channel is the exact negation of
/// the left. A *correctly* paired downmix averages `value` and `-value`
/// to exactly `0.0` for every frame — not approximately, since IEEE 754
/// subtraction of a value from itself is exact — so any test using this
/// can assert on the output being all zeros. A frame-alignment bug (a
/// leftover sample from the wrong side of a channel boundary pairing
/// with the wrong partner) would instead surface as a nonzero residual,
/// which a same-channel test like [`stereo_sine`] cannot distinguish
/// from a correctly downmixed tone.
fn antisymmetric_stereo(frequency_hz: f32, frame_count: usize) -> Vec<f32> {
    let mut samples = Vec::with_capacity(frame_count * 2);
    for n in 0..frame_count {
        let phase = 2.0 * PI * frequency_hz * (n as f32) / SOURCE_SAMPLE_RATE_HZ as f32;
        let value = phase.sin();
        samples.push(value);
        samples.push(-value);
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

/// A same-value-on-both-channels signal (as every other test here uses)
/// cannot tell a correct downmix from one that pairs a leftover sample
/// with the wrong partner across a frame boundary — both give back the
/// same tone either way. [`antisymmetric_stereo`] closes that gap:
/// paired correctly, every mono sample is exactly `0.0`; paired even one
/// frame off, it is not. Combined with the same misaligned 5-sample push
/// pattern as the test above, this is a direct, exact check that
/// `raw_leftover` never drifts a sample across a channel boundary.
#[test]
fn misaligned_pushes_never_pair_samples_across_a_frame_boundary() {
    let frame_count = CHUNK_FRAMES_IN_HINT * 2;
    let input = antisymmetric_stereo(1_000.0, frame_count);

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

    assert_eq!(output.len(), frame_count / 3);
    assert!(
        output.iter().all(|&sample| sample == 0.0),
        "a correctly paired downmix of value/-value must be exactly zero throughout; \
         a nonzero sample means raw_leftover paired across the wrong frame boundary"
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

    // Deterministic, not approximate: rubato rounds 44.1 kHz -> 16 kHz to
    // 882-frame input / 320-frame output chunks (gcd(44100, 16000) =
    // 100), and 88_200 input frames is exactly 100 such chunks — so the
    // exact output length also catches a defect that drops some chunks,
    // not just the original bug, which produced 0.
    assert_eq!(
        output.len(),
        32_000,
        "44.1 kHz input must resample to exactly 100 chunks of 320, not be silently discarded or truncated"
    );

    let at_tone = goertzel_magnitude(&output, TARGET_SAMPLE_RATE_HZ as f32, 1_000.0);
    let at_other = goertzel_magnitude(&output, TARGET_SAMPLE_RATE_HZ as f32, 5_000.0);
    assert!(
        at_tone > at_other * 10.0,
        "the 1 kHz tone must survive a non-48 kHz source rate: {at_tone} vs {at_other}"
    );
}
