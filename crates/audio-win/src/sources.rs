//! The one edge module in this crate: everything that actually calls into
//! `wasapi` or `sysinfo` lives here, over the pure functions in
//! [`crate::process_tree`] and [`crate::sessions`].
//!
//! Not exercised by any test in this issue — the spec's risk table notes
//! `windows-latest` CI runners have no audio device, so this module gets
//! compile checks only; [`crate::process_tree`] and [`crate::sessions`]
//! carry the real, device-free logic tests.

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use transcriber_audio::{AudioSource, SourceFactory, SourceFactoryError};
use transcriber_core::{CaptureSubject, StreamIdentity};
use wasapi::{Device, DeviceEnumerator, Direction};

use crate::process_tree::ProcessSnapshot;
use crate::sessions::{RenderSession, SessionActivity, active_capture_subjects};

/// The Windows [`SourceFactory`]: enumerates applications with an active
/// render stream via WASAPI and `sysinfo`.
///
/// Only [`SourceFactory::list_subjects`] is implemented so far. `open_remote`
/// (issue #12, real loopback capture and the identity re-check via
/// [`crate::identity_still_matches`]) and `open_local` (issue #13,
/// microphone capture with echo cancellation) each return an explicit
/// [`SourceFactoryError::Open`] rather than a value that looks like a
/// working stream.
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
        _subject: &CaptureSubject,
    ) -> Result<Box<dyn AudioSource>, SourceFactoryError> {
        Err(SourceFactoryError::Open {
            identity: StreamIdentity::Remote,
            reason: "Windows loopback capture is not implemented yet (issue #12)".to_string(),
        })
    }

    fn open_local(&self) -> Result<Option<Box<dyn AudioSource>>, SourceFactoryError> {
        Err(SourceFactoryError::Open {
            identity: StreamIdentity::Local,
            reason: "Windows microphone capture is not implemented yet (issue #13)".to_string(),
        })
    }
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
