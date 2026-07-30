//! The application a session was consented to capture.

/// The application selected as the capture source.
///
/// Carries exactly what `list-sources` shows the user before the consent
/// prompt: the process name and the resolved process-tree root PID — the
/// same root that the loopback capture in `audio-win` targets. Re-checked by
/// name and start time between listing and capture, so a recycled PID never
/// silently swaps the subject (see the spec's identity-recheck decision);
/// that check is a concern of `audio-win`, not of this value type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CaptureSubject {
    process_name: String,
    root_pid: u32,
}

impl CaptureSubject {
    /// Names the application at `root_pid`, the resolved process-tree root.
    #[must_use]
    pub fn new(process_name: impl Into<String>, root_pid: u32) -> Self {
        Self {
            process_name: process_name.into(),
            root_pid,
        }
    }

    /// The process name as shown by `list-sources`.
    #[must_use]
    pub fn process_name(&self) -> &str {
        &self.process_name
    }

    /// The resolved process-tree root PID that would be captured.
    #[must_use]
    pub fn root_pid(&self) -> u32 {
        self.root_pid
    }
}
