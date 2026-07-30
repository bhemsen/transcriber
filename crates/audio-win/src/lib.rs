//! Windows [`transcriber_audio::SourceFactory`]: enumerates the applications
//! with an active render stream and resolves each to its process-tree root,
//! and captures both the `Remote` (process-tree loopback) and `Local`
//! (microphone) streams.
//!
//! Everything WASAPI- or `sysinfo`-shaped lives behind [`WindowsSources`],
//! this crate's one edge type
//! (`docs/specs/spec-capture-foundation.md`, Windows-backend prior
//! decisions). The ancestor walk, the session filter/dedup, the
//! re-identification check and the microphone edge's AEC-degradation and
//! absent-device decisions are all pure functions over caller-supplied
//! data — the spec's Verification list requires them testable on a CI
//! runner with no audio device, so they never call into a device or the
//! process table themselves.
//!
//! [`transcriber_audio::SourceFactory::list_subjects`], `open_remote`
//! (issue #12, real WASAPI loopback capture — see [`crate::loopback`] for
//! the pure packet-mapping logic behind it) and `open_local` (issue #13,
//! microphone capture with OS echo cancellation — see [`crate::microphone`]
//! for the pure AEC-degradation and absent-device decisions behind it) are
//! all implemented.

mod loopback;
mod microphone;
mod process_tree;
mod sessions;
mod sources;

pub use process_tree::identity_still_matches;
pub use sources::WindowsSources;
