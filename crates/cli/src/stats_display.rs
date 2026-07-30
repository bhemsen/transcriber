//! Formats the six required per-stream statistics — frame count, duration,
//! level, loss count, gap count and the AEC state — the manual QA gate's
//! source-isolation, stream-separation and echo-cancellation checks read
//! directly off this output.

use std::io::Write;

use transcriber_audio::SourceDegradation;
use transcriber_core::StreamIdentity;
use transcriber_session::{Session, StreamStats};

use crate::language::Language;
use crate::text::stream_label;

/// Prints one line per open stream with its current statistics — `Local`
/// prints an absence notice instead when there is no microphone stream.
pub(crate) fn print_snapshot(session: &Session, lang: Language, output: &mut impl Write) {
    if let Some(stats) = session.stream_stats(StreamIdentity::Remote) {
        let _ = writeln!(
            output,
            "{}",
            format_stat_line(StreamIdentity::Remote, &stats, lang)
        );
    }
    match session.stream_stats(StreamIdentity::Local) {
        Some(stats) => {
            let _ = writeln!(
                output,
                "{}",
                format_stat_line(StreamIdentity::Local, &stats, lang)
            );
        }
        None => {
            let _ = writeln!(
                output,
                "  [{}] {}",
                stream_label(StreamIdentity::Local, lang),
                local_absent(lang)
            );
        }
    }
}

/// One stream's statistics as a single, labelled line — a stable scale
/// (peak dBFS) so the isolation and AEC checks can compare `Remote` and
/// `Local` at a glance instead of reading a raw float.
fn format_stat_line(identity: StreamIdentity, stats: &StreamStats, lang: Language) -> String {
    let label = stream_label(identity, lang);
    let aec = format_degradation(stats.degradation, lang);
    format!(
        "  [{label}] frames={} duration={:.2}s level={:.1} dBFS loss={} gaps={} aec={aec}",
        stats.frame_count,
        stats.duration.as_secs_f64(),
        stats.level_dbfs,
        stats.loss_count,
        stats.gap_count,
    )
}

/// The AEC-state column: `ok` when no degradation was disclosed, an
/// explicit warning otherwise — visible in every snapshot, not just once at
/// start.
fn format_degradation(degradation: Option<SourceDegradation>, lang: Language) -> &'static str {
    match (degradation, lang) {
        (None, Language::De) => "ok",
        (None, Language::En) => "ok",
        (Some(SourceDegradation::EchoCancellationUnavailable), Language::De) => {
            "Warnung: nicht verfügbar"
        }
        (Some(SourceDegradation::EchoCancellationUnavailable), Language::En) => {
            "warning: unavailable"
        }
    }
}

/// Printed for `Local` instead of a statistics line when there is no
/// microphone stream.
fn local_absent(lang: Language) -> &'static str {
    match lang {
        Language::De => "kein Mikrofon-Strom",
        Language::En => "no microphone stream",
    }
}

#[cfg(test)]
mod tests;
