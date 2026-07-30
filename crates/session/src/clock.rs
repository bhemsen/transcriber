//! The common zero every stream's device timeline is normalised against —
//! the spec's prior decision ("Ringpuffer, Zeitachse und Resampling"): the
//! microphone and the captured application have independent device clocks,
//! so a raw [`DeviceTimestamp`] from one stream is meaningless next to
//! another's without a shared reference point.

use std::time::{Duration, Instant};

use transcriber_audio::DeviceTimestamp;

/// Wall-clock instant [`crate::Session::start`] records once, up front, and
/// every stream's [`StreamClock`] is anchored against.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SessionZero(Instant);

impl SessionZero {
    /// Records "now" as the session's zero point.
    pub(crate) fn record() -> Self {
        Self(Instant::now())
    }

    /// Wall-clock time elapsed since this zero was recorded.
    fn elapsed(&self) -> Duration {
        self.0.elapsed()
    }
}

/// Normalises one stream's device timeline onto a [`SessionZero`].
///
/// A stream's [`DeviceTimestamp`] ticks are relative to when its own device
/// started, not to the session — two independently clocked devices never
/// agree on tick zero. [`Self::anchor`] records how much wall-clock time had
/// already elapsed since the session's zero at the moment this stream's
/// capture thread started polling; [`Self::normalize`] then anchors the
/// device's own tick zero on the first frame it ever sees, so every later
/// frame's normalised offset is that wall-clock anchor plus however many
/// ticks have advanced past the first one.
pub(crate) struct StreamClock {
    anchor_elapsed: Duration,
    device_zero_ticks: Option<i64>,
}

impl StreamClock {
    /// Anchors a new stream's clock to `session_zero` at the moment its
    /// capture thread starts polling, before any frame has arrived.
    pub(crate) fn anchor(session_zero: SessionZero) -> Self {
        Self {
            anchor_elapsed: session_zero.elapsed(),
            device_zero_ticks: None,
        }
    }

    /// Normalises `timestamp` to a duration since the session's zero,
    /// anchoring this stream's device-zero tick on the first call.
    pub(crate) fn normalize(&mut self, timestamp: DeviceTimestamp) -> Duration {
        let zero_ticks = *self.device_zero_ticks.get_or_insert(timestamp.ticks());
        let delta_ticks = (timestamp.ticks() - zero_ticks).max(0);
        self.anchor_elapsed + Duration::from_nanos((delta_ticks as u64) * 100)
    }
}

#[cfg(test)]
mod tests {
    use super::{SessionZero, StreamClock};
    use std::thread;
    use std::time::Duration;
    use transcriber_audio::DeviceTimestamp;

    #[test]
    fn a_streams_own_first_frame_normalises_to_its_anchor() {
        let zero = SessionZero::record();
        let mut clock = StreamClock::anchor(zero);
        let offset = clock.normalize(DeviceTimestamp::from_ticks(12_345));
        assert!(offset < Duration::from_secs(1));
    }

    #[test]
    fn ticks_advance_the_normalised_offset_by_the_same_wall_clock_span() {
        let zero = SessionZero::record();
        let mut clock = StreamClock::anchor(zero);
        let first = clock.normalize(DeviceTimestamp::from_ticks(1_000));
        // 10_000_000 ticks at 100 ns each is exactly one second.
        let second = clock.normalize(DeviceTimestamp::from_ticks(1_000 + 10_000_000));
        assert_eq!(second - first, Duration::from_secs(1));
    }

    /// Two streams anchored to the *same* session zero at different
    /// wall-clock moments must disagree on their offset for an identical
    /// device tick — proof that the session zero, not each device's own
    /// clock, is what makes the two timelines comparable at all.
    #[test]
    fn two_streams_anchored_later_carry_a_larger_offset_for_the_same_tick() {
        let zero = SessionZero::record();
        let mut first = StreamClock::anchor(zero);
        thread::sleep(Duration::from_millis(15));
        let mut second = StreamClock::anchor(zero);

        let first_offset = first.normalize(DeviceTimestamp::from_ticks(0));
        let second_offset = second.normalize(DeviceTimestamp::from_ticks(0));
        assert!(second_offset > first_offset);
    }
}
