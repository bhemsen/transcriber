//! The consent attestation: proof that a session may start.

use std::time::SystemTime;

/// Which wording of the attestation text a user confirmed.
///
/// Versioned so that a later change to the wording (a new
/// `ATTESTATION_V2_DE` / `ATTESTATION_V2_EN` pair) can be told apart from
/// this one in a protocol header, without losing the record of what an
/// earlier user actually agreed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AttestationVersion {
    /// The wording in [`ATTESTATION_V1_DE`] / [`ATTESTATION_V1_EN`].
    V1,
}

/// Proof that the user confirmed the attestation text before a session
/// started.
///
/// The fields are private and [`ConsentAttestation::new`] is the only way to
/// build one, requiring both the confirmation time and the attested text
/// version — there is no default and no partial constructor. This type
/// *is* the consent gate's proof: an escape hatch here would defeat the
/// guarantee that a session cannot exist without it (see
/// `docs/constitution.md`, `Session::start(consent: ConsentAttestation)`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsentAttestation {
    confirmed_at: SystemTime,
    version: AttestationVersion,
}

impl ConsentAttestation {
    /// Records that the user confirmed `version` of the attestation text at
    /// `confirmed_at`.
    #[must_use]
    pub fn new(confirmed_at: SystemTime, version: AttestationVersion) -> Self {
        Self {
            confirmed_at,
            version,
        }
    }

    /// The point in time the user confirmed the attestation.
    #[must_use]
    pub fn confirmed_at(&self) -> SystemTime {
        self.confirmed_at
    }

    /// Which wording of the attestation text was confirmed.
    #[must_use]
    pub fn version(&self) -> AttestationVersion {
        self.version
    }
}

/// German source wording of the version-1 attestation text.
///
/// Binding legal text, not prose to improve — verbatim from
/// `docs/specs/spec-capture-foundation.md`, "Attestation-Text V1". German is
/// the source; [`ATTESTATION_V1_EN`] is translated 1:1 from it and carries
/// the same version number. Displayed in full, never shortened, next to the
/// confirmation checkbox (`docs/design.md`, Consent dialog).
pub const ATTESTATION_V1_DE: &str = "Ich bestätige, dass ich alle Gesprächsteilnehmer vor Beginn der Erfassung über die Mitschrift informiert habe und dass ihr Einverständnis vorliegt.\n\nMir ist bewusst, dass das Aufzeichnen oder Mitschreiben des nicht öffentlich gesprochenen Wortes ohne Einverständnis der Sprechenden strafbar ist (§ 201 StGB).\n\nDas entstehende Protokoll enthält personenbezogene Daten. Für seine Aufbewahrung und Löschung bin ich verantwortlich.\n\nEs entsteht zu keinem Zeitpunkt eine Ton- oder Bildaufnahme, und kein Stimmprofil überlebt diese Sitzung.\n\nDiese Bestätigung wird mit Zeitstempel im Protokollkopf festgehalten.\n\nIch bestätige das Vorstehende.";

/// English wording of the version-1 attestation text, translated 1:1 from
/// [`ATTESTATION_V1_DE`] and carrying the same version number.
pub const ATTESTATION_V1_EN: &str = "I confirm that I have informed all conversation participants about the transcript before the start of the capture and that their consent has been given.\n\nI am aware that recording or transcribing the non-publicly spoken word without the consent of the speakers is a criminal offense (§ 201 StGB).\n\nThe resulting protocol contains personal data. I am responsible for its retention and deletion.\n\nAt no point does an audio or video recording occur, and no voice profile survives this session.\n\nThis confirmation is recorded with a timestamp in the protocol header.\n\nI confirm the above.";

#[cfg(test)]
mod tests {
    use super::{ATTESTATION_V1_DE, ATTESTATION_V1_EN, AttestationVersion, ConsentAttestation};
    use std::time::SystemTime;

    #[test]
    fn constructor_carries_both_timestamp_and_version() {
        let now = SystemTime::now();
        let attestation = ConsentAttestation::new(now, AttestationVersion::V1);
        assert_eq!(attestation.confirmed_at(), now);
        assert_eq!(attestation.version(), AttestationVersion::V1);
    }

    #[test]
    fn de_and_en_texts_carry_the_legal_reference() {
        assert!(ATTESTATION_V1_DE.contains("§ 201 StGB"));
        assert!(ATTESTATION_V1_EN.contains("§ 201 StGB"));
    }

    #[test]
    fn de_and_en_texts_are_five_paragraphs_plus_the_checkbox_label() {
        assert_eq!(ATTESTATION_V1_DE.split("\n\n").count(), 6);
        assert_eq!(ATTESTATION_V1_EN.split("\n\n").count(), 6);
    }

    /// Guards the distinction the whole project rests on: what we do is a
    /// *capture* (`Erfassung`), never a *recording* (`Aufnahme`/`Aufzeichnen`)
    /// — see `docs/vision.md`'s non-goals and `CLAUDE.md` rule 1. Paragraph 1
    /// names our own action and must say "capture", not "recording"; only
    /// paragraph 2 (the criminal-law reference, which is about recording in
    /// general) and paragraph 4 (asserting that no recording ever happens)
    /// may say "recording".
    #[test]
    fn en_text_names_its_own_action_a_capture_not_a_recording() {
        let first_paragraph = ATTESTATION_V1_EN.split("\n\n").next().unwrap_or_default();
        assert!(first_paragraph.contains("start of the capture"));
        assert!(!first_paragraph.contains("recording"));
    }

    #[test]
    fn de_and_en_texts_confirm_no_recording_survives_the_session() {
        assert!(ATTESTATION_V1_DE.contains("Ton- oder Bildaufnahme"));
        assert!(ATTESTATION_V1_DE.contains("kein Stimmprofil überlebt diese Sitzung"));
        assert!(ATTESTATION_V1_EN.contains("audio or video recording"));
        assert!(ATTESTATION_V1_EN.contains("no voice profile survives this session"));
    }
}
