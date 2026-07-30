use std::io::Cursor;

use transcriber_audio::TestToneSources;
use transcriber_core::ATTESTATION_V1_EN;

use super::resolve_and_start;
use crate::error::CliError;
use crate::language::Language;

/// [`TestToneSources`] always hands out this one synthetic root PID — see
/// `transcriber_audio::test_tone`.
const TEST_TONE_PID: u32 = 1;

fn factory() -> TestToneSources {
    let Ok(factory) = TestToneSources::new() else {
        panic!("fixed 48 kHz stereo format is always valid");
    };
    factory.with_total_frames(480)
}

fn run_with_input(
    line: &str,
) -> (
    Result<Option<transcriber_session::Session>, CliError>,
    String,
) {
    let factory = factory();
    let mut input = Cursor::new(line.as_bytes().to_vec());
    let mut output = Vec::new();

    let result = resolve_and_start(
        &factory,
        TEST_TONE_PID,
        Language::En,
        &mut input,
        &mut output,
    );
    (result, String::from_utf8_lossy(&output).into_owned())
}

/// The load-bearing proof for the whole consent gate: refusing confirmation
/// must return `Ok(None)` — never a [`transcriber_session::Session`] — so
/// there is structurally nothing that could have started a capture thread.
#[test]
fn refusing_consent_starts_no_session() {
    let (result, output) = run_with_input("no\n");

    let Ok(outcome) = result else {
        panic!("a refusal must not be an error");
    };
    assert!(outcome.is_none(), "refusal must not construct a Session");
    assert!(
        output.contains("Remote: 0 frames, Local: 0 frames"),
        "the refusal notice must literally show both frame counters at 0, got: {output:?}"
    );
}

/// Every non-affirmative input the spec calls out by name must refuse the
/// same way, exercised through the real command wiring rather than only
/// the isolated parsing function in `consent.rs`.
#[test]
fn every_ambiguous_input_starts_no_session() {
    for line in ["", "\n", "   \n", "y\n", "n\n"] {
        let (result, _) = run_with_input(line);
        let Ok(outcome) = result else {
            panic!("ambiguous input {line:?} must not be an error");
        };
        assert!(
            outcome.is_none(),
            "input {line:?} must not construct a Session"
        );
    }
}

/// Confirming with the exact word must actually construct a real, running
/// [`transcriber_session::Session`] — the other half of the consent-gate
/// proof: refusal *and* confirmation both take the one true path through
/// `resolve_and_start`.
#[test]
fn confirming_consent_starts_a_real_session() {
    let (result, _) = run_with_input("yes\n");

    let Ok(outcome) = result else {
        panic!("a valid confirmation must not be an error");
    };
    assert!(outcome.is_some(), "confirmation must construct a Session");
}

/// The resolved root must be shown, and the attestation text must be the
/// unabridged `core` constant — never re-typed, truncated or paraphrased —
/// both *before* the confirmation was ever read.
#[test]
fn the_resolved_root_and_the_full_attestation_are_shown_before_confirmation() {
    let (_, output) = run_with_input("no\n");

    assert!(output.contains("PID"));
    assert!(output.contains("test-tone"));
    assert!(
        output.contains(ATTESTATION_V1_EN),
        "the attestation must appear verbatim, not paraphrased or truncated"
    );

    let root_position = output.find("PID").unwrap_or(usize::MAX);
    let attestation_position = output.find(ATTESTATION_V1_EN).unwrap_or(0);
    assert!(
        root_position < attestation_position,
        "the resolved root must be shown before the attestation/consent prompt"
    );
}

#[test]
fn an_unknown_pid_is_reported_rather_than_silently_ignored() {
    let factory = factory();
    let mut input = Cursor::new(b"yes\n".to_vec());
    let mut output = Vec::new();

    let result = resolve_and_start(&factory, 999_999, Language::En, &mut input, &mut output);

    let Err(error) = result else {
        panic!("an unknown PID must be reported as an error, not silently accepted");
    };
    let CliError::SubjectNotFound { pid } = error else {
        panic!("expected CliError::SubjectNotFound for an unknown PID");
    };
    assert_eq!(pid, 999_999);
}
