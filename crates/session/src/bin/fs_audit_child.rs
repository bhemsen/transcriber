//! Child process spawned by `tests/fs_audit.rs`, the device-free session
//! FS audit.
//!
//! The audit needs a real process boundary to redirect `TMP`/`TEMP`:
//! `std::env::set_var` is `unsafe` in edition 2024 and `forbid(unsafe_code)`
//! covers test code too, so an in-process redirect is not a valid route
//! (spec, "Gates und Dokumente"). This binary is that child process — it
//! runs one whole session (start -> capture real frames from the test tone
//! -> stop) inside whatever working directory and `TMP`/`TEMP` the parent
//! test gave it (`Command::current_dir` / `Command::env`), then reports how
//! many frames each stream actually pulled on stdout. That report is the
//! parent's proof the child did real work — a session that silently failed
//! to start would otherwise "write 0 files" and pass the audit vacuously.
//!
//! Never invoked directly: only ever spawned via
//! `env!("CARGO_BIN_EXE_fs_audit_child")` from `tests/fs_audit.rs`.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use transcriber_audio::{
    AudioSource, AudioSourceError, AudioSourceEvent, SourceFactory, StreamFormat, TestToneSources,
};
use transcriber_core::{AttestationVersion, CaptureSubject, ConsentAttestation, StreamIdentity};
use transcriber_session::{CapturePlan, Session};

/// Frames of synthetic tone each stream produces before its source reports
/// [`AudioSourceEvent::Ended`] on its own — generous enough that the frame
/// counters below are unambiguously nonzero, small enough that the whole
/// child exits in well under a second.
const TOTAL_FRAMES: u64 = 48_000;

/// How long the child lets both capture threads run before calling
/// `Session::stop` explicitly — wide enough for the parent's polling loop
/// (`tests/fs_audit.rs`) to observe the watched directories at least once
/// while this process is alive.
const CAPTURE_WINDOW: Duration = Duration::from_millis(200);

/// Wraps a source and counts every [`AudioSourceEvent::Frame`] it actually
/// produces — this child's proof that the session did real work, not just
/// started and exited.
struct CountingSource {
    inner: Box<dyn AudioSource>,
    frames_pulled: Arc<AtomicU64>,
}

impl AudioSource for CountingSource {
    fn identity(&self) -> StreamIdentity {
        self.inner.identity()
    }

    fn format(&self) -> StreamFormat {
        self.inner.format()
    }

    fn pull(&mut self, timeout: Duration) -> Result<AudioSourceEvent, AudioSourceError> {
        let event = self.inner.pull(timeout)?;
        if matches!(event, AudioSourceEvent::Frame(_)) {
            self.frames_pulled.fetch_add(1, Ordering::Relaxed);
        }
        Ok(event)
    }
}

/// Prints `message` to stderr and exits with a distinct, non-zero code —
/// this child's only way to fail loudly, since `unwrap`/`expect` are denied
/// outside tests and this binary is not a test.
fn fail(message: &str) -> ! {
    eprintln!("fs_audit_child: {message}");
    std::process::exit(2);
}

/// Opens both streams through `factory`, each wrapped in a counter, and
/// assembles the plan [`Session::start`] needs.
fn build_plan(
    factory: &TestToneSources,
    subject: CaptureSubject,
) -> (CapturePlan, Arc<AtomicU64>, Arc<AtomicU64>) {
    let remote_frames = Arc::new(AtomicU64::new(0));
    let local_frames = Arc::new(AtomicU64::new(0));

    let Ok(remote_source) = factory.open_remote(&subject) else {
        fail("test factory failed to open the remote source");
    };
    let remote_source: Box<dyn AudioSource> = Box::new(CountingSource {
        inner: remote_source,
        frames_pulled: Arc::clone(&remote_frames),
    });

    let Ok(local_source) = factory.open_local() else {
        fail("test factory failed to open the local source");
    };
    let local_source = local_source.map(|inner| {
        Box::new(CountingSource {
            inner,
            frames_pulled: Arc::clone(&local_frames),
        }) as Box<dyn AudioSource>
    });

    (
        CapturePlan::new(subject, remote_source, local_source),
        remote_frames,
        local_frames,
    )
}

fn main() {
    let Ok(factory) = TestToneSources::new() else {
        fail("fixed 48 kHz stereo format is always valid");
    };
    let factory = factory.with_total_frames(TOTAL_FRAMES);

    let Ok(mut subjects) = factory.list_subjects() else {
        fail("test factory failed to list subjects");
    };
    let Some(subject) = subjects.pop() else {
        fail("test factory listed no subject");
    };

    let (plan, remote_frames, local_frames) = build_plan(&factory, subject);
    let consent = ConsentAttestation::new(SystemTime::now(), AttestationVersion::V1);

    let Ok(mut session) = Session::start(consent, plan) else {
        fail("session failed to start");
    };

    std::thread::sleep(CAPTURE_WINDOW);

    if session.stop().is_err() {
        fail("session failed to stop cleanly");
    }

    println!("remote_frames={}", remote_frames.load(Ordering::Relaxed));
    println!("local_frames={}", local_frames.load(Ordering::Relaxed));
}
