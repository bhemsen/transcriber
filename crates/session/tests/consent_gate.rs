//! Runs the `trybuild` `compile_fail` fixtures under `tests/compile_fail/` —
//! the permanent, machine-checked proof that `Session::start` cannot be
//! called without a `ConsentAttestation` (spec, "Gates und Dokumente": "Das
//! Consent-Gate wird per `compile_fail`-Fall (`trybuild`) belegt, nicht per
//! Review-Urteil").
//!
//! Deliberately has no `.stderr` expectation file alongside the fixture:
//! `trybuild` then only asserts that the fixture fails to compile, not the
//! exact diagnostic text, which is sensitive to the compiler version — a
//! stable, minimal expectation that will not go red on the next toolchain
//! bump (see the implementer's own note on this gate).

#[test]
fn session_start_requires_a_consent_attestation() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
