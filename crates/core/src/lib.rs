//! Domain types shared across the workspace.
//!
//! This crate holds no I/O and no platform dependencies; `thiserror` is
//! permitted (from phase 3 additionally `zeroize`).
//! Everything that touches audio buffers lives in `transcriber-audio`, everything
//! that writes lives in `transcriber-protocol` — see `docs/architecture.md`.
//!
//! Phase 1 brings the domain types a consented session is built from:
//! [`StreamIdentity`], [`CaptureSubject`], [`SessionId`], [`SessionState`]
//! and [`ConsentAttestation`] with the versioned attestation text. The
//! `Session` orchestrator itself lives in `transcriber-session`, not
//! here — so that crates depending on `core` never pull in its threads,
//! buffers or platform code transitively.

mod consent;
mod error;
mod session_id;
mod state;
mod stream;
mod subject;

pub use consent::{ATTESTATION_V1_DE, ATTESTATION_V1_EN, AttestationVersion, ConsentAttestation};
pub use error::SessionStateError;
pub use session_id::SessionId;
pub use state::SessionState;
pub use stream::StreamIdentity;
pub use subject::CaptureSubject;
