//! The concrete WASAPI microphone [`AudioSource`], captured on the default
//! communications-role device with the OS's own echo cancellation.
//!
//! Mirrors [`super::loopback_client`]'s worker-thread pattern end to end —
//! see that module's doc for why: `wasapi::AudioClient`, `AudioCaptureClient`
//! and `Handle` are not `Send`, but [`AudioSource`] requires `Send` on every
//! implementor, so the WASAPI objects live entirely on one dedicated
//! thread and [`MicrophoneSource`] itself holds only channel endpoints and
//! plain data — no `unsafe` here either. Also reuses `crate::loopback`'s
//! pure packet logic (`bytes_to_f32_samples`, `frame_for_packet`,
//! `timeout_millis`, `PacketFlags`, `GapCounters`) unchanged: decoding, gap
//! counting and the timeout conversion are identical for an ordinary
//! capture client — only the device lookup and the extra AEC setup differ
//! from the loopback client, and live in the [`open_client`] submodule
//! (split out purely to keep both files under the constitution's
//! 400-line-per-module guideline, not a second edge — the same reasoning
//! [`super::loopback_client`] itself already documents).

mod open_client;

use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use thiserror::Error;
use transcriber_audio::{
    AudioSource, AudioSourceError, AudioSourceEvent, FormatError, SourceDegradation, StreamFormat,
};
use transcriber_core::StreamIdentity;

use open_client::OpenClient;

/// Explicit sample rate this client uses — the same format the loopback
/// client is fixed to, so both streams share one resampler configuration
/// downstream (spec's prior decision).
const MIC_SAMPLE_RATE_HZ: usize = 48_000;

/// Channel count for [`MIC_SAMPLE_RATE_HZ`]'s format.
const MIC_CHANNELS: usize = 2;

/// Failure opening the microphone client — mirrors
/// [`super::loopback_client::LoopbackOpenError`], with no variant for "no
/// microphone present": that outcome is `Ok(None)`, not an error, handled
/// entirely inside [`MicrophoneSource::open`].
#[derive(Debug, Error)]
pub(super) enum MicrophoneOpenError {
    /// [`StreamFormat::new`] rejected this module's own constants — never
    /// expected in practice, but the type must still be handled.
    #[error("failed to build the fixed stream format: {0}")]
    Format(#[from] FormatError),
    /// The worker thread could not open or start the WASAPI client, and
    /// reported why before exiting.
    #[error("failed to open the WASAPI microphone client: {0}")]
    Wasapi(String),
    /// [`thread::Builder::spawn`] itself failed (an OS resource limit, not
    /// a WASAPI failure).
    #[error("could not spawn the capture thread: {0}")]
    Thread(String),
    /// The worker thread's channel closed before it reported an open
    /// outcome — it must have panicked.
    #[error("the capture thread exited before it could open the device")]
    ThreadExited,
}

/// One capture-thread response to a `pull` request — the same shape
/// [`super::loopback_client`] uses for its own worker.
type ThreadResponse = Result<AudioSourceEvent, String>;

/// What the worker thread reports back to [`MicrophoneSource::open`] after
/// its one attempt to open the device.
enum ReadyReport {
    /// The client opened and started; carries whatever AEC degradation
    /// `open_client`'s AEC chain decided.
    Opened(Option<SourceDegradation>),
    /// No default communications capture device exists — the spec's
    /// "missing microphone is a warning, not an abort" outcome.
    Absent,
    /// A real WASAPI failure, reported as text.
    Failed(String),
}

/// The `Local` stream's [`AudioSource`]: the default communications-role
/// microphone, captured on a dedicated worker thread with OS echo
/// cancellation enabled where the platform supports it (see this module's
/// doc for why a worker thread, not a direct field, holds the WASAPI
/// objects).
///
/// `pull` sends its `timeout` to the worker over `request_tx` and blocks on
/// `response_rx` for the matching reply — the same bounded rendezvous
/// [`super::loopback_client::LoopbackSource`] uses, so no unbounded buffer
/// exists on this stream either.
pub(super) struct MicrophoneSource {
    request_tx: Option<SyncSender<Duration>>,
    response_rx: Receiver<ThreadResponse>,
    format: StreamFormat,
    worker: Option<JoinHandle<()>>,
    ended: bool,
    degradation: Option<SourceDegradation>,
}

impl MicrophoneSource {
    /// Spawns the worker thread, which opens the default communications
    /// capture device in the fixed 48 kHz stereo float format and reports
    /// whether it opened, found no device, or failed.
    ///
    /// `Ok(None)` is the spec's ordinary "no microphone present" outcome —
    /// never [`MicrophoneOpenError`], which is reserved for a real failure.
    pub(super) fn open() -> Result<Option<Self>, MicrophoneOpenError> {
        let format = StreamFormat::new(MIC_SAMPLE_RATE_HZ as u32, MIC_CHANNELS as u16)?;
        let (ready_tx, ready_rx) = mpsc::channel::<ReadyReport>();
        let (request_tx, request_rx) = mpsc::sync_channel::<Duration>(0);
        let (response_tx, response_rx) = mpsc::sync_channel::<ThreadResponse>(0);
        let worker = thread::Builder::new()
            .name("audio-win-microphone".to_string())
            .spawn(move || run_worker(ready_tx, request_rx, response_tx))
            .map_err(|error| MicrophoneOpenError::Thread(error.to_string()))?;
        match ready_rx.recv() {
            Ok(ReadyReport::Opened(degradation)) => Ok(Some(Self {
                request_tx: Some(request_tx),
                response_rx,
                format,
                worker: Some(worker),
                ended: false,
                degradation,
            })),
            Ok(ReadyReport::Absent) => Ok(None),
            Ok(ReadyReport::Failed(reason)) => Err(MicrophoneOpenError::Wasapi(reason)),
            Err(_) => Err(MicrophoneOpenError::ThreadExited),
        }
    }
}

impl Drop for MicrophoneSource {
    /// Identical shutdown to [`super::loopback_client::LoopbackSource`]'s
    /// `Drop`: closing the request channel ends the worker's `recv()` loop,
    /// dropping the WASAPI client and stopping the stream, then the thread
    /// is joined so none outlives this source.
    fn drop(&mut self) {
        self.request_tx = None;
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl AudioSource for MicrophoneSource {
    fn identity(&self) -> StreamIdentity {
        StreamIdentity::Local
    }

    fn format(&self) -> StreamFormat {
        self.format
    }

    fn pull(&mut self, timeout: Duration) -> Result<AudioSourceEvent, AudioSourceError> {
        if self.ended {
            return Ok(AudioSourceEvent::Ended);
        }
        let Some(sender) = &self.request_tx else {
            self.ended = true;
            return Ok(AudioSourceEvent::Ended);
        };
        if sender.send(timeout).is_err() {
            self.ended = true;
            return Ok(AudioSourceEvent::Ended);
        }
        match self.response_rx.recv() {
            Ok(Ok(AudioSourceEvent::Ended)) => {
                self.ended = true;
                Ok(AudioSourceEvent::Ended)
            }
            Ok(Ok(event)) => Ok(event),
            Ok(Err(reason)) => Err(AudioSourceError::Failed { reason }),
            Err(_) => {
                self.ended = true;
                Ok(AudioSourceEvent::Ended)
            }
        }
    }

    fn degradation(&self) -> Option<SourceDegradation> {
        self.degradation
    }
}

/// The worker thread's body — the same shape as
/// [`super::loopback_client::run_worker`], with a third
/// [`ReadyReport::Absent`] outcome the loopback client has no equivalent
/// for (a process tree, unlike a microphone, is never simply "not there").
fn run_worker(
    ready_tx: mpsc::Sender<ReadyReport>,
    request_rx: Receiver<Duration>,
    response_tx: SyncSender<ThreadResponse>,
) {
    let mut client = match OpenClient::open() {
        Ok(Some(client)) => {
            let degradation = client.degradation();
            if ready_tx.send(ReadyReport::Opened(degradation)).is_err() {
                return;
            }
            client
        }
        Ok(None) => {
            let _ = ready_tx.send(ReadyReport::Absent);
            return;
        }
        Err(reason) => {
            let _ = ready_tx.send(ReadyReport::Failed(reason));
            return;
        }
    };
    while let Ok(timeout) = request_rx.recv() {
        let response = client.next_event(timeout);
        let ended = matches!(response, Ok(AudioSourceEvent::Ended));
        if response_tx.send(response).is_err() || ended {
            break;
        }
    }
}
