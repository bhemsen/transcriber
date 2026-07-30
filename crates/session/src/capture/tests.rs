use super::StreamCapture;
use crate::clock::SessionZero;
use crate::event::SessionEvent;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use transcriber_audio::{
    AudioSource, AudioSourceError, AudioSourceEvent, SourceDegradation, StreamFormat,
    TestToneSource,
};
use transcriber_core::StreamIdentity;

fn format() -> StreamFormat {
    let Ok(format) = StreamFormat::new(48_000, 2) else {
        panic!("48 kHz stereo is always valid");
    };
    format
}

fn events() -> broadcast::Sender<SessionEvent> {
    broadcast::channel(8).0
}

#[test]
fn stop_joins_a_bounded_stream_and_then_zeroes_it() {
    let source =
        TestToneSource::new(StreamIdentity::Remote, format(), 440.0).with_total_frames(480);
    let Ok(mut capture) = StreamCapture::spawn(
        StreamIdentity::Remote,
        Box::new(source),
        SessionZero::record(),
        events(),
    ) else {
        panic!("spawning a capture thread must not fail");
    };

    capture.signal_stop();
    let Ok(()) = capture.join() else {
        panic!("a freshly spawned thread must join cleanly");
    };

    assert!(capture.elapsed() > Duration::ZERO);
    assert!(!capture.all_zero(), "sanity: real samples were captured");

    capture.zeroize();
    assert!(capture.all_zero());
}

/// A `StreamCapture` dropped *without* ever calling `signal_stop`/`join`
/// — a panic between `Session::start` and `stop`, or an early
/// `?`-return in a caller — must still stop its thread and zero its
/// buffer. Uses an *unbounded* tone (no `with_total_frames`): only
/// `Drop`, never the source running out of frames on its own, can be
/// responsible for the thread stopping here.
#[test]
fn dropping_without_stop_still_stops_the_thread_and_zeroes_the_buffer() {
    let source = TestToneSource::new(StreamIdentity::Remote, format(), 440.0);
    let Ok(capture) = StreamCapture::spawn(
        StreamIdentity::Remote,
        Box::new(source),
        SessionZero::record(),
        events(),
    ) else {
        panic!("spawning a capture thread must not fail");
    };

    // Wait for the first frame — the synthetic tone never blocks on
    // `pull`, so this settles almost immediately.
    while capture.elapsed() == Duration::ZERO {
        std::thread::yield_now();
    }
    let ring = Arc::clone(&capture.ring);
    assert!(
        !capture.all_zero(),
        "sanity: real audio must have been captured before drop"
    );

    drop(capture);

    let all_zero = match ring.lock() {
        Ok(ring) => ring.all_zero(),
        Err(poisoned) => poisoned.into_inner().all_zero(),
    };
    assert!(
        all_zero,
        "dropping a StreamCapture without an explicit stop() must still zero its buffer"
    );
}

/// A source that always discloses the AEC degradation — proves
/// [`StreamCapture::degradation`] actually surfaces what
/// [`AudioSource::degradation`] reports, not just the trait's `None`
/// default that [`TestToneSource`] itself always returns.
struct DegradedSource(TestToneSource);

impl AudioSource for DegradedSource {
    fn identity(&self) -> StreamIdentity {
        self.0.identity()
    }

    fn format(&self) -> StreamFormat {
        self.0.format()
    }

    fn pull(&mut self, timeout: Duration) -> Result<AudioSourceEvent, AudioSourceError> {
        self.0.pull(timeout)
    }

    fn degradation(&self) -> Option<SourceDegradation> {
        Some(SourceDegradation::EchoCancellationUnavailable)
    }
}

#[test]
fn degradation_reported_by_the_source_is_reachable_after_spawn() {
    let inner = TestToneSource::new(StreamIdentity::Local, format(), 660.0).with_total_frames(480);
    let Ok(mut capture) = StreamCapture::spawn(
        StreamIdentity::Local,
        Box::new(DegradedSource(inner)),
        SessionZero::record(),
        events(),
    ) else {
        panic!("spawning a capture thread must not fail");
    };

    assert_eq!(
        capture.degradation(),
        Some(SourceDegradation::EchoCancellationUnavailable)
    );

    capture.signal_stop();
    let Ok(()) = capture.join() else {
        panic!("a freshly spawned thread must join cleanly");
    };
}

/// Proves [`StreamCapture::stats`] actually reflects real captured audio —
/// not just the zeroed defaults a fresh reader would report — and that
/// `frame_count` lines up with the stereo format's channel count.
#[test]
fn stats_reflect_real_captured_audio() {
    let source =
        TestToneSource::new(StreamIdentity::Remote, format(), 440.0).with_total_frames(480);
    let Ok(mut capture) = StreamCapture::spawn(
        StreamIdentity::Remote,
        Box::new(source),
        SessionZero::record(),
        events(),
    ) else {
        panic!("spawning a capture thread must not fail");
    };

    capture.signal_stop();
    let Ok(()) = capture.join() else {
        panic!("a freshly spawned thread must join cleanly");
    };

    let stats = capture.stats();
    assert_eq!(stats.frame_count, 480);
    assert!(stats.level_dbfs.is_finite());
    assert_eq!(stats.loss_count, 0);
    assert_eq!(stats.gap_count, 0);
    assert_eq!(stats.degradation, None);
}
