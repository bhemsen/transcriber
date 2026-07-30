//! Live per-stream statistics `cli` needs to print while a session runs —
//! the reader access issue #9's decision log deliberately deferred to this
//! issue's own spec acceptance ("Per-stream statistics: frame count,
//! duration, level, loss count, gap count, the AEC state").

use std::time::Duration;

use transcriber_audio::{ReaderId, RingBuffer, SourceDegradation};
use zeroize::Zeroizing;

/// Samples drained per [`RingBuffer::read`] call while building a
/// [`StreamStats`] snapshot — large enough that a roughly one-second poll
/// interval at 48 kHz stereo drains in a handful of iterations, small
/// enough to stay a fixed, modest allocation.
const STATS_SCRATCH_SAMPLES: usize = 4096;

/// The lowest level [`StreamStats::level_dbfs`] ever reports. True digital
/// silence has no finite dBFS value (`20 * log10(0.0)` is `-inf`), and a
/// floor is easier to read next to ordinary numbers than a special-cased
/// "-inf".
const SILENCE_FLOOR_DBFS: f32 = -96.0;

/// One stream's live statistics — what `cli` prints to make the spec's
/// source-isolation, stream-separation and echo-cancellation checks
/// observable.
#[derive(Debug, Clone, Copy)]
pub struct StreamStats {
    /// Audio frames (one sample per channel) captured so far.
    pub frame_count: u64,
    /// How far into the session this stream's clock has advanced —
    /// [`crate::Session::elapsed`].
    pub duration: Duration,
    /// Peak amplitude of the samples read since the previous snapshot, in
    /// dBFS, clamped at [`SILENCE_FLOOR_DBFS`]. Unchanged from the previous
    /// snapshot when nothing new was available to read this time.
    pub level_dbfs: f32,
    /// Samples lost because this reader fell behind an overwriting writer —
    /// the spec's risk table: loss is a reported event, never a silent
    /// state.
    pub loss_count: u64,
    /// `data_discontinuity` and `timestamp_error` gaps combined.
    pub gap_count: u64,
    /// The capability degradation this stream's source reported when
    /// opened, if any — today only ever
    /// [`SourceDegradation::EchoCancellationUnavailable`] on `Local`.
    pub degradation: Option<SourceDegradation>,
}

/// Accumulates [`StreamStats`] across repeated polls of one stream's ring
/// buffer, through a dedicated reader registered at
/// [`crate::capture::StreamCapture::spawn`] time — before the capture
/// thread starts, so it sees every sample from the very first frame.
pub(crate) struct StatsReader {
    reader: ReaderId,
    channels: u16,
    total_samples: u64,
    last_level_dbfs: f32,
    scratch: Zeroizing<Vec<f32>>,
}

impl StatsReader {
    /// Registers a fresh reader on `ring`.
    pub(crate) fn register(ring: &mut RingBuffer, channels: u16) -> Self {
        Self {
            reader: ring.add_reader(),
            channels: channels.max(1),
            total_samples: 0,
            last_level_dbfs: SILENCE_FLOOR_DBFS,
            scratch: Zeroizing::new(vec![0.0; STATS_SCRATCH_SAMPLES]),
        }
    }

    /// Drains every sample currently available from `ring`, folding it into
    /// a fresh [`StreamStats`] snapshot — a full drain every call, so this
    /// reader only ever reports loss for a genuine stall (the calling
    /// thread blocked longer than the ring buffer's capacity), never for
    /// its own ordinary polling cadence.
    pub(crate) fn snapshot(
        &mut self,
        ring: &mut RingBuffer,
        duration: Duration,
        degradation: Option<SourceDegradation>,
    ) -> StreamStats {
        if let Some(peak) = self.drain(ring) {
            self.last_level_dbfs = peak_to_dbfs(peak);
        }
        StreamStats {
            frame_count: self.total_samples / u64::from(self.channels),
            duration,
            level_dbfs: self.last_level_dbfs,
            loss_count: ring.loss_count(self.reader),
            gap_count: ring.discontinuity_count() + ring.timestamp_error_count(),
            degradation,
        }
    }

    /// Reads every sample `ring` currently holds for this reader, zeroing
    /// the scratch buffer after each chunk once its peak has been folded
    /// in. Returns the peak amplitude seen this call, or `None` if nothing
    /// was available to read.
    fn drain(&mut self, ring: &mut RingBuffer) -> Option<f32> {
        let mut peak: Option<f32> = None;
        loop {
            let read = ring.read(self.reader, &mut self.scratch);
            if read == 0 {
                break;
            }
            let chunk_peak = self.scratch[..read]
                .iter()
                .fold(0.0_f32, |acc, sample| acc.max(sample.abs()));
            peak = Some(peak.map_or(chunk_peak, |current| current.max(chunk_peak)));
            self.total_samples += read as u64;
            self.scratch[..read].fill(0.0);
            if read < self.scratch.len() {
                break;
            }
        }
        peak
    }
}

/// Converts a peak linear amplitude to dBFS, clamped at
/// [`SILENCE_FLOOR_DBFS`] — `0.0` amplitude has no finite decibel value.
fn peak_to_dbfs(peak: f32) -> f32 {
    if peak <= 0.0 {
        return SILENCE_FLOOR_DBFS;
    }
    (20.0 * peak.log10()).max(SILENCE_FLOOR_DBFS)
}

#[cfg(test)]
mod tests;
