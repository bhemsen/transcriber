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
    // Derived from the Win32 HRESULT formula rather than repeating the
    // implementation's own literal, so a transposed digit in either place
    // would be caught: HRESULT_FROM_WIN32(ERROR_NOT_FOUND) ==
    // (FACILITY_WIN32 << 16) | 0x8000_0000 | ERROR_NOT_FOUND.
    const FACILITY_WIN32: u32 = 7;
    const ERROR_NOT_FOUND: u32 = 1168;
    let e_notfound = ((FACILITY_WIN32 << 16) | 0x8000_0000 | ERROR_NOT_FOUND) as i32;
    assert!(microphone_absent(e_notfound));
}

#[test]
fn an_unrelated_hresult_is_not_mistaken_for_an_absent_microphone() {
    // E_ACCESSDENIED (0x80070005) - a different real failure than "no
    // device", used here only to prove the classifier is an exact match on
    // the one documented code, not a loose "any Win32 HRESULT counts as
    // absent" heuristic. (The Windows microphone privacy toggle, this
    // phase's other human prerequisite, actually fails later — at
    // `IAudioClient::Initialize` — so it never reaches this classifier at
    // all; it surfaces as an ordinary `Err` from a later step in
    // `OpenClient::open`, not via this comparison.)
    assert!(!microphone_absent(0x8007_0005_u32 as i32));
}
