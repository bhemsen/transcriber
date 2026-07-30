//! The concrete WASAPI loopback [`AudioSource`]: opens
//! `AudioClient::new_application_loopback_client` for one process tree and
//! maps its packets to [`Frame`]s via the pure logic in [`crate::loopback`].
//!
//! Split out of `sources.rs` purely to keep both files under the
//! constitution's 400-line-per-module guideline — this submodule is still
//! part of this crate's one edge (`super`'s module doc), not a second one.
//!
//! **Why a worker thread, not a direct field:** `wasapi::AudioClient`,
//! `AudioCaptureClient` and `Handle` each wrap a raw COM or Win32 pointer
//! (`windows_core::IUnknown`'s `NonNull<c_void>`, `HANDLE`'s `*mut c_void`)
//! with no `Send` impl, but [`AudioSource`] requires `Send` on every
//! implementor. Making [`LoopbackSource`] itself `Send` by holding those
//! types directly would need `unsafe impl Send`, which
//! `#![forbid(unsafe_code)]` in this crate does not allow — see the spec's
//! Decision log for the discovery and this resolution. Instead, the WASAPI
//! objects are opened and used entirely on one dedicated thread spawned by
//! [`LoopbackSource::open`], and never leave it; [`LoopbackSource`] itself
//! only holds channel endpoints and plain data, which are `Send` on their
//! own merits — no `unsafe` anywhere in this module.

use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use thiserror::Error;
use transcriber_audio::{
    AudioSource, AudioSourceError, AudioSourceEvent, DeviceTimestamp, FormatError, Frame,
    StreamFormat,
};
use transcriber_core::StreamIdentity;
use wasapi::{
    AudioCaptureClient, AudioClient, Direction, Handle, SampleType, StreamMode, WaveFormat,
};

use crate::loopback::{
    GapCounters, PacketFlags, bytes_to_f32_samples, frame_for_packet, timeout_millis,
};

/// Explicit sample rate both Windows-backend clients use (spec's prior
/// decision): `GetMixFormat` returns `E_NOTIMPL` on a process-loopback
/// client, so the format cannot be read back and must be supplied here
/// instead.
const LOOPBACK_SAMPLE_RATE_HZ: usize = 48_000;

/// Channel count for [`LOOPBACK_SAMPLE_RATE_HZ`]'s format.
const LOOPBACK_CHANNELS: usize = 2;

/// Failure opening the loopback client for a subject.
#[derive(Debug, Error)]
pub(super) enum LoopbackOpenError {
    /// [`StreamFormat::new`] rejected this module's own constants — never
    /// expected in practice, but the type must still be handled.
    #[error("failed to build the fixed stream format: {0}")]
    Format(#[from] FormatError),
    /// The worker thread could not open or start the WASAPI client, and
    /// reported why before exiting.
    #[error("failed to open the WASAPI loopback client: {0}")]
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

/// One capture-thread response to a `pull` request: an ordinary
/// [`AudioSourceEvent`], or a genuinely unclassified failure the caller
/// reports as [`AudioSourceError::Failed`].
type ThreadResponse = Result<AudioSourceEvent, String>;

/// The `Remote` stream's [`AudioSource`]: one process tree's render audio,
/// captured on a dedicated worker thread via
/// [`AudioClient::new_application_loopback_client`] (see this module's doc
/// for why a worker thread, not a direct field, holds the WASAPI objects).
///
/// `pull` sends its `timeout` to the worker over `request_tx` and blocks on
/// `response_rx` for the matching reply — a rendezvous, not a queue: each
/// channel holds at most one in-flight value, so no unbounded buffer exists
/// here either, matching the same fixed-size invariant the ring buffer and
/// the WASAPI read itself (sized only from `get_next_packet_size()`) keep.
pub(super) struct LoopbackSource {
    request_tx: Option<SyncSender<Duration>>,
    response_rx: Receiver<ThreadResponse>,
    format: StreamFormat,
    worker: Option<JoinHandle<()>>,
    ended: bool,
}

impl LoopbackSource {
    /// Spawns the worker thread for `root_pid`'s process tree in the fixed
    /// 48 kHz stereo float format and waits for it to report whether the
    /// WASAPI client opened and started.
    pub(super) fn open(root_pid: u32) -> Result<Self, LoopbackOpenError> {
        let format = StreamFormat::new(LOOPBACK_SAMPLE_RATE_HZ as u32, LOOPBACK_CHANNELS as u16)?;
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let (request_tx, request_rx) = mpsc::sync_channel::<Duration>(0);
        let (response_tx, response_rx) = mpsc::sync_channel::<ThreadResponse>(0);
        let worker = thread::Builder::new()
            .name("audio-win-loopback".to_string())
            .spawn(move || run_worker(root_pid, ready_tx, request_rx, response_tx))
            .map_err(|error| LoopbackOpenError::Thread(error.to_string()))?;
        match ready_rx.recv() {
            Ok(Ok(())) => {}
            Ok(Err(reason)) => return Err(LoopbackOpenError::Wasapi(reason)),
            Err(_) => return Err(LoopbackOpenError::ThreadExited),
        }
        Ok(Self {
            request_tx: Some(request_tx),
            response_rx,
            format,
            worker: Some(worker),
            ended: false,
        })
    }
}

impl Drop for LoopbackSource {
    /// Closes the request channel — the worker's `recv()` then errors and
    /// its loop ends, dropping the WASAPI client (stopping the stream) —
    /// and joins the thread so none outlives this source.
    ///
    /// Bounded by whatever `pull` timeout the worker is mid-wait on when
    /// this runs, since it only notices the closed channel between calls;
    /// never unbounded, since [`Handle::wait_for_event`] itself always
    /// returns by that timeout.
    fn drop(&mut self) {
        self.request_tx = None;
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl AudioSource for LoopbackSource {
    fn identity(&self) -> StreamIdentity {
        StreamIdentity::Remote
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
}

/// The worker thread's body: opens the client, reports the outcome via
/// `ready_tx`, then answers one `request_rx` timeout at a time until either
/// the request channel closes (this source was dropped) or a response
/// itself is [`AudioSourceEvent::Ended`] (the device was invalidated —
/// nothing more to do).
fn run_worker(
    root_pid: u32,
    ready_tx: mpsc::Sender<Result<(), String>>,
    request_rx: Receiver<Duration>,
    response_tx: SyncSender<ThreadResponse>,
) {
    let mut client = match OpenClient::open(root_pid) {
        Ok(client) => {
            if ready_tx.send(Ok(())).is_err() {
                return;
            }
            client
        }
        Err(reason) => {
            let _ = ready_tx.send(Err(reason));
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

/// The opened WASAPI objects, owned end to end by [`run_worker`]'s thread
/// and never sent elsewhere — see this module's doc for why that is what
/// makes [`LoopbackSource`] itself `Send`-able without `unsafe`.
struct OpenClient {
    audio_client: AudioClient,
    capture_client: AudioCaptureClient,
    event: Handle,
    bytes_per_frame: usize,
    counters: GapCounters,
}

impl OpenClient {
    /// Opens the loopback client for `root_pid`'s process tree and starts
    /// the stream.
    ///
    /// `include_tree = true` maps to
    /// `PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE`: captures the
    /// whole process tree resolved onto `root_pid`, not just that one PID
    /// (spec's Windows-backend prior decisions). `buffer_duration_hns: 0`
    /// because `get_device_period()` does not work on this client either —
    /// only the microphone client (issue #13) uses a real period.
    /// `initialize_mta()` here registers this worker thread in the
    /// process's MTA before any further WASAPI call on it.
    fn open(root_pid: u32) -> Result<Self, String> {
        wasapi::initialize_mta()
            .ok()
            .map_err(|error| error.to_string())?;
        let wave_format = WaveFormat::new(
            32,
            32,
            &SampleType::Float,
            LOOPBACK_SAMPLE_RATE_HZ,
            LOOPBACK_CHANNELS,
            None,
        );
        let bytes_per_frame = wave_format.get_blockalign() as usize;
        let mut audio_client = AudioClient::new_application_loopback_client(root_pid, true)
            .map_err(|error| error.to_string())?;
        let mode = StreamMode::EventsShared {
            autoconvert: true,
            buffer_duration_hns: 0,
        };
        audio_client
            .initialize_client(&wave_format, &Direction::Capture, &mode)
            .map_err(|error| error.to_string())?;
        let event = audio_client
            .set_get_eventhandle()
            .map_err(|error| error.to_string())?;
        let capture_client = audio_client
            .get_audiocaptureclient()
            .map_err(|error| error.to_string())?;
        audio_client
            .start_stream()
            .map_err(|error| error.to_string())?;
        Ok(Self {
            audio_client,
            capture_client,
            event,
            bytes_per_frame,
            counters: GapCounters::default(),
        })
    }

    /// Answers one `pull` request: waits up to `timeout` for the capture
    /// event, then reads whatever packet, if any, is ready.
    fn next_event(&mut self, timeout: Duration) -> ThreadResponse {
        match self.wait_for_packet(timeout) {
            Ok(true) => {}
            Ok(false) => return Ok(AudioSourceEvent::Idle),
            Err(()) => return Ok(AudioSourceEvent::Ended),
        }
        match self.read_next_frame() {
            Ok(Some(frame)) => Ok(AudioSourceEvent::Frame(frame)),
            Ok(None) => Ok(AudioSourceEvent::Idle),
            Err(()) => Ok(AudioSourceEvent::Ended),
        }
    }

    /// Waits for the capture event, up to `timeout`.
    ///
    /// `Ok(true)` means a new packet may be ready; `Ok(false)` is the
    /// spec's "a wait timeout is silence, not an error" decision — the
    /// caller reports [`AudioSourceEvent::Idle`] and keeps capturing. Any
    /// other wait failure is folded into `Err(())`, the same "treat as
    /// ended" outcome a read error produces — `get_audiosessioncontrol` is
    /// unavailable on this client, so stream end has no separate channel.
    fn wait_for_packet(&self, timeout: Duration) -> Result<bool, ()> {
        match self.event.wait_for_event(timeout_millis(timeout)) {
            Ok(()) => Ok(true),
            Err(wasapi::WasapiError::EventTimeout) => Ok(false),
            Err(_) => Err(()),
        }
    }

    /// Reads exactly one packet — sized from `get_next_packet_size()` alone
    /// — and maps it to the [`Frame`] this source hands the ring buffer.
    ///
    /// `Ok(None)` means nothing was available to fetch this call (this
    /// client only ever reports `Some`, but `None` is handled the same way
    /// an observed `Some(0)` is, per the spec's reading of
    /// `get_next_packet_size`'s contract); `Err(())` means the read itself
    /// failed — the only way this client signals an invalidated device.
    fn read_next_frame(&mut self) -> Result<Option<Frame>, ()> {
        let frame_count = match self.capture_client.get_next_packet_size() {
            Ok(Some(count)) if count > 0 => count,
            Ok(_) => return Ok(None),
            Err(_) => return Err(()),
        };
        let mut raw = vec![0u8; frame_count as usize * self.bytes_per_frame];
        let (returned_frames, buffer_info) = self
            .capture_client
            .read_from_device(&mut raw)
            .map_err(|_| ())?;
        if returned_frames == 0 {
            return Ok(None);
        }
        let len_in_bytes = returned_frames as usize * self.bytes_per_frame;
        let samples = bytes_to_f32_samples(&raw[..len_in_bytes]);
        let interleaved_len = samples.len();
        let timestamp = DeviceTimestamp::from_ticks(buffer_info.timestamp as i64);
        let flags = PacketFlags {
            data_discontinuity: buffer_info.flags.data_discontinuity,
            silent: buffer_info.flags.silent,
            timestamp_error: buffer_info.flags.timestamp_error,
        };
        Ok(Some(frame_for_packet(
            timestamp,
            interleaved_len,
            samples,
            flags,
            &mut self.counters,
        )))
    }
}

impl Drop for OpenClient {
    /// Stops the stream so the device is released promptly instead of
    /// waiting for the underlying COM objects' own teardown order.
    fn drop(&mut self) {
        let _ = self.audio_client.stop_stream();
    }
}
