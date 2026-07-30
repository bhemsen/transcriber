//! What [`crate::Session::start`] needs besides the consent: the chosen
//! subject and its already-opened sources.

use transcriber_audio::AudioSource;
use transcriber_core::CaptureSubject;

/// The chosen capture subject plus its opened sources, ready to hand to
/// [`crate::Session::start`].
///
/// Deliberately lives in `session`, not `core` (spec's prior decision):
/// `core` is I/O-free, and a `Box<dyn AudioSource>` is neither. `remote` is
/// mandatory — [`transcriber_audio::SourceFactory::open_remote`] never
/// returns an absent stream — while `local` is `None` for the ordinary
/// "no microphone present" case, the spec's decision that this is a warning
/// carried on the running session, never a reason to refuse the plan.
///
/// Has no `Debug` or `Clone`: a `Box<dyn AudioSource>` cannot meaningfully
/// support either without risking a platform resource being duplicated or a
/// stream's state being printed.
pub struct CapturePlan {
    subject: CaptureSubject,
    remote: Box<dyn AudioSource>,
    local: Option<Box<dyn AudioSource>>,
}

impl CapturePlan {
    /// Builds a plan for `subject`, with its `Remote` source already opened
    /// and its `Local` (microphone) source opened if one is present.
    #[must_use]
    pub fn new(
        subject: CaptureSubject,
        remote: Box<dyn AudioSource>,
        local: Option<Box<dyn AudioSource>>,
    ) -> Self {
        Self {
            subject,
            remote,
            local,
        }
    }

    /// The application this plan captures.
    #[must_use]
    pub fn subject(&self) -> &CaptureSubject {
        &self.subject
    }

    /// Whether this plan opened a microphone source.
    #[must_use]
    pub fn has_local_source(&self) -> bool {
        self.local.is_some()
    }

    /// Consumes the plan, handing back its subject and both opened sources —
    /// [`crate::Session::start`]'s only caller.
    pub(crate) fn into_parts(
        self,
    ) -> (
        CaptureSubject,
        Box<dyn AudioSource>,
        Option<Box<dyn AudioSource>>,
    ) {
        (self.subject, self.remote, self.local)
    }
}

#[cfg(test)]
mod tests {
    use super::CapturePlan;
    use transcriber_audio::{SourceFactory, TestToneSources};

    fn factory() -> TestToneSources {
        let Ok(sources) = TestToneSources::new() else {
            panic!("fixed 48 kHz stereo format is always valid");
        };
        sources
    }

    #[test]
    fn carries_the_subject_and_reports_local_presence() {
        let sources = factory();
        let Ok(mut subjects) = sources.list_subjects() else {
            panic!("test factory always lists one subject");
        };
        let subject = subjects.remove(0);
        let Ok(remote) = sources.open_remote(&subject) else {
            panic!("test factory never fails to open");
        };
        let Ok(local) = sources.open_local() else {
            panic!("test factory never fails to open");
        };

        let plan = CapturePlan::new(subject.clone(), remote, local);
        assert_eq!(plan.subject(), &subject);
        assert!(plan.has_local_source());
    }

    #[test]
    fn reports_no_local_source_without_a_microphone() {
        let sources = factory().without_microphone();
        let Ok(mut subjects) = sources.list_subjects() else {
            panic!("test factory always lists one subject");
        };
        let subject = subjects.remove(0);
        let Ok(remote) = sources.open_remote(&subject) else {
            panic!("test factory never fails to open");
        };
        let Ok(local) = sources.open_local() else {
            panic!("test factory never fails to open");
        };

        let plan = CapturePlan::new(subject, remote, local);
        assert!(!plan.has_local_source());
    }
}
