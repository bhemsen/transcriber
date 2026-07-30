//! Device-free logic tests for [`super::frame_for_packet`],
//! [`super::bytes_to_f32_samples`] and [`super::timeout_millis`] — see
//! `crate::sources` for the WASAPI edge this module's logic sits behind,
//! which cannot be exercised without a device.

use std::time::Duration;

use transcriber_audio::{DeviceTimestamp, Frame, GapCause};

use super::{GapCounters, PacketFlags, bytes_to_f32_samples, frame_for_packet, timeout_millis};

/// No flag set — the "ordinary packet" baseline the other cases each flip
/// exactly one field of.
fn no_flags() -> PacketFlags {
    PacketFlags {
        data_discontinuity: false,
        silent: false,
        timestamp_error: false,
    }
}

#[test]
fn unflagged_packet_becomes_a_samples_frame_and_touches_no_counter() {
    let mut counters = GapCounters::default();
    let frame = frame_for_packet(
        DeviceTimestamp::from_ticks(10),
        2,
        vec![0.5, -0.5],
        no_flags(),
        &mut counters,
    );
    let Frame::Samples { samples, .. } = frame else {
        panic!("an unflagged packet must become Frame::Samples");
    };
    assert_eq!(&*samples, &[0.5, -0.5]);
    assert_eq!(counters, GapCounters::default());
}

#[test]
fn discontinuity_flag_becomes_a_gap_on_its_own_counter() {
    let mut counters = GapCounters::default();
    let frame = frame_for_packet(
        DeviceTimestamp::from_ticks(10),
        4,
        vec![1.0, 1.0, 1.0, 1.0],
        PacketFlags {
            data_discontinuity: true,
            ..no_flags()
        },
        &mut counters,
    );
    let Frame::Gap { cause, len, .. } = frame else {
        panic!("data_discontinuity must become Frame::Gap");
    };
    assert_eq!(cause, GapCause::Discontinuity);
    assert_eq!(len, 4);
    assert_eq!(counters.discontinuity, 1);
    assert_eq!(counters.timestamp_error, 0);
}

#[test]
fn timestamp_error_flag_becomes_a_gap_on_a_separate_counter() {
    let mut counters = GapCounters::default();
    let frame = frame_for_packet(
        DeviceTimestamp::from_ticks(20),
        4,
        vec![1.0, 1.0, 1.0, 1.0],
        PacketFlags {
            timestamp_error: true,
            ..no_flags()
        },
        &mut counters,
    );
    let Frame::Gap { cause, .. } = frame else {
        panic!("timestamp_error must become Frame::Gap");
    };
    assert_eq!(cause, GapCause::TimestampError);
    assert_eq!(counters.timestamp_error, 1);
    assert_eq!(counters.discontinuity, 0);
}

#[test]
fn both_gap_flags_in_one_packet_only_record_discontinuity() {
    // Not expected from a real device, but the mapping must still be total
    // and deterministic rather than double-counting or panicking.
    let mut counters = GapCounters::default();
    let frame = frame_for_packet(
        DeviceTimestamp::from_ticks(30),
        4,
        vec![1.0, 1.0, 1.0, 1.0],
        PacketFlags {
            data_discontinuity: true,
            timestamp_error: true,
            ..no_flags()
        },
        &mut counters,
    );
    let Frame::Gap { cause, .. } = frame else {
        panic!("a flagged packet must become Frame::Gap");
    };
    assert_eq!(cause, GapCause::Discontinuity);
    assert_eq!(counters.discontinuity, 1);
    assert_eq!(counters.timestamp_error, 0);
}

#[test]
fn silent_flag_materialises_dense_zeros_not_a_gap() {
    let mut counters = GapCounters::default();
    let frame = frame_for_packet(
        DeviceTimestamp::from_ticks(40),
        3,
        vec![9.0, 9.0, 9.0], // whatever the device buffer held must be ignored
        PacketFlags {
            silent: true,
            ..no_flags()
        },
        &mut counters,
    );
    let Frame::Samples { samples, .. } = frame else {
        panic!("silent must materialise as Frame::Samples of zeros, not a gap");
    };
    assert_eq!(&*samples, &[0.0, 0.0, 0.0]);
    assert_eq!(counters, GapCounters::default());
}

#[test]
fn bytes_decode_to_native_endian_f32_samples() {
    let mut bytes = 1.5f32.to_ne_bytes().to_vec();
    bytes.extend_from_slice(&(-2.5f32).to_ne_bytes());
    assert_eq!(bytes_to_f32_samples(&bytes), vec![1.5, -2.5]);
}

#[test]
fn a_trailing_partial_chunk_is_dropped_not_panicked_on() {
    let mut bytes = 1.0f32.to_ne_bytes().to_vec();
    bytes.push(0xFF);
    bytes.push(0xFF);
    assert_eq!(bytes_to_f32_samples(&bytes), vec![1.0]);
}

#[test]
fn timeout_converts_to_the_expected_millisecond_count() {
    assert_eq!(timeout_millis(Duration::from_millis(500)), 500);
    assert_eq!(timeout_millis(Duration::ZERO), 0);
}

#[test]
fn timeout_saturates_instead_of_overflowing_past_u32_max_millis() {
    let far_future = Duration::from_secs(u64::from(u32::MAX) + 10);
    assert_eq!(timeout_millis(far_future), u32::MAX);
}
