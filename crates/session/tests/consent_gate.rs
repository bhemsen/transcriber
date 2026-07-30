//! Runs the `trybuild` `compile_fail` fixtures under `tests/compile_fail/` —
//! the permanent, machine-checked proof that `Session::start` cannot be
//! called without a `ConsentAttestation` (spec, "Gates und Dokumente": "Das
//! Consent-Gate wird per `compile_fail`-Fall (`trybuild`) belegt, nicht per
//! Review-Urteil").
//!
//! `trybuild` requires a checked-in `.stderr` file alongside a
//! `compile_fail` fixture to accept it as passing at all (without one it
//! reports the case as "wip" and fails, writing the actual output for
//! review) — see `tests/compile_fail/session_requires_consent_attestation.stderr`.
//! The fixture is deliberately written to fail with a plain type mismatch
//! (E0308: "expected `ConsentAttestation`, found `()`") rather than an
//! argument-count mismatch (E0061), whose newer "help: provide the
//! argument" suggestion text is more likely to reword on a toolchain bump
//! than E0308's short, long-stable core message — the "stable, minimal
//! expectation" this gate is built around.

#[test]
fn session_start_requires_a_consent_attestation() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
