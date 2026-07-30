//! One packet handed to [`crate::RingBuffer::push`], and the WASAPI buffer
//! conditions this phase must not silently ignore.

use zeroize::Zeroizing;

/// Device-clock position of a frame, in 100-nanosecond ticks — the unit
/// WASAPI's `BufferInfo.timestamp` reports.
///
/// Carries the *device's own* timeline, not a wall-clock arrival time, which
/// buffering latency would distort. Normalising every stream's ticks against
/// a common session zero is `Session::start`'s job in `transcriber-session`,
/// not this crate's — see the spec's prior decisions ("Ringpuffer, Zeitachse
/// und Resampling").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeviceTimestamp(i64);

impl DeviceTimestamp {
    /// Wraps a raw 100-ns tick count read from the device.
    #[must_use]
    pub fn from_ticks(ticks: i64) -> Self {
        Self(ticks)
    }

    /// The raw 100-ns tick count.
    #[must_use]
    pub fn ticks(self) -> i64 {
        self.0
    }
}

/// Why a [`Frame::gap`] was recorded instead of samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GapCause {
    /// WASAPI's `data_discontinuity` flag: samples were dropped between this
    /// packet and the previous one.
    Discontinuity,
    /// WASAPI's `timestamp_error` flag: the device declares its own chosen
    /// time base unreliable for this packet.
    TimestampError,
}

/// One packet of PCM handed to [`crate::RingBuffer::push`].
///
/// `Debug` never prints sample content, only the metadata a reviewer needs
/// to diagnose a stream (timestamp, length, gap cause) — see the spec's
/// acceptance criteria for this issue.
pub enum Frame {
    /// Ordinary samples, or WASAPI's `silent` flag materialised as zeros so
    /// the sample axis stays dense — [`RingBuffer::push`](crate::RingBuffer::push)
    /// stores both the same way.
    Samples {
        /// Device timeline position of the first sample.
        timestamp: DeviceTimestamp,
        /// Interleaved PCM samples.
        samples: Zeroizing<Vec<f32>>,
    },
    /// An explicit, counted gap: `data_discontinuity` or `timestamp_error`.
    /// No sample data is stored for a gap — there is nothing trustworthy to
    /// store, see the spec's prior decisions.
    Gap {
        /// Device timeline position the gap starts at.
        timestamp: DeviceTimestamp,
        /// How many samples' worth of timeline the gap spans.
        len: usize,
        /// Which WASAPI flag produced this gap.
        cause: GapCause,
    },
}

impl Frame {
    /// Builds a frame of ordinary samples.
    #[must_use]
    pub fn samples(timestamp: DeviceTimestamp, samples: Vec<f32>) -> Self {
        Self::Samples {
            timestamp,
            samples: Zeroizing::new(samples),
        }
    }

    /// Builds a silent frame: `len` samples, materialised as zeros
    /// regardless of what the device buffer actually held, so the sample
    /// axis stays dense across silence.
    #[must_use]
    pub fn silent(timestamp: DeviceTimestamp, len: usize) -> Self {
        Self::Samples {
            timestamp,
            samples: Zeroizing::new(vec![0.0; len]),
        }
    }

    /// Builds an explicit gap frame for `cause`.
    #[must_use]
    pub fn gap(timestamp: DeviceTimestamp, len: usize, cause: GapCause) -> Self {
        Self::Gap {
            timestamp,
            len,
            cause,
        }
    }

    /// Device timeline position this frame starts at.
    #[must_use]
    pub fn timestamp(&self) -> DeviceTimestamp {
        match self {
            Self::Samples { timestamp, .. } | Self::Gap { timestamp, .. } => *timestamp,
        }
    }

    /// How many samples' worth of timeline this frame spans.
    #[must_use]
    pub fn sample_count(&self) -> usize {
        match self {
            Self::Samples { samples, .. } => samples.len(),
            Self::Gap { len, .. } => *len,
        }
    }
}

impl std::fmt::Debug for Frame {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Samples { timestamp, samples } => formatter
                .debug_struct("Frame::Samples")
                .field("timestamp", timestamp)
                .field("len", &samples.len())
                .finish(),
            Self::Gap {
                timestamp,
                len,
                cause,
            } => formatter
                .debug_struct("Frame::Gap")
                .field("timestamp", timestamp)
                .field("len", len)
                .field("cause", cause)
                .finish(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DeviceTimestamp, Frame, GapCause};

    #[test]
    fn silent_frame_materialises_zeros_of_the_requested_length() {
        let frame = Frame::silent(DeviceTimestamp::from_ticks(0), 4);
        assert_eq!(frame.sample_count(), 4);
        let Frame::Samples { samples, .. } = frame else {
            panic!("silent() must build a Samples frame");
        };
        assert_eq!(&*samples, &[0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn gap_carries_its_length_and_cause_without_sample_data() {
        let frame = Frame::gap(DeviceTimestamp::from_ticks(10), 7, GapCause::TimestampError);
        assert_eq!(frame.sample_count(), 7);
        assert_eq!(frame.timestamp(), DeviceTimestamp::from_ticks(10));
    }

    /// Proves the `Debug` claim rather than trusting the implementation: a
    /// distinctive sample value must never show up in the formatted output.
    #[test]
    fn debug_never_prints_sample_content() {
        let frame = Frame::samples(DeviceTimestamp::from_ticks(0), vec![42.5, -1.0]);
        let rendered = format!("{frame:?}");
        assert!(!rendered.contains("42.5"));
        assert!(rendered.contains("len"));
        assert!(rendered.contains('2'));
    }
}
