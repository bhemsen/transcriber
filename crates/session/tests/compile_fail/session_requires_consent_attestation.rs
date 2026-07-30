// Proves `Session::start` cannot be called without a real
// `ConsentAttestation` — the vision's "nachweisbar" requirement
// (docs/vision.md), machine-checked rather than left to a review judgement
// (spec, "Gates und Dokumente").
//
// `Session::start` is the *only* constructor and its first parameter is a
// `ConsentAttestation`. The construction below builds a fully valid
// `CapturePlan` — the same lines `tests/session_lifecycle.rs` runs
// successfully — and passes `()` where the consent belongs, so this fails
// to compile for exactly one reason: a plain type mismatch (E0308) on the
// first argument, the compiler's shortest and longest-stable diagnostic
// shape, deliberately chosen over an argument-count mismatch (E0061) whose
// "help: provide the argument" suggestion text is newer and more likely to
// reword across a toolchain bump.

use transcriber_audio::{SourceFactory, TestToneSources};
use transcriber_session::{CapturePlan, Session};

fn main() {
    let factory = TestToneSources::new().unwrap();
    let subject = factory.list_subjects().unwrap().remove(0);
    let remote = factory.open_remote(&subject).unwrap();
    let local = factory.open_local().unwrap();
    let plan = CapturePlan::new(subject, remote, local);

    // `()` where a `ConsentAttestation` belongs — must not compile.
    let _session = Session::start((), plan);
}
