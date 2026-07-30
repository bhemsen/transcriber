//! Pure decisions for the microphone edge, kept free of `wasapi`/`windows`
//! so they run on a CI runner with no audio device — the same discipline
//! [`crate::loopback`]'s module doc holds the packet-mapping logic it
//! exposes to, logic the microphone edge
//! (`crate::sources::microphone_client`) reuses unchanged.

use transcriber_audio::SourceDegradation;

/// The HRESULT `IMMDeviceEnumerator::GetDefaultAudioEndpoint` returns when
/// no device exists for the requested role and direction — mmdeviceapi's
/// `E_NOTFOUND`, `HRESULT_FROM_WIN32(ERROR_NOT_FOUND)` (`0x8007_0490`).
///
/// Compared as a plain `i32` rather than `windows_core::HRESULT`, so this
/// stays testable without a device — see [`microphone_absent`].
const NO_DEFAULT_DEVICE_HRESULT: i32 = 0x8007_0490_u32 as i32;

/// Whether a failed default-communications-capture-device lookup means "no
/// microphone is present" — the spec's `Ok(None)`, not an error — rather
/// than a real failure `crate::sources::WindowsSources::open_local` must
/// report.
///
/// `hresult` is the raw `windows_core::HRESULT` code the edge extracts
/// before calling this, e.g. a denied microphone privacy permission
/// (`E_ACCESSDENIED`, a different code) must **not** be mistaken for an
/// absent device — that failure needs to surface loudly, not be swallowed
/// as "nothing to warn about".
pub(crate) fn microphone_absent(hresult: i32) -> bool {
    hresult == NO_DEFAULT_DEVICE_HRESULT
}

/// Maps the `is_aec_supported()` probe to the spec's disclosed-degradation
/// decision: supported carries no degradation; unsupported becomes
/// [`SourceDegradation::EchoCancellationUnavailable`] rather than refusing
/// to start (spec's acceptance-gate decision — warn and continue).
pub(crate) fn aec_degradation(is_supported: bool) -> Option<SourceDegradation> {
    if is_supported {
        None
    } else {
        Some(SourceDegradation::EchoCancellationUnavailable)
    }
}

#[cfg(test)]
mod tests;
