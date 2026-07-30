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
//! Two env vars, both read only by this binary and set only by the
//! harness-level tests in `tests/fs_audit.rs` (never by the main audit
//! test), let the parent prove its detection actually works end to end
//! through a *real* child process rather than only against an in-test
//! fixture: [`LEAK_EVIDENCE_ENV_VAR`] leaves a real file behind in both
//! watched directories, and [`TRANSIENT_WRITE_ENV_VAR`] writes one, holds
//! it for a given duration, then deletes it before exit.
//!
//! Never invoked directly: only ever spawned via
//! `env!("CARGO_BIN_EXE_fs_audit_child")` from `tests/fs_audit.rs`.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use transcriber_audio::{
    AudioSource, AudioSourceError, AudioSourceEvent, SourceDegradation, SourceFactory,
    StreamFormat, TestToneSources,
};
use transcriber_core::{AttestationVersion, CaptureSubject, ConsentAttestation, StreamIdentity};
use transcriber_session::{CapturePlan, Session};

/// Frames of synthetic tone each stream produces before its source reports
/// [`AudioSourceEvent::Ended`] on its own — large enough to keep both
/// capture threads genuinely busy for a while (see the module doc on
/// `tests/fs_audit.rs`'s poll interval), small enough that the whole child
/// still exits in well under the parent's wait deadline.
const TOTAL_FRAMES: u64 = 2_000_000;

/// How long the child sleeps after `Session::start` returns, before calling
/// `Session::stop` explicitly — on top of however long the capture threads
/// stay genuinely busy processing [`TOTAL_FRAMES`].
const CAPTURE_WINDOW: Duration = Duration::from_millis(100);

/// Set (to any value) only by `tests/fs_audit.rs`'s
/// `the_real_harness_notices_a_leak_from_a_real_child_process` — leaves a
/// real, persistent file behind in the working directory and in the
/// (redirected) temp directory after an otherwise ordinary session, so that
/// test can prove the parent's actual watch-and-diff machinery notices a
/// leak from a real process, not just from an in-test fixture.
const LEAK_EVIDENCE_ENV_VAR: &str = "FS_AUDIT_CHILD_LEAK_EVIDENCE";

/// Set (to a millisecond count) only by `tests/fs_audit.rs`'s
/// `catches_a_transient_write_that_outlives_the_poll_interval` — writes a
/// file, holds it open for that long, then deletes it before this process
/// exits. Proves polling *can* catch a write immediately followed by a
/// delete, for a file that lives at least a few poll intervals; it cannot
/// prove polling catches every such file regardless of how briefly it
/// exists — no wall-clock poll from a separate process can, without an
/// OS-level filesystem watch, which this phase does not introduce.
const TRANSIENT_WRITE_ENV_VAR: &str = "FS_AUDIT_CHILD_TRANSIENT_WRITE_MS";

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

    fn degradation(&self) -> Option<SourceDegradation> {
        self.inner.degradation()
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

/// See [`LEAK_EVIDENCE_ENV_VAR`]. A no-op unless that variable is set.
fn leak_evidence_if_requested() {
    if std::env::var_os(LEAK_EVIDENCE_ENV_VAR).is_none() {
        return;
    }
    let _ = std::fs::write(
        "leaked-into-cwd.txt",
        b"deliberate leak for a harness-level test",
    );
    let _ = std::fs::write(
        std::env::temp_dir().join("leaked-into-tmp.txt"),
        b"deliberate leak for a harness-level test",
    );
}

/// See [`TRANSIENT_WRITE_ENV_VAR`]. A no-op unless that variable is set to a
/// valid millisecond count.
fn simulate_transient_write_if_requested() {
    let Some(value) = std::env::var_os(TRANSIENT_WRITE_ENV_VAR) else {
        return;
    };
    let Some(hold_ms) = value.to_str().and_then(|text| text.parse::<u64>().ok()) else {
        return;
    };
    let path = "transient-evidence.txt";
    let _ = std::fs::write(path, b"written, then removed, before this process exits");
    std::thread::sleep(Duration::from_millis(hold_ms));
    let _ = std::fs::remove_file(path);
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

    simulate_transient_write_if_requested();
    std::thread::sleep(CAPTURE_WINDOW);

    if session.stop().is_err() {
        fail("session failed to stop cleanly");
    }

    leak_evidence_if_requested();

    println!("remote_frames={}", remote_frames.load(Ordering::Relaxed));
    println!("local_frames={}", local_frames.load(Ordering::Relaxed));
}
