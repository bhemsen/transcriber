use std::time::Duration;

use transcriber_audio::{DeviceTimestamp, Frame, GapCause, RingBuffer, StreamFormat};

use super::StatsReader;

/// A tiny format keeps sample counts easy to reason about by hand.
fn tiny_format() -> StreamFormat {
    let Ok(format) = StreamFormat::new(10, 2) else {
        panic!("10 Hz stereo must be a valid format");
    };
    format
}

fn push(buffer: &mut RingBuffer, values: &[f32]) {
    buffer.push(Frame::samples(
        DeviceTimestamp::from_ticks(0),
        values.to_vec(),
    ));
}

#[test]
fn a_fresh_reader_over_an_empty_buffer_reports_all_zero() {
    let mut buffer = RingBuffer::new(tiny_format());
    let mut reader = StatsReader::register(&mut buffer, 2);

    let stats = reader.snapshot(&mut buffer, Duration::ZERO, None);

    assert_eq!(stats.frame_count, 0);
    assert_eq!(stats.loss_count, 0);
    assert_eq!(stats.gap_count, 0);
    assert_eq!(stats.level_dbfs, super::SILENCE_FLOOR_DBFS);
}

#[test]
fn frame_count_is_interleaved_samples_divided_by_channel_count() {
    let mut buffer = RingBuffer::new(tiny_format());
    let mut reader = StatsReader::register(&mut buffer, 2);

    push(&mut buffer, &[0.1, 0.1, 0.2, 0.2, 0.3, 0.3]);
    let stats = reader.snapshot(&mut buffer, Duration::from_secs(1), None);

    assert_eq!(stats.frame_count, 3);
    assert_eq!(stats.duration, Duration::from_secs(1));
}

#[test]
fn level_reflects_the_loudest_sample_read_since_the_previous_snapshot() {
    let mut buffer = RingBuffer::new(tiny_format());
    let mut reader = StatsReader::register(&mut buffer, 2);

    push(&mut buffer, &[0.5, -0.5]);
    let stats = reader.snapshot(&mut buffer, Duration::ZERO, None);

    let expected = 20.0 * 0.5_f32.log10();
    assert!((stats.level_dbfs - expected).abs() < 0.01);
}

/// A poll that finds nothing new must keep reporting the last known level,
/// not reset to silence — otherwise a live meter would flicker to the floor
/// between every two polls even while audio keeps arriving.
#[test]
fn level_holds_steady_across_a_poll_with_nothing_new_to_read() {
    let mut buffer = RingBuffer::new(tiny_format());
    let mut reader = StatsReader::register(&mut buffer, 2);

    push(&mut buffer, &[0.5, -0.5]);
    let first = reader.snapshot(&mut buffer, Duration::ZERO, None);
    let second = reader.snapshot(&mut buffer, Duration::ZERO, None);

    assert_eq!(first.level_dbfs, second.level_dbfs);
}

#[test]
fn true_silence_reports_the_floor_level() {
    let mut buffer = RingBuffer::new(tiny_format());
    let mut reader = StatsReader::register(&mut buffer, 2);

    buffer.push(Frame::silent(DeviceTimestamp::from_ticks(0), 4));
    let stats = reader.snapshot(&mut buffer, Duration::ZERO, None);

    assert_eq!(stats.level_dbfs, super::SILENCE_FLOOR_DBFS);
}

#[test]
fn gap_count_combines_discontinuity_and_timestamp_error() {
    let mut buffer = RingBuffer::new(tiny_format());
    let mut reader = StatsReader::register(&mut buffer, 2);

    buffer.push(Frame::gap(
        DeviceTimestamp::from_ticks(0),
        2,
        GapCause::Discontinuity,
    ));
    buffer.push(Frame::gap(
        DeviceTimestamp::from_ticks(1),
        2,
        GapCause::TimestampError,
    ));
    let stats = reader.snapshot(&mut buffer, Duration::ZERO, None);

    assert_eq!(stats.gap_count, 2);
}

/// Registering the reader *before* the buffer receives any samples, then
/// falling behind by more than one full capacity, must surface real loss —
/// the same contract `RingBuffer::read` already gives any other reader.
#[test]
fn a_reader_that_falls_behind_reports_loss() {
    let mut buffer = RingBuffer::new(tiny_format());
    let mut reader = StatsReader::register(&mut buffer, 2);
    let capacity = buffer.capacity();

    for i in 0..(capacity * 3) {
        push(&mut buffer, &[i as f32]);
    }
    let stats = reader.snapshot(&mut buffer, Duration::ZERO, None);

    assert!(stats.loss_count > 0);
}

#[test]
fn degradation_is_passed_through_unchanged() {
    use transcriber_audio::SourceDegradation;

    let mut buffer = RingBuffer::new(tiny_format());
    let mut reader = StatsReader::register(&mut buffer, 2);

    let stats = reader.snapshot(
        &mut buffer,
        Duration::ZERO,
        Some(SourceDegradation::EchoCancellationUnavailable),
    );

    assert_eq!(
        stats.degradation,
        Some(SourceDegradation::EchoCancellationUnavailable)
    );
}
