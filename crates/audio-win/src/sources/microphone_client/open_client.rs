//! The opened WASAPI objects behind [`super::MicrophoneSource`] — device
//! lookup, the AEC chain, and the per-packet read — split out of
//! `microphone_client.rs` purely to keep both files under the
//! constitution's 400-line-per-module guideline, not a second edge (see
//! that module's doc).

use std::time::Duration;

use transcriber_audio::{AudioSourceEvent, DeviceTimestamp, Frame, SourceDegradation};
use wasapi::{
    AudioCaptureClient, AudioClient, AudioClientProperties, Device, DeviceEnumerator, Direction,
    Handle, Role, SampleType, StreamCategory, StreamMode, WaveFormat,
};

use super::{MIC_CHANNELS, MIC_SAMPLE_RATE_HZ, ThreadResponse};
use crate::loopback::{
    GapCounters, PacketFlags, bytes_to_f32_samples, frame_for_packet, timeout_millis,
};
use crate::microphone::{aec_degradation, microphone_absent};

/// The opened WASAPI objects, owned end to end by
/// [`super::run_worker`]'s thread and never sent elsewhere — see
/// `microphone_client`'s module doc for why that is what makes
/// [`super::MicrophoneSource`] itself `Send`-able without `unsafe`.
pub(super) struct OpenClient {
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
    pub(super) fn open() -> Result<Option<Self>, String> {
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
        let degradation = enable_echo_cancellation(&audio_client)?;
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

    /// The capability degradation this client opened under, if any — what
    /// [`super::run_worker`] reports back to [`super::MicrophoneSource::open`].
    pub(super) fn degradation(&self) -> Option<SourceDegradation> {
        self.degradation
    }

    /// Answers one `pull` request: waits up to `timeout` for the capture
    /// event, then reads whatever packet, if any, is ready — identical to
    /// [`super::super::loopback_client`]'s method of the same name.
    pub(super) fn next_event(&mut self, timeout: Duration) -> ThreadResponse {
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
    /// to [`super::super::loopback_client`]'s handling of the same event.
    fn wait_for_packet(&self, timeout: Duration) -> Result<bool, ()> {
        match self.event.wait_for_event(timeout_millis(timeout)) {
            Ok(()) => Ok(true),
            Err(wasapi::WasapiError::EventTimeout) => Ok(false),
            Err(_) => Err(()),
        }
    }

    /// Reads exactly one packet — sized from `get_next_packet_size()` alone
    /// — and maps it to the [`Frame`] this source hands the ring buffer.
    /// Identical shape to [`super::super::loopback_client`]'s method of the
    /// same name; only the client it reads from differs.
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
/// `set_echo_cancellation_render_endpoint` — passing `None` as the render
/// reference endpoint so Windows selects it, rather than this crate
/// guessing a role for it. Two independent reasons, both recorded in the
/// spec's Decision log:
///
/// - The checked `wasapi` example resolves the *Console*-role default
///   render device, not Communications — but the capture side of this very
///   function picks `Role::Communications` specifically because that is
///   "the same one the call client itself uses" (this issue's own
///   framing). The same reasoning argues against Console on the render
///   side too, and there is no guarantee the two roles resolve to the same
///   physical device. Pointing the AEC reference at the wrong endpoint
///   would silently fail to cancel the echo while `is_aec_supported()`
///   still reports `true` — the exact failure this feature exists to
///   prevent, with no disclosed degradation to catch it.
/// - `wasapi` 0.23.0's `set_echo_cancellation_render_endpoint` drops the
///   `HSTRING` backing its `PCWSTR` argument before the COM call that reads
///   it runs (`wasapi-0.23.0/src/api.rs:1898-1912` — the temporary is not
///   lifetime-extended through `.as_ptr()`), a dangling-pointer bug in the
///   pinned dependency reachable only on the `Some(id)` branch. `None`
///   avoids constructing the `HSTRING` at all.
///
/// A `false` probe becomes the disclosed degradation
/// (`crate::microphone::aec_degradation`) rather than a failure; any error
/// past that point is a real one — see [`OpenClient::open`]'s doc for why
/// this deliberately does not fold into a second degradation.
fn enable_echo_cancellation(
    audio_client: &AudioClient,
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
    control
        .set_echo_cancellation_render_endpoint(None)
        .map_err(|error| error.to_string())?;
    Ok(aec_degradation(true))
}
