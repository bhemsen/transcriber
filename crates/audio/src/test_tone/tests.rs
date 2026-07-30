//! Tests for [`super::TestToneSource`] and [`super::TestToneSources`], split
//! into their own file so the shipped code in `test_tone.rs` stays well
//! under the constitution's 400-line guideline
//! (`docs/constitution.md`, Architecture principles).

use std::f64::consts::PI;
use std::num::NonZeroU64;
use std::time::{Duration, SystemTime};

use transcriber_core::{CaptureSubject, StreamIdentity};

use super::{TestToneSource, TestToneSources};
use crate::factory::SourceFactory;
use crate::format::StreamFormat;
use crate::frame::Frame;
use crate::source::{AudioSource, AudioSourceEvent};

fn stereo_format() -> StreamFormat {
    let Ok(format) = StreamFormat::new(48_000, 2) else {
        panic!("48 kHz stereo must be a valid format");
    };
    format
}

/// The tone's documented definition — `amplitude * sin(2*pi*f*n/sr)` at
/// frame index `n` — evaluated independently of [`TestToneSource`]'s own
/// phase-accumulator implementation, so a test using this can catch a
/// drifted or wrongly-scaled implementation rather than just restate it.
fn expected_sample(n: u64, frequency_hz: f32, sample_rate_hz: u32) -> f32 {
    let n = n as f64;
    let phase = 2.0 * PI * f64::from(frequency_hz) * n / f64::from(sample_rate_hz);
    (phase.sin() as f32) * 0.2
}

fn pull_frame(source: &mut TestToneSource) -> Frame {
    match source.pull(Duration::from_millis(50)) {
        Ok(AudioSourceEvent::Frame(frame)) => frame,
        other => panic!("expected a frame, got {other:?}"),
    }
}

fn nonzero(n: u64) -> NonZeroU64 {
    let Some(value) = NonZeroU64::new(n) else {
        panic!("test constant must be nonzero");
    };
    value
}

#[test]
fn identity_and_format_match_construction_arguments() {
    let format = stereo_format();
    let source = TestToneSource::new(StreamIdentity::Local, format, 440.0);
    assert_eq!(source.identity(), StreamIdentity::Local);
    assert_eq!(source.format(), format);
}

#[test]
fn pull_produces_the_documented_sine_values() {
    let format = stereo_format();
    let mut source = TestToneSource::new(StreamIdentity::Remote, format, 1_000.0);

    let Frame::Samples { samples, .. } = pull_frame(&mut source) else {
        panic!("the test tone must produce a Samples frame");
    };
    for frame_index in 0..5u64 {
        let expected = expected_sample(frame_index, 1_000.0, 48_000);
        let left = samples[(frame_index * 2) as usize];
        let right = samples[(frame_index * 2 + 1) as usize];
        assert!((left - expected).abs() < 1e-4, "left {left} vs {expected}");
        assert_eq!(left, right, "both channels must carry the same tone");
    }
}

/// The second chunk must start exactly where the first left off — frame
/// index `FRAMES_PER_PULL`, not `0` — proving the phase accumulator carries
/// across `pull()` calls instead of restarting the waveform at each one.
#[test]
fn the_phase_accumulator_stays_continuous_across_pulls() {
    let format = stereo_format();
    let mut source = TestToneSource::new(StreamIdentity::Remote, format, 1_000.0);

    let _ = pull_frame(&mut source);
    let Frame::Samples { samples, .. } = pull_frame(&mut source) else {
        panic!("the test tone must produce a Samples frame");
    };
    let expected = expected_sample(TestToneSource::FRAMES_PER_PULL, 1_000.0, 48_000);
    let left = samples[0];
    assert!((left - expected).abs() < 1e-4, "left {left} vs {expected}");
}

#[test]
fn amplitude_never_exceeds_the_configured_ceiling() {
    let format = stereo_format();
    let mut source = TestToneSource::new(StreamIdentity::Remote, format, 5_000.0);
    for _ in 0..10 {
        let Frame::Samples { samples, .. } = pull_frame(&mut source) else {
            panic!("the test tone must produce a Samples frame");
        };
        assert!(
            samples
                .iter()
                .all(|&sample| sample.abs() <= 0.2 + f32::EPSILON),
            "no sample may exceed the configured amplitude ceiling"
        );
    }
}

/// The [`AudioSourceEvent::Idle`]/[`AudioSourceEvent::Ended`] split matters
/// precisely because a bounded source must stop exactly at its budget, not
/// one chunk early or late, and must keep reporting `Ended` afterwards
/// rather than resuming or panicking.
#[test]
fn bounded_tone_ends_exactly_after_its_frame_budget() {
    let format = stereo_format();
    let frame_budget = TestToneSource::FRAMES_PER_PULL * 2 + 100; // spans three pulls
    let mut source =
        TestToneSource::new(StreamIdentity::Remote, format, 440.0).with_total_frames(frame_budget);

    let mut total_frames = 0u64;
    loop {
        let event = match source.pull(Duration::from_millis(10)) {
            Ok(event) => event,
            Err(error) => panic!("a bounded test tone must not error: {error}"),
        };
        match event {
            AudioSourceEvent::Frame(frame) => total_frames += frame.sample_count() as u64 / 2,
            AudioSourceEvent::Ended => break,
            AudioSourceEvent::Idle => panic!("the test tone never reports Idle"),
        }
    }
    assert_eq!(total_frames, frame_budget);

    let Ok(AudioSourceEvent::Ended) = source.pull(Duration::from_millis(10)) else {
        panic!("Ended must be terminal: a further pull must keep reporting it");
    };
}

/// The device-free stand-in for the Windows backend's event-wait timeout:
/// *exactly* every third `pull()` (not merely three calls out of nine, in
/// whichever position) must report `Idle`, and the phase accumulator must
/// not move during an `Idle` call — checked by comparing the first sample
/// of the next frame against [`expected_sample`] at the frame index the
/// tone would be at if only the six frame-bearing pulls ever advanced it,
/// which catches a cadence off-by-one or a phase-advancing `Idle` branch
/// that a plain idle/frame *count* could not. Without this mechanism at
/// all, `#9`'s capture loop and `#14`'s CLI would ship their `Idle`
/// handling untested until it first meets a real device.
#[test]
fn with_idle_every_reports_idle_on_the_configured_cadence() {
    let format = stereo_format();
    let mut source =
        TestToneSource::new(StreamIdentity::Remote, format, 440.0).with_idle_every(nonzero(3));

    let mut expected_frame_index = 0u64;
    for pull_number in 1..=9u64 {
        let event = match source.pull(Duration::from_millis(10)) {
            Ok(event) => event,
            Err(error) => panic!("the test tone must not error: {error}"),
        };
        if pull_number % 3 == 0 {
            assert!(
                matches!(event, AudioSourceEvent::Idle),
                "pull #{pull_number} must be Idle, got {event:?}"
            );
            continue;
        }
        let AudioSourceEvent::Frame(Frame::Samples { samples, .. }) = event else {
            panic!("pull #{pull_number} must produce a Samples frame, got {event:?}");
        };
        let expected = expected_sample(expected_frame_index, 440.0, 48_000);
        assert!(
            (samples[0] - expected).abs() < 1e-4,
            "pull #{pull_number}: an intervening Idle pull must not move the phase accumulator; \
             got {} vs {expected}",
            samples[0]
        );
        expected_frame_index += TestToneSource::FRAMES_PER_PULL;
    }
}

/// A session test that only ever sees `Box<dyn SourceFactory>` — the entire
/// rationale for making the factory injectable — must still be able to
/// drive an opened stream through `Idle` and to a bounded `Ended`, not just
/// the direct [`TestToneSource`] constructor.
#[test]
fn factory_pass_through_configures_idle_and_bounded_opened_sources() {
    let Ok(factory) = TestToneSources::new() else {
        panic!("the fixed 48 kHz stereo format must always construct");
    };
    let frame_budget = TestToneSource::FRAMES_PER_PULL * 2;
    let factory = factory
        .with_idle_every(nonzero(2))
        .with_total_frames(frame_budget);
    let subject = CaptureSubject::new("anything", 1234, SystemTime::now());
    let Ok(mut remote) = factory.open_remote(&subject) else {
        panic!("opening the synthetic remote source must not fail");
    };

    let mut idle_calls = 0;
    let mut total_frames = 0u64;
    loop {
        match remote.pull(Duration::from_millis(10)) {
            Ok(AudioSourceEvent::Idle) => idle_calls += 1,
            Ok(AudioSourceEvent::Frame(frame)) => total_frames += frame.sample_count() as u64 / 2,
            Ok(AudioSourceEvent::Ended) => break,
            Err(error) => panic!("a bounded test tone must not error: {error}"),
        }
    }
    assert!(
        idle_calls > 0,
        "the factory's with_idle_every must reach the opened source"
    );
    assert_eq!(total_frames, frame_budget);
}

#[test]
fn list_subjects_returns_the_one_synthetic_subject() {
    let Ok(factory) = TestToneSources::new() else {
        panic!("the fixed 48 kHz stereo format must always construct");
    };
    let Ok(subjects) = factory.list_subjects() else {
        panic!("the test-tone factory must never fail to enumerate");
    };
    assert_eq!(subjects.len(), 1);
    assert_eq!(subjects[0].process_name(), "test-tone");
}

#[test]
fn open_remote_and_open_local_produce_distinct_identities() {
    let Ok(factory) = TestToneSources::new() else {
        panic!("the fixed 48 kHz stereo format must always construct");
    };
    let subject = CaptureSubject::new("anything", 1234, SystemTime::now());
    let Ok(remote) = factory.open_remote(&subject) else {
        panic!("opening the synthetic remote source must not fail");
    };
    let Ok(Some(local)) = factory.open_local() else {
        panic!("a microphone-present factory must open a local source");
    };
    assert_eq!(remote.identity(), StreamIdentity::Remote);
    assert_eq!(local.identity(), StreamIdentity::Local);
}

/// `open_remote` has no way to look up a real application yet — the test
/// tone stands in for whichever subject is passed, so this only proves the
/// contract "any subject opens *a* remote stream", not identity matching
/// (that belongs to `audio-win`'s real implementation).
#[test]
fn open_remote_ignores_which_subject_is_passed() {
    let Ok(factory) = TestToneSources::new() else {
        panic!("the fixed 48 kHz stereo format must always construct");
    };
    let subject_a = CaptureSubject::new("app-a", 100, SystemTime::now());
    let subject_b = CaptureSubject::new("app-b", 200, SystemTime::now());
    let Ok(a) = factory.open_remote(&subject_a) else {
        panic!("opening the synthetic remote source must not fail");
    };
    let Ok(b) = factory.open_remote(&subject_b) else {
        panic!("opening the synthetic remote source must not fail");
    };
    assert_eq!(a.identity(), StreamIdentity::Remote);
    assert_eq!(b.identity(), StreamIdentity::Remote);
}

/// The spec's prior decision: a missing microphone is `Ok(None)`, a warning
/// for the caller to surface, never an `Err`.
#[test]
fn without_microphone_makes_open_local_return_none() {
    let Ok(factory) = TestToneSources::new() else {
        panic!("the fixed 48 kHz stereo format must always construct");
    };
    let factory = factory.without_microphone();
    let Ok(local) = factory.open_local() else {
        panic!("a missing microphone must be Ok(None), not an error");
    };
    assert!(local.is_none());
}

#[test]
fn source_factory_is_object_safe() {
    let Ok(factory) = TestToneSources::new() else {
        panic!("the fixed 48 kHz stereo format must always construct");
    };
    let factory: Box<dyn SourceFactory> = Box::new(factory);
    let Ok(subjects) = factory.list_subjects() else {
        panic!("the test-tone factory must never fail to enumerate");
    };
    assert_eq!(subjects.len(), 1);
}

/// `session` (the next issue) moves a `Box<dyn AudioSource>` onto its own
/// capture thread — this must compile and run, which it only does because
/// [`crate::source::AudioSource`] carries `Send` as a supertrait.
#[test]
fn a_boxed_source_can_move_onto_its_own_capture_thread() {
    let format = stereo_format();
    let source: Box<dyn AudioSource> =
        Box::new(TestToneSource::new(StreamIdentity::Remote, format, 220.0));

    let handle = std::thread::spawn(move || {
        let mut source = source;
        matches!(
            source.pull(Duration::from_millis(10)),
            Ok(AudioSourceEvent::Frame(_))
        )
    });

    let Ok(produced_a_frame) = handle.join() else {
        panic!("the capture thread must not panic");
    };
    assert!(produced_a_frame);
}
