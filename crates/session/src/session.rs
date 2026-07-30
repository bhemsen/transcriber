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
    /// Signals *every* stream before joining *any* of them (via
    /// [`Self::signal_all_streams_to_stop`]) — joining one stream that
    /// panicked must not `?`-return before a later stream's stop flag was
    /// ever set, which would leave it running with nothing left to tell it
    /// to stop. [`Drop`] mirrors this same ordering for a `Session`
    /// abandoned without an explicit `stop()`.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::State`] if this session is not `Capturing`,
    /// or [`SessionError::CaptureThreadPanicked`] if a capture thread
    /// panicked instead of returning normally — after every stream has
    /// still been signalled and joined.
    pub fn request_stop(&mut self) -> Result<(), SessionError> {
        self.state = self.state.request_stop()?;
        self.signal_all_streams_to_stop();
        let result = self.join_all_streams();
        let _ = self.events.send(SessionEvent::StateChanged(self.state));
        result
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
        self.zero_all_streams();
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

    /// Signals every stream to stop, without waiting for any of them.
    /// Shared by [`Session::request_stop`] and [`Drop`] — both must signal
    /// every stream before joining any of them.
    fn signal_all_streams_to_stop(&mut self) {
        self.remote.signal_stop();
        if let Some(local) = self.local.as_mut() {
            local.signal_stop();
        }
    }

    /// Joins every stream. Both joins always run, even if the first one
    /// errors, so a panicked stream never leaves a later one un-joined; the
    /// first error encountered (if any) is returned once every join has
    /// completed.
    fn join_all_streams(&mut self) -> Result<(), SessionError> {
        let remote_result = self.remote.join();
        let local_result = self.local.as_mut().map_or(Ok(()), StreamCapture::join);
        remote_result?;
        local_result
    }

    /// Explicitly zeroes every stream's buffer. Shared by [`Session::end`]
    /// and [`Drop`].
    fn zero_all_streams(&self) {
        self.remote.zeroize();
        if let Some(local) = self.local.as_ref() {
            local.zeroize();
        }
    }
}

impl Drop for Session {
    /// Mirrors [`Session::request_stop`]'s ordering for a `Session`
    /// dropped without an explicit `stop()` — a panic before it was
    /// called, or an early `?`-return in a caller. Without this, Rust's
    /// own field-by-field drop order (`remote` before `local`) would run
    /// *`remote`'s* [`StreamCapture`] `Drop` — which itself signals *and
    /// joins*, blocking — before `local`'s stop flag was ever set,
    /// reintroducing a bounded version of the same bug `request_stop` was
    /// fixed for. Idempotent: a `Session` already `stop()`ped hits only
    /// no-ops here, and the field-by-field `Drop`s that run immediately
    /// after this one returns then find nothing left to do either.
    fn drop(&mut self) {
        self.signal_all_streams_to_stop();
        let _ = self.join_all_streams();
        self.zero_all_streams();
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
mod tests;
