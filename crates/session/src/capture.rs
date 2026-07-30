//! One stream's capture thread and the fixed ring buffer it owns.
//!
//! Ownership boundary (spec): an [`AudioSource`] only *produces* frames
//! through `pull` and does no unbounded buffering of its own — this module
//! is the other half, owning the thread that loops `pull` and the
//! [`RingBuffer`] each frame lands in.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use tokio::sync::broadcast;
use transcriber_audio::{AudioSource, AudioSourceEvent, RingBuffer, SourceDegradation};
use transcriber_core::StreamIdentity;

use crate::clock::{SessionZero, StreamClock};
use crate::error::SessionError;
use crate::event::SessionEvent;
use crate::stats::{StatsReader, StreamStats};

/// How long a capture thread waits for [`AudioSource::pull`] before looping
/// back to check whether it has been asked to stop. The synthetic test
/// tone ignores this value entirely — it never blocks — so in tests this
/// only bounds how promptly a stop request is noticed; a real backend's
/// event-wait timeout is a different, backend-owned duration.
const PULL_TIMEOUT: Duration = Duration::from_millis(200);

/// One stream's capture thread, its fixed [`RingBuffer`], and the
/// session-relative position its clock has reached.
pub(crate) struct StreamCapture {
    identity: StreamIdentity,
    ring: Arc<Mutex<RingBuffer>>,
    position: Arc<Mutex<Duration>>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    degradation: Option<SourceDegradation>,
    stats: Mutex<StatsReader>,
}

impl StreamCapture {
    /// Spawns a dedicated thread that loops `source.pull`, pushing frames
    /// into a fresh [`RingBuffer`] sized for `source`'s format, until it is
    /// asked to stop or the source itself signals
    /// [`AudioSourceEvent::Ended`].
    pub(crate) fn spawn(
        identity: StreamIdentity,
        source: Box<dyn AudioSource>,
        session_zero: SessionZero,
        events: broadcast::Sender<SessionEvent>,
    ) -> Result<Self, SessionError> {
        let degradation = source.degradation();
        let format = source.format();
        let ring = Arc::new(Mutex::new(RingBuffer::new(format)));
        // Registered here, before the capture thread ever runs, so the
        // statistics reader sees every sample from the very first frame —
        // `RingBuffer::add_reader` only sees samples pushed after it is
        // called.
        let stats = Mutex::new(match ring.lock() {
            Ok(mut ring) => StatsReader::register(&mut ring, format.channels()),
            Err(poisoned) => StatsReader::register(&mut poisoned.into_inner(), format.channels()),
        });
        let position = Arc::new(Mutex::new(Duration::ZERO));
        let stop = Arc::new(AtomicBool::new(false));

        let thread_ring = Arc::clone(&ring);
        let thread_position = Arc::clone(&position);
        let thread_stop = Arc::clone(&stop);

        let handle = thread::Builder::new()
            .name(format!("capture-{identity:?}"))
            .spawn(move || {
                capture_loop(
                    identity,
                    source,
                    &thread_ring,
                    &thread_position,
                    &thread_stop,
                    session_zero,
                    &events,
                );
            })
            .map_err(|error| SessionError::ThreadSpawn {
                identity,
                reason: error.to_string(),
            })?;

        Ok(Self {
            identity,
            ring,
            position,
            stop,
            handle: Some(handle),
            degradation,
            stats,
        })
    }

    /// Signals the capture thread to stop, without waiting for it.
    /// Idempotent — safe to call more than once, including from
    /// [`Drop::drop`].
    ///
    /// Deliberately separate from [`Self::join`]: a caller stopping several
    /// streams must signal *every* one before joining *any* of them, or an
    /// error joining one stream (e.g. a panicked thread) would `?`-return
    /// before a later stream's flag was ever set — leaving it running with
    /// nothing left to stop it. See [`crate::Session::request_stop`].
    pub(crate) fn signal_stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    /// Blocks until the capture thread has stopped. Call
    /// [`Self::signal_stop`] first on every stream that must stop.
    /// Idempotent: a second call after the handle has already been taken
    /// is a no-op `Ok(())`.
    pub(crate) fn join(&mut self) -> Result<(), SessionError> {
        let Some(handle) = self.handle.take() else {
            return Ok(());
        };
        handle
            .join()
            .map_err(|_| SessionError::CaptureThreadPanicked {
                identity: self.identity,
            })
    }

    /// Explicitly zeroes every sample this stream's buffer currently holds —
    /// `docs/constitution.md`: PCM is zeroed at session end, not merely
    /// dropped.
    pub(crate) fn zeroize(&self) {
        match self.ring.lock() {
            Ok(mut ring) => ring.zeroize(),
            Err(poisoned) => poisoned.into_inner().zeroize(),
        }
    }

    /// The capability degradation this stream's source reported when it was
    /// opened, if any.
    pub(crate) fn degradation(&self) -> Option<SourceDegradation> {
        self.degradation
    }

    /// How far into the session this stream's clock has advanced, relative
    /// to the shared session zero — `Duration::ZERO` until its first frame
    /// arrives.
    pub(crate) fn elapsed(&self) -> Duration {
        match self.position.lock() {
            Ok(position) => *position,
            Err(poisoned) => *poisoned.into_inner(),
        }
    }

    /// A live snapshot of this stream's statistics — frame count, duration,
    /// level, loss count, gap count and the AEC state — what `cli` polls to
    /// print [`crate::Session::stream_stats`].
    pub(crate) fn stats(&self) -> StreamStats {
        let elapsed = self.elapsed();
        let degradation = self.degradation();
        let mut stats = match self.stats.lock() {
            Ok(stats) => stats,
            Err(poisoned) => poisoned.into_inner(),
        };
        match self.ring.lock() {
            Ok(mut ring) => stats.snapshot(&mut ring, elapsed, degradation),
            Err(poisoned) => stats.snapshot(&mut poisoned.into_inner(), elapsed, degradation),
        }
    }

    /// True if every sample this stream's buffer currently holds is zero —
    /// see [`Self::zeroize`].
    #[cfg(test)]
    pub(crate) fn all_zero(&self) -> bool {
        match self.ring.lock() {
            Ok(ring) => ring.all_zero(),
            Err(poisoned) => poisoned.into_inner().all_zero(),
        }
    }
}

impl Drop for StreamCapture {
    /// Best-effort cleanup for a `StreamCapture` dropped without going
    /// through [`crate::Session::stop`] — a panic between `Session::start`
    /// and `stop`, or an early `?`-return in a caller, must not leave a
    /// capture thread running or a buffer un-zeroed; that would break both
    /// "no thread is still writing" and the constitution's zeroing
    /// guarantee. Idempotent: calling this after `stop` already ran is a
    /// harmless no-op (`join` on an already-taken handle, `zeroize` on an
    /// already-zero buffer).
    fn drop(&mut self) {
        self.signal_stop();
        let _ = self.join();
        self.zeroize();
    }
}

/// Runs on a stream's dedicated capture thread until `stop` is set or the
/// source itself ends: pulls one event at a time, pushes frames into `ring`,
/// updates `position` from the normalised device timeline, and publishes a
/// [`SessionEvent`] on the way out.
fn capture_loop(
    identity: StreamIdentity,
    mut source: Box<dyn AudioSource>,
    ring: &Arc<Mutex<RingBuffer>>,
    position: &Arc<Mutex<Duration>>,
    stop: &Arc<AtomicBool>,
    session_zero: SessionZero,
    events: &broadcast::Sender<SessionEvent>,
) {
    let mut clock = StreamClock::anchor(session_zero);
    // `stop` is checked *after* handling each pull, never before one: a
    // stop request racing the thread's very first scheduling must not be
    // able to skip pulling entirely, or a bounded source that already has
    // data ready (this crate's own tests, via `TestToneSource`) could end
    // up contributing nothing before `Session::stop` zeroes an empty
    // buffer — indistinguishable from the zeroing never having run at all.
    loop {
        match source.pull(PULL_TIMEOUT) {
            Ok(AudioSourceEvent::Frame(frame)) => {
                let normalized = clock.normalize(frame.timestamp());
                // Push into `ring` *before* publishing `position`: a
                // reader that observes a new `position` must never be able
                // to look at `ring` and find the corresponding data still
                // missing. The two locks make this a real happens-before
                // guarantee, not just an ordering that is usually true —
                // `ring`'s unlock here is sequenced-before `position`'s
                // unlock below (same thread), and that unlock (release)
                // synchronizes-with a later reader's `position` lock
                // (acquire); by transitivity the ring push happens-before
                // that reader's own subsequent `ring` lock, *provided* the
                // reader locks `position` before `ring`, matching this
                // order (as every reader here does — see `elapsed()` then
                // `all_zero()` in the tests below). A future reader that
                // inverted that order would lose the guarantee.
                match ring.lock() {
                    Ok(mut ring) => ring.push(frame),
                    Err(_) => {
                        let _ = events.send(SessionEvent::StreamFailed {
                            identity,
                            reason: "ring buffer lock poisoned".to_string(),
                        });
                        break;
                    }
                }
                if let Ok(mut position) = position.lock() {
                    *position = normalized;
                }
            }
            Ok(AudioSourceEvent::Idle) => {}
            Ok(AudioSourceEvent::Ended) => {
                let _ = events.send(SessionEvent::StreamEnded { identity });
                break;
            }
            Err(error) => {
                let _ = events.send(SessionEvent::StreamFailed {
                    identity,
                    reason: error.to_string(),
                });
                break;
            }
        }
        if stop.load(Ordering::Relaxed) {
            break;
        }
    }
}

#[cfg(test)]
mod tests;
