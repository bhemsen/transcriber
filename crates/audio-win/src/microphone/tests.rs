//! Device-free logic tests for [`super::aec_degradation`] and
//! [`super::microphone_absent`] — see `crate::sources::microphone_client`
//! for the WASAPI edge these decisions sit behind, which cannot be
//! exercised without a device.

use transcriber_audio::SourceDegradation;

use super::{aec_degradation, microphone_absent};

#[test]
fn aec_supported_carries_no_degradation() {
    assert_eq!(aec_degradation(true), None);
}

#[test]
fn aec_unsupported_discloses_echo_cancellation_unavailable() {
    assert_eq!(
        aec_degradation(false),
        Some(SourceDegradation::EchoCancellationUnavailable)
    );
}

#[test]
fn the_documented_no_default_device_hresult_is_recognised_as_absent() {
    assert!(microphone_absent(0x8007_0490_u32 as i32));
}

#[test]
fn an_unrelated_hresult_is_not_mistaken_for_an_absent_microphone() {
    // E_ACCESSDENIED (0x80070005) - the spec's other named human
    // prerequisite (the Windows microphone privacy toggle switched off) —
    // must surface as a real failure, not be swallowed as "no microphone".
    assert!(!microphone_absent(0x8007_0005_u32 as i32));
}
