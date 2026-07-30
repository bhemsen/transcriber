//! Tests for [`crate::session::Session`], split into its own file to keep
//! `session.rs` under the constitution's 400-line-per-module ceiling.

use super::Session;
use crate::plan::CapturePlan;
use std::thread;
use std::time::{Duration, SystemTime};
use transcriber_audio::{
    AudioSource, AudioSourceError, AudioSourceEvent, SourceFactory, StreamFormat, TestToneSources,
};
use transcriber_core::{AttestationVersion, ConsentAttestation, SessionState, StreamIdentity};

fn consent() -> ConsentAttestation {
    ConsentAttestation::new(SystemTime::now(), AttestationVersion::V1)
}

fn plan(factory: &TestToneSources) -> CapturePlan {
    let Ok(mut subjects) = factory.list_subjects() else {
        panic!("test factory always lists one subject");
    };
    let subject = subjects.remove(0);
    let Ok(remote) = factory.open_remote(&subject) else {
        panic!("test factory never fails to open");
    };
    let Ok(local) = factory.open_local() else {
        panic!("test factory never fails to open");
    };
    CapturePlan::new(subject, remote, local)
}

fn bounded_factory() -> TestToneSources {
    let Ok(factory) = TestToneSources::new() else {
        panic!("fixed 48 kHz stereo format is always valid");
    };
    factory.with_total_frames(480)
}

fn started_session(factory: &TestToneSources) -> Session {
    let Ok(session) = Session::start(consent(), plan(factory)) else {
        panic!("a consented start with valid sources must succeed");
    };
    session
}

#[test]
fn stop_zeroes_every_stream_buffer_no_non_zero_sample_remains() {
    let factory = bounded_factory();
    let mut session = started_session(&factory);

    let Ok(()) = session.request_stop() else {
        panic!("a Capturing session must accept request_stop");
    };
    assert!(
        !session.remote.all_zero(),
        "sanity: real audio must have been captured before stop zeroed it"
    );
    let local_had_samples = session
        .local
        .as_ref()
        .is_some_and(|local| !local.all_zero());
    assert!(local_had_samples, "sanity: the local stream captured too");

    let Ok(()) = session.end() else {
        panic!("a Stopping session must accept end");
    };

    assert!(session.remote.all_zero());
    assert!(session.local.as_ref().is_some_and(|local| local.all_zero()));
}

#[test]
fn ended_is_terminal_and_rejects_a_second_stop() {
    let factory = bounded_factory();
    let mut session = started_session(&factory);

    let Ok(()) = session.stop() else {
        panic!("stop must succeed from Capturing");
    };
    assert_eq!(session.state(), SessionState::Ended);

    assert!(session.request_stop().is_err());
    assert!(session.end().is_err());
}

#[test]
fn a_missing_microphone_warns_instead_of_aborting() {
    let factory = bounded_factory().without_microphone();
    let session = started_session(&factory);

    assert_eq!(session.state(), SessionState::Capturing);
    assert!(!session.has_local_stream());
    assert!(session.local_degradation().is_none());
}

/// A source whose every `pull` panics — stands in for a capture thread
/// that crashes instead of returning normally.
struct PanickingSource(StreamFormat);

impl AudioSource for PanickingSource {
    fn identity(&self) -> StreamIdentity {
        StreamIdentity::Remote
    }

    fn format(&self) -> StreamFormat {
        self.0
    }

    fn pull(&mut self, _timeout: Duration) -> Result<AudioSourceEvent, AudioSourceError> {
        panic!("synthetic capture-thread panic for the request_stop regression test");
    }
}

/// Regression test: `request_stop` must signal *every* stream's stop
/// flag before joining *any* of them. An earlier version joined
/// `remote` first and `?`-returned on
/// `SessionError::CaptureThreadPanicked` before ever signalling
/// `local`, leaving an unbounded `local` stream running — indistinguishable
/// from the outside except that it kept capturing live PCM past the
/// point `request_stop` returned.
///
/// Uses an *unbounded* local tone: only a real stop signal, never the
/// source running out of frames on its own, can be responsible for its
/// thread having already stopped by the time this returns.
#[test]
fn request_stop_still_stops_local_even_when_remote_panics() {
    let Ok(factory) = TestToneSources::new() else {
        panic!("fixed 48 kHz stereo format is always valid");
    };
    let Ok(mut subjects) = factory.list_subjects() else {
        panic!("test factory always lists one subject");
    };
    let subject = subjects.remove(0);
    let Ok(local) = factory.open_local() else {
        panic!("test factory never fails to open");
    };
    let Ok(format) = StreamFormat::new(48_000, 2) else {
        panic!("48 kHz stereo is always valid");
    };
    let remote: Box<dyn AudioSource> = Box::new(PanickingSource(format));
    let plan = CapturePlan::new(subject, remote, local);

    let Ok(mut session) = Session::start(consent(), plan) else {
        panic!("a consented start with valid sources must succeed");
    };

    let stop_result = session.request_stop();
    assert!(
        stop_result.is_err(),
        "a panicking remote source must surface as an error"
    );

    // `join()` on `local` is synchronous: if it already ran as part of
    // this `request_stop` call, the thread is fully finished by now and
    // `elapsed()` can never change again, sleep or not.
    let elapsed_right_after_stop = session.elapsed(StreamIdentity::Local);
    thread::sleep(Duration::from_millis(30));
    let elapsed_later = session.elapsed(StreamIdentity::Local);
    assert_eq!(
        elapsed_right_after_stop, elapsed_later,
        "local's capture thread must already be stopped once request_stop returns, \
             even though remote panicked"
    );
}
