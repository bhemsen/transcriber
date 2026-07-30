//! The one edge module in this crate: everything that actually calls into
//! `wasapi` or `sysinfo` lives here (this file) or in its
//! [`loopback_client`] and [`microphone_client`] submodules — split out
//! purely to keep every file under the constitution's 400-line-per-module
//! guideline, not a second or third edge — over the pure functions in
//! [`crate::process_tree`], [`crate::sessions`], [`crate::loopback`] and
//! [`crate::microphone`].
//!
//! Only [`loopback_client::LoopbackSource`]'s and
//! [`microphone_client::MicrophoneSource`]'s methods are exercised by a
//! real capture path — the spec's risk table notes `windows-latest` CI
//! runners have no audio device, so this edge gets compile checks only;
//! [`crate::process_tree`], [`crate::sessions`], [`crate::loopback`] and
//! [`crate::microphone`] carry the real, device-free logic tests.

mod loopback_client;
mod microphone_client;

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use transcriber_audio::{AudioSource, SourceFactory, SourceFactoryError};
use transcriber_core::{CaptureSubject, StreamIdentity};
use wasapi::{Device, DeviceEnumerator, Direction};

use crate::process_tree::{ProcessSnapshot, identity_still_matches};
use crate::sessions::{RenderSession, SessionActivity, active_capture_subjects};
use loopback_client::LoopbackSource;
use microphone_client::MicrophoneSource;

/// The Windows [`SourceFactory`]: enumerates applications with an active
/// render stream via WASAPI and `sysinfo`, opens the `Remote` loopback
/// stream for one of them, and opens the `Local` microphone stream with OS
/// echo cancellation where the platform supports it.
///
/// [`SourceFactory::list_subjects`], [`SourceFactory::open_remote`] and
/// [`SourceFactory::open_local`] are all implemented.
#[derive(Debug, Default)]
pub struct WindowsSources;

impl WindowsSources {
    /// Builds a factory. Talks to no device or process table until a
    /// [`SourceFactory`] method is called.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl SourceFactory for WindowsSources {
    fn list_subjects(&self) -> Result<Vec<CaptureSubject>, SourceFactoryError> {
        let sessions = render_sessions()?;
        let processes = process_snapshots();
        Ok(active_capture_subjects(
            &sessions,
            &processes,
            std::process::id(),
        ))
    }

    fn open_remote(
        &self,
        subject: &CaptureSubject,
    ) -> Result<Box<dyn AudioSource>, SourceFactoryError> {
        verify_subject_identity(subject)?;
        let source =
            LoopbackSource::open(subject.root_pid()).map_err(|error| SourceFactoryError::Open {
                identity: StreamIdentity::Remote,
                reason: error.to_string(),
            })?;
        Ok(Box::new(source))
    }

    fn open_local(&self) -> Result<Option<Box<dyn AudioSource>>, SourceFactoryError> {
        match MicrophoneSource::open() {
            Ok(Some(source)) => Ok(Some(Box::new(source))),
            Ok(None) => Ok(None),
            Err(error) => Err(SourceFactoryError::Open {
                identity: StreamIdentity::Local,
                reason: error.to_string(),
            }),
        }
    }
}

/// Re-checks `subject`'s process identity against a fresh process-table
/// lookup, failing loudly on a mismatch rather than opening whatever
/// process now sits at that PID — Windows recycles PIDs, and the spec calls
/// a mismatch here "a consent breach, not just a bug" (Windows-backend
/// prior decisions). A PID with no current process-table entry at all (the
/// process exited) is treated the same as a mismatch: there is nothing left
/// to re-identify.
fn verify_subject_identity(subject: &CaptureSubject) -> Result<(), SourceFactoryError> {
    let processes = process_snapshots();
    let still_matches = processes
        .get(&subject.root_pid())
        .is_some_and(|process| identity_still_matches(subject, &process.name, process.started_at));
    if still_matches {
        return Ok(());
    }
    Err(SourceFactoryError::Open {
        identity: StreamIdentity::Remote,
        reason: format!(
            "process at PID {} no longer matches '{}' as consented to at listing time \
             (recycled PID or the process exited)",
            subject.root_pid(),
            subject.process_name(),
        ),
    })
}

/// Wraps any failure from this module's WASAPI calls into the one
/// enumeration error [`SourceFactory::list_subjects`] can return.
fn enumeration_failed(error: impl std::fmt::Display) -> SourceFactoryError {
    SourceFactoryError::Enumeration {
        reason: error.to_string(),
    }
}

/// Every audio session on every active render device, translated to
/// [`RenderSession`] — the only function in this crate that calls into
/// `wasapi`.
///
/// Enumerates the `Direction::Render` device collection, not
/// `Direction::Capture` — the spec calls out that the reference example
/// enumerates the latter, which would list who is *listening*, not who is
/// *rendering* audio.
fn render_sessions() -> Result<Vec<RenderSession>, SourceFactoryError> {
    wasapi::initialize_mta().ok().map_err(enumeration_failed)?;
    let enumerator = DeviceEnumerator::new().map_err(enumeration_failed)?;
    let devices = enumerator
        .get_device_collection(&Direction::Render)
        .map_err(enumeration_failed)?;
    let mut sessions = Vec::new();
    for device in &devices {
        let device = device.map_err(enumeration_failed)?;
        sessions.extend(sessions_for_device(&device)?);
    }
    Ok(sessions)
}

/// The render sessions of one device — the session enumerator hangs off
/// the device, not the system, so [`render_sessions`] must call this once
/// per device in the collection rather than enumerating sessions globally.
fn sessions_for_device(device: &Device) -> Result<Vec<RenderSession>, SourceFactoryError> {
    let manager = device
        .get_iaudiosessionmanager()
        .map_err(enumeration_failed)?;
    let enumerator = manager
        .get_audiosessionenumerator()
        .map_err(enumeration_failed)?;
    let count = enumerator.get_count().map_err(enumeration_failed)?;
    let mut sessions = Vec::new();
    for index in 0..count {
        let control = enumerator.get_session(index).map_err(enumeration_failed)?;
        let state = control.get_state().map_err(enumeration_failed)?;
        let process_id = control.get_process_id().map_err(enumeration_failed)?;
        sessions.push(RenderSession {
            process_id,
            activity: session_activity(&state),
        });
    }
    Ok(sessions)
}

/// Translates `wasapi`'s session state into this crate's own vocabulary —
/// see [`crate::sessions`]'s module doc for why [`RenderSession`] does not
/// borrow `wasapi::SessionState` directly.
fn session_activity(state: &wasapi::SessionState) -> SessionActivity {
    match state {
        wasapi::SessionState::Active => SessionActivity::Active,
        wasapi::SessionState::Inactive => SessionActivity::Inactive,
        wasapi::SessionState::Expired => SessionActivity::Expired,
    }
}

/// Every process currently visible to `sysinfo`, translated to
/// [`ProcessSnapshot`] and keyed by PID — the only function in this crate
/// that calls into `sysinfo`.
///
/// Deliberately `ProcessRefreshKind::nothing()`, not `everything()`: name,
/// parent PID and start time — the only three fields this function reads —
/// are populated unconditionally when `sysinfo` discovers a process, not
/// gated by the refresh kind. `everything()` would additionally pull every
/// visible process's command line and environment block (on Windows, via
/// `ReadProcessMemory` on each target's PEB) into this process's
/// non-zeroized heap for data nothing here uses — over-collection this
/// project's data-minimisation principle does not allow.
fn process_snapshots() -> HashMap<u32, ProcessSnapshot> {
    let refresh =
        sysinfo::RefreshKind::nothing().with_processes(sysinfo::ProcessRefreshKind::nothing());
    let system = sysinfo::System::new_with_specifics(refresh);
    system
        .processes()
        .values()
        .map(|process| {
            let snapshot = ProcessSnapshot {
                name: process.name().to_string_lossy().into_owned(),
                parent_pid: process.parent().map(sysinfo::Pid::as_u32),
                started_at: SystemTime::UNIX_EPOCH + Duration::from_secs(process.start_time()),
            };
            (process.pid().as_u32(), snapshot)
        })
        .collect()
}
