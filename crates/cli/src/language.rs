//! The bilingual UI choice (`docs/constitution.md`, Conventions: "UI-Texte
//! zweisprachig Deutsch und Englisch") for a terminal harness: a `--lang`
//! flag picking one language per run, defaulting to German — see the
//! spec's Decision log for the rationale over printing both at once.

use clap::ValueEnum;
use transcriber_core::{ATTESTATION_V1_DE, ATTESTATION_V1_EN};

/// Which language this invocation's output and the displayed attestation
/// text are in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum Language {
    /// German — the project documentation's source language.
    De,
    /// English.
    En,
}

impl Language {
    /// The versioned attestation text in this language, verbatim from
    /// `core` — never re-typed, truncated or paraphrased.
    pub(crate) fn attestation_text(self) -> &'static str {
        match self {
            Self::De => ATTESTATION_V1_DE,
            Self::En => ATTESTATION_V1_EN,
        }
    }

    /// The one word that counts as an explicit affirmative confirmation —
    /// everything else, including a near-miss, is a refusal.
    pub(crate) fn confirmation_word(self) -> &'static str {
        match self {
            Self::De => "ja",
            Self::En => "yes",
        }
    }
}
