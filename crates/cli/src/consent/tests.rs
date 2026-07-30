use std::io::Cursor;

use super::read_consent;
use crate::language::Language;

fn confirmed(line: &str, lang: Language) -> bool {
    let mut cursor = Cursor::new(line.as_bytes().to_vec());
    read_consent(&mut cursor, lang)
}

#[test]
fn the_exact_word_confirms() {
    assert!(confirmed("ja\n", Language::De));
    assert!(confirmed("yes\n", Language::En));
}

#[test]
fn the_exact_word_confirms_regardless_of_case_or_surrounding_whitespace() {
    assert!(confirmed("  JA  \n", Language::De));
    assert!(confirmed("YES\r\n", Language::En));
}

/// Every one of these is a plausible thing a user might type or a stray
/// keystroke might produce, and every one of them must refuse in both
/// languages — a stray newline treated as "yes" would be a consent bug,
/// not a UX nit.
#[test]
fn ambiguous_input_never_confirms_in_either_language() {
    let ambiguous = ["", "\n", "   \n", "y\n", "n\n", "yes please\n", "nope\n"];
    for line in ambiguous {
        assert!(
            !confirmed(line, Language::De),
            "German: {line:?} must refuse"
        );
        assert!(
            !confirmed(line, Language::En),
            "English: {line:?} must refuse"
        );
    }
}

/// `read_line` on an already-exhausted reader (end-of-input, e.g. a closed
/// stdin or Ctrl+D/Ctrl+Z) returns `Ok(0)` with an empty string — must
/// refuse exactly like any other non-affirmative input.
#[test]
fn end_of_input_refuses() {
    let mut cursor = Cursor::new(Vec::new());
    assert!(!read_consent(&mut cursor, Language::De));
}

#[test]
fn the_wrong_languages_word_does_not_confirm() {
    assert!(!confirmed("yes\n", Language::De));
    assert!(!confirmed("ja\n", Language::En));
}
