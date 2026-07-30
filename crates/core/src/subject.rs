//! The application a session was consented to capture.

use std::time::SystemTime;

/// The application selected as the capture source.
///
/// Carries exactly what `list-sources` shows the user before the consent
/// prompt: the process name, the resolved process-tree root PID — the same
/// root that the loopback capture in `audio-win` targets — and the root
/// process's start time. The start time exists solely as evidence for the
/// re-identification the spec requires between listing and capture: by
/// *name and start time*, not the PID alone, because Windows recycles PIDs —
/// a PID match alone cannot tell today's process at that PID apart from a
/// different one that happened to reuse it (see the spec's identity-recheck
/// decision). Performing that check is a concern of `audio-win`, not of
/// this value type; this type only carries the evidence.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CaptureSubject {
    process_name: String,
    root_pid: u32,
    started_at: SystemTime,
}

impl CaptureSubject {
    /// Names the application at `root_pid`, the resolved process-tree root,
    /// which started running at `started_at`.
    #[must_use]
    pub fn new(process_name: impl Into<String>, root_pid: u32, started_at: SystemTime) -> Self {
        Self {
            process_name: process_name.into(),
            root_pid,
            started_at,
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

    /// When the root process started — the evidence a backend's
    /// re-identification check compares against a fresh process-table
    /// lookup at capture time, so a recycled PID cannot silently swap the
    /// subject.
    #[must_use]
    pub fn started_at(&self) -> SystemTime {
        self.started_at
    }
}

#[cfg(test)]
mod tests {
    use super::CaptureSubject;
    use std::time::SystemTime;

    #[test]
    fn accessors_return_what_new_was_given() {
        let started_at = SystemTime::now();
        let subject = CaptureSubject::new("teams.exe", 1234, started_at);
        assert_eq!(subject.process_name(), "teams.exe");
        assert_eq!(subject.root_pid(), 1234);
        assert_eq!(subject.started_at(), started_at);
    }
}
