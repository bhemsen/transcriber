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
//! capture client — only the device lookup and the extra AEC setup below
//! differ from the loopback client.

use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use thiserror::Error;
use transcriber_audio::{
    AudioSource, AudioSourceError, AudioSourceEvent, DeviceTimestamp, FormatError, Frame,
    SourceDegradation, StreamFormat,
};
use transcriber_core::StreamIdentity;
use wasapi::{
    AudioCaptureClient, AudioClient, AudioClientProperties, Device, DeviceEnumerator, Direction,
    Handle, Role, SampleType, StreamCategory, StreamMode, WaveFormat,
};

use crate::loopback::{
    GapCounters, PacketFlags, bytes_to_f32_samples, frame_for_packet, timeout_millis,
};
use crate::microphone::{aec_degradation, microphone_absent};

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
    /// [`enable_echo_cancellation`] decided.
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
            let degradation = client.degradation;
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

/// Looks up the default communications-role capture device — `Ok(None)`
/// exactly when `crate::microphone::microphone_absent` recognises the
/// HRESULT `GetDefaultAudioEndpoint` returns for "no such device".
fn open_communications_device(enumerator: &DeviceEnumerator) -> Result<Option<Device>, String> {
    match enumerator.get_default_device_for_role(&Direction::Capture, &Role::Communications) {
        Ok(device) => Ok(Some(device)),
        Err(wasapi::WasapiError::Windows(error)) if microphone_absent(error.code().0) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

/// Sets the communications stream category and initializes the client.
///
/// `StreamCategory::Communications` must be set via `set_properties`
/// **before** `initialize_client` — the order is binding (spec's
/// Windows-backend prior decisions) — and only on this client: the
/// category is invalid on the loopback stream, which correctly never sets
/// it. `min_period` (from [`AudioClient::get_device_period`]) becomes the
/// buffer duration — unlike the loopback client, this one's device period
/// is real (spec's prior decision).
fn configure_capture_client(
    audio_client: &mut AudioClient,
    wave_format: &WaveFormat,
    min_period: i64,
) -> Result<(), String> {
    let properties = AudioClientProperties::new().set_category(StreamCategory::Communications);
    audio_client
        .set_properties(properties)
        .map_err(|error| error.to_string())?;
    let mode = StreamMode::EventsShared {
        autoconvert: true,
        buffer_duration_hns: min_period,
    };
    audio_client
        .initialize_client(wave_format, &Direction::Capture, &mode)
        .map_err(|error| error.to_string())
}

/// Runs the spec's AEC chain — `is_aec_supported` -> `get_aec_control` ->
/// `set_echo_cancellation_render_endpoint` — using the default render
/// device as the reference stream, exactly as the checked `wasapi` example
/// does (spec's Windows-backend prior decisions). A `false` probe becomes
/// the disclosed degradation (`crate::microphone::aec_degradation`) rather
/// than a failure; any error past that point is a real one — see
/// [`OpenClient::open`]'s doc for why this deliberately does not fold into
/// a second degradation.
fn enable_echo_cancellation(
    audio_client: &AudioClient,
    enumerator: &DeviceEnumerator,
) -> Result<Option<SourceDegradation>, String> {
    let supported = audio_client
        .is_aec_supported()
        .map_err(|error| error.to_string())?;
    if !supported {
        return Ok(aec_degradation(false));
    }
    let control = audio_client
        .get_aec_control()
        .map_err(|error| error.to_string())?;
    let render_device = enumerator
        .get_default_device(&Direction::Render)
        .map_err(|error| error.to_string())?;
    let render_endpoint_id = render_device.get_id().map_err(|error| error.to_string())?;
    control
        .set_echo_cancellation_render_endpoint(Some(render_endpoint_id))
        .map_err(|error| error.to_string())?;
    Ok(aec_degradation(true))
}

/// The opened WASAPI objects, owned end to end by [`run_worker`]'s thread
/// and never sent elsewhere — see this module's doc for why that is what
/// makes [`MicrophoneSource`] itself `Send`-able without `unsafe`.
struct OpenClient {
    audio_client: AudioClient,
    capture_client: AudioCaptureClient,
    event: Handle,
    bytes_per_frame: usize,
    counters: GapCounters,
    degradation: Option<SourceDegradation>,
}

impl OpenClient {
    /// Opens the default communications-role capture device, enables echo
    /// cancellation where supported, and starts the stream.
    ///
    /// `Ok(None)` means [`open_communications_device`] found no such
    /// device — the ordinary "no microphone present" outcome, not a real
    /// failure. Every other step's failure is `Err`, including a failure in
    /// the AEC chain past a `true` `is_aec_supported()` probe: the spec's
    /// "warn and continue" decision covers the probe itself, not an
    /// inconsistency beyond it, so that case is surfaced rather than
    /// silently folded into a second degradation.
    fn open() -> Result<Option<Self>, String> {
        wasapi::initialize_mta()
            .ok()
            .map_err(|error| error.to_string())?;
        let enumerator = DeviceEnumerator::new().map_err(|error| error.to_string())?;
        let Some(device) = open_communications_device(&enumerator)? else {
            return Ok(None);
        };
        let mut audio_client = device
            .get_iaudioclient()
            .map_err(|error| error.to_string())?;
        let wave_format = WaveFormat::new(
            32,
            32,
            &SampleType::Float,
            MIC_SAMPLE_RATE_HZ,
            MIC_CHANNELS,
            None,
        );
        let bytes_per_frame = wave_format.get_blockalign() as usize;
        let (_default_period, min_period) = audio_client
            .get_device_period()
            .map_err(|error| error.to_string())?;
        configure_capture_client(&mut audio_client, &wave_format, min_period)?;
        let degradation = enable_echo_cancellation(&audio_client, &enumerator)?;
        let event = audio_client
            .set_get_eventhandle()
            .map_err(|error| error.to_string())?;
        let capture_client = audio_client
            .get_audiocaptureclient()
            .map_err(|error| error.to_string())?;
        audio_client
            .start_stream()
            .map_err(|error| error.to_string())?;
        Ok(Some(Self {
            audio_client,
            capture_client,
            event,
            bytes_per_frame,
            counters: GapCounters::default(),
            degradation,
        }))
    }

    /// Answers one `pull` request: waits up to `timeout` for the capture
    /// event, then reads whatever packet, if any, is ready — identical to
    /// [`super::loopback_client`]'s method of the same name.
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

    /// Waits for the capture event, up to `timeout` — `Ok(false)` is the
    /// spec's "a wait timeout is silence, not an error" decision, identical
    /// to [`super::loopback_client`]'s handling of the same event.
    fn wait_for_packet(&self, timeout: Duration) -> Result<bool, ()> {
        match self.event.wait_for_event(timeout_millis(timeout)) {
            Ok(()) => Ok(true),
            Err(wasapi::WasapiError::EventTimeout) => Ok(false),
            Err(_) => Err(()),
        }
    }

    /// Reads exactly one packet — sized from `get_next_packet_size()` alone
    /// — and maps it to the [`Frame`] this source hands the ring buffer.
    /// Identical shape to [`super::loopback_client`]'s method of the same
    /// name; only the client it reads from differs.
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
