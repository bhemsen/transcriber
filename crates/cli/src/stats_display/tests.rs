use std::time::Duration;

use transcriber_audio::{SourceDegradation, SourceFactory, TestToneSources};
use transcriber_core::{AttestationVersion, ConsentAttestation, StreamIdentity};
use transcriber_session::{CapturePlan, Session, StreamStats};

use super::print_snapshot;
use crate::language::Language;

fn sample_stats() -> StreamStats {
    StreamStats {
        frame_count: 48_000,
        duration: Duration::from_secs(1),
        level_dbfs: -18.4,
        loss_count: 0,
        gap_count: 0,
        degradation: None,
    }
}

#[test]
fn a_snapshot_line_names_the_stream_and_carries_every_required_statistic() {
    use super::format_stat_line;

    let line = format_stat_line(StreamIdentity::Remote, &sample_stats(), Language::En);

    assert!(line.contains("Remote"));
    assert!(line.contains("frames=48000"));
    assert!(line.contains("duration=1.00s"));
    assert!(line.contains("-18.4"));
    assert!(line.contains("loss=0"));
    assert!(line.contains("gaps=0"));
    assert!(line.contains("aec=ok"));
}

#[test]
fn a_disclosed_degradation_is_a_visible_warning_not_ok() {
    use super::format_stat_line;

    let mut stats = sample_stats();
    stats.degradation = Some(SourceDegradation::EchoCancellationUnavailable);
    let line = format_stat_line(StreamIdentity::Local, &stats, Language::En);

    assert!(line.contains("warning"));
    assert!(!line.contains("aec=ok"));
}

fn started_session(factory: &TestToneSources) -> Session {
    let Ok(mut subjects) = factory.list_subjects() else {
        panic!("test factory always lists one subject");
    };
    let subject = subjects.remove(0);
    let Ok(remote) = factory.open_remote(&subject) else {
        panic!("test factory never fails to open");
    };
    let Ok(local) = factory.open_local() else {
        panic!("test factory never fails to open");
    };
    let plan = CapturePlan::new(subject, remote, local);
    let consent = ConsentAttestation::new(std::time::SystemTime::now(), AttestationVersion::V1);
    let Ok(session) = Session::start(consent, plan) else {
        panic!("a consented start with valid sources must succeed");
    };
    session
}

#[test]
fn print_snapshot_reports_both_streams_by_default() {
    let Ok(factory) = TestToneSources::new() else {
        panic!("fixed 48 kHz stereo format is always valid");
    };
    let factory = factory.with_total_frames(480);
    let session = started_session(&factory);
    let mut output = Vec::new();

    print_snapshot(&session, Language::En, &mut output);

    let rendered = String::from_utf8_lossy(&output);
    assert!(rendered.contains("Remote"));
    assert!(rendered.contains("Local"));
}

#[test]
fn print_snapshot_names_a_missing_microphone_instead_of_a_stats_line() {
    let Ok(factory) = TestToneSources::new() else {
        panic!("fixed 48 kHz stereo format is always valid");
    };
    let factory = factory.with_total_frames(480).without_microphone();
    let session = started_session(&factory);
    let mut output = Vec::new();

    print_snapshot(&session, Language::En, &mut output);

    let rendered = String::from_utf8_lossy(&output);
    assert!(rendered.contains("no microphone stream"));
}
