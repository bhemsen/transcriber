//! Domain types shared across the workspace.
//!
//! This crate holds no I/O and depends on nothing outside the standard library.
//! Everything that touches audio buffers lives in `transcriber-audio`, everything
//! that writes lives in `transcriber-protocol` — see `docs/architecture.md`.
//!
//! The types themselves are introduced by the phase that needs them (phase 1
//! brings `Session` and `ConsentAttestation`); this crate exists from the start so
//! the dependency direction is fixed before any code depends on it.

/// Placeholder so the crate compiles before phase 1 lands its domain types.
///
/// Remove together with the first real type.
#[doc(hidden)]
pub const PLACEHOLDER: () = ();
