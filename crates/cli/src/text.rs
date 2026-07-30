//! Small, bilingual UI strings shared by more than one command — the
//! per-command prompts and notices live next to the command that uses them
//! instead, so this stays a short, general vocabulary.

use transcriber_core::StreamIdentity;

use crate::language::Language;

/// Header printed above `list-sources`' rows.
pub(crate) fn list_sources_header(lang: Language) -> &'static str {
    match lang {
        Language::De => "Anwendungen mit aktivem Wiedergabe-Strom:",
        Language::En => "Applications with an active render stream:",
    }
}

/// Printed instead of any rows when nothing is currently eligible.
pub(crate) fn list_sources_empty(lang: Language) -> &'static str {
    match lang {
        Language::De => "  (keine — läuft eine Anwendung mit hörbarer Wiedergabe?)",
        Language::En => "  (none — is an application currently playing audible audio?)",
    }
}

/// Human label for a [`StreamIdentity`], used in every per-stream line this
/// binary prints.
pub(crate) fn stream_label(identity: StreamIdentity, lang: Language) -> &'static str {
    match (identity, lang) {
        (StreamIdentity::Remote, Language::De) => "Gegenseite",
        (StreamIdentity::Remote, Language::En) => "Remote",
        (StreamIdentity::Local, Language::De) => "Ich (Mikrofon)",
        (StreamIdentity::Local, Language::En) => "Local (microphone)",
    }
}

/// One line naming the resolved capture subject — printed *before* the
/// consent prompt, spec's prior decision ("Die CLI zeigt die aufgelöste
/// Wurzel vor der Consent-Abfrage").
pub(crate) fn resolved_subject_line(process_name: &str, root_pid: u32, lang: Language) -> String {
    match lang {
        Language::De => format!("Aufgelöste Erfassungs-Wurzel: {process_name} (PID {root_pid})"),
        Language::En => format!("Resolved capture root: {process_name} (PID {root_pid})"),
    }
}
