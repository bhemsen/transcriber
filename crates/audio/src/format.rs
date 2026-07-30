//! Sample rate and channel layout of a PCM stream.

use thiserror::Error;

/// Sample rate and channel count of a stream of interleaved `f32` samples.
///
/// Every stream this phase captures is configured to exactly one sample
/// representation — 32-bit float, see the spec's prior decisions on the
/// Windows backend — so this type only needs to carry the two dimensions
/// that vary between streams: how many samples make up one second, and how
/// many channels interleave into one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StreamFormat {
    sample_rate_hz: u32,
    channels: u16,
}

impl StreamFormat {
    /// Builds a format, rejecting a rate or channel count that could not
    /// carry any audio.
    pub fn new(sample_rate_hz: u32, channels: u16) -> Result<Self, FormatError> {
        if sample_rate_hz == 0 {
            return Err(FormatError::ZeroSampleRate);
        }
        if channels == 0 {
            return Err(FormatError::ZeroChannels);
        }
        Ok(Self {
            sample_rate_hz,
            channels,
        })
    }

    /// Samples per second, in Hz.
    #[must_use]
    pub fn sample_rate_hz(&self) -> u32 {
        self.sample_rate_hz
    }

    /// Interleaved channel count.
    #[must_use]
    pub fn channels(&self) -> u16 {
        self.channels
    }

    /// Interleaved `f32` samples making up one second of this format —
    /// `sample_rate_hz * channels`.
    #[must_use]
    pub fn interleaved_samples_per_second(&self) -> u64 {
        u64::from(self.sample_rate_hz) * u64::from(self.channels)
    }
}

/// An invalid [`StreamFormat`] that no reader could interpret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum FormatError {
    /// A sample rate of 0 Hz carries no audio.
    #[error("sample rate must be greater than 0 Hz")]
    ZeroSampleRate,
    /// 0 channels carries no audio.
    #[error("channel count must be greater than 0")]
    ZeroChannels,
}

#[cfg(test)]
mod tests {
    use super::{FormatError, StreamFormat};

    #[test]
    fn accepts_the_windows_backend_format() {
        let Ok(format) = StreamFormat::new(48_000, 2) else {
            panic!("48 kHz stereo must be a valid format");
        };
        assert_eq!(format.sample_rate_hz(), 48_000);
        assert_eq!(format.channels(), 2);
        assert_eq!(format.interleaved_samples_per_second(), 96_000);
    }

    #[test]
    fn rejects_zero_sample_rate() {
        assert_eq!(StreamFormat::new(0, 2), Err(FormatError::ZeroSampleRate));
    }

    #[test]
    fn rejects_zero_channels() {
        assert_eq!(StreamFormat::new(48_000, 0), Err(FormatError::ZeroChannels));
    }
}
