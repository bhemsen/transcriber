//! `capture --pid <PID>`: resolve the subject, gate on consent, open both
//! streams and print live per-stream statistics until the user stops it.

use std::io::{BufRead, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, SystemTime};

use transcriber_audio::SourceFactory;
use transcriber_core::{AttestationVersion, CaptureSubject, ConsentAttestation, StreamIdentity};
use transcriber_session::{CapturePlan, Session};

use crate::consent::{print_attestation, read_consent};
use crate::error::CliError;
use crate::language::Language;
use crate::stats_display::print_snapshot;
use crate::text::{resolved_subject_line, stream_label};

/// How often the running capture loop refreshes its statistics snapshot.
const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// How often the stop loop checks for a pending stop request — finer than
/// [`POLL_INTERVAL`] so pressing Enter is noticed promptly rather than only
/// on the next full snapshot.
const STOP_CHECK_INTERVAL: Duration = Duration::from_millis(100);

/// Runs `capture --pid <pid>`: resolves the subject, gates on consent,
/// opens both streams, then prints live statistics until the user presses
/// Enter.
pub(crate) fn run(factory: &dyn SourceFactory, pid: u32, lang: Language) -> Result<(), CliError> {
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let mut stdout = std::io::stdout();

    let Some(mut session) = resolve_and_start(factory, pid, lang, &mut input, &mut stdout)? else {
        return Ok(());
    };

    run_capture_loop(&mut session, lang, &mut stdout);
    Ok(())
}

/// The testable heart of `capture`: resolves `pid` against a fresh
/// [`SourceFactory::list_subjects`] call, shows the resolved root *before*
/// the consent prompt, gates on an explicit confirmation, opens both
/// streams and starts a [`Session`].
///
/// Returns `Ok(None)` — never a [`Session`] — the moment consent is
/// refused: nothing before that point opened a stream or started a capture
/// thread. Device-free with a [`transcriber_audio::TestToneSources`]
/// factory, which is exactly what makes this the function a test drives to
/// prove the consent gate is real.
///
/// # Errors
///
/// Returns [`CliError::SubjectNotFound`] if `pid` names no currently
/// active render stream, or whatever [`SourceFactory::open_remote`],
/// [`SourceFactory::open_local`] or [`Session::start`] returns.
pub(crate) fn resolve_and_start(
    factory: &dyn SourceFactory,
    pid: u32,
    lang: Language,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<Option<Session>, CliError> {
    let subject = find_subject(factory, pid)?;
    let _ = writeln!(
        output,
        "{}",
        resolved_subject_line(subject.process_name(), subject.root_pid(), lang)
    );

    print_attestation(output, lang);
    if !read_consent(input, lang) {
        print_refused(output, lang);
        return Ok(None);
    }

    let consent = ConsentAttestation::new(SystemTime::now(), AttestationVersion::V1);
    let remote = factory.open_remote(&subject)?;
    let local = factory.open_local()?;
    let has_local = local.is_some();
    let plan = CapturePlan::new(subject, remote, local);
    let session = Session::start(consent, plan)?;

    report_post_start_state(&session, has_local, lang, output);
    Ok(Some(session))
}

/// Finds the subject matching `pid` in a fresh listing. The spec's second,
/// binding identity re-check (name *and* start time, guarding against a
/// recycled PID) happens a moment later, inside
/// [`SourceFactory::open_remote`] itself, right before the stream opens.
fn find_subject(factory: &dyn SourceFactory, pid: u32) -> Result<CaptureSubject, CliError> {
    let subjects = factory.list_subjects()?;
    subjects
        .into_iter()
        .find(|subject| subject.root_pid() == pid)
        .ok_or(CliError::SubjectNotFound { pid })
}

/// Runs the live statistics loop until the user presses Enter (or stdin
/// closes), then stops the session and prints one final snapshot.
///
/// Known limitation, recorded rather than solved here: Ctrl+C terminates
/// the process immediately, without running [`Session`]'s `Drop` — a
/// graceful signal handler would need either a dependency outside this
/// phase's approved list or `unsafe` FFI, both out of scope. "Press Enter"
/// is the one supported graceful stop.
fn run_capture_loop(session: &mut Session, lang: Language, output: &mut impl Write) {
    let stop_requested = Arc::new(AtomicBool::new(false));
    spawn_stop_listener(Arc::clone(&stop_requested));

    let ticks_per_snapshot =
        (POLL_INTERVAL.as_millis() / STOP_CHECK_INTERVAL.as_millis()).max(1) as u32;
    let mut ticks_since_snapshot = 0u32;

    print_snapshot(session, lang, output);
    while !stop_requested.load(Ordering::Relaxed) {
        thread::sleep(STOP_CHECK_INTERVAL);
        ticks_since_snapshot += 1;
        if ticks_since_snapshot >= ticks_per_snapshot {
            print_snapshot(session, lang, output);
            ticks_since_snapshot = 0;
        }
    }

    let _ = writeln!(output, "\n{}", stopping_notice(lang));
    let _ = session.stop();
    print_snapshot(session, lang, output);
}

/// Blocks on one line from stdin (Enter, or end-of-input) on a dedicated
/// thread, then sets `flag` — the harness's only stop control.
fn spawn_stop_listener(flag: Arc<AtomicBool>) {
    thread::spawn(move || {
        let mut line = String::new();
        let _ = std::io::stdin().lock().read_line(&mut line);
        flag.store(true, Ordering::Relaxed);
    });
}

/// Prints the notices a fresh [`Session`] warrants: a missing microphone,
/// and either stream's disclosed AEC degradation, plus the stop hint.
fn report_post_start_state(
    session: &Session,
    has_local: bool,
    lang: Language,
    output: &mut impl Write,
) {
    let _ = writeln!(output, "\n{}", started_notice(lang));
    if !has_local {
        let _ = writeln!(output, "{}", mic_missing_warning(lang));
    }
    if session.remote_degradation().is_some() {
        let _ = writeln!(output, "{}", aec_warning(StreamIdentity::Remote, lang));
    }
    if session.local_degradation().is_some() {
        let _ = writeln!(output, "{}", aec_warning(StreamIdentity::Local, lang));
    }
    let _ = writeln!(output, "{}\n", stop_hint(lang));
}

/// Printed once consent was refused — makes the spec's "both frame
/// counters stay at 0" requirement literally readable, without a
/// [`Session`] ever existing to query.
fn print_refused(output: &mut impl Write, lang: Language) {
    let message = match lang {
        Language::De => {
            "Abgebrochen: keine Bestätigung. Keine Erfassung gestartet \
             (Gegenseite: 0 Frames, Ich (Mikrofon): 0 Frames)."
        }
        Language::En => {
            "Aborted: consent was not confirmed. No capture started \
             (Remote: 0 frames, Local: 0 frames)."
        }
    };
    let _ = writeln!(output, "\n{message}");
}

/// Printed once the session has actually started.
fn started_notice(lang: Language) -> &'static str {
    match lang {
        Language::De => "Erfassung läuft.",
        Language::En => "Capture started.",
    }
}

/// Printed once, right after start, if no microphone was found — the
/// spec's decision that this is a warning, not an abort.
fn mic_missing_warning(lang: Language) -> &'static str {
    match lang {
        Language::De => "Warnung: kein Mikrofon gefunden — nur die Gegenseite wird erfasst.",
        Language::En => "Warning: no microphone found — capturing the remote side only.",
    }
}

/// Printed once, right after start, for a stream whose source disclosed
/// [`transcriber_audio::SourceDegradation::EchoCancellationUnavailable`].
fn aec_warning(identity: StreamIdentity, lang: Language) -> String {
    let label = stream_label(identity, lang);
    match lang {
        Language::De => format!(
            "Warnung: Echokompensation für {label} nicht verfügbar — \
             Ton der Gegenseite kann über Lautsprecher einstreuen."
        ),
        Language::En => format!(
            "Warning: echo cancellation unavailable for {label} — \
             the other side's audio may bleed in over speakers."
        ),
    }
}

/// Printed once, right after start, telling the user how to stop.
fn stop_hint(lang: Language) -> &'static str {
    match lang {
        Language::De => "Zum Beenden Enter drücken.",
        Language::En => "Press Enter to stop.",
    }
}

/// Printed once the user asked to stop, before the final snapshot.
fn stopping_notice(lang: Language) -> &'static str {
    match lang {
        Language::De => "Beende Erfassung …",
        Language::En => "Stopping capture …",
    }
}

#[cfg(test)]
mod tests;
