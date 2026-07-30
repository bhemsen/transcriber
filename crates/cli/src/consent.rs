//! The consent gate: display the attestation in full, then read one line
//! and decide, without any ambiguity, whether it counts as the explicit
//! affirmative act the spec requires.

use std::io::{BufRead, Write};

use crate::language::Language;

/// Prints the versioned attestation text in full — verbatim from `core`,
/// never abridged (spec's prior decision, `docs/design.md`'s Consent
/// dialog) — followed by the exact-word confirmation prompt.
pub(crate) fn print_attestation(output: &mut impl Write, lang: Language) {
    let intro = match lang {
        Language::De => "Bitte lesen und bestätigen Sie:",
        Language::En => "Please read and confirm:",
    };
    let _ = writeln!(output, "\n{intro}\n");
    let _ = writeln!(output, "{}\n", lang.attestation_text());
    let _ = writeln!(output, "{}", confirm_prompt(lang));
}

/// The exact-word prompt — anything else typed, including a near-miss, a
/// blank line or end-of-input, is a refusal (see [`is_explicit_confirmation`]).
fn confirm_prompt(lang: Language) -> String {
    let word = lang.confirmation_word();
    match lang {
        Language::De => format!("Zum Bestätigen genau \"{word}\" eingeben (sonst: Abbruch): "),
        Language::En => format!("Type exactly \"{word}\" to confirm (anything else refuses): "),
    }
}

/// Reads one line from `input` and decides whether it is the explicit
/// affirmative act the spec requires — never blocking on anything but a
/// single line, and never treating a non-affirmative response as consent.
///
/// Ambiguous input never counts: an empty line, whitespace, end-of-input
/// (`read_line` returning `Ok(0)`), or any word other than the exact,
/// language-specific confirmation word all resolve to `false`.
pub(crate) fn read_consent(input: &mut impl BufRead, lang: Language) -> bool {
    let mut line = String::new();
    if input.read_line(&mut line).is_err() {
        return false;
    }
    is_explicit_confirmation(&line, lang)
}

/// The pure decision behind [`read_consent`] — kept separate so the
/// consent gate's actual logic is unit-testable without any I/O at all.
fn is_explicit_confirmation(line: &str, lang: Language) -> bool {
    line.trim().eq_ignore_ascii_case(lang.confirmation_word())
}

#[cfg(test)]
mod tests;
