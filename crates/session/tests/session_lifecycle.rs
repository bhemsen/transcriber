//! Black-box coverage of `Session`'s public lifecycle, entirely device-free
//! via `TestToneSources` (issue #7) — the only way anything here runs on a
//! CI runner with no audio device (spec, risk table).
//!
//! `build_plan` here is reused, line for line, by
//! `tests/compile_fail/session_requires_consent_attestation.rs` — so a typo
//! in that construction code would already fail *this* file first, not the
//! compile_fail fixture, which must fail to compile for exactly one reason:
//! the missing `ConsentAttestation`.

use std::time::SystemTime;

use transcriber_audio::{SourceFactory, TestToneSources};
use transcriber_core::{AttestationVersion, ConsentAttestation, StreamIdentity};
use transcriber_session::{CapturePlan, Session, SessionEvent, SessionState};

fn consent() -> ConsentAttestation {
    ConsentAttestation::new(SystemTime::now(), AttestationVersion::V1)
}

fn build_plan(factory: &TestToneSources) -> CapturePlan {
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

fn new_factory() -> TestToneSources {
    let Ok(factory) = TestToneSources::new() else {
        panic!("fixed 48 kHz stereo format is always valid");
    };
    factory
}

fn bounded_factory() -> TestToneSources {
    new_factory().with_total_frames(480)
}

fn started_session(factory: &TestToneSources) -> Session {
    let Ok(session) = Session::start(consent(), build_plan(factory)) else {
        panic!("a consented start with valid sources must succeed");
    };
    session
}

#[test]
fn start_reaches_capturing_with_both_streams_and_no_tokio_runtime() {
    let session = started_session(&new_factory());

    assert_eq!(session.state(), SessionState::Capturing);
    assert!(session.has_local_stream());
    assert!(session.remote_degradation().is_none());
}

#[test]
fn a_missing_microphone_is_a_warning_not_an_abort() {
    let factory = new_factory().without_microphone();
    let session = started_session(&factory);

    assert_eq!(session.state(), SessionState::Capturing);
    assert!(!session.has_local_stream());
    assert!(session.elapsed(StreamIdentity::Local).is_none());
    assert!(session.elapsed(StreamIdentity::Remote).is_some());
}

#[test]
fn stop_moves_through_both_edges_to_ended() {
    let mut session = started_session(&bounded_factory());

    let Ok(()) = session.request_stop() else {
        panic!("a Capturing session must accept request_stop");
    };
    assert_eq!(session.state(), SessionState::Stopping);

    let Ok(()) = session.end() else {
        panic!("a Stopping session must accept end");
    };
    assert_eq!(session.state(), SessionState::Ended);
}

#[test]
fn ended_is_terminal_and_never_restarts() {
    let mut session = started_session(&bounded_factory());

    let Ok(()) = session.stop() else {
        panic!("stop must succeed from Capturing");
    };

    assert!(session.request_stop().is_err());
    assert!(session.end().is_err());
}

#[test]
fn subscribers_receive_the_lifecycle_events() {
    let mut session = started_session(&bounded_factory());
    let mut receiver = session.subscribe();

    let Ok(()) = session.stop() else {
        panic!("stop must succeed from Capturing");
    };

    let mut states = Vec::new();
    while let Ok(event) = receiver.try_recv() {
        if let SessionEvent::StateChanged(state) = event {
            states.push(state);
        }
    }
    assert_eq!(states, vec![SessionState::Stopping, SessionState::Ended]);
}
