//! The synthetic tone [`AudioSource`] and its [`SourceFactory`] — the
//! constitution's debug path ("Für Debugging existiert ein synthetischer
//! Testton-Pfad", `docs/constitution.md`) and the only way anything in this
//! phase's tests run without an audio device (spec, risk table: "CI-Runner
//! ohne Audiogerät").
//!
//! Every sample is computed from a phase accumulator on demand: no file, no
//! cached asset, nothing that outlives the fields on [`TestToneSource`]
//! itself.

use std::time::Duration;

use transcriber_core::{CaptureSubject, StreamIdentity};

use crate::factory::{SourceFactory, SourceFactoryError};
use crate::format::{FormatError, StreamFormat};
use crate::frame::{DeviceTimestamp, Frame};
use crate::source::{AudioSource, AudioSourceError, AudioSourceEvent};

/// Root PID [`TestToneSources`] hands out for its one synthetic subject.
/// Deliberately not `0` or `4` — the sentinel PIDs the Windows backend's
/// enumeration excludes (spec, Windows-backend prior decisions) — so this
/// value never looks like one of those by accident.
const SYNTHETIC_ROOT_PID: u32 = 1;

/// A deterministic sine tone: a phase accumulator that resumes exactly where
/// the previous [`AudioSource::pull`] left off, so the waveform stays
/// continuous across chunk boundaries instead of clicking at each one.
pub struct TestToneSource {
    identity: StreamIdentity,
    format: StreamFormat,
    frequency_hz: f32,
    phase: f64,
    frames_emitted: u64,
    total_frames: Option<u64>,
}

impl TestToneSource {
    /// Native-format frames produced per [`AudioSource::pull`] call that has
    /// data left to give — 10 ms at 48 kHz, the Windows backend's own packet
    /// granularity (`get_next_packet_size()`, spec's prior decisions), kept
    /// the same here so a downstream resampler sees comparable chunk sizes.
    const FRAMES_PER_PULL: u64 = 480;

    /// Amplitude well under full scale, so nothing here ever clips
    /// regardless of channel count.
    const AMPLITUDE: f32 = 0.2;

    /// Builds an unbounded tone at `frequency_hz`, in `format`, tagged
    /// `identity`.
    #[must_use]
    pub fn new(identity: StreamIdentity, format: StreamFormat, frequency_hz: f32) -> Self {
        Self {
            identity,
            format,
            frequency_hz,
            phase: 0.0,
            frames_emitted: 0,
            total_frames: None,
        }
    }

    /// Bounds the tone to `frame_count` frames: the first `pull()` at or
    /// past that count reports [`AudioSourceEvent::Ended`] instead of more
    /// samples, so a test can drive a source to a deterministic end without
    /// a device signalling one.
    #[must_use]
    pub fn with_total_frames(mut self, frame_count: u64) -> Self {
        self.total_frames = Some(frame_count);
        self
    }

    /// Frames the next chunk may contain: the usual
    /// [`Self::FRAMES_PER_PULL`], clamped to whatever is left before
    /// [`Self::total_frames`], if bounded.
    fn next_chunk_len(&self) -> u64 {
        match self.total_frames {
            Some(total) => Self::FRAMES_PER_PULL.min(total.saturating_sub(self.frames_emitted)),
            None => Self::FRAMES_PER_PULL,
        }
    }

    /// Device-timeline position of the next sample this source will emit,
    /// derived from frames already emitted rather than tracked as separate
    /// state — one source of truth for "how far in" this tone is.
    fn current_timestamp(&self) -> DeviceTimestamp {
        let ticks =
            u128::from(self.frames_emitted) * 10_000_000 / u128::from(self.format.sample_rate_hz());
        DeviceTimestamp::from_ticks(ticks as i64)
    }

    /// Advances the phase accumulator by `frames` worth of samples,
    /// replicated across every channel into a fresh, interleaved `Vec` —
    /// `format.channels()` samples per frame.
    fn synthesize(&mut self, frames: u64) -> Vec<f32> {
        let channels = usize::from(self.format.channels());
        let sample_rate = f64::from(self.format.sample_rate_hz());
        let phase_step = 2.0 * std::f64::consts::PI * f64::from(self.frequency_hz) / sample_rate;
        let mut samples = Vec::with_capacity(frames as usize * channels);
        for _ in 0..frames {
            let value = (self.phase.sin() as f32) * Self::AMPLITUDE;
            samples.extend(std::iter::repeat_n(value, channels));
            self.phase = (self.phase + phase_step) % (2.0 * std::f64::consts::PI);
        }
        samples
    }
}

impl AudioSource for TestToneSource {
    fn identity(&self) -> StreamIdentity {
        self.identity
    }

    fn format(&self) -> StreamFormat {
        self.format
    }

    fn pull(&mut self, _timeout: Duration) -> Result<AudioSourceEvent, AudioSourceError> {
        if let Some(total) = self.total_frames {
            if self.frames_emitted >= total {
                return Ok(AudioSourceEvent::Ended);
            }
        }
        let len = self.next_chunk_len();
        let timestamp = self.current_timestamp();
        let samples = self.synthesize(len);
        self.frames_emitted += len;
        Ok(AudioSourceEvent::Frame(Frame::samples(timestamp, samples)))
    }
}

/// [`SourceFactory`] built entirely from [`TestToneSource`] — the only
/// factory this phase needs to exercise `audio`, and (from the next issue)
/// `session`, on a CI runner with no audio device (spec, risk table).
pub struct TestToneSources {
    format: StreamFormat,
    remote_frequency_hz: f32,
    local_frequency_hz: f32,
    has_microphone: bool,
}

impl TestToneSources {
    /// Frequency for the synthetic `Remote` stream — arbitrary, but distinct
    /// from [`Self::LOCAL_FREQUENCY_HZ`] so a test session with both streams
    /// open can tell them apart on tone alone.
    const REMOTE_FREQUENCY_HZ: f32 = 440.0;
    /// Frequency for the synthetic `Local` (microphone) stream.
    const LOCAL_FREQUENCY_HZ: f32 = 660.0;

    /// Builds a factory producing 48 kHz stereo f32 tones — the format both
    /// Windows backend clients are given explicitly (spec's prior
    /// decisions) — with both streams available.
    ///
    /// # Errors
    ///
    /// Never fails for this fixed format; returns [`FormatError`] rather
    /// than asserting the invariant with a panic, so this stays honest if a
    /// later caller ever threads a variable format through here.
    pub fn new() -> Result<Self, FormatError> {
        Ok(Self {
            format: StreamFormat::new(48_000, 2)?,
            remote_frequency_hz: Self::REMOTE_FREQUENCY_HZ,
            local_frequency_hz: Self::LOCAL_FREQUENCY_HZ,
            has_microphone: true,
        })
    }

    /// Makes [`SourceFactory::open_local`] behave like a machine with no
    /// microphone: `Ok(None)` — the spec's decision that this is a warning,
    /// not an abort — instead of opening a second tone.
    #[must_use]
    pub fn without_microphone(mut self) -> Self {
        self.has_microphone = false;
        self
    }
}

impl SourceFactory for TestToneSources {
    fn list_subjects(&self) -> Result<Vec<CaptureSubject>, SourceFactoryError> {
        Ok(vec![CaptureSubject::new("test-tone", SYNTHETIC_ROOT_PID)])
    }

    fn open_remote(
        &self,
        _subject: &CaptureSubject,
    ) -> Result<Box<dyn AudioSource>, SourceFactoryError> {
        Ok(Box::new(TestToneSource::new(
            StreamIdentity::Remote,
            self.format,
            self.remote_frequency_hz,
        )))
    }

    fn open_local(&self) -> Result<Option<Box<dyn AudioSource>>, SourceFactoryError> {
        if !self.has_microphone {
            return Ok(None);
        }
        Ok(Some(Box::new(TestToneSource::new(
            StreamIdentity::Local,
            self.format,
            self.local_frequency_hz,
        ))))
    }
}

#[cfg(test)]
mod tests;
