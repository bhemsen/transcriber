//! The session orchestrator: the type-bound consent gate, the capture
//! threads it owns, and the lifecycle that ends in zeroed buffers.

use std::time::Duration;

use tokio::sync::broadcast;
use transcriber_audio::{AudioSource, SourceDegradation};
use transcriber_core::{
    CaptureSubject, ConsentAttestation, SessionId, SessionState, StreamIdentity,
};

use crate::capture::StreamCapture;
use crate::clock::SessionZero;
use crate::error::SessionError;
use crate::event::SessionEvent;
use crate::plan::CapturePlan;

/// Bounded lag between an event being published and the slowest subscriber
/// reading it before `broadcast` starts reporting it as missed — generous
/// for the handful of lifecycle events this phase emits.
const EVENT_BUS_CAPACITY: usize = 32;

/// A consented, running (or having run) capture session.
///
/// [`Session::start`] is the *only* constructor and its first parameter is
/// a [`ConsentAttestation`] — there is no `Default`, no public field, no
/// builder and nothing `#[cfg(test)]`-gated that bypasses it. Every field
/// below is private for exactly that reason: a public field would be a
/// second way to assemble a `Session`.
///
/// No `Clone`, no `Default`: states `Capturing -> Stopping -> Ended` have
/// exactly two edges and `Ended` is terminal, so a session is never
/// duplicated or restarted (`docs/specs/spec-capture-foundation.md`).
pub struct Session {
    id: SessionId,
    consent: ConsentAttestation,
    subject: CaptureSubject,
    state: SessionState,
    remote: StreamCapture,
    local: Option<StreamCapture>,
    events: broadcast::Sender<SessionEvent>,
}

impl Session {
    /// Starts a session: records a common session zero every stream's
    /// device timeline is normalised against, then spawns one capture
    /// thread per opened source in `plan`.
    ///
    /// A missing microphone (`plan`'s `local` source absent) is a warning,
    /// not an abort — the session starts with the `Remote` stream alone and
    /// publishes [`SessionEvent::MicrophoneUnavailable`].
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::ThreadSpawn`] if the operating system refuses
    /// to start a stream's capture thread.
    pub fn start(consent: ConsentAttestation, plan: CapturePlan) -> Result<Self, SessionError> {
        let (subject, remote_source, local_source) = plan.into_parts();
        let session_zero = SessionZero::record();
        let (events, _receiver) = broadcast::channel(EVENT_BUS_CAPACITY);

        let remote = StreamCapture::spawn(
            StreamIdentity::Remote,
            remote_source,
            session_zero,
            events.clone(),
        )?;
        announce_degradation(&events, StreamIdentity::Remote, remote.degradation());

        let local = Self::start_local(local_source, session_zero, &events)?;

        Ok(Self {
            id: SessionId::new(),
            consent,
            subject,
            state: SessionState::Capturing,
            remote,
            local,
            events,
        })
    }

    /// Spawns the `Local` (microphone) stream if `local_source` opened one,
    /// otherwise publishes [`SessionEvent::MicrophoneUnavailable`] and
    /// returns `None` — the spec's decision that an absent microphone
    /// continues the session rather than refusing it.
    fn start_local(
        local_source: Option<Box<dyn AudioSource>>,
        session_zero: SessionZero,
        events: &broadcast::Sender<SessionEvent>,
    ) -> Result<Option<StreamCapture>, SessionError> {
        let Some(source) = local_source else {
            let _ = events.send(SessionEvent::MicrophoneUnavailable);
            return Ok(None);
        };
        let capture =
            StreamCapture::spawn(StreamIdentity::Local, source, session_zero, events.clone())?;
        announce_degradation(events, StreamIdentity::Local, capture.degradation());
        Ok(Some(capture))
    }

    /// Process-unique identifier of this session.
    #[must_use]
    pub fn id(&self) -> SessionId {
        self.id
    }

    /// The proof this session could start at all.
    #[must_use]
    pub fn consent(&self) -> &ConsentAttestation {
        &self.consent
    }

    /// The application this session captures.
    #[must_use]
    pub fn subject(&self) -> &CaptureSubject {
        &self.subject
    }

    /// Where this session is in its lifecycle.
    #[must_use]
    pub fn state(&self) -> SessionState {
        self.state
    }

    /// Whether this session has a `Local` (microphone) stream — `false`
    /// after a warned, microphone-less start, never after an abort.
    #[must_use]
    pub fn has_local_stream(&self) -> bool {
        self.local.is_some()
    }

    /// The capability degradation the `Remote` stream's source reported
    /// when opened, if any.
    #[must_use]
    pub fn remote_degradation(&self) -> Option<SourceDegradation> {
        self.remote.degradation()
    }

    /// The capability degradation the `Local` stream's source reported when
    /// opened — `None` both when there is no degradation and when there is
    /// no `Local` stream at all.
    #[must_use]
    pub fn local_degradation(&self) -> Option<SourceDegradation> {
        self.local.as_ref().and_then(StreamCapture::degradation)
    }

    /// How far into the session `identity`'s clock has advanced, relative
    /// to the common session zero recorded at [`Session::start`] —
    /// `None` only for `Local` when there is no microphone stream.
    #[must_use]
    pub fn elapsed(&self, identity: StreamIdentity) -> Option<Duration> {
        match identity {
            StreamIdentity::Remote => Some(self.remote.elapsed()),
            StreamIdentity::Local => self.local.as_ref().map(StreamCapture::elapsed),
        }
    }

    /// Subscribes to this session's event bus. Publishing
    /// (`tokio::sync::broadcast::Sender::send`) never needs a running
    /// tokio runtime, but *receiving* asynchronously does — no runtime is
    /// entered anywhere on the capture path itself.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<SessionEvent> {
        self.events.subscribe()
    }

    /// Moves `Capturing -> Stopping`: signals every capture thread to stop,
    /// then blocks until each has, so no thread is still writing once this
    /// returns.
    ///
    /// Signals *every* stream before joining *any* of them — joining one
    /// stream that panicked must not `?`-return before a later stream's
    /// stop flag was ever set, which would leave it running with nothing
    /// left to tell it to stop.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::State`] if this session is not `Capturing`,
    /// or [`SessionError::CaptureThreadPanicked`] if a capture thread
    /// panicked instead of returning normally — after every stream has
    /// still been signalled and joined.
    pub fn request_stop(&mut self) -> Result<(), SessionError> {
        self.state = self.state.request_stop()?;
        self.remote.signal_stop();
        if let Some(local) = self.local.as_mut() {
            local.signal_stop();
        }

        let remote_result = self.remote.join();
        let local_result = self.local.as_mut().map_or(Ok(()), StreamCapture::join);
        let _ = self.events.send(SessionEvent::StateChanged(self.state));
        remote_result?;
        local_result
    }

    /// Moves `Stopping -> Ended`: explicitly zeroes every stream's buffer —
    /// `docs/constitution.md`, PCM is zeroed at session end, not merely
    /// dropped. Terminal: no edge leads out of `Ended`.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::State`] if this session is not `Stopping`.
    pub fn end(&mut self) -> Result<(), SessionError> {
        self.state = self.state.end()?;
        self.remote.zeroize();
        if let Some(local) = self.local.as_ref() {
            local.zeroize();
        }
        let _ = self.events.send(SessionEvent::StateChanged(self.state));
        Ok(())
    }

    /// Convenience for [`Session::request_stop`] immediately followed by
    /// [`Session::end`] — both edges of the state machine in one call, for
    /// a caller with no reason to observe the intermediate `Stopping`
    /// state.
    ///
    /// # Errors
    ///
    /// Whatever [`Session::request_stop`] or [`Session::end`] returns.
    pub fn stop(&mut self) -> Result<(), SessionError> {
        self.request_stop()?;
        self.end()
    }
}

/// Publishes [`SessionEvent::StreamDegraded`] for `identity` if `degradation`
/// is present — the one place [`Session::start`] and
/// [`Session::start_local`] share this check.
fn announce_degradation(
    events: &broadcast::Sender<SessionEvent>,
    identity: StreamIdentity,
    degradation: Option<SourceDegradation>,
) {
    if let Some(degradation) = degradation {
        let _ = events.send(SessionEvent::StreamDegraded {
            identity,
            degradation,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::Session;
    use crate::plan::CapturePlan;
    use std::thread;
    use std::time::{Duration, SystemTime};
    use transcriber_audio::{
        AudioSource, AudioSourceError, AudioSourceEvent, SourceFactory, StreamFormat,
        TestToneSources,
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
}
