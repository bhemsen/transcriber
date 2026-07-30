//! Windows [`transcriber_audio::SourceFactory`]: enumerates the applications
//! with an active render stream and resolves each to its process-tree root.
//!
//! Everything WASAPI- or `sysinfo`-shaped lives behind [`WindowsSources`],
//! this crate's one edge type
//! (`docs/specs/spec-capture-foundation.md`, Windows-backend prior
//! decisions). The ancestor walk, the session filter/dedup and the
//! re-identification check are pure functions over caller-supplied data —
//! the spec's Verification list requires them testable on a CI runner with
//! no audio device, so they never call into a device or the process table
//! themselves.
//!
//! [`transcriber_audio::SourceFactory::list_subjects`] is the only
//! [`transcriber_audio::SourceFactory`] method implemented so far.
//! `open_remote` (issue #12) and `open_local` (issue #13) each return an
//! explicit [`transcriber_audio::SourceFactoryError::Open`] rather than a
//! value that looks like a working stream — a Rust trait impl must be
//! complete, and a stub that looks like success is exactly how a broken
//! capture path ships green.

mod process_tree;
mod sessions;
mod sources;

pub use process_tree::identity_still_matches;
pub use sources::WindowsSources;
